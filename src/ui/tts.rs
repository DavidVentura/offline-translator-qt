use std::collections::BTreeMap;

use qmetaobject::QString;

use crate::IoEvent;
use crate::catalog_state::format_size;
use translator::InstalledTtsPack;

use crate::model::{
    DownloadProgress, FeatureKind, Language, TtsVoicePickerRegion, TtsVoiceSelection,
};
use crate::tts::SpeechSpeed;

use super::{AppBridge, ManageTtsVoicePackListItem, TtsVoiceListItem};

/// A voice pack the picker asked for that the download bus has not finished with yet. Requests
/// queue behind each other on the bus, so only one is ever `Running`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PackDownloadState {
    Queued,
    Running(f32),
}

/// Flattens installed packs into the main-screen voice list. Multi-speaker packs keep their pack
/// name as a group label; a single-speaker pack is one voice named after the pack, with an empty
/// speaker name since the model has none to select by.
pub fn installed_voice_items(packs: Vec<InstalledTtsPack>) -> Vec<TtsVoiceListItem> {
    packs
        .into_iter()
        .flat_map(|pack| {
            let multi_speaker = pack.voices.len() > 1;
            pack.voices
                .into_iter()
                .map(move |speaker| TtsVoiceListItem {
                    pack_id: pack.pack_id.clone().into(),
                    name: if multi_speaker {
                        speaker.name.clone().into()
                    } else {
                        QString::default()
                    },
                    display_name: if multi_speaker {
                        speaker.name.into()
                    } else {
                        pack.display_name.clone().into()
                    },
                    pack_display_name: if multi_speaker {
                        pack.display_name.clone().into()
                    } else {
                        QString::default()
                    },
                })
        })
        .collect()
}

/// The stored choice wins while it still exists; otherwise the first installed voice stands in,
/// matching what the synthesizer does when given no pack.
pub(crate) fn resolve_tts_voice_selection<'a>(
    items: &'a [TtsVoiceListItem],
    stored: Option<&TtsVoiceSelection>,
) -> Option<&'a TtsVoiceListItem> {
    stored
        .and_then(|selection| {
            items.iter().find(|item| {
                item.pack_id.to_string() == selection.pack_id
                    && item.name.to_string() == selection.speaker.as_deref().unwrap_or("")
            })
        })
        .or_else(|| items.first())
}

