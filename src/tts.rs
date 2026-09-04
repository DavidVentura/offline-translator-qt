use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

use translator::{PcmAudio, SpeechChunk, TranslatorSession};

use crate::model::TtsVoiceSelection;
use crate::ui::UiCallbacks;
use audio_sink::{AudioSink, PcmSamples, SampleRate};

/// Playback rate multiplier, clamped to the range the speed slider offers and quantized to the
/// slider's step so a persisted value round-trips exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeechSpeed(f32);

impl SpeechSpeed {
    pub const MIN: f32 = 0.7;
    pub const MAX: f32 = 2.0;
    pub const STEP: f32 = 0.1;

    pub fn new(value: f32) -> Self {
        let clamped = value.clamp(Self::MIN, Self::MAX);
        let quantized = (clamped / Self::STEP).round() * Self::STEP;
        Self(quantized.clamp(Self::MIN, Self::MAX))
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

impl Default for SpeechSpeed {
    fn default() -> Self {
        Self::new(1.0)
    }
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
    speech_speed: SpeechSpeed,
    voice: Option<TtsVoiceSelection>,
    ui: UiCallbacks,
) {
    let generation = PLAYBACK_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    (ui.set_tts_state)(true, false);

    thread::spawn(move || {
        let result = play_text_streaming(
            &session,
            &language_code,
            &text,
            speech_speed.value(),
            voice.as_ref(),
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

fn play_chunk(
    playback: &AudioSink,
    rate: SampleRate,
    chunk: &QueuedAudioChunk,
    should_stop: &mut impl audio_sink::StopSignal,
) -> Result<(), String> {
    let samples = PcmSamples::new(rate, &chunk.audio.pcm_samples);
    playback
        .play(samples, should_stop)
        .map_err(|e| e.to_string())?;
    let Some(pause_after_ms) = chunk.pause_after_ms else {
        return Ok(());
    };
    let pause_ms = u64::try_from(pause_after_ms)
        .map_err(|_| format!("negative pause between speech chunks: {pause_after_ms} ms"))?;
    if pause_ms == 0 {
        return Ok(());
    }
    playback
        .play_silence(Duration::from_millis(pause_ms), should_stop)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn play_text_streaming(
    session: &Arc<TranslatorSession>,
    language_code: &str,
    text: &str,
    speech_speed: f32,
    voice: Option<&TtsVoiceSelection>,
    generation: u64,
    ui: &UiCallbacks,
) -> Result<(), String> {
    let planning_started_at = Instant::now();
    let planned_chunks = catch_tts_panic(|| -> Result<Option<Vec<SpeechChunk>>, String> {
        match session.plan_speech_chunks(
            language_code,
            text,
            voice.map(|voice| voice.pack_id.as_str()),
            translator::UrlsAndHashtags::Skip,
        ) {
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
    let producer_voice = voice.cloned();

    thread::spawn(move || {
        produce_audio_chunks(
            planned_chunks,
            tx,
            generation,
            producer_session,
            producer_language_code,
            speech_speed,
            producer_voice,
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
    let rate = SampleRate::try_from(first_chunk.audio.sample_rate).map_err(|e| e.to_string())?;
    let playback = AudioSink::open(rate).map_err(|e| e.to_string())?;
    (ui.set_tts_state)(false, true);
    play_chunk(&playback, rate, &first_chunk, &mut should_stop)?;

    while let Some(chunk) = recv_chunk(&rx, &mut should_stop)? {
        play_chunk(&playback, rate, &chunk, &mut should_stop)?;
    }

    if should_stop() {
        eprintln!("tts.stream: playback interrupted generation={generation}");
        let _ = playback.discard();
    } else {
        eprintln!("tts.stream: playback finished generation={generation}");
        let _ = playback.drain();
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
    voice: Option<TtsVoiceSelection>,
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
                voice.as_ref().and_then(|voice| voice.speaker.as_deref()),
                chunk.is_phonemes,
                voice.as_ref().map(|voice| voice.pack_id.as_str()),
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

#[cfg(test)]
mod tests {
    use super::SpeechSpeed;

    #[test]
    fn speed_is_clamped_and_quantized() {
        assert_eq!(SpeechSpeed::new(0.5).value(), SpeechSpeed::MIN);
        assert_eq!(SpeechSpeed::new(3.0).value(), SpeechSpeed::MAX);
        assert_eq!(SpeechSpeed::new(1.0).value(), 1.0);
        assert!((SpeechSpeed::new(1.24).value() - 1.2).abs() < 1e-6);
        assert!((SpeechSpeed::new(1.26).value() - 1.3).abs() < 1e-6);
    }
}
