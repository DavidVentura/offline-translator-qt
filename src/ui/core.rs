use std::sync::Arc;

use qmetaobject::QString;
use translator::TranslatorSession;

use crate::IoEvent;
use crate::model::{FeatureKind, Language, Screen};
use crate::settings::{Settings, save_settings};

use super::AppBridge;

impl AppBridge {
    pub fn new(
        languages: Vec<Language>,
        bus_tx: std::sync::mpsc::Sender<IoEvent>,
        asset_dir: String,
        config_dir: String,
        data_dir: String,
        settings: Settings,
        session: Arc<TranslatorSession>,
    ) -> Self {
        let current_screen = std::env::var("START_SCREEN")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(Screen::NoLanguages.as_i32());
        let desktop_mode = std::env::var_os("CLICKABLE_DESKTOP_MODE").is_some();
        let mut app = AppBridge {
            current_screen,
            bus_tx: Some(bus_tx),
            session: Some(session),
            previous_screen: Screen::Translation,
            desktop_mode,
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
        app.tts_playback_speed = settings.tts_playback_speed.clamp(0.5, 2.0);
        app.tts_voice_overrides = settings.tts_voice_overrides.clone();

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
            tts_voice_overrides: self.tts_voice_overrides.clone(),
        };
        save_settings(&self.config_dir, &settings);
    }

    pub(crate) fn set_detected_language_code_value(&mut self, code: &str) {
        if self.detected_language_code != code {
            self.detected_language_code = code.to_string();
            self.refresh_detected_language();
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
