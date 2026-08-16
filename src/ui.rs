mod callbacks;
mod core;
mod dictionary;
mod document;
mod image;
mod languages;
mod transliteration;
mod tts;
mod types;

pub(crate) use self::core::UiScale;
pub use callbacks::{ImageResult, UiCallbacks, create_ui_callbacks};
pub use types::{
    DictionaryPopupRowItem, LanguageListItem, ManageLanguageListItem, ManageTtsVoicePackListItem,
    SelectionPillItem, TtsVoiceListItem,
};

use qmetaobject::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::sync::mpsc::Sender;
use translator::tarkka::WordWithTaggedEntries;

use crate::IoEvent;
use crate::model::{FeatureKind, Language, Screen};

#[derive(QObject, Default)]
pub struct AppBridge {
    base: qt_base_class!(trait QObject),

    pub current_screen: qt_property!(i32; NOTIFY current_screen_changed),
    pub current_screen_changed: qt_signal!(),

    pub disable_auto_detect: qt_property!(bool; NOTIFY disable_auto_detect_changed),
    pub disable_auto_detect_changed: qt_signal!(),

    pub has_languages: qt_property!(bool; NOTIFY has_languages_changed),
    pub has_languages_changed: qt_signal!(),

    pub input_text: qt_property!(QString; NOTIFY input_text_changed),
    pub input_text_changed: qt_signal!(),

    pub output_text: qt_property!(QString; NOTIFY output_text_changed),
    pub output_text_changed: qt_signal!(),
    pub input_transliteration: qt_property!(QString; NOTIFY input_transliteration_changed),
    pub input_transliteration_changed: qt_signal!(),
    pub output_transliteration: qt_property!(QString; NOTIFY output_transliteration_changed),
    pub output_transliteration_changed: qt_signal!(),

    pub image_mode: qt_property!(bool; NOTIFY image_mode_changed),
    pub image_mode_changed: qt_signal!(),

    pub image_viewer_open: qt_property!(bool; NOTIFY image_viewer_open_changed),
    pub image_viewer_open_changed: qt_signal!(),

    pub live_camera_open: qt_property!(bool; NOTIFY live_camera_open_changed),
    pub live_camera_open_changed: qt_signal!(),

    // Whether the live pipeline detects/translates. When false the live screen
    // is a plain viewfinder for snapping a still into the image OCR pipeline.
    pub live_ocr_active: qt_property!(bool; NOTIFY live_ocr_active_changed),
    pub live_ocr_active_changed: qt_signal!(),

    // Per-frame counter; LiveCameraItem binds `frameTick` to it so each new
    // camera frame schedules a GPU present. Carries no pixels.
    pub live_frame_tick: qt_property!(i32; NOTIFY live_frame_tick_changed),
    pub live_frame_tick_changed: qt_signal!(),

    pub tts_available: qt_property!(bool; NOTIFY tts_available_changed),
    pub tts_available_changed: qt_signal!(),

    pub tts_loading: qt_property!(bool; NOTIFY tts_loading_changed),
    pub tts_loading_changed: qt_signal!(),

    pub tts_playing: qt_property!(bool; NOTIFY tts_playing_changed),
    pub tts_playing_changed: qt_signal!(),

    pub selected_image_url: qt_property!(QString; NOTIFY selected_image_url_changed),
    pub selected_image_url_changed: qt_signal!(),
    pub processed_image: qt_property!(QImage; NOTIFY processed_image_changed),
    pub processed_image_changed: qt_signal!(),
    pub share_image_url: qt_property!(QString; NOTIFY share_image_url_changed),
    pub share_image_url_changed: qt_signal!(),

    pub processed_image_width: qt_property!(f32; NOTIFY processed_image_width_changed),
    pub processed_image_width_changed: qt_signal!(),

    pub processed_image_height: qt_property!(f32; NOTIFY processed_image_height_changed),
    pub processed_image_height_changed: qt_signal!(),

    pub source_language_name: qt_property!(QString; NOTIFY source_language_name_changed),
    pub source_language_name_changed: qt_signal!(),

