use std::sync::Arc;

use qmetaobject::QString;
use translator::TranslatorSession;

use crate::IoEvent;
use crate::model::{FeatureKind, Language, Screen};
use crate::settings::{Settings, save_settings};
use crate::tts::SpeechSpeed;
use crate::uri_handler::LaunchIntent;

use super::AppBridge;

/// Which shell the app is running under. Ubuntu Touch is the only one with content-hub and a
/// phone-shaped screen; a native run, `clickable desktop`, and anything else are all desktops.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Platform {
    Desktop,
    UbuntuTouch,
}

impl Platform {
    /// Detects the phone rather than the desktop, so anything unrecognized — including a plain
    /// `cargo run` outside clickable — gets the desktop affordances instead of silently taking
    /// the content-hub path that only exists on device.
    pub(crate) fn detect() -> Self {
        // Explicit override wins, so `clickable desktop` and desktop-dev.sh keep working even if
        // the launcher happens to set the device variables.
        let forced_desktop = std::env::var_os("CLICKABLE_DESKTOP_MODE").is_some();
        // ubuntu-app-launch sets APP_ID for every click app it starts; desktops do not.
        let platform = match std::env::var_os("APP_ID") {
            Some(_) if !forced_desktop => Platform::UbuntuTouch,
            _ => Platform::Desktop,
        };
        eprintln!("platform: {platform:?} (forced_desktop={forced_desktop})");
        platform
    }

    pub(crate) fn is_desktop(self) -> bool {
        matches!(self, Platform::Desktop)
    }
}

/// Where the UI's density comes from. Every `dp()` value in the QML is authored against the 8px
/// grid unit, so the factor below is what turns those into device pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum UiScale {
    /// Lomiri publishes the device's grid unit in `GRID_UNIT_PX`, read from
    /// `/etc/ubuntu-touch-session.d/<device>.conf`. On Ubuntu Touch it is the only density signal
    /// worth trusting: ports such as the Fairphone 5 report a panel DPI that is far too low, and
    /// anything derived from `Screen.pixelDensity` inherits that mistake.
    ShellGridUnit(f64),
    /// Nothing published a density, so Qt's own high-DPI handling owns the screen scaling and QML
    /// keeps working in logical pixels.
    QtManaged,
}

const BASELINE_GRID_UNIT_PX: f64 = 8.0;

/// The desktop UI font size the QML pixel metrics were laid out against. Desktops disagree on
/// how large "normal" text is -- ~19px here, 12px on Windows -- so the metrics are scaled by how
/// far the host's default font differs, rather than imposing one desktop's convention on all.
const BASELINE_UI_FONT_PX: f64 = 19.0;

impl UiScale {
    pub(crate) fn detect() -> Self {
        let Some(raw) = std::env::var_os("GRID_UNIT_PX") else {
            return UiScale::QtManaged;
        };
        let grid_unit = raw
            .to_str()
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|px| px.is_finite() && *px > 0.0);
        match grid_unit {
            Some(px) => UiScale::ShellGridUnit(px),
            None => {
                eprintln!("ui.scale: ignoring unusable GRID_UNIT_PX={raw:?}");
                UiScale::QtManaged
            }
        }
    }

    /// `qt_ui_font_px` yields the host desktop's default UI font size. It is a closure because
    /// only the desktop arm needs it: a shell that publishes GRID_UNIT_PX has already decided the
    /// density, so on Lomiri the Qt font is never consulted and cannot fail the app.
    pub(crate) fn factor(self, qt_ui_font_px: impl FnOnce() -> f64) -> f64 {
        match self {
            UiScale::ShellGridUnit(px) => px / BASELINE_GRID_UNIT_PX,
            UiScale::QtManaged => qt_ui_font_px() / BASELINE_UI_FONT_PX,
        }
    }

    /// Qt has to be told about high-DPI scaling before its application object exists, and only
    /// when the shell did not already hand us a factor — doing both would scale the UI twice.
    pub(crate) fn qt_owns_scaling(self) -> bool {
        matches!(self, UiScale::QtManaged)
    }
}

