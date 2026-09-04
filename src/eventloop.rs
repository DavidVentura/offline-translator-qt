use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use cld2::{Format, detect_language};
use translator::TranslatorSession;
use translator::http::{BindAddress, HttpServer, HttpServerConfig};

use crate::catalog_state::languages_from_overview;
use crate::document::{DocumentError, DocumentEvent, DocumentProgress};
use crate::download;
use crate::image_ocr;
use crate::model::{DownloadProgress, DownloadTarget};
use crate::rendered_image_item::qimage_from_rgba_bytes;
use crate::settings::HttpServerBind;
use crate::tts;
use crate::ui::{ImageResult, SelectionPillItem, UiCallbacks, installed_voice_items};
use crate::{AppPaths, IoEvent};

pub fn run_eventloop(bus_rx: Receiver<IoEvent>, ui: UiCallbacks, session: Arc<TranslatorSession>) {
    let mut app_paths = None::<AppPaths>;
    let mut document_cancel = None::<Arc<AtomicBool>>;
    let mut http_server = None::<(HttpServerConfig, HttpServer)>;

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
                let target = DownloadTarget {
                    code,
                    feature,
                    tts_pack_id: selected_tts_pack_id,
                };
                match app_paths.clone() {
                    None => println!("no app path, cant download"),
                    Some(app_paths) => {
                        let plan = session.plan_download(
                            &target.code,
                            target.feature.into(),
                            target.tts_pack_id.as_deref(),
                        );
                        match plan {
                            None => eprintln!("No download plan for {target:?}"),
                            Some(plan) => {
                                if let Err(err) =
                                    download_feature(&target, &plan, &app_paths.data, &ui)
                                {
                                    eprintln!("Download failed for {}: {err}", target.code);
                                }
                            }
                        }
                    }
                }
                (ui.set_feature_progress)(target, DownloadProgress::Ended);

                session.refresh_snapshot();
                (ui.set_languages)(languages_from_overview(session.language_overview()));
            }
            IoEvent::DeleteLanguage { code, feature } => {
                let delete_plan = session.prepare_delete(&code, feature.into());
                session.apply_delete_plan(&delete_plan);
                (ui.set_languages)(languages_from_overview(session.language_overview()));
            }
            IoEvent::DeleteTtsPack { pack_id } => {
                let delete_plan = session.prepare_delete_tts_pack(&pack_id);
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
            IoEvent::RefreshTtsVoices { language_code } => {
                let items = installed_voice_items(session.installed_tts_voices(&language_code));
                eprintln!(
                    "tts.voices: language={} installed {} voice(s)",
                    language_code,
                    items.len()
                );
                (ui.set_tts_voices)(items);
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
                voice,
            } => {
                tts::play_text_async(
                    Arc::clone(&session),
                    language_code,
                    text,
                    speech_speed,
                    voice,
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
                    &|boxes, _w, _h| {
                        (ui.set_detected_regions)(
                            boxes
                                .iter()
                                .map(|b| SelectionPillItem {
                                    cx: b.oriented_box.cx,
                                    cy: b.oriented_box.cy,
                                    width: b.oriented_box.width,
                                    height: b.oriented_box.height,
                                    angle_degrees: b.oriented_box.angle_radians.to_degrees(),
                                })
                                .collect(),
                        );
                    },
                );

                match result {
                    Ok(image_translation) => {
                        let ui_start = Instant::now();
                        let width = image_translation.image_width;
                        let height = image_translation.image_height;
                        (ui.set_image_result)(ImageResult {
                            translated: qimage_from_rgba_bytes(
                                width,
                                height,
                                &image_translation.rendered_rgba_bytes,
                            ),
                            original: qimage_from_rgba_bytes(
                                width,
                                height,
                                &image_translation.original_rgba_bytes,
                            ),
                            source_words: image_translation.source_words,
                            translated_words: image_translation.translated_words,
                        });
                        // The text panes stay empty: the translation is read off the image
                        // itself now, and selection covers copying it. Detection still runs, to
                        // drive the detected-language card.
                        send_detection_to_ui(&image_translation.extracted_text, &ui);
                        println!("image_ocr postprocess ui={:?}", ui_start.elapsed());
                    }
                    Err(message) => {
                        (ui.set_input_text)(String::new());
                        (ui.set_image_error)(message.clone());
                        (ui.set_output_text)(message);
                    }
                }
                println!("image translation took {:?}", start.elapsed());
            }
            IoEvent::DocumentTranslationRequest {
                input_path,
                from,
                to,
                translate_pdf_images,
            } => {
                let Some(app_paths) = app_paths.clone() else {
                    (ui.set_document_event)(DocumentEvent::Failed {
                        message: "app paths not initialized".to_string(),
                    });
                    continue;
                };
                let cancel = Arc::new(AtomicBool::new(false));
                document_cancel = Some(cancel.clone());
                spawn_document_job(
                    Arc::clone(&session),
                    ui.clone(),
                    app_paths,
                    input_path,
                    from,
                    to,
                    translate_pdf_images,
                    cancel,
                );
            }
            IoEvent::CancelDocumentTranslation => {
                if let Some(cancel) = document_cancel.take() {
                    cancel.store(true, Ordering::Relaxed);
                    session.cancel_ongoing_work();
                }
            }
            IoEvent::StartHttpServer {
                port,
                bind,
                ocr,
                translate_pdf_images,
            } => {
                let Some(app_paths) = app_paths.as_ref() else {
                    (ui.set_http_server_status)("app paths not initialized".to_string());
                    continue;
                };
                let config = HttpServerConfig {
                    bind: match bind {
                        HttpServerBind::Localhost => BindAddress::Localhost,
                        HttpServerBind::AllInterfaces => BindAddress::AllInterfaces,
                    },
                    port,
                    output_dir: format!("{}/translate_file", app_paths.cache),
                    ocr,
                    translate_pdf_images,
                };
                if http_server
                    .as_ref()
                    .is_some_and(|(running, _)| *running == config)
                {
                    continue;
                }
                if let Some((_, server)) = http_server.take() {
                    server.stop();
                }
                let sessions = Arc::clone(&session);
                let started = translator::http::start(
                    config.clone(),
                    Arc::new(move || Some(Arc::clone(&sessions))),
                    font_provider::system_fonts(),
                );
                let status = match started {
                    Ok(server) => {
                        let status = format!("Listening on {}", server.local_addr());
                        http_server = Some((config, server));
                        status
                    }
                    Err(error) => format!("Failed to start: {error}"),
                };
                (ui.set_http_server_status)(status);
            }
            IoEvent::StopHttpServer => {
                if let Some((_, server)) = http_server.take() {
                    server.stop();
                }
                (ui.set_http_server_status)(String::new());
            }
            IoEvent::Shutdown => {
                if let Some((_, server)) = http_server.take() {
                    server.stop();
                }
                tts::stop_playback();
                println!("shutdown signal, exiting");
                break;
            }
        }
    }
    println!("all senders done, closing");
}