    pub target_language_name: qt_property!(QString; NOTIFY target_language_name_changed),
    pub target_language_name_changed: qt_signal!(),

    pub installed_from_language_names: qt_property!(QStringList; NOTIFY installed_from_language_names_changed),
    pub installed_from_language_names_changed: qt_signal!(),

    pub installed_to_language_names: qt_property!(QStringList; NOTIFY installed_to_language_names_changed),
    pub installed_to_language_names_changed: qt_signal!(),

    pub swap_enabled: qt_property!(bool; NOTIFY swap_enabled_changed),
    pub swap_enabled_changed: qt_signal!(),

    pub detected_language_name: qt_property!(QString; NOTIFY detected_language_name_changed),
    pub detected_language_name_changed: qt_signal!(),

    pub detected_language_installed: qt_property!(bool; NOTIFY detected_language_installed_changed),
    pub detected_language_installed_changed: qt_signal!(),

    pub detected_language_progress: qt_property!(f32; NOTIFY detected_language_progress_changed),
    pub detected_language_progress_changed: qt_signal!(),

    pub show_missing_card: qt_property!(bool; NOTIFY show_missing_card_changed),
    pub show_missing_card_changed: qt_signal!(),

    pub active_tab: qt_property!(i32; NOTIFY active_tab_changed),
    pub active_tab_changed: qt_signal!(),

    pub manage_filter_text: qt_property!(QString; NOTIFY manage_filter_text_changed),
    pub manage_filter_text_changed: qt_signal!(),

    pub manage_tts_picker_open: qt_property!(bool; NOTIFY manage_tts_picker_open_changed),
    pub manage_tts_picker_open_changed: qt_signal!(),

    pub manage_tts_picker_language_name: qt_property!(QString; NOTIFY manage_tts_picker_language_name_changed),
    pub manage_tts_picker_language_name_changed: qt_signal!(),

    pub installed_languages_model: qt_property!(RefCell<SimpleListModel<LanguageListItem>>; CONST),
    pub available_languages_model: qt_property!(RefCell<SimpleListModel<LanguageListItem>>; CONST),
    pub manage_languages_model: qt_property!(RefCell<SimpleListModel<ManageLanguageListItem>>; CONST),
    pub manage_tts_picker_model: qt_property!(RefCell<SimpleListModel<ManageTtsVoicePackListItem>>; CONST),
    pub tts_voice_options_model: qt_property!(RefCell<SimpleListModel<TtsVoiceListItem>>; CONST),
    pub dictionary_popup_rows_model: qt_property!(RefCell<SimpleListModel<DictionaryPopupRowItem>>; CONST),
    pub selection_pills_model: qt_property!(RefCell<SimpleListModel<SelectionPillItem>>; CONST),
    /// Detector output shown while recognition runs, so the user sees which regions are being
    /// scanned instead of a bare "Translating" label.
    pub scan_boxes_model: qt_property!(RefCell<SimpleListModel<SelectionPillItem>>; CONST),

    pub scan_active: qt_property!(bool; NOTIFY scan_active_changed),
    pub scan_active_changed: qt_signal!(),

    /// Set when an image translation fails. The only reason the text pane still appears in image
    /// mode, since that is where the message lands.
    pub image_error: qt_property!(QString; NOTIFY image_error_changed),
    pub image_error_changed: qt_signal!(),

    /// True once an image carries selectable words, which is what switches the screen from the
    /// text output pane to on-image selection.
    pub image_words_ready: qt_property!(bool; NOTIFY image_words_ready_changed),
    pub image_words_ready_changed: qt_signal!(),

    pub image_show_original: qt_property!(bool; NOTIFY image_show_original_changed),
    pub image_show_original_changed: qt_signal!(),