impl AppBridge {
    pub fn new(
        languages: Vec<Language>,
        bus_tx: std::sync::mpsc::Sender<IoEvent>,
        asset_dir: String,
        config_dir: String,
        data_dir: String,
        settings: Settings,
        session: Arc<TranslatorSession>,
        ui_scale: UiScale,
        qt_ui_font_px: impl FnOnce() -> f64,
    ) -> Self {
        let current_screen = std::env::var("START_SCREEN")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(Screen::NoLanguages.as_i32());
        let desktop_mode = Platform::detect().is_desktop();
        // Asked of the document dispatcher rather than of a cfg, so the picker's filters can
        // never drift from the extensions translate_document actually dispatches on.
        let pdf_available = crate::document::supported_document_extension("probe.pdf").is_some();
        let scale_factor = ui_scale.factor(qt_ui_font_px);
        eprintln!("ui.scale: {ui_scale:?} factor={scale_factor}");
        let mut app = AppBridge {
            current_screen,
            bus_tx: Some(bus_tx),
            session: Some(session),
            previous_screen: Screen::Translation,
            desktop_mode,
            live_camera_available: cfg!(feature = "live"),
            pdf_available,
            ui_scale: scale_factor,
            doc_translate_images: true,
            ..Default::default()
        };
        if cfg!(debug_assertions) {
            let automation_from = std::env::var("AUTOMATION_FROM").unwrap_or_default();
            let automation_to = std::env::var("AUTOMATION_TO").unwrap_or_default();
            let automation_text = std::env::var("AUTOMATION_TEXT").unwrap_or_default();
            let automation_screenshot_path =
                std::env::var("AUTOMATION_SCREENSHOT_PATH").unwrap_or_default();
            app.automation_enabled = !automation_from.is_empty()
                || !automation_to.is_empty()
                || !automation_text.is_empty()
                || !automation_screenshot_path.is_empty();
            app.automation_from = QString::from(automation_from);
            app.automation_to = QString::from(automation_to);
            app.automation_text = QString::from(automation_text);
            app.automation_screenshot_path = QString::from(automation_screenshot_path);
            app.automation_quit_after_screenshot = std::env::var("AUTOMATION_QUIT")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        }
        app.asset_dir = asset_dir;
        app.config_dir = config_dir;
        app.data_dir = data_dir;
        app.source_language_code = "en".to_string();
        app.target_language_code = "en".to_string();
        app.source_language_name = QString::from("English");
        app.target_language_name = QString::from("English");
        app.live_ocr_active = true;

        app.disable_auto_detect = settings.disable_auto_detect;
        app.ocr_background_mode = QString::from(settings.ocr_background_mode);
        app.ocr_min_confidence = settings.ocr_min_confidence;
        app.ocr_max_image_size = settings.ocr_max_image_size;
        app.catalog_index_url = QString::from(settings.catalog_index_url);
        app.disable_ocr = settings.disable_ocr;
        app.show_transliteration_output = settings.show_transliteration_output;
        app.show_transliteration_input = settings.show_transliteration_input;
        app.tts_playback_speed = SpeechSpeed::new(settings.tts_playback_speed).value();
        app.tts_playback_speed_min = SpeechSpeed::MIN;
        app.tts_playback_speed_max = SpeechSpeed::MAX;
        app.tts_playback_speed_step = SpeechSpeed::STEP;
        app.tts_voice_selections = settings.tts_voice_selections.clone();

        app.set_languages_value(languages);

        if let Some(lang) = app
            .find_language_by_code(&settings.default_from_code)
            .cloned()
        {
            app.set_source_language_by_name(lang.name);
        }
        if let Some(lang) = app
            .find_language_by_code(&settings.default_to_code)
            .cloned()
        {
            app.set_target_language_by_name(lang.name);
        }

        app
    }

    pub(crate) fn persist_settings(&self) {
        let settings = Settings {
            default_from_code: self.source_language_code.clone(),
            default_to_code: self.target_language_code.clone(),
            ocr_background_mode: self.ocr_background_mode.to_string(),
            ocr_min_confidence: self.ocr_min_confidence,
            ocr_max_image_size: self.ocr_max_image_size,
            catalog_index_url: self.catalog_index_url.to_string(),
            disable_ocr: self.disable_ocr,
            disable_auto_detect: self.disable_auto_detect,
            show_transliteration_output: self.show_transliteration_output,
            show_transliteration_input: self.show_transliteration_input,
            tts_playback_speed: self.tts_playback_speed,
            tts_voice_selections: self.tts_voice_selections.clone(),
        };
        save_settings(&self.config_dir, &settings);
    }

    pub(crate) fn set_detected_language_code_value(&mut self, code: &str) {
        if self.detected_language_code != code {
            self.detected_language_code = code.to_string();
            self.refresh_detected_language();
        }
    }

    pub(crate) fn defer_launch_intent(&mut self, intent: LaunchIntent) {
        self.pending_launch_intent = Some(intent);
    }

    pub(crate) fn apply_launch_intent(&mut self, intent: LaunchIntent) {
        match intent {
            LaunchIntent::LiveCamera => {
                if self.disable_ocr || !self.has_languages {
                    eprintln!(
                        "uri handler: dropping camera intent (disable_ocr={} has_languages={})",
                        self.disable_ocr, self.has_languages
                    );
                    return;
                }
                self.open_live_camera();
            }
        }
    }

    pub(crate) fn set_current_screen(&mut self, screen: Screen) {
        let screen = screen.as_i32();
        if self.current_screen != screen {
            self.current_screen = screen;
            self.current_screen_changed();
        }
        if screen != Screen::ManageLanguages.as_i32() {
            self.set_manage_tts_picker_open_value(false);
        }
    }

    pub(crate) fn send_feature_request(
        &self,
        code: String,
        feature: FeatureKind,
        download: bool,
        selected_tts_pack_id: Option<String>,
    ) {
        let event = if download {
            IoEvent::DownloadRequest {
                code,
                feature,
                selected_tts_pack_id,
            }
        } else {
            IoEvent::DeleteLanguage { code, feature }
        };
        self.send_io(event);
    }

    pub(crate) fn send_io(&self, event: IoEvent) {
        if let Some(bus_tx) = &self.bus_tx {
            bus_tx.send(event).unwrap();
        }
    }
}
