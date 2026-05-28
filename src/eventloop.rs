use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use cld2::{Format, detect_language};
use translator::TranslatorSession;

use crate::catalog_state::languages_from_overview;
use crate::download;
use crate::image_ocr;
use crate::model::FeatureKind;
use crate::rendered_image_item::qimage_from_rgba_bytes;
use crate::tts;
use crate::ui::{TtsVoiceListItem, UiCallbacks};
use crate::{AppPaths, IoEvent};

pub fn run_eventloop(bus_rx: Receiver<IoEvent>, ui: UiCallbacks, session: Arc<TranslatorSession>) {
    let mut app_paths = None::<AppPaths>;

    while let Ok(msg) = bus_rx.recv() {
        match msg {
            IoEvent::SetAppPaths(path) => {
                app_paths = Some(path.clone());

                let load_start = Instant::now();
                std::fs::create_dir_all(&path.data).expect("can't make data dir");
                std::fs::create_dir_all(&path.config).expect("can't make config dir");

                session.refresh_snapshot();
                (ui.set_languages)(languages_from_overview(session.language_overview()));
                println!("Load took {:?}", load_start.elapsed());
            }
            IoEvent::DownloadRequest {
                code,
                feature,
                selected_tts_pack_id,
            } => {
                let Some(app_paths) = app_paths.clone() else {
                    println!("no app path, cant download");
                    continue;
                };

                if let Some(plan) =
                    session.plan_download(&code, feature.into(), selected_tts_pack_id.as_deref())
                    && let Err(err) = download_feature(&code, feature, &plan, &app_paths.data, &ui)
                {
                    eprintln!("Download failed for {code}: {err}");
                }

                session.refresh_snapshot();
                (ui.set_languages)(languages_from_overview(session.language_overview()));
            }
            IoEvent::DeleteLanguage { code, feature } => {
                let delete_plan = session.prepare_delete(&code, feature.into());
                session.apply_delete_plan(&delete_plan);
                (ui.set_languages)(languages_from_overview(session.language_overview()));
            }
            IoEvent::TranslationRequest { text, from, to } => {
                send_detection_to_ui(&text, &ui);

                let start = Instant::now();

                let result = session.translate_text(&from, &to, &text).map_err(|error| {
                    if error.is_missing_asset() {
                        format!("Missing installed language pair {from}->{to}")
                    } else {
                        error.message
                    }
                });

                let text = match result {
                    Ok(result) => result,
                    Err(message) => message,
                };
                println!("translation took {:?} = '{}'", start.elapsed(), text);
                (ui.set_output_text)(text);
            }
            IoEvent::RefreshTtsVoices {
                language_code,
                selected_voice_name,
            } => {
                match tts::load_tts_voices(
                    &session,
                    &language_code,
                    (!selected_voice_name.is_empty()).then_some(selected_voice_name.as_str()),
                ) {
                    Ok(result) => {
                        let mut items = vec![TtsVoiceListItem {
                            name: String::new().into(),
                            display_name: "Default".to_string().into(),
                        }];
                        items.extend(result.voices.into_iter().map(|voice| TtsVoiceListItem {
                            name: voice.name.into(),
                            display_name: voice.display_name.into(),
                        }));
                        (ui.set_tts_voices)(
                            result.available,
                            items,
                            result.selected_voice_name,
                            result.selected_voice_display_name,
                        );
                    }
                    Err(err) => {
                        eprintln!("Failed to load TTS voices: {err}");
                        (ui.set_tts_voices)(
                            false,
                            Vec::new(),
                            String::new(),
                            "Default".to_string(),
                        );
                    }
                }
            }
            IoEvent::WarmTtsModel { language_code } => {
                if let Err(err) = tts::warm_tts_model(&session, &language_code) {
                    eprintln!("Failed to warm TTS model for {language_code}: {err}");
                }
            }
            IoEvent::SpeakRequest {
                language_code,
                text,
                speech_speed,
                voice_name,
            } => {
                tts::play_text_async(
                    Arc::clone(&session),
                    language_code,
                    text,
                    speech_speed,
                    (!voice_name.is_empty()).then_some(voice_name),
                    ui.clone(),
                );
            }
            IoEvent::StopTts => {
                tts::stop_playback();
                (ui.set_tts_state)(false, false);
            }
            IoEvent::ImageTranslationRequest {
                image_path,
                from,
                to,
                min_confidence,
                max_image_size,
                background_mode,
            } => {
                let start = Instant::now();
                let result = image_ocr::translate_image_with_session(
                    &session,
                    std::path::Path::new(&image_path),
                    &from,
                    &to,
                    min_confidence,
                    max_image_size,
                    &background_mode,
                );

                match result {
                    Ok(image_translation) => {
                        let ui_start = Instant::now();
                        (ui.set_processed_image)(qimage_from_rgba_bytes(
                            image_translation.image_width,
                            image_translation.image_height,
                            &image_translation.rendered_rgba_bytes,
                        ));
                        send_detection_to_ui(&image_translation.extracted_text, &ui);
                        (ui.set_input_text)(image_translation.extracted_text);
                        (ui.set_output_text)(image_translation.translated_text);
                        println!("image_ocr postprocess ui={:?}", ui_start.elapsed());
                    }
                    Err(message) => {
                        (ui.set_input_text)(String::new());
                        (ui.set_output_text)(message);
                    }
                }
                println!("image translation took {:?}", start.elapsed());
            }
            IoEvent::Shutdown => {
                tts::stop_playback();
                println!("shutdown signal, exiting");
                break;
            }
        }
    }
    println!("all senders done, closing");
}

fn download_feature(
    code: &str,
    feature: FeatureKind,
    plan: &translator::DownloadPlan,
    data_path: &str,
    ui: &UiCallbacks,
) -> Result<(), String> {
    let total_size = plan.total_size.max(1) as usize;
    let total_downloaded = Arc::new(AtomicUsize::new(0));
    let download_complete = Arc::new(AtomicBool::new(false));

    (ui.set_feature_progress)(code.to_string(), feature, 0.00001);

    let progress_total_downloaded = total_downloaded.clone();
    let progress_download_complete = download_complete.clone();
    let progress_ui = ui.clone();
    let progress_code = code.to_string();

    let progress_thread = thread::spawn(move || {
        const UPDATE_THRESHOLD: usize = 1024 * 1024;
        const UPDATE_INTERVAL: Duration = Duration::from_millis(120);
        let mut last_update = 0;

        while !progress_download_complete.load(Ordering::Relaxed) {
            thread::sleep(UPDATE_INTERVAL);

            let current = progress_total_downloaded.load(Ordering::Relaxed);
            if current.saturating_sub(last_update) >= UPDATE_THRESHOLD {
                let percent = current as f32 / total_size as f32;
                (progress_ui.set_feature_progress)(progress_code.clone(), feature, percent);
                last_update = current;
            }
        }
    });

    let result = download::execute_download_plan(data_path, plan, total_downloaded);
    download_complete.store(true, Ordering::Relaxed);
    progress_thread.join().expect("Progress thread panicked");

    result
}

fn send_detection_to_ui(text: &str, ui: &UiCallbacks) {
    let (detected, reliable) = detect_language(text, Format::Text);

    let code = match (detected, reliable) {
        (Some(c), cld2::Reliable) => c.0,
        _ => "",
    };
    (ui.set_detected_language_code)(code.to_string());
}
