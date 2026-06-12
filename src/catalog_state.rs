use std::io::Read;

use translator::{LanguageCatalog, LanguageOverview, parse_and_validate_catalog};

use crate::data::INDEX_JSON;
use crate::model::{Direction, Language, TtsVoicePackOption, TtsVoicePickerRegion};

pub fn bundled_catalog() -> LanguageCatalog {
    let mut decoder = flate2::read::GzDecoder::new(INDEX_JSON);
    let mut json = String::new();
    decoder
        .read_to_string(&mut json)
        .expect("bundled index should decompress");

    let catalog = parse_and_validate_catalog(&json).expect("bundled index should parse");
    eprintln!(
        "catalog loaded: format={} languages={}",
        catalog.format_version,
        catalog.language_list().len()
    );
    catalog
}

pub fn languages_from_overview(overview: Vec<LanguageOverview>) -> Vec<Language> {
    let mut languages = overview
        .into_iter()
        .map(|entry| {
            let library_language = entry.language;
            let code = library_language.code.clone();
            let availability = entry.availability;

            let direction = match (
                availability.has_to_english || code == "en",
                availability.has_from_english || code == "en",
            ) {
                (true, true) => Direction::Both,
                (true, false) => Direction::FromOnly,
                (false, true) => Direction::ToOnly,
                (false, false) => {
                    if code == "en" {
                        Direction::Both
                    } else {
                        Direction::FromOnly
                    }
                }
            };

            let tts_voice_picker_regions = entry
                .tts_voice_regions
                .into_iter()
                .map(|region| TtsVoicePickerRegion {
                    code: region.code,
                    display_name: region.display_name,
                    voices: region
                        .voices
                        .into_iter()
                        .map(|voice| TtsVoicePackOption {
                            pack_id: voice.pack_info.pack_id,
                            display_name: voice.pack_info.display_name,
                            quality: voice.pack_info.quality,
                            size_bytes: voice.pack_info.size_bytes,
                            installed: voice.installed,
                        })
                        .collect(),
                })
                .collect();

            let built_in = library_language.is_english();
            Language {
                code,
                name: library_language.display_name,
                script: library_language.script,
                dictionary_code: library_language.dictionary_code,
                direction,
                built_in,
                // English is the pivot: its overview core size sums every en↔X
                // pair (~2 GB), but there is nothing to download for it.
                core_size_bytes: if built_in { 0 } else { entry.core_size_bytes },
                core_installed: entry.core_installed,
                core_progress: 0.0,
                dictionary_size_bytes: entry.dictionary_size_bytes,
                dictionary_installed: entry.dictionary_installed,
                dictionary_progress: 0.0,
                tts_size_bytes: entry.tts_size_bytes,
                tts_installed: availability.tts_files,
                tts_progress: 0.0,
                tts_voice_picker_regions,
            }
        })
        .collect::<Vec<_>>();

    languages.sort_by(|left, right| left.name.cmp(&right.name));
    languages
}

pub fn format_size(size_bytes: u64) -> String {
    const ONE_KB: u64 = 1024;
    const ONE_MB: u64 = 1024 * 1024;

    match size_bytes {
        0..ONE_KB => "<1 KB".to_string(),
        ONE_KB..ONE_MB => format!("{} KB", size_bytes / ONE_KB),
        _ => format!("{} MB", size_bytes / ONE_MB),
    }
}

pub fn total_size(language: &Language) -> u64 {
    language.core_size_bytes + language.dictionary_size_bytes + language.tts_size_bytes
}
