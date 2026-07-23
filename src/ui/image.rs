use qmetaobject::{QImage, QString};
use translator::selection::{self, WritingAxis};

use crate::IoEvent;
use crate::rendered_image_item::qimage_from_rgba_bytes;

use super::{AppBridge, ImageLayers, ImageResult, SelectionPillItem};

impl AppBridge {
    pub(crate) fn set_image_mode_value(&mut self, value: bool) {
        if self.image_mode != value {
            self.image_mode = value;
            self.image_mode_changed();
        }
    }

    pub(crate) fn set_selected_image_url_value(&mut self, url: String) {
        let url = QString::from(url);
        if self.selected_image_url != url {
            self.selected_image_url = url;
            self.selected_image_url_changed();
        }
    }

    pub(crate) fn set_processed_image_value(&mut self, image: QImage) {
        let size = image.size();
        let width = size.width as f32;
        let height = size.height as f32;
        if self.processed_image != image {
            self.processed_image = image;
            self.processed_image_changed();
        }
        if (self.processed_image_width - width).abs() > f32::EPSILON {
            self.processed_image_width = width;
            self.processed_image_width_changed();
        }
        if (self.processed_image_height - height).abs() > f32::EPSILON {
            self.processed_image_height = height;
            self.processed_image_height_changed();
        }
    }

    pub(crate) fn set_detected_regions_value(&mut self, boxes: Vec<SelectionPillItem>) {
        let active = !boxes.is_empty();
        self.scan_boxes_model.borrow_mut().reset_data(boxes);
        if self.scan_active != active {
            self.scan_active = active;
            self.scan_active_changed();
        }
    }

    pub(crate) fn set_image_error_value(&mut self, message: String) {
        let message = QString::from(message);
        if self.image_error != message {
            self.image_error = message;
            self.image_error_changed();
        }
    }

    fn end_scan(&mut self) {
        self.scan_boxes_model.borrow_mut().reset_data(Vec::new());
        if self.scan_active {
            self.scan_active = false;
            self.scan_active_changed();
        }
    }

    pub(crate) fn set_image_result_value(&mut self, result: ImageResult) {
        self.end_scan();
        self.set_image_error_value(String::new());
        let layers = ImageLayers {
            translated: result.translated,
            original: result.original,
            source_words: result.source_words,
            translated_words: result.translated_words,
        };
        let ready = !layers.source_words.is_empty() || !layers.translated_words.is_empty();
        self.set_processed_image_value(layers.translated.clone());
        self.image_layers = Some(layers);
        self.clear_image_selection_impl();
        self.set_image_show_original_value(false);
        if self.image_words_ready != ready {
            self.image_words_ready = ready;
            self.image_words_ready_changed();
        }
    }

    fn set_image_show_original_value(&mut self, value: bool) {
        if self.image_show_original == value {
            return;
        }
        self.image_show_original = value;
        self.image_show_original_changed();
        if let Some(layers) = &self.image_layers {
            let shown = if value {
                layers.original.clone()
            } else {
                layers.translated.clone()
            };
            self.set_processed_image_value(shown);
        }
    }

    /// Flipping changes which word layer is under the pointer, so the old selection — indices into
    /// the other layer — cannot survive it.
    pub(crate) fn toggle_image_original_impl(&mut self) {
        let flipped = !self.image_show_original;
        self.clear_image_selection_impl();
        self.set_image_show_original_value(flipped);
    }

    fn current_words(&self) -> &[translator::ocr::PositionedWord] {
        self.image_layers
            .as_ref()
            .map(|layers| layers.words(self.image_show_original))
            .unwrap_or_default()
    }

    pub(crate) fn image_word_at_impl(&mut self, x: f32, y: f32) -> i32 {
        selection::hit_test_word(self.current_words(), x, y).map_or(-1, |index| index as i32)
    }

    /// `anchor` is the word a drag started from, or negative when there is none; it pins the
    /// selection to one writing axis so a horizontal drag cannot sweep into a vertical column.
    pub(crate) fn image_nearest_word_impl(&mut self, x: f32, y: f32, anchor: i32) -> i32 {
        let words = self.current_words();
        let axis: Option<WritingAxis> = (anchor >= 0)
            .then(|| selection::word_axis(words, anchor as u32))
            .flatten();
        selection::nearest_word(words, x, y, axis).map_or(-1, |index| index as i32)
    }