struct PickerRow {
    installed: bool,
    region_name: String,
    display_name: String,
    item: ManageTtsVoicePackListItem,
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Installed voices first so the delete actions sit together, then the catalog's remaining
/// voices; the region only appears once it disambiguates between several.
pub(crate) fn manage_tts_picker_items(
    regions: &[TtsVoicePickerRegion],
    downloads: &BTreeMap<String, PackDownloadState>,
) -> Vec<ManageTtsVoicePackListItem> {
    let show_region = regions.len() > 1;
    let mut rows = regions
        .iter()
        .flat_map(|region| {
            let region_name = if region.display_name.is_empty() {
                region.code.clone()
            } else {
                region.display_name.clone()
            };
            region.voices.iter().map(move |voice| {
                let quality = voice
                    .quality
                    .as_deref()
                    .map(capitalize_first)
                    .unwrap_or_else(|| "Default quality".to_string());
                let mut meta = vec![quality, format_size(voice.size_bytes)];
                if show_region {
                    meta.push(region_name.clone());
                }
                let download = downloads.get(&voice.pack_id).copied();
                let section_text = if voice.installed {
                    "Downloaded"
                } else {
                    "Available"
                };
                PickerRow {
                    installed: voice.installed,
                    region_name: region_name.clone(),
                    display_name: voice.display_name.clone(),
                    item: ManageTtsVoicePackListItem {
                        pack_id: voice.pack_id.clone().into(),
                        section_text: section_text.into(),
                        voice_display_name: voice.display_name.clone().into(),
                        meta_text: meta.join(" · ").into(),
                        installed: voice.installed,
                        queued: download == Some(PackDownloadState::Queued),
                        progress: match download {
                            Some(PackDownloadState::Running(fraction)) => fraction,
                            _ => 0.0,
                        },
                    },
                }
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .installed
            .cmp(&left.installed)
            .then_with(|| left.region_name.cmp(&right.region_name))
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    rows.into_iter().map(|row| row.item).collect()
}

impl AppBridge {
    pub(crate) fn set_tts_state_value(&mut self, loading: bool, playing: bool) {
        if self.tts_loading != loading {
            self.tts_loading = loading;
            self.tts_loading_changed();
        }
        if self.tts_playing != playing {
            self.tts_playing = playing;
            self.tts_playing_changed();
        }
    }

    pub(crate) fn set_tts_voices_value(&mut self, items: Vec<TtsVoiceListItem>) {
        let item_count = items.len() as i32;
        self.tts_voice_options_model.borrow_mut().reset_data(items);

        if self.tts_voice_option_count != item_count {
            self.tts_voice_option_count = item_count;
            self.tts_voice_option_count_changed();
        }

        self.apply_selected_voice();
    }

    /// Recomputes the selected-voice properties from the stored choice and the current list.
    fn apply_selected_voice(&mut self) {
        let stored = self.tts_voice_selections.get(&self.target_language_code);
        let (pack_id, name, display_name) = {
            let model = self.tts_voice_options_model.borrow();
            let items = model.iter().cloned().collect::<Vec<_>>();
            match resolve_tts_voice_selection(&items, stored) {
                Some(item) => (
                    item.pack_id.clone(),
                    item.name.clone(),
                    item.display_name.clone(),
                ),
                None => (QString::default(), QString::default(), "Default".into()),
            }
        };
        self.set_selected_voice_properties(pack_id, name, display_name);
    }

    fn set_selected_voice_properties(
        &mut self,
        pack_id: QString,
        name: QString,
        display_name: QString,
    ) {
        if self.tts_selected_voice_pack_id != pack_id {
            self.tts_selected_voice_pack_id = pack_id;
            self.tts_selected_voice_pack_id_changed();
        }
        if self.tts_selected_voice_name != name {
            self.tts_selected_voice_name = name;
            self.tts_selected_voice_name_changed();
        }
        if self.tts_selected_voice_display_name != display_name {
            self.tts_selected_voice_display_name = display_name;
            self.tts_selected_voice_display_name_changed();
        }
    }

    pub(crate) fn set_manage_tts_picker_open_value(&mut self, value: bool) {
        if self.manage_tts_picker_open != value {
            self.manage_tts_picker_open = value;
            self.manage_tts_picker_open_changed();
        }

        if !value {
            self.manage_tts_picker_language_code.clear();
        }
    }

    pub(crate) fn set_manage_tts_picker_language_name_value(&mut self, value: String) {
        let value = QString::from(value);
        if self.manage_tts_picker_language_name != value {
            self.manage_tts_picker_language_name = value;
            self.manage_tts_picker_language_name_changed();
        }
    }

    pub(crate) fn toggle_speak_output_impl(&mut self) {
        if self.tts_loading || self.tts_playing {
            eprintln!("tts.ui: speaker pressed while active; stopping playback");
            self.stop_tts();
            return;
        }

        let text = self.output_text.to_string();
        if text.trim().is_empty() || !self.tts_available {
            eprintln!(
                "tts.ui: speaker pressed but ignored text_empty={} tts_available={}",
                text.trim().is_empty(),
                self.tts_available
            );
            return;
        }

        let voice = self
            .tts_voice_selections
            .get(&self.target_language_code)
            .cloned();
        let speech_speed = SpeechSpeed::new(self.tts_playback_speed);
        eprintln!(
            "tts.ui: speaker pressed target_language={} chars={} speed={} voice={:?}",
            self.target_language_code,
            text.chars().count(),
            speech_speed.value(),
            voice
        );

        self.send_io(IoEvent::SpeakRequest {
            language_code: self.target_language_code.clone(),
            text,
            speech_speed,
            voice,
        });
    }

    pub(crate) fn prepare_tts_options_impl(&mut self) {
        if self.tts_available {
            eprintln!(
                "tts.ui: opening voice options target_language={}",
                self.target_language_code
            );
            self.refresh_tts_voices();
        }
    }

    pub(crate) fn set_tts_playback_speed_impl(&mut self, value: f32) {
        let quantized = SpeechSpeed::new(value).value();
        if (self.tts_playback_speed - quantized).abs() > f32::EPSILON {
            self.tts_playback_speed = quantized;
            self.tts_playback_speed_changed();
            self.persist_settings();
        }
    }

    pub(crate) fn set_tts_voice_impl(&mut self, selection: TtsVoiceSelection) {
        self.tts_voice_selections
            .insert(self.target_language_code.clone(), selection);
        self.persist_settings();
        self.apply_selected_voice();
    }

    pub(crate) fn open_tts_download_picker_impl(&mut self, code: String) {
        let Some(language) = self.find_language_by_code(&code).cloned() else {
            return;
        };

        let voices = language
            .tts_voice_picker_regions
            .iter()
            .flat_map(|region| region.voices.iter())
            .collect::<Vec<_>>();

        if voices.is_empty() {
            self.send_feature_request(code, FeatureKind::Tts, true, None);
            return;
        }

        if let [single] = voices.as_slice()
            && !single.installed
        {
            self.send_feature_request(code, FeatureKind::Tts, true, Some(single.pack_id.clone()));
            return;
        }

        self.manage_tts_picker_language_code = language.code;
        self.set_manage_tts_picker_language_name_value(language.name);
        self.refresh_manage_tts_picker_model();
        self.set_manage_tts_picker_open_value(true);
    }

    pub(crate) fn download_tts_pack_impl(&mut self, pack_id: String) {
        if self.manage_tts_picker_language_code.is_empty() {
            return;
        }
        self.manage_tts_pack_downloads
            .insert(pack_id.clone(), PackDownloadState::Queued);
        self.send_feature_request(
            self.manage_tts_picker_language_code.clone(),
            FeatureKind::Tts,
            true,
            Some(pack_id.clone()),
        );
        self.update_manage_tts_pack_item(&pack_id);
    }

    pub(crate) fn delete_tts_pack_impl(&mut self, pack_id: String) {
        let code = self.manage_tts_picker_language_code.clone();
        if code.is_empty() {
            return;
        }
        if code == self.target_language_code && (self.tts_loading || self.tts_playing) {
            self.stop_tts();
        }
        self.drop_voice_override_for_pack(&code, &pack_id);
        self.send_io(IoEvent::DeleteTtsPack { pack_id });
    }

    /// A stored choice pointing into the pack being deleted would make the next speak request
    /// fail on a missing pack; dropping it lets the first remaining voice take over.
    fn drop_voice_override_for_pack(&mut self, code: &str, pack_id: &str) {
        let selected_in_pack = self
            .tts_voice_selections
            .get(code)
            .is_some_and(|selection| selection.pack_id == pack_id);
        if !selected_in_pack {
            return;
        }
        self.tts_voice_selections.remove(code);
        self.persist_settings();
    }

    pub(crate) fn set_manage_tts_pack_progress(
        &mut self,
        pack_id: &str,
        progress: DownloadProgress,
    ) {
        match progress {
            DownloadProgress::Running(fraction) => {
                self.manage_tts_pack_downloads
                    .insert(pack_id.to_string(), PackDownloadState::Running(fraction));
            }
            DownloadProgress::Ended => {
                self.manage_tts_pack_downloads.remove(pack_id);
            }
        }
        self.update_manage_tts_pack_item(pack_id);
    }

    fn manage_tts_picker_language(&self) -> Option<&Language> {
        if self.manage_tts_picker_language_code.is_empty() {
            return None;
        }
        self.find_language_by_code(&self.manage_tts_picker_language_code)
    }

    pub(crate) fn refresh_manage_tts_picker_model(&mut self) {
        let Some(language) = self.manage_tts_picker_language() else {
            return;
        };
        let items = manage_tts_picker_items(
            &language.tts_voice_picker_regions,
            &self.manage_tts_pack_downloads,
        );
        self.manage_tts_picker_model.borrow_mut().reset_data(items);
    }

    fn update_manage_tts_pack_item(&mut self, pack_id: &str) {
        let Some(language) = self.manage_tts_picker_language() else {
            return;
        };
        let pack_id = QString::from(pack_id);
        let replacement = manage_tts_picker_items(
            &language.tts_voice_picker_regions,
            &self.manage_tts_pack_downloads,
        )
        .into_iter()
        .find(|item| item.pack_id == pack_id);
        let Some(replacement) = replacement else {
            return;
        };
        let mut model = self.manage_tts_picker_model.borrow_mut();
        let index = model.iter().position(|item| item.pack_id == pack_id);
        if let Some(index) = index {
            model.change_line(index, replacement);
        }
    }

    pub(crate) fn refresh_tts_voices(&mut self) {
        self.send_io(IoEvent::RefreshTtsVoices {
            language_code: self.target_language_code.clone(),
        });
    }

    pub(crate) fn eager_load_tts_destination(&mut self) {
        if !self.tts_available || self.target_language_code.is_empty() {
            self.tts_prewarmed_language_code.clear();
            return;
        }

        if self.tts_prewarmed_language_code == self.target_language_code {
            return;
        }

        eprintln!(
            "tts.ui: eager loading destination target_language={}",
            self.target_language_code
        );
        self.tts_prewarmed_language_code = self.target_language_code.clone();
        self.send_io(IoEvent::WarmTtsModel {
            language_code: self.target_language_code.clone(),
        });
    }

    pub(crate) fn refresh_tts_availability(&mut self) {
        let available = self
            .find_language_by_code(&self.target_language_code)
            .map(|language| language.tts_installed)
            .unwrap_or(false);

        if self.tts_available != available {
            self.tts_available = available;
            self.tts_available_changed();
        }

        if !available {
            self.tts_prewarmed_language_code.clear();
        } else {
            self.eager_load_tts_destination();
        }
    }

    pub(crate) fn reset_tts_voice_selection_state(&mut self) {
        self.tts_voice_options_model
            .borrow_mut()
            .reset_data(Vec::new());
        if self.tts_voice_option_count != 0 {
            self.tts_voice_option_count = 0;
            self.tts_voice_option_count_changed();
        }
        self.set_selected_voice_properties(
            QString::default(),
            QString::default(),
            "Default".into(),
        );
    }

    pub(crate) fn stop_tts(&mut self) {
        self.send_io(IoEvent::StopTts);
        self.set_tts_state_value(false, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TtsVoicePackOption;

    fn voice(pack_id: &str, display_name: &str, installed: bool) -> TtsVoicePackOption {
        TtsVoicePackOption {
            pack_id: pack_id.to_string(),
            display_name: display_name.to_string(),
            quality: Some("medium".to_string()),
            size_bytes: 16 * 1024 * 1024,
            installed,
        }
    }

    fn regions() -> Vec<TtsVoicePickerRegion> {
        vec![
            TtsVoicePickerRegion {
                code: "en_US".to_string(),
                display_name: "United States".to_string(),
                voices: vec![
                    voice("us-amy", "amy", false),
                    voice("us-hfc", "hfc_female", true),
                ],
            },
            TtsVoicePickerRegion {
                code: "en_GB".to_string(),
                display_name: "Great Britain".to_string(),
                voices: vec![voice("gb-alan", "alan", false)],
            },
        ]
    }

    fn pack(pack_id: &str, display_name: &str, speakers: &[&str]) -> InstalledTtsPack {
        InstalledTtsPack {
            pack_id: pack_id.to_string(),
            display_name: display_name.to_string(),
            voices: speakers
                .iter()
                .enumerate()
                .map(|(index, name)| translator::TtsSpeakerEntry {
                    name: name.to_string(),
                    speaker_id: index as i32,
                })
                .collect(),
        }
    }

    #[test]
    fn single_speaker_packs_read_as_one_voice_and_multi_speaker_packs_group() {
        let items = installed_voice_items(vec![
            pack("us-amy", "amy", &["amy"]),
            pack("us-libritts", "libritts", &["speaker_0", "speaker_1"]),
        ]);
        let rows = items
            .iter()
            .map(|item| {
                (
                    item.pack_id.to_string(),
                    item.name.to_string(),
                    item.display_name.to_string(),
                    item.pack_display_name.to_string(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows,
            vec![
                ("us-amy".into(), "".into(), "amy".into(), "".into()),
                (
                    "us-libritts".into(),
                    "speaker_0".into(),
                    "speaker_0".into(),
                    "libritts".into()
                ),
                (
                    "us-libritts".into(),
                    "speaker_1".into(),
                    "speaker_1".into(),
                    "libritts".into()
                ),
            ]
        );
    }

    #[test]
    fn stored_selection_wins_only_while_installed() {
        let items = installed_voice_items(vec![
            pack("us-amy", "amy", &["amy"]),
            pack("gb-alan", "alan", &["alan"]),
        ]);
        let stored = TtsVoiceSelection {
            pack_id: "gb-alan".to_string(),
            speaker: None,
        };
        let resolved = resolve_tts_voice_selection(&items, Some(&stored)).unwrap();
        assert_eq!(resolved.pack_id.to_string(), "gb-alan");

        let stale = TtsVoiceSelection {
            pack_id: "gone".to_string(),
            speaker: None,
        };
        let resolved = resolve_tts_voice_selection(&items, Some(&stale)).unwrap();
        assert_eq!(resolved.pack_id.to_string(), "us-amy");

        assert!(resolve_tts_voice_selection(&[], None).is_none());
    }

    #[test]
    fn installed_voices_lead_then_region_then_name() {
        let items = manage_tts_picker_items(&regions(), &BTreeMap::new());
        let order = items
            .iter()
            .map(|item| (item.section_text.to_string(), item.pack_id.to_string()))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            vec![
                ("Downloaded".to_string(), "us-hfc".to_string()),
                ("Available".to_string(), "gb-alan".to_string()),
                ("Available".to_string(), "us-amy".to_string()),
            ]
        );
    }

    #[test]
    fn region_only_named_when_several_regions() {
        let multi = manage_tts_picker_items(&regions(), &BTreeMap::new());
        assert_eq!(
            multi[0].meta_text.to_string(),
            "Medium · 16 MB · United States"
        );

        let single = manage_tts_picker_items(&regions()[..1], &BTreeMap::new());
        assert_eq!(single[0].meta_text.to_string(), "Medium · 16 MB");
    }

    #[test]
    fn download_state_maps_onto_rows() {
        let downloads = BTreeMap::from([
            ("us-amy".to_string(), PackDownloadState::Running(0.4)),
            ("gb-alan".to_string(), PackDownloadState::Queued),
        ]);
        let items = manage_tts_picker_items(&regions(), &downloads);
        let by_id = |id: &str| {
            items
                .iter()
                .find(|item| item.pack_id.to_string() == id)
                .unwrap()
        };
        assert!(!by_id("us-amy").queued);
        assert_eq!(by_id("us-amy").progress, 0.4);
        assert!(by_id("gb-alan").queued);
        assert_eq!(by_id("gb-alan").progress, 0.0);
        assert!(!by_id("us-hfc").queued);
        assert_eq!(by_id("us-hfc").progress, 0.0);
    }
}