/// `{data}/translated/{stem}.{from}-{to}.{ext}` — under the app's own data
/// dir so a ContentHub export can read it.
fn document_output_path(data_dir: &str, input_path: &str, from: &str, to: &str) -> String {
    let input = std::path::Path::new(input_path);
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    format!("{data_dir}/translated/{stem}.{from}-{to}.{ext}")
}

#[allow(clippy::too_many_arguments)]
fn spawn_document_job(
    session: Arc<TranslatorSession>,
    ui: UiCallbacks,
    app_paths: AppPaths,
    input_path: String,
    from: String,
    to: String,
    translate_pdf_images: bool,
    cancel: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let output_path = document_output_path(&app_paths.data, &input_path, &from, &to);
        let start = Instant::now();
        let progress_ui = ui.clone();
        let fonts = font_provider::system_fonts();
        let options = translator::document::DocumentOptions {
            forced_source_code: Some(&from),
            target_code: &to,
            translate_pdf_images,
            txt_layout: translator::txt::TxtLayout::Preserve,
            fonts: &*fonts,
        };
        let result = translator::document::translate_document_path(
            &session,
            &input_path,
            &output_path,
            &options,
            &move |progress: DocumentProgress| {
                (progress_ui.set_document_event)(DocumentEvent::Progress(progress));
            },
            &|| cancel.load(Ordering::Relaxed),
        );
        println!("document translation took {:?}", start.elapsed());

        let event = match result {
            Ok(()) => DocumentEvent::Done { output_path },
            Err(DocumentError::Cancelled) => DocumentEvent::Cancelled,
            Err(DocumentError::Other(message)) => DocumentEvent::Failed { message },
        };
        (ui.set_document_event)(event);
    });
}

fn download_feature(
    target: &DownloadTarget,
    plan: &translator::DownloadPlan,
    data_path: &str,
    ui: &UiCallbacks,
) -> Result<(), String> {
    let total_size = plan.total_size.max(1) as usize;
    let total_downloaded = Arc::new(AtomicUsize::new(0));
    let download_complete = Arc::new(AtomicBool::new(false));

    (ui.set_feature_progress)(target.clone(), DownloadProgress::Running(0.00001));

    let progress_total_downloaded = total_downloaded.clone();
    let progress_download_complete = download_complete.clone();
    let progress_ui = ui.clone();
    let progress_target = target.clone();

    let progress_thread = thread::spawn(move || {
        const UPDATE_THRESHOLD: usize = 1024 * 1024;
        const UPDATE_INTERVAL: Duration = Duration::from_millis(120);
        let mut last_update = 0;

        while !progress_download_complete.load(Ordering::Relaxed) {
            thread::sleep(UPDATE_INTERVAL);

            let current = progress_total_downloaded.load(Ordering::Relaxed);
            if current.saturating_sub(last_update) >= UPDATE_THRESHOLD {
                let percent = current as f32 / total_size as f32;
                (progress_ui.set_feature_progress)(
                    progress_target.clone(),
                    DownloadProgress::Running(percent),
                );
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
