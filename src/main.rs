// cpp! expands its body token-by-token recursively; the persistent-texture
// block in rendered_image_item.rs needs a higher limit than the default.
#![recursion_limit = "1024"]

mod catalog_state;
mod data;
mod document;
mod download;
mod eventloop;
mod image_ocr;
#[cfg(feature = "live")]
mod live_camera_item;
#[cfg(feature = "live")]
mod live_gpu;
#[cfg(feature = "live")]
mod live_ocr;
mod model;
mod rendered_image_item;
mod settings;
mod tts;
mod ui;
mod uri_handler;

use cpp::cpp;
use qmetaobject::*;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;

use translator::TranslatorSession;

use crate::catalog_state::{bundled_catalog, languages_from_overview};
use crate::model::FeatureKind;
use crate::settings::load_settings;
use crate::ui::{AppBridge, create_ui_callbacks};
use crate::uri_handler::LaunchIntent;

const APP_NAME: &str = "dev.davidv.translator";

cpp! {{
    #include <QtCore/QCoreApplication>
    #include <QtCore/QString>
}}

#[cfg(feature = "live")]
unsafe extern "C" {
    fn register_live_ocr_filter();
}

#[derive(Clone, Debug)]
struct AppPaths {
    config: String,
    data: String,
}

enum IoEvent {
    DownloadRequest {
        code: String,
        feature: FeatureKind,
        selected_tts_pack_id: Option<String>,
    },
    DeleteLanguage {
        code: String,
        feature: FeatureKind,
    },
    SetAppPaths(AppPaths),
    TranslationRequest {
        text: String,
        from: String,
        to: String,
    },
    ImageTranslationRequest {
        image_path: String,
        from: String,
        to: String,
        min_confidence: u32,
        max_image_size: u32,
        background_mode: String,
    },
    DocumentTranslationRequest {
        input_path: String,
        from: String,
        to: String,
        translate_pdf_images: bool,
    },
    CancelDocumentTranslation,
    RefreshTtsVoices {
        language_code: String,
        selected_voice_name: String,
    },
    WarmTtsModel {
        language_code: String,
    },
    SpeakRequest {
        language_code: String,
        text: String,
        speech_speed: f32,
        voice_name: String,
    },
    StopTts,
    Shutdown,
}

#[cfg(unix)]
fn get_app_paths() -> AppPaths {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/home/{}", whoami::username())));
    let data_root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));

    AppPaths {
        data: data_root.join(APP_NAME).display().to_string(),
        config: config_root.join(APP_NAME).display().to_string(),
    }
}

