use qmetaobject::{QString, QStringList};

use crate::model::{Direction, DownloadProgress, DownloadTarget, FeatureKind, Language, Screen};

use super::AppBridge;
use super::types::{
    language_to_list_item, manage_language_to_list_item, update_manage_progress_item,
    update_progress_list_item,
};

impl AppBridge {
    pub(crate) fn set_languages_value(&mut self, mut languages: Vec<Language>) {
        languages.sort_by(|left, right| left.name.cmp(&right.name));
        eprintln!("ui.set_languages_value: {} languages", languages.len());
        self.all_languages = languages;
        self.refresh_language_views();
        self.refresh_manage_tts_picker_model();

        if self.current_screen == Screen::NoLanguages.as_i32() && self.has_languages {
            self.set_current_screen(Screen::Translation);
        }

        // Consumed on the first list either way: a URI that arrived at startup should not fire
        // minutes later, once the user has downloaded a language by hand.
        if let Some(intent) = self.pending_launch_intent.take() {
            self.apply_launch_intent(intent);
        }
    }

    pub(crate) fn set_feature_progress_value(
        &mut self,
        target: &DownloadTarget,
        progress: DownloadProgress,
    ) {
        if let Some(pack_id) = &target.tts_pack_id {
            self.set_manage_tts_pack_progress(pack_id, progress);
        }

        let Some(language) = self
            .all_languages
            .iter_mut()
            .find(|language| language.code == target.code)
        else {
            return;
        };

        let fraction = match progress {
            DownloadProgress::Running(fraction) => fraction,
            DownloadProgress::Ended => 0.0,
        };
        match target.feature {
            FeatureKind::Core => language.core_progress = fraction,
            FeatureKind::Dictionary => language.dictionary_progress = fraction,
            FeatureKind::Tts => language.tts_progress = fraction,
        }

        let language = language.clone();
        update_progress_list_item(
            &mut self.installed_languages_model.borrow_mut(),
            &language,
            false,
        );
        update_progress_list_item(
            &mut self.available_languages_model.borrow_mut(),
            &language,
            true,
        );
        if self.manage_filter.is_empty()
            || language
                .name
                .to_lowercase()
                .contains(self.manage_filter.as_str())
        {
            update_manage_progress_item(
                &mut self.manage_languages_model.borrow_mut(),
                &language,
                self.expanded_languages.contains(&language.code),
            );
        }
        self.refresh_detected_language();
    }

    pub(crate) fn set_source_language_by_name(&mut self, name: String) {
        if let Some(language) = self
            .all_languages
            .iter()
            .find(|language| language.name == name)
            .cloned()
        {
            self.source_language_code = language.code.clone();
            let qname = QString::from(language.name);
            if self.source_language_name != qname {
                self.source_language_name = qname;
                self.source_language_name_changed();
            }
            #[cfg(feature = "live")]
            crate::live_ocr::set_live_languages(
                &self.source_language_code,
                &self.target_language_code,
            );
            self.stop_tts();
            self.refresh_swap_enabled();
            self.refresh_detected_language();
            // While the live camera is open the text/image view is hidden;
            // re-running its (heavy) OCR/translation on every pill change is
            // wasted work — the live pipeline is retargeted above instead.
            if !self.live_camera_open {
                self.refresh_translation_content();
            }
            self.refresh_input_transliteration();
            self.persist_settings();
        }
    }

    pub(crate) fn set_target_language_by_name(&mut self, name: String) {
        if let Some(language) = self
            .all_languages
            .iter()
            .find(|language| language.name == name)
            .cloned()
        {
            self.target_language_code = language.code.clone();
            let qname = QString::from(language.name);
            if self.target_language_name != qname {
                self.target_language_name = qname;
                self.target_language_name_changed();
            }
            #[cfg(feature = "live")]
            crate::live_ocr::set_live_languages(
                &self.source_language_code,
                &self.target_language_code,
            );
            self.stop_tts();
            self.refresh_swap_enabled();
            if !self.live_camera_open {
                self.refresh_translation_content();
            }
            self.tts_prewarmed_language_code.clear();
            self.refresh_tts_availability();
            self.reset_tts_voice_selection_state();
            self.refresh_output_transliteration();
            self.persist_settings();
        }
    }