    pub selection_active: qt_property!(bool; NOTIFY selection_changed),
    pub selection_text: qt_property!(QString; NOTIFY selection_changed),
    pub selection_start_x: qt_property!(f32; NOTIFY selection_changed),
    pub selection_start_y: qt_property!(f32; NOTIFY selection_changed),
    pub selection_end_x: qt_property!(f32; NOTIFY selection_changed),
    pub selection_end_y: qt_property!(f32; NOTIFY selection_changed),
    pub selection_left: qt_property!(f32; NOTIFY selection_changed),
    pub selection_top: qt_property!(f32; NOTIFY selection_changed),
    pub selection_right: qt_property!(f32; NOTIFY selection_changed),
    pub selection_bottom: qt_property!(f32; NOTIFY selection_changed),
    pub selection_changed: qt_signal!(),

    pub image_word_at: qt_method!(
        fn image_word_at(&mut self, x: f32, y: f32) -> i32 {
            self.image_word_at_impl(x, y)
        }
    ),
    pub image_nearest_word: qt_method!(
        fn image_nearest_word(&mut self, x: f32, y: f32, anchor: i32) -> i32 {
            self.image_nearest_word_impl(x, y, anchor)
        }
    ),
    pub set_image_selection: qt_method!(
        fn set_image_selection(&mut self, start: i32, end: i32) {
            self.set_image_selection_impl(start, end);
        }
    ),
    pub clear_image_selection: qt_method!(
        fn clear_image_selection(&mut self) {
            self.clear_image_selection_impl();
        }
    ),
    pub toggle_image_original: qt_method!(
        fn toggle_image_original(&mut self) {
            self.toggle_image_original_impl();
        }
    ),

    pub desktop_mode: qt_property!(bool; CONST),
    pub ui_scale: qt_property!(f64; CONST),
    pub automation_enabled: qt_property!(bool; CONST),
    pub automation_from: qt_property!(QString; CONST),
    pub automation_to: qt_property!(QString; CONST),
    pub automation_text: qt_property!(QString; CONST),
    pub automation_screenshot_path: qt_property!(QString; CONST),
    pub automation_quit_after_screenshot: qt_property!(bool; CONST),

    pub ocr_background_mode: qt_property!(QString; NOTIFY ocr_background_mode_changed),
    pub ocr_background_mode_changed: qt_signal!(),

    pub ocr_min_confidence: qt_property!(i32; NOTIFY ocr_min_confidence_changed),
    pub ocr_min_confidence_changed: qt_signal!(),

    pub ocr_max_image_size: qt_property!(i32; NOTIFY ocr_max_image_size_changed),
    pub ocr_max_image_size_changed: qt_signal!(),

    pub doc_drawer_open: qt_property!(bool; NOTIFY doc_drawer_open_changed),
    pub doc_drawer_open_changed: qt_signal!(),
    pub doc_file_name: qt_property!(QString; NOTIFY doc_file_name_changed),
    pub doc_file_name_changed: qt_signal!(),
    pub doc_file_size: qt_property!(QString; NOTIFY doc_file_size_changed),
    pub doc_file_size_changed: qt_signal!(),
    pub doc_is_pdf: qt_property!(bool; NOTIFY doc_is_pdf_changed),
    pub doc_is_pdf_changed: qt_signal!(),
    pub doc_translate_images: qt_property!(bool; NOTIFY doc_translate_images_changed),
    pub doc_translate_images_changed: qt_signal!(),

    pub doc_progress_open: qt_property!(bool; NOTIFY doc_progress_open_changed),
    pub doc_progress_open_changed: qt_signal!(),
    pub doc_progress_label: qt_property!(QString; NOTIFY doc_progress_label_changed),
    pub doc_progress_label_changed: qt_signal!(),
    pub doc_text_fraction: qt_property!(f32; NOTIFY doc_text_fraction_changed),
    pub doc_text_fraction_changed: qt_signal!(),
    pub doc_show_pdf_phases: qt_property!(bool; NOTIFY doc_show_pdf_phases_changed),
    pub doc_show_pdf_phases_changed: qt_signal!(),
    pub doc_images_current: qt_property!(i32; NOTIFY doc_images_current_changed),
    pub doc_images_current_changed: qt_signal!(),
    pub doc_images_total: qt_property!(i32; NOTIFY doc_images_total_changed),
    pub doc_images_total_changed: qt_signal!(),
    pub doc_raster_current: qt_property!(i32; NOTIFY doc_raster_current_changed),
    pub doc_raster_current_changed: qt_signal!(),
    pub doc_raster_total: qt_property!(i32; NOTIFY doc_raster_total_changed),
    pub doc_raster_total_changed: qt_signal!(),