#[cfg(windows)]
fn get_app_paths() -> AppPaths {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("C:/Users/{}/AppData/Local", whoami::username())));
    let roaming = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!("C:/Users/{}/AppData/Roaming", whoami::username()))
        });

    AppPaths {
        data: local.join(APP_NAME).display().to_string(),
        config: roaming.join(APP_NAME).display().to_string(),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,translator=debug"),
    )
    .init();

    let args: Vec<String> = std::env::args().collect();
    #[cfg(feature = "live")]
    if let Some(pos) = args.iter().position(|a| a == "--bench-live") {
        let image = args.get(pos + 1).cloned().unwrap_or_default();
        let from = args
            .get(pos + 2)
            .cloned()
            .unwrap_or_else(|| "en".to_string());
        let to = args
            .get(pos + 3)
            .cloned()
            .unwrap_or_else(|| "nl".to_string());
        let max_side = args
            .get(pos + 4)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1000);
        let app_paths = get_app_paths();
        let session = Arc::new(TranslatorSession::from_catalog(
            bundled_catalog(),
            app_paths.data.clone(),
        ));
        live_ocr::run_benchmark(session, &image, &from, &to, max_side, 60);
        return Ok(());
    }
    if let Some(pos) = args.iter().position(|a| a == "--translate-doc") {
        let input = args.get(pos + 1).cloned().unwrap_or_default();
        let from = args
            .get(pos + 2)
            .cloned()
            .unwrap_or_else(|| "en".to_string());
        let to = args
            .get(pos + 3)
            .cloned()
            .unwrap_or_else(|| "nl".to_string());
        let pdf_images = args.iter().any(|a| a == "--pdf-images");
        let app_paths = get_app_paths();
        let session = Arc::new(TranslatorSession::from_catalog(
            bundled_catalog(),
            app_paths.data.clone(),
        ));
        session.refresh_snapshot();
        let output = format!("{input}.{from}-{to}.out");
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let result = document::translate_document(
            &session,
            &input,
            &output,
            &from,
            &to,
            pdf_images,
            &cancel,
            &|progress| eprintln!("doc progress: {progress:?}"),
        );
        eprintln!("doc result: {result:?} output={output}");
        return Ok(());
    }

    qmetaobject::log::init_qt_to_rust();

    let ui_scale = ui::UiScale::detect();
    if ui_scale.qt_owns_scaling() {
        cpp!(unsafe [] {
            QCoreApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
        });
    }

    // QSettings refuses to guess a file path without these, so every QSettings
    // in this process — ours and the ones qtubuntu-camera opens for us — would
    // otherwise be a no-op. The organization doubles as the config directory
    // name, which under click confinement has to be the package name.
    let identity = QString::from(APP_NAME);
    cpp!(unsafe [identity as "QString"] {
        QCoreApplication::setOrganizationName(identity);
        QCoreApplication::setOrganizationDomain(identity);
        QCoreApplication::setApplicationName(identity);
    });

    qml_register_type::<rendered_image_item::RenderedImageItem>(
        c"TranslatorUi",
        1,
        0,
        c"RenderedImageItem",
    );
    #[cfg(feature = "live")]
    {
        qml_register_type::<live_camera_item::LiveCameraItem>(
            c"TranslatorUi",
            1,
            0,
            c"LiveCameraItem",
        );
        unsafe { register_live_ocr_filter() };
    }

    let (bus_tx, bus_rx) = mpsc::channel::<IoEvent>();
    let app_paths = get_app_paths();
    let catalog = bundled_catalog();
    let session = Arc::new(TranslatorSession::from_catalog(
        catalog,
        app_paths.data.clone(),
    ));
    #[cfg(feature = "live")]
    live_ocr::init_live_pipeline(Arc::clone(&session));
    let initial_languages = languages_from_overview(session.language_overview());
    let main_qml = find_main_qml()?;
    let asset_dir = find_asset_dir(&main_qml)?;
    let settings = load_settings(&app_paths.config);
    let mut engine = QmlEngine::new();
    let app = QObjectBox::new(AppBridge::new(
        initial_languages,
        bus_tx.clone(),
        asset_dir,
        app_paths.config.clone(),
        app_paths.data.clone(),
        settings,
        Arc::clone(&session),
        ui_scale,
    ));

    engine.set_object_property("app".into(), app.pinned());

    let uri_app = QPointer::from(app.pinned().borrow());
    let deliver_intent = queued_callback(move |intent: LaunchIntent| {
        if let Some(app) = uri_app.as_pinned() {
            app.borrow_mut().apply_launch_intent(intent);
        }
    });
    uri_handler::install(deliver_intent);

    let ui_callbacks = create_ui_callbacks(QPointer::from(app.pinned().borrow()));
    #[cfg(feature = "live")]
    live_ocr::set_live_frame_tick(ui_callbacks.notify_live_frame.clone());
    let session_for_loop = Arc::clone(&session);
    let jh = std::thread::spawn(move || {
        eventloop::run_eventloop(bus_rx, ui_callbacks, session_for_loop)
    });

    bus_tx.send(IoEvent::SetAppPaths(app_paths)).unwrap();
    if let Some(intent) = uri_handler::intent_from_args(&args) {
        app.pinned().borrow_mut().defer_launch_intent(intent);
    }
    engine.load_file(main_qml.into());
    engine.exec();

    // Tear down the engine (and its live QML bindings) before the "app" context
    // object it reads from. The reverse order destroys `app` while bindings
    // still reference it, spewing "property of null" TypeErrors during teardown.
    drop(engine);

    bus_tx.send(IoEvent::Shutdown).unwrap();
    drop(bus_tx);
    jh.join().unwrap();

    Ok(())
}

fn find_main_qml() -> Result<String, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir.join("qml/Main.qml");
    if std::env::var_os("CLICKABLE_DESKTOP_MODE").is_some() && dev_path.exists() {
        return Ok(dev_path.canonicalize()?.display().to_string());
    }

    let exe = std::env::current_exe()?;
    if let Some(qml_path) = exe
        .parent()
        .and_then(|bin_dir| bin_dir.parent())
        .map(|qml_dir| qml_dir.join("Main.qml"))
        .filter(|path| path.exists())
    {
        return Ok(qml_path.display().to_string());
    }

    if let Some(qml_path) = exe
        .parent()
        .and_then(|bin_dir| bin_dir.parent())
        .map(|prefix| {
            prefix
                .join("share")
                .join(env!("CARGO_PKG_NAME"))
                .join("qml")
                .join("Main.qml")
        })
        .filter(|path| path.exists())
    {
        return Ok(qml_path.display().to_string());
    }

    // Windows ships qml/ and assets/ next to the executable; there is no
    // prefix to hang share/ off.
    if let Some(qml_path) = exe
        .parent()
        .map(|bin_dir| bin_dir.join("qml").join("Main.qml"))
        .filter(|path| path.exists())
    {
        return Ok(qml_path.display().to_string());
    }

    if dev_path.exists() {
        return Ok(dev_path.display().to_string());
    }

    Err("Could not locate Main.qml".into())
}

fn find_asset_dir(main_qml: &str) -> Result<String, Box<dyn Error>> {
    let main_qml = PathBuf::from(main_qml);
    let candidates = [
        main_qml.parent().map(|dir| dir.join("../assets")),
        main_qml.parent().map(|dir| dir.join("../../assets")),
    ];

    for candidate in candidates.into_iter().flatten() {
        let candidate = candidate.canonicalize().unwrap_or(candidate);
        if candidate.exists() {
            return Ok(candidate.display().to_string());
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = manifest_dir.join("assets");
    if dev_path.exists() {
        return Ok(dev_path.canonicalize()?.display().to_string());
    }

    Err("Could not locate assets directory".into())
}