    pub(crate) fn swap_languages_impl(&mut self) {
        let source = self.source_language_name.to_string();
        let target = self.target_language_name.to_string();
        self.set_source_language_by_name(target);
        self.set_target_language_by_name(source);
    }

    pub(crate) fn set_disable_auto_detect_impl(&mut self, value: bool) {
        if self.disable_auto_detect != value {
            self.disable_auto_detect = value;
            self.disable_auto_detect_changed();
            self.refresh_detected_language();
            self.persist_settings();
        }
    }

    pub(crate) fn missing_language_action_impl(&mut self) {
        let detected_code = self.detected_language_code.clone();
        if detected_code.is_empty() {
            return;
        }

        if let Some(language) = self
            .all_languages
            .iter()
            .find(|language| language.code == detected_code)
            .cloned()
        {
            if language.core_installed || language.built_in {
                self.set_source_language_by_name(language.name);
            } else {
                self.send_feature_request(language.code, FeatureKind::Core, true, None);
            }
        }
    }

    pub(crate) fn refresh_manage_model(&mut self) {
        let manage_items = self
            .all_languages
            .iter()
            .filter(|language| {
                self.manage_filter.is_empty()
                    || language
                        .name
                        .to_lowercase()
                        .contains(self.manage_filter.as_str())
            })
            .cloned()
            .map(|language| {
                manage_language_to_list_item(
                    &language,
                    self.expanded_languages.contains(&language.code),
                )
            })
            .collect::<Vec<_>>();

        self.manage_languages_model
            .borrow_mut()
            .reset_data(manage_items);
    }

    pub(crate) fn refresh_language_views(&mut self) {
        let installed_items = self
            .all_languages
            .iter()
            .filter(|language| language.core_installed || language.built_in)
            .cloned()
            .map(language_to_list_item)
            .collect::<Vec<_>>();
        let available_items = self
            .all_languages
            .iter()
            .filter(|language| !language.core_installed && !language.built_in)
            .cloned()
            .map(language_to_list_item)
            .collect::<Vec<_>>();

        let manage_items = self
            .all_languages
            .iter()
            .filter(|language| {
                self.manage_filter.is_empty()
                    || language
                        .name
                        .to_lowercase()
                        .contains(self.manage_filter.as_str())
            })
            .cloned()
            .map(|language| {
                manage_language_to_list_item(
                    &language,
                    self.expanded_languages.contains(&language.code),
                )
            })
            .collect::<Vec<_>>();

        eprintln!(
            "ui.refresh_language_views: installed={} available={} manage={} filter='{}'",
            installed_items.len(),
            available_items.len(),
            manage_items.len(),
            self.manage_filter
        );

        self.installed_languages_model
            .borrow_mut()
            .reset_data(installed_items);
        self.available_languages_model
            .borrow_mut()
            .reset_data(available_items);
        eprintln!("ui.refresh_language_views: resetting manage model");
        self.manage_languages_model
            .borrow_mut()
            .reset_data(manage_items);

        let from_names = self
            .all_languages
            .iter()
            .filter(|language| self.is_language_available(language, true))
            .map(|language| QString::from(language.name.clone()))
            .collect::<QStringList>();
        if self.installed_from_language_names != from_names {
            self.installed_from_language_names = from_names;
            self.installed_from_language_names_changed();
        }

        let to_names = self
            .all_languages
            .iter()
            .filter(|language| self.is_language_available(language, false))
            .map(|language| QString::from(language.name.clone()))
            .collect::<QStringList>();
        if self.installed_to_language_names != to_names {
            self.installed_to_language_names = to_names;
            self.installed_to_language_names_changed();
        }

        let has_languages = self
            .all_languages
            .iter()
            .any(|language| !language.built_in && language.core_installed);
        if self.has_languages != has_languages {
            self.has_languages = has_languages;
            self.has_languages_changed();
        }

        self.ensure_selected_languages_are_valid();
        self.refresh_swap_enabled();
        self.refresh_detected_language();
        self.tts_prewarmed_language_code.clear();
        self.refresh_tts_availability();
        self.reset_tts_voice_selection_state();
        self.refresh_input_transliteration();
        self.refresh_output_transliteration();
    }