    pub doc_done_open: qt_property!(bool; NOTIFY doc_done_open_changed),
    pub doc_done_open_changed: qt_signal!(),
    pub doc_error: qt_property!(QString; NOTIFY doc_error_changed),
    pub doc_error_changed: qt_signal!(),
    pub doc_output_url: qt_property!(QString; NOTIFY doc_output_url_changed),
    pub doc_output_url_changed: qt_signal!(),
    pub doc_output_name: qt_property!(QString; NOTIFY doc_output_name_changed),
    pub doc_output_name_changed: qt_signal!(),

    pub catalog_index_url: qt_property!(QString; NOTIFY catalog_index_url_changed),
    pub catalog_index_url_changed: qt_signal!(),

    pub disable_ocr: qt_property!(bool; NOTIFY disable_ocr_changed),
    pub disable_ocr_changed: qt_signal!(),

    pub show_transliteration_output: qt_property!(bool; NOTIFY show_transliteration_output_changed),
    pub show_transliteration_output_changed: qt_signal!(),

    pub show_transliteration_input: qt_property!(bool; NOTIFY show_transliteration_input_changed),
    pub show_transliteration_input_changed: qt_signal!(),

    pub tts_playback_speed: qt_property!(f32; NOTIFY tts_playback_speed_changed),
    pub tts_playback_speed_changed: qt_signal!(),

    pub tts_selected_voice_name: qt_property!(QString; NOTIFY tts_selected_voice_name_changed),
    pub tts_selected_voice_name_changed: qt_signal!(),

    pub tts_selected_voice_display_name: qt_property!(QString; NOTIFY tts_selected_voice_display_name_changed),
    pub tts_selected_voice_display_name_changed: qt_signal!(),

    pub tts_voice_option_count: qt_property!(i32; NOTIFY tts_voice_option_count_changed),
    pub tts_voice_option_count_changed: qt_signal!(),

    pub dictionary_popup_open: qt_property!(bool; NOTIFY dictionary_popup_open_changed),
    pub dictionary_popup_open_changed: qt_signal!(),

    pub dictionary_popup_word: qt_property!(QString; NOTIFY dictionary_popup_word_changed),
    pub dictionary_popup_word_changed: qt_signal!(),

    pub dictionary_popup_subtitle: qt_property!(QString; NOTIFY dictionary_popup_subtitle_changed),
    pub dictionary_popup_subtitle_changed: qt_signal!(),

    pub dictionary_popup_primary_label: qt_property!(QString; NOTIFY dictionary_popup_primary_label_changed),
    pub dictionary_popup_primary_label_changed: qt_signal!(),

    pub dictionary_popup_secondary_label: qt_property!(QString; NOTIFY dictionary_popup_secondary_label_changed),
    pub dictionary_popup_secondary_label_changed: qt_signal!(),

    pub dictionary_popup_has_secondary: qt_property!(bool; NOTIFY dictionary_popup_has_secondary_changed),
    pub dictionary_popup_has_secondary_changed: qt_signal!(),

    pub dictionary_popup_selected_entry_index: qt_property!(i32; NOTIFY dictionary_popup_selected_entry_index_changed),
    pub dictionary_popup_selected_entry_index_changed: qt_signal!(),

    pub toast_message: qt_property!(QString; NOTIFY toast_message_changed),
    pub toast_message_changed: qt_signal!(),

    pub toast_visible: qt_property!(bool; NOTIFY toast_visible_changed),
    pub toast_visible_changed: qt_signal!(),

