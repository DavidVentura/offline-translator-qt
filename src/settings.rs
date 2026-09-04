use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::TtsVoiceSelection;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_lang_code")]
    pub default_from_code: String,
    #[serde(default = "default_lang_code")]
    pub default_to_code: String,
    #[serde(default = "default_ocr_background_mode")]
    pub ocr_background_mode: String,
    #[serde(default = "default_ocr_min_confidence")]
    pub ocr_min_confidence: i32,
    #[serde(default = "default_ocr_max_image_size")]
    pub ocr_max_image_size: i32,
    #[serde(default = "default_catalog_index_url")]
    pub catalog_index_url: String,
    #[serde(default)]
    pub disable_ocr: bool,
    #[serde(default)]
    pub disable_auto_detect: bool,
    #[serde(default)]
    pub show_transliteration_output: bool,
    #[serde(default)]
    pub show_transliteration_input: bool,
    #[serde(default = "default_tts_playback_speed")]
    pub tts_playback_speed: f32,
    #[serde(default)]
    pub tts_voice_selections: BTreeMap<String, TtsVoiceSelection>,
}

fn default_lang_code() -> String {
    "en".to_string()
}
fn default_ocr_background_mode() -> String {
    "Auto-detect Colors".to_string()
}
fn default_ocr_min_confidence() -> i32 {
    75
}
fn default_ocr_max_image_size() -> i32 {
    1000
}
fn default_catalog_index_url() -> String {
    "https://offline-translator.davidv.dev/index".to_string()
}
fn default_tts_playback_speed() -> f32 {
    1.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_from_code: default_lang_code(),
            default_to_code: default_lang_code(),
            ocr_background_mode: default_ocr_background_mode(),
            ocr_min_confidence: default_ocr_min_confidence(),
            ocr_max_image_size: default_ocr_max_image_size(),
            catalog_index_url: default_catalog_index_url(),
            disable_ocr: false,
            disable_auto_detect: false,
            show_transliteration_output: false,
            show_transliteration_input: false,
            tts_playback_speed: default_tts_playback_speed(),
            tts_voice_selections: BTreeMap::new(),
        }
    }
}

pub fn load_settings(config_dir: &str) -> Settings {
    let path = Path::new(config_dir).join("settings.json");
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Settings::default(),
    };
    eprintln!("settings: loading from {}", path.display());
    let settings: Settings = serde_json::from_str(&contents).unwrap_or_default();
    eprintln!(
        "settings: loaded from_code={} to_code={} ocr_bg={}",
        settings.default_from_code, settings.default_to_code, settings.ocr_background_mode
    );
    settings
}

pub fn save_settings(config_dir: &str, settings: &Settings) {
    let path = Path::new(config_dir).join("settings.json");
    let json = serde_json::to_string_pretty(settings).expect("settings should serialize");
    let _ = fs::write(path, json);
}
