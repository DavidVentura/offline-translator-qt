use qmetaobject::{QImage, QString};

use crate::IoEvent;
use crate::rendered_image_item::qimage_from_rgba_bytes;

use super::{AppBridge, ImageOverlayListItem};

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
        if self.processed_image != image {
            self.processed_image = image;
            self.processed_image_changed();
        }
    }

    pub(crate) fn set_live_camera_image_value(&mut self, image: QImage) {
        self.live_camera_image = image;
        self.live_camera_image_changed();
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

    pub(crate) fn set_image_overlay_value(
        &mut self,
        items: Vec<ImageOverlayListItem>,
        width: f32,
        height: f32,
    ) {
        self.image_overlay_model.borrow_mut().reset_data(items);

        if (self.processed_image_width - width).abs() > f32::EPSILON {
            self.processed_image_width = width;
            self.processed_image_width_changed();
        }
        if (self.processed_image_height - height).abs() > f32::EPSILON {
            self.processed_image_height = height;
            self.processed_image_height_changed();
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
            self.set_image_overlay_value(Vec::new(), width as f32, height as f32);
        } else {
            self.set_processed_image_value(QImage::default());
            self.set_image_overlay_value(Vec::new(), 0.0, 0.0);
        }
        self.set_input_text_value(String::new());
        self.set_output_text_value("Running OCR...".to_string());
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
        self.set_image_overlay_value(
            Vec::new(),
            self.processed_image_width,
            self.processed_image_height,
        );
        self.set_image_viewer_open_value(false);
        self.set_output_text_value("Running OCR...".to_string());
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