    pub asset_url: qt_method!(
        fn asset_url(&self, name: QString) -> QString {
            format!("file://{}/{}", self.asset_dir, name).into()
        }
    ),
    pub save_automation_screenshot: qt_method!(
        fn save_automation_screenshot(&self, path: QString) -> bool {
            let path_str = path.to_string();
            println!("automation rust save_window_screenshot path={}", path_str);
            let ok = crate::rendered_image_item::save_window_screenshot(&path_str);
            println!(
                "automation rust save_window_screenshot result={} path={}",
                ok, path_str
            );
            ok
        }
    ),
    pub automation_log: qt_method!(
        fn automation_log(&self, message: QString) {
            println!("automation qml {}", message);
        }
    ),

    pub set_from: qt_method!(
        fn set_from(&mut self, name: QString) {
            self.set_source_language_by_name(name.to_string());
        }
    ),
    pub set_to: qt_method!(
        fn set_to(&mut self, name: QString) {
            self.set_target_language_by_name(name.to_string());
        }
    ),
    pub swap_languages: qt_method!(
        fn swap_languages(&mut self) {
            self.swap_languages_impl();
        }
    ),
    pub process_text: qt_method!(
        fn process_text(&mut self, text: QString) {
            self.process_text_impl(text.to_string());
        }
    ),
    pub download_language: qt_method!(
        fn download_language(&mut self, code: QString) {
            self.send_feature_request(code.to_string(), FeatureKind::Core, true, None);
        }
    ),
    pub delete_language: qt_method!(
        fn delete_language(&mut self, code: QString) {
            self.send_feature_request(code.to_string(), FeatureKind::Core, false, None);
        }
    ),
    pub download_feature: qt_method!(
        fn download_feature(&mut self, code: QString, feature: i32) {
            if let Some(feature) = FeatureKind::from_i32(feature) {
                self.send_feature_request(code.to_string(), feature, true, None);
            }
        }
    ),
    pub delete_feature: qt_method!(
        fn delete_feature(&mut self, code: QString, feature: i32) {
            if let Some(feature) = FeatureKind::from_i32(feature) {
                self.send_feature_request(code.to_string(), feature, false, None);
            }
        }
    ),
    pub download_all_features: qt_method!(
        fn download_all_features(&mut self, code: QString) {
            let code = code.to_string();
            if let Some(language) = self.find_language_by_code(&code).cloned() {
                if language.core_size_bytes > 0 && !language.core_installed {
                    self.send_feature_request(code.clone(), FeatureKind::Core, true, None);
                }
                if language.dictionary_size_bytes > 0 && !language.dictionary_installed {
                    self.send_feature_request(code.clone(), FeatureKind::Dictionary, true, None);
                }
                if language.tts_size_bytes > 0 && !language.tts_installed {
                    self.send_feature_request(code, FeatureKind::Tts, true, None);
                }
            }
        }
    ),
    pub delete_all_features: qt_method!(
        fn delete_all_features(&mut self, code: QString) {
            let code = code.to_string();
            if let Some(language) = self.find_language_by_code(&code).cloned() {
                if language.tts_size_bytes > 0 && language.tts_installed {
                    self.send_feature_request(code.clone(), FeatureKind::Tts, false, None);
                }
                if language.dictionary_size_bytes > 0 && language.dictionary_installed {
                    self.send_feature_request(code.clone(), FeatureKind::Dictionary, false, None);
                }
                if language.core_size_bytes > 0 && language.core_installed {
                    self.send_feature_request(code, FeatureKind::Core, false, None);
                }
            }
        }
    ),
    pub toggle_manage_language: qt_method!(
        fn toggle_manage_language(&mut self, code: QString) {
            let code = code.to_string();
            let expanded = if !self.expanded_languages.remove(&code) {
                self.expanded_languages.insert(code.clone());
                true
            } else {
                false
            };

            let visible = self.manage_filter.is_empty()
                || self
                    .find_language_by_code(&code)
                    .map(|language| {
                        language
                            .name
                            .to_lowercase()
                            .contains(self.manage_filter.as_str())
                    })
                    .unwrap_or(false);

            if visible && let Some(language) = self.find_language_by_code(&code).cloned() {
                types::update_manage_progress_item(
                    &mut self.manage_languages_model.borrow_mut(),
                    &language,
                    expanded,
                );
            }
        }
    ),
    pub set_manage_filter: qt_method!(
        fn set_manage_filter(&mut self, text: QString) {
            let text = text.to_string();
            let qtext = QString::from(text.clone());
            if self.manage_filter_text == qtext {
                return;
            }
            self.manage_filter_text = qtext;
            self.manage_filter_text_changed();
            self.manage_filter = text.to_lowercase();
            self.refresh_manage_model();
        }
    ),
    pub show_settings: qt_method!(
        fn show_settings(&mut self) {
            self.set_current_screen(Screen::Settings);
        }
    ),
    pub back_from_settings: qt_method!(
        fn back_from_settings(&mut self) {
            self.set_current_screen(Screen::Translation);
        }
    ),
    pub show_manage_languages: qt_method!(
        fn show_manage_languages(&mut self) {
            self.previous_screen = Screen::Settings;
            self.set_current_screen(Screen::ManageLanguages);
        }
    ),
    pub back_from_manage_languages: qt_method!(
        fn back_from_manage_languages(&mut self) {
            if self.previous_screen == Screen::Settings {
                self.set_current_screen(Screen::Settings);
            } else if self.has_languages {
                self.set_current_screen(Screen::Translation);
            } else {
                self.set_current_screen(Screen::NoLanguages);
            }
        }
    ),
    pub finish_language_setup: qt_method!(
        fn finish_language_setup(&mut self) {
            if self.has_languages {
                self.set_current_screen(Screen::Translation);
            }
        }
    ),
    pub set_disable_auto_detect_value: qt_method!(
        fn set_disable_auto_detect_value(&mut self, value: bool) {
            self.set_disable_auto_detect_impl(value);
        }
    ),
    pub set_active_tab: qt_method!(
        fn set_active_tab(&mut self, tab: i32) {
            if self.active_tab != tab {
                self.active_tab = tab;
                self.active_tab_changed();
            }
        }
    ),
    pub missing_language_action: qt_method!(
        fn missing_language_action(&mut self) {
            self.missing_language_action_impl();
        }
    ),
    pub camera_clicked: qt_method!(
        fn camera_clicked(&self) {
            println!("Camera clicked");
        }
    ),
    pub open_live_camera: qt_method!(
        fn open_live_camera(&mut self) {
            self.live_camera_open = true;
            self.live_camera_open_changed();
            #[cfg(feature = "live")]
            {
                crate::live_ocr::set_live_languages(
                    &self.source_language_code,
                    &self.target_language_code,
                );
                crate::live_ocr::set_live_active(self.live_ocr_active);
            }
        }
    ),
    pub set_live_ocr_active: qt_method!(
        fn set_live_ocr_active(&mut self, value: bool) {
            if self.live_ocr_active != value {
                self.live_ocr_active = value;
                self.live_ocr_active_changed();
            }
            #[cfg(feature = "live")]
            crate::live_ocr::set_live_active(value);
        }
    ),
    pub capture_dir: qt_method!(
        fn capture_dir(&self) -> QString {
            let dir = std::path::Path::new(&self.data_dir).join("live_captures");
            let _ = std::fs::create_dir_all(&dir);
            QString::from(dir.display().to_string())
        }
    ),
    pub close_live_camera: qt_method!(
        fn close_live_camera(&mut self) {
            self.live_camera_open = false;
            self.live_camera_open_changed();
            // Park the pipeline so the residual tracker/OCR state doesn't
            // chew on the last frame in the background.
            #[cfg(feature = "live")]
            crate::live_ocr::set_live_active(false);
        }
    ),
    pub set_live_viewport: qt_method!(
        fn set_live_viewport(&self, width: i32, height: i32) {
            #[cfg(feature = "live")]
            crate::live_ocr::set_live_viewport(width.max(0) as u32, height.max(0) as u32);
            #[cfg(not(feature = "live"))]
            let _ = (width, height);
        }
    ),
    pub set_camera_orientation: qt_method!(
        fn set_camera_orientation(&self, degrees: i32) {
            #[cfg(feature = "live")]
            crate::live_gpu::set_camera_orientation_degrees(degrees);
            #[cfg(not(feature = "live"))]
            let _ = degrees;
        }
    ),
    pub request_live_acquire: qt_method!(
        fn request_live_acquire(&self) {
            #[cfg(feature = "live")]
            crate::live_ocr::request_acquire();
        }
    ),
    pub process_image_selection: qt_method!(
        fn process_image_selection(&mut self, url: QString) {
            self.process_image_selection_impl(url.to_string());
        }
    ),
    pub process_file_selection: qt_method!(
        fn process_file_selection(&mut self, url: QString) {
            self.process_file_selection_impl(url.to_string());
        }
    ),
    pub start_document_translation: qt_method!(
        fn start_document_translation(&mut self) {
            self.start_document_translation_impl();
        }
    ),
    pub cancel_document_translation: qt_method!(
        fn cancel_document_translation(&mut self) {
            self.cancel_document_translation_impl();
        }
    ),
    pub close_document_drawer: qt_method!(
        fn close_document_drawer(&mut self) {
            if self.doc_drawer_open {
                self.doc_drawer_open = false;
                self.doc_drawer_open_changed();
            }
        }
    ),
    pub close_document_done: qt_method!(
        fn close_document_done(&mut self) {
            if self.doc_done_open {
                self.doc_done_open = false;
                self.doc_done_open_changed();
            }
        }
    ),
    pub clear_selected_image: qt_method!(
        fn clear_selected_image(&mut self) {
            self.original_image_path.clear();
            self.stop_tts();
            self.set_image_mode_value(false);
            self.set_image_viewer_open_value(false);
            self.set_selected_image_url_value(String::new());
            self.set_processed_image_value(QImage::default());
            self.set_share_image_url_value(String::new());
        }
    ),
    pub open_image_viewer: qt_method!(
        fn open_image_viewer(&mut self) {
            if self.image_mode && !self.selected_image_url.to_string().is_empty() {
                self.set_image_viewer_open_value(true);
            }
        }
    ),
    pub close_image_viewer: qt_method!(
        fn close_image_viewer(&mut self) {
            self.set_image_viewer_open_value(false);
        }
    ),
    pub toggle_speak_output: qt_method!(
        fn toggle_speak_output(&mut self) {
            self.toggle_speak_output_impl();
        }
    ),
    pub prepare_tts_options: qt_method!(
        fn prepare_tts_options(&mut self) {
            self.prepare_tts_options_impl();
        }
    ),
    pub set_tts_playback_speed_value: qt_method!(
        fn set_tts_playback_speed_value(&mut self, value: f32) {
            self.set_tts_playback_speed_impl(value);
        }
    ),
    pub set_tts_voice_name: qt_method!(
        fn set_tts_voice_name(&mut self, value: QString) {
            self.set_tts_voice_name_impl(value.to_string());
        }
    ),
    pub open_tts_download_picker: qt_method!(
        fn open_tts_download_picker(&mut self, code: QString) {
            self.open_tts_download_picker_impl(code.to_string());
        }
    ),
    pub close_tts_download_picker: qt_method!(
        fn close_tts_download_picker(&mut self) {
            self.set_manage_tts_picker_open_value(false);
        }
    ),
    pub lookup_output_dictionary: qt_method!(
        fn lookup_output_dictionary(&mut self, word: QString) {
            self.lookup_dictionary_for_language(
                &word.to_string(),
                &self.target_language_code.clone(),
            );
        }
    ),
    pub close_dictionary_popup: qt_method!(
        fn close_dictionary_popup(&mut self) {
            self.close_dictionary_popup_impl();
        }
    ),
    pub clear_toast: qt_method!(
        fn clear_toast(&mut self) {
            self.clear_toast_impl();
        }
    ),
    pub select_dictionary_popup_entry: qt_method!(
        fn select_dictionary_popup_entry(&mut self, index: i32) {
            self.select_dictionary_popup_entry_impl(index);
        }
    ),
    pub download_tts_pack: qt_method!(
        fn download_tts_pack(&mut self, pack_id: QString) {
            if self.manage_tts_picker_language_code.is_empty() {
                return;
            }
            self.send_feature_request(
                self.manage_tts_picker_language_code.clone(),
                FeatureKind::Tts,
                true,
                Some(pack_id.to_string()),
            );
            self.set_manage_tts_picker_open_value(false);
        }
    ),

