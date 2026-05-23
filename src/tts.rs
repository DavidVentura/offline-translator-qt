use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use translator::{PcmAudio, SpeechChunk, TranslatorSession, TtsVoiceOption};

use crate::pulse::PulsePlaybackStream;
use crate::ui::UiCallbacks;

pub struct TtsVoiceRefresh {
    pub available: bool,
    pub voices: Vec<TtsVoiceOption>,
    pub selected_voice_name: String,
    pub selected_voice_display_name: String,
}

static PLAYBACK_GENERATION: AtomicU64 = AtomicU64::new(0);
const SYNTHESIS_QUEUE_DEPTH: usize = 2;
const STREAM_POLL_INTERVAL_MS: u64 = 50;

fn chunk_preview(text: &str) -> String {
    const MAX_CHARS: usize = 64;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut iter = normalized.chars();
    let preview = iter.by_ref().take(MAX_CHARS).collect::<String>();
    if iter.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

pub fn stop_playback() {
    PLAYBACK_GENERATION.fetch_add(1, Ordering::SeqCst);
}

pub fn load_tts_voices(
    session: &TranslatorSession,
    language_code: &str,
    selected_voice_name: Option<&str>,
) -> Result<TtsVoiceRefresh, String> {
    let voices = match catch_tts_panic(|| -> Result<Option<Vec<TtsVoiceOption>>, String> {
        match session.available_tts_voices(language_code) {
            Ok(voices) => Ok(Some(voices)),
            Err(err) if err.is_missing_asset() => Ok(None),
            Err(err) => Err(err.message),
        }
    })? {
        Some(voices) => voices,
        None => {
            return Ok(TtsVoiceRefresh {
                available: false,
                voices: Vec::new(),
                selected_voice_name: String::new(),
                selected_voice_display_name: "Default".to_string(),
            });
        }
    };

    let voice_preview = voices
        .iter()
        .take(8)
        .map(|voice| format!("{}={}", voice.name, voice.speaker_id))
        .collect::<Vec<_>>();
    eprintln!(
        "tts.load_tts_voices: language={} returned {} voice(s) preview={:?}{}",
        language_code,
        voices.len(),
        voice_preview,
        if voices.len() > voice_preview.len() {
            " ..."
        } else {
            ""
        }
    );

    let selected_voice_name = selected_voice_name
        .filter(|value| voices.iter().any(|voice| voice.name == **value))
        .map(ToOwned::to_owned)
        .unwrap_or_default();

    let selected_voice_display_name = if selected_voice_name.is_empty() || voices.len() <= 1 {
        "Default".to_string()
    } else {
        voices
            .iter()
            .find(|voice| voice.name == selected_voice_name)
            .map(|voice| voice.display_name.clone())
            .unwrap_or_else(|| "Default".to_string())
    };

    Ok(TtsVoiceRefresh {
        available: true,
        voices,
        selected_voice_name,
        selected_voice_display_name,
    })
}

pub fn warm_tts_model(session: &TranslatorSession, language_code: &str) -> Result<bool, String> {
    let started_at = Instant::now();

    let ready = catch_tts_panic(|| match session.warm_tts_model(language_code) {
        Ok(()) => Ok(true),
        Err(err) if err.is_missing_asset() => Ok(false),
        Err(err) => Err(err.message),
    })?;

    eprintln!(
        "tts.warm: ready language={} took_ms={}",
        language_code,
        started_at.elapsed().as_millis()
    );

    Ok(ready)
}

pub fn play_text_async(
    session: Arc<TranslatorSession>,
    language_code: String,
    text: String,
    speech_speed: f32,
    voice_name: Option<String>,
    ui: UiCallbacks,
) {
    let generation = PLAYBACK_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    (ui.set_tts_state)(true, false);

    thread::spawn(move || {
        let result = play_text_streaming(
            &session,
            &language_code,
            &text,
            speech_speed,
            voice_name.as_deref(),
            generation,
            &ui,
        );

        if PLAYBACK_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }

        match result {
            Ok(()) => {
                if PLAYBACK_GENERATION.load(Ordering::SeqCst) == generation {
                    (ui.set_tts_state)(false, false);
                }
            }
            Err(err) => {
                eprintln!("TTS streaming failed: {err}");
                if PLAYBACK_GENERATION.load(Ordering::SeqCst) == generation {
                    (ui.set_tts_state)(false, false);
                }
            }
        }
    });
}

#[derive(Debug)]
struct QueuedAudioChunk {
    chunk_index: usize,
    audio: PcmAudio,
    pause_after_ms: Option<i32>,
}