    pub(crate) fn ensure_selected_languages_are_valid(&mut self) {
        if !self.is_language_selectable(&self.source_language_code, true)
            && let Some(language) = self.first_selectable_language(true)
        {
            self.source_language_code = language.code.clone();
            let qname = QString::from(language.name);
            if self.source_language_name != qname {
                self.source_language_name = qname;
                self.source_language_name_changed();
            }
        }

        if !self.is_language_selectable(&self.target_language_code, false)
            && let Some(language) = self.first_selectable_language(false)
        {
            self.target_language_code = language.code.clone();
            let qname = QString::from(language.name);
            if self.target_language_name != qname {
                self.target_language_name = qname;
                self.target_language_name_changed();
            }
        }
    }

    pub(crate) fn refresh_swap_enabled(&mut self) {
        let enabled = self
            .find_language_by_code(&self.source_language_code)
            .zip(self.find_language_by_code(&self.target_language_code))
            .map(|(source, target)| {
                matches!(source.direction, Direction::Both)
                    && matches!(target.direction, Direction::Both)
            })
            .unwrap_or(false);

        if self.swap_enabled != enabled {
            self.swap_enabled = enabled;
            self.swap_enabled_changed();
        }
    }

    pub(crate) fn refresh_detected_language(&mut self) {
        let visible = !self.disable_auto_detect && !self.detected_language_code.is_empty();
        let detected = self.find_language_by_code(&self.detected_language_code);
        let (name, installed, progress, show_card) = match detected {
            Some(language) => {
                let show = visible
                    && !matches!(language.direction, Direction::ToOnly)
                    && language.code != self.source_language_code;
                (
                    QString::from(language.name.clone()),
                    language.core_installed || language.built_in,
                    language.core_progress,
                    show,
                )
            }
            None => (QString::default(), false, 0.0, false),
        };

        if self.detected_language_name != name {
            self.detected_language_name = name;
            self.detected_language_name_changed();
        }
        if self.detected_language_installed != installed {
            self.detected_language_installed = installed;
            self.detected_language_installed_changed();
        }
        if (self.detected_language_progress - progress).abs() > f32::EPSILON {
            self.detected_language_progress = progress;
            self.detected_language_progress_changed();
        }
        if self.show_missing_card != show_card {
            self.show_missing_card = show_card;
            self.show_missing_card_changed();
        }
    }

    pub(crate) fn is_language_available(&self, language: &Language, source: bool) -> bool {
        (language.core_installed || language.built_in)
            && if source {
                matches!(language.direction, Direction::FromOnly | Direction::Both)
            } else {
                matches!(language.direction, Direction::ToOnly | Direction::Both)
            }
    }

    pub(crate) fn is_language_selectable(&self, code: &str, source: bool) -> bool {
        self.find_language_by_code(code)
            .map(|language| self.is_language_available(language, source))
            .unwrap_or(false)
    }

    pub(crate) fn first_selectable_language(&self, source: bool) -> Option<Language> {
        self.all_languages
            .iter()
            .find(|language| self.is_language_available(language, source))
            .cloned()
    }

    pub(crate) fn find_language_by_code(&self, code: &str) -> Option<&Language> {
        self.all_languages
            .iter()
            .find(|language| language.code == code)
    }
}