    pub set_ocr_background_mode_value: qt_method!(
        fn set_ocr_background_mode_value(&mut self, value: QString) {
            if self.ocr_background_mode != value {
                self.ocr_background_mode = value;
                self.ocr_background_mode_changed();
                self.persist_settings();
            }
        }
    ),
    pub set_ocr_min_confidence_value: qt_method!(
        fn set_ocr_min_confidence_value(&mut self, value: i32) {
            if self.ocr_min_confidence != value {
                self.ocr_min_confidence = value;
                self.ocr_min_confidence_changed();
                self.persist_settings();
            }
        }
    ),
    pub set_ocr_max_image_size_value: qt_method!(
        fn set_ocr_max_image_size_value(&mut self, value: i32) {
            if self.ocr_max_image_size != value {
                self.ocr_max_image_size = value;
                self.ocr_max_image_size_changed();
                self.persist_settings();
            }
        }
    ),
    pub set_catalog_index_url_value: qt_method!(
        fn set_catalog_index_url_value(&mut self, value: QString) {
            if self.catalog_index_url != value {
                self.catalog_index_url = value;
                self.catalog_index_url_changed();
                self.persist_settings();
            }
        }
    ),
    pub set_disable_ocr_value: qt_method!(
        fn set_disable_ocr_value(&mut self, value: bool) {
            if self.disable_ocr != value {
                self.disable_ocr = value;
                self.disable_ocr_changed();
                self.persist_settings();
            }
        }
    ),
    pub set_show_transliteration_output_value: qt_method!(
        fn set_show_transliteration_output_value(&mut self, value: bool) {
            if self.show_transliteration_output != value {
                self.show_transliteration_output = value;
                self.show_transliteration_output_changed();
                self.refresh_output_transliteration();
                self.persist_settings();
            }
        }
    ),
    pub set_show_transliteration_input_value: qt_method!(
        fn set_show_transliteration_input_value(&mut self, value: bool) {
            if self.show_transliteration_input != value {
                self.show_transliteration_input = value;
                self.show_transliteration_input_changed();
                self.refresh_input_transliteration();
                self.persist_settings();
            }
        }
    ),