fn play_text_streaming(
    session: &Arc<TranslatorSession>,
    language_code: &str,
    text: &str,
    speech_speed: f32,
    voice_name: Option<&str>,
    generation: u64,
    ui: &UiCallbacks,
) -> Result<(), String> {
    let planning_started_at = Instant::now();
    let planned_chunks = catch_tts_panic(|| -> Result<Option<Vec<SpeechChunk>>, String> {
        match session.plan_speech_chunks(language_code, text, None) {
            Ok(chunks) => Ok(Some(chunks)),
            Err(err) if err.is_missing_asset() => Ok(None),
            Err(err) => Err(err.message),
        }
    })?
    .ok_or_else(|| format!("No TTS voice installed for {}", language_code))?;

    if planned_chunks.is_empty() {
        return Err("Nothing to speak".to_string());
    }

    eprintln!(
        "tts.stream: language={} planning_took_ms={} planned {} chunk(s)",
        language_code,
        planning_started_at.elapsed().as_millis(),
        planned_chunks.len()
    );

    let (tx, rx) = sync_channel::<Result<QueuedAudioChunk, String>>(SYNTHESIS_QUEUE_DEPTH);
    let producer_session = Arc::clone(session);
    let producer_language_code = language_code.to_string();
    let producer_voice_name = voice_name.map(ToOwned::to_owned);

    thread::spawn(move || {
        produce_audio_chunks(
            planned_chunks,
            tx,
            generation,
            producer_session,
            producer_language_code,
            speech_speed,
            producer_voice_name,
        );
    });

    let mut should_stop = || PLAYBACK_GENERATION.load(Ordering::SeqCst) != generation;
    let first_chunk = match recv_chunk(&rx, &mut should_stop)? {
        Some(chunk) => chunk,
        None => return Ok(()),
    };

    eprintln!(
        "tts.stream: playback starting sample_rate={} first_chunk={}",
        first_chunk.audio.sample_rate, first_chunk.chunk_index
    );
    let playback = PulsePlaybackStream::new(first_chunk.audio.sample_rate)?;
    (ui.set_tts_state)(false, true);
    playback.write_audio(&first_chunk.audio, &mut should_stop)?;
    if let Some(pause_after_ms) = first_chunk.pause_after_ms {
        playback.write_pause_ms(pause_after_ms, &mut should_stop)?;
    }

    while let Some(chunk) = recv_chunk(&rx, &mut should_stop)? {
        playback.write_audio(&chunk.audio, &mut should_stop)?;
        if let Some(pause_after_ms) = chunk.pause_after_ms {
            playback.write_pause_ms(pause_after_ms, &mut should_stop)?;
        }
    }

    if should_stop() {
        eprintln!("tts.stream: playback interrupted generation={generation}");
        let _ = playback.flush();
    } else {
        eprintln!("tts.stream: playback finished generation={generation}");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn produce_audio_chunks(
    planned_chunks: Vec<SpeechChunk>,
    tx: SyncSender<Result<QueuedAudioChunk, String>>,
    generation: u64,
    session: Arc<TranslatorSession>,
    language_code: String,
    speech_speed: f32,
    voice_name: Option<String>,
) {
    for (chunk_index, chunk) in planned_chunks.into_iter().enumerate() {
        if PLAYBACK_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }

        eprintln!(
            "tts.stream.synth.start: chunk={} phonemes={} pause_ms={:?} chars={} text='{}'",
            chunk_index,
            chunk.is_phonemes,
            chunk.pause_after_ms,
            chunk.content.chars().count(),
            chunk_preview(&chunk.content)
        );
        let pcm = match catch_tts_panic(|| {
            match session.synthesize_pcm(
                &language_code,
                &chunk.content,
                speech_speed,
                voice_name.as_deref(),
                chunk.is_phonemes,
                None,
            ) {
                Ok(audio) => Ok(audio),
                Err(err) if err.is_missing_asset() => {
                    Err(format!("No TTS voice installed for {language_code}"))
                }
                Err(err) => Err(err.message),
            }
        }) {
            Ok(pcm) => pcm,
            Err(err) => {
                eprintln!(
                    "tts.stream.synth.error: chunk={} error={}",
                    chunk_index, err
                );
                let _ = tx.send(Err(err));
                return;
            }
        };

        eprintln!(
            "tts.stream.synth.done: chunk={} sample_rate={} samples={}",
            chunk_index,
            pcm.sample_rate,
            pcm.pcm_samples.len()
        );

        if PLAYBACK_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }

        if tx
            .send(Ok(QueuedAudioChunk {
                chunk_index,
                audio: pcm,
                pause_after_ms: chunk.pause_after_ms,
            }))
            .is_err()
        {
            return;
        }
    }
}

fn recv_chunk<F>(
    rx: &std::sync::mpsc::Receiver<Result<QueuedAudioChunk, String>>,
    should_stop: &mut F,
) -> Result<Option<QueuedAudioChunk>, String>
where
    F: FnMut() -> bool,
{
    loop {
        if should_stop() {
            return Ok(None);
        }

        match rx.recv_timeout(Duration::from_millis(STREAM_POLL_INTERVAL_MS)) {
            Ok(Ok(chunk)) => return Ok(Some(chunk)),
            Ok(Err(err)) => return Err(err),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
}

fn catch_tts_panic<T, F>(f: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        if let Some(message) = payload.downcast_ref::<&str>() {
            format!("TTS runtime panicked: {message}")
        } else if let Some(message) = payload.downcast_ref::<String>() {
            format!("TTS runtime panicked: {message}")
        } else {
            "TTS runtime panicked".to_string()
        }
    })?
}