    pub(crate) fn set_image_selection_impl(&mut self, start: i32, end: i32) {
        if start < 0 || end < 0 {
            self.clear_image_selection_impl();
            return;
        }
        let (start, end) = (start as u32, end as u32);
        let Some(view) = selection::resolve_selection(self.current_words(), start, end) else {
            self.clear_image_selection_impl();
            return;
        };

        self.selection_anchor = Some((start, end));
        {
            let mut pills = self.selection_pills_model.borrow_mut();
            pills.reset_data(
                view.pills
                    .iter()
                    .map(|pill| SelectionPillItem {
                        cx: pill.cx,
                        cy: pill.cy,
                        width: pill.width,
                        height: pill.height,
                        angle_degrees: pill.angle_radians.to_degrees(),
                    })
                    .collect(),
            );
        }
        self.selection_active = true;
        self.selection_text = QString::from(view.text);
        self.selection_start_x = view.start_handle.x;
        self.selection_start_y = view.start_handle.y;
        self.selection_end_x = view.end_handle.x;
        self.selection_end_y = view.end_handle.y;
        self.selection_left = view.bounds.left as f32;
        self.selection_top = view.bounds.top as f32;
        self.selection_right = view.bounds.right as f32;
        self.selection_bottom = view.bounds.bottom as f32;
        self.selection_changed();
    }

    pub(crate) fn clear_image_selection_impl(&mut self) {
        if !self.selection_active && self.selection_anchor.is_none() {
            return;
        }
        self.selection_anchor = None;
        self.selection_pills_model
            .borrow_mut()
            .reset_data(Vec::new());
        self.selection_active = false;
        self.selection_text = QString::from("");
        self.selection_changed();
    }

    pub(crate) fn bump_live_frame_tick(&mut self) {
        self.live_frame_tick = self.live_frame_tick.wrapping_add(1);
        self.live_frame_tick_changed();
    }

    pub(crate) fn set_share_image_url_value(&mut self, url: String) {
        let url = QString::from(url);
        if self.share_image_url != url {
            self.share_image_url = url;
            self.share_image_url_changed();
        }
    }

    pub(crate) fn set_image_viewer_open_value(&mut self, value: bool) {
        if self.image_viewer_open != value {
            self.image_viewer_open = value;
            self.image_viewer_open_changed();
        }
    }

    pub(crate) fn process_text_impl(&mut self, text: String) {
        let qtext = QString::from(text.clone());
        if self.input_text != qtext {
            self.input_text = qtext;
            self.input_text_changed();
        }
        self.refresh_input_transliteration();

        self.stop_tts();

        self.send_io(IoEvent::TranslationRequest {
            text,
            from: self.source_language_code.clone(),
            to: self.target_language_code.clone(),
        });
    }

    pub(crate) fn process_image_selection_impl(&mut self, url: String) {
        if self.disable_ocr {
            self.set_output_text_value("OCR is disabled in settings".to_string());
            return;
        }

        if url.is_empty() {
            return;
        }

        let Some(path) = crate::image_ocr::resolve_local_path(&url) else {
            self.set_output_text_value("Couldn't open the selected image".to_string());
            return;
        };

        let preview =
            crate::image_ocr::load_preview_rgba(&path, self.ocr_max_image_size.max(0) as u32).ok();

        self.original_image_path = path.display().to_string();
        self.stop_tts();
        self.set_image_mode_value(true);
        self.set_image_viewer_open_value(false);
        self.set_selected_image_url_value(url.clone());
        self.set_share_image_url_value(url);
        if let Some((rgba_bytes, width, height)) = preview {
            self.set_processed_image_value(qimage_from_rgba_bytes(width, height, &rgba_bytes));
        } else {
            self.set_processed_image_value(QImage::default());
        }
        self.end_scan();
        self.set_image_error_value(String::new());
        self.set_input_text_value(String::new());
        self.set_output_text_value(String::new());
        self.set_detected_language_code_value("");

        self.send_io(IoEvent::ImageTranslationRequest {
            image_path: self.original_image_path.clone(),
            from: self.source_language_code.clone(),
            to: self.target_language_code.clone(),
            min_confidence: self.ocr_min_confidence.max(0) as u32,
            max_image_size: self.ocr_max_image_size.max(0) as u32,
            background_mode: self.ocr_background_mode.to_string(),
        });
    }

    pub(crate) fn refresh_translation_content(&mut self) {
        if self.image_mode {
            self.rerun_current_image();
        } else {
            self.retranslate();
        }
    }

    pub(crate) fn rerun_current_image(&mut self) {
        if self.original_image_path.is_empty() {
            return;
        }

        self.stop_tts();
        self.set_image_viewer_open_value(false);
        self.end_scan();
        self.set_image_error_value(String::new());
        self.set_detected_language_code_value("");

        self.send_io(IoEvent::ImageTranslationRequest {
            image_path: self.original_image_path.clone(),
            from: self.source_language_code.clone(),
            to: self.target_language_code.clone(),
            min_confidence: self.ocr_min_confidence.max(0) as u32,
            max_image_size: self.ocr_max_image_size.max(0) as u32,
            background_mode: self.ocr_background_mode.to_string(),
        });
    }

    pub(crate) fn retranslate(&mut self) {
        self.stop_tts();
        self.send_io(IoEvent::TranslationRequest {
            text: self.input_text.to_string(),
            from: self.source_language_code.clone(),
            to: self.target_language_code.clone(),
        });
    }
}