    all_languages: Vec<Language>,
    source_language_code: String,
    target_language_code: String,
    detected_language_code: String,
    previous_screen: Screen,
    bus_tx: Option<Sender<IoEvent>>,
    session: Option<std::sync::Arc<translator::TranslatorSession>>,
    asset_dir: String,
    config_dir: String,
    data_dir: String,
    tts_voice_overrides: BTreeMap<String, String>,
    tts_prewarmed_language_code: String,
    original_image_path: String,
    /// Both rendered layers, held so the original/translated flip is a swap rather than a re-run.
    image_layers: Option<ImageLayers>,
    selection_anchor: Option<(u32, u32)>,
    pending_document_path: String,
    manage_filter: String,
    expanded_languages: HashSet<String>,
    manage_tts_picker_language_code: String,
    dictionary_popup_lookup_language_code: String,
    dictionary_popup_data: Option<WordWithTaggedEntries>,
}

/// The two drawable layers of a translated image plus the words selectable on each. Flipping
/// swaps both together, so the text under the pointer always matches the picture.
pub(crate) struct ImageLayers {
    pub translated: QImage,
    pub original: QImage,
    pub source_words: Vec<translator::ocr::PositionedWord>,
    pub translated_words: Vec<translator::ocr::PositionedWord>,
}

impl ImageLayers {
    pub(crate) fn words(&self, show_original: bool) -> &[translator::ocr::PositionedWord] {
        if show_original {
            &self.source_words
        } else {
            &self.translated_words
        }
    }
}
