use qmetaobject::QString;

use crate::IoEvent;
use crate::catalog_state::format_size;
use crate::document::{DocumentEvent, DocumentProgress};

use super::AppBridge;

impl AppBridge {
    /// Route a picked file: supported document types open the translate
    /// drawer; anything else goes down the existing image-OCR path.
    pub(crate) fn process_file_selection_impl(&mut self, url: String) {
        if url.is_empty() {
            return;
        }
        let Some(path) = crate::image_ocr::resolve_local_path(&url) else {
            self.set_output_text_value("Couldn't open the selected file".to_string());
            return;
        };
        let path_str = path.display().to_string();
        let Some(ext) = crate::document::supported_document_extension(&path_str) else {
            self.process_image_selection_impl(url);
            return;
        };

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document")
            .to_string();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        self.pending_document_path = path_str;
        self.doc_file_name = QString::from(name);
        self.doc_file_name_changed();
        self.doc_file_size = QString::from(format_size(size));
        self.doc_file_size_changed();
        let is_pdf = ext == "pdf";
        if self.doc_is_pdf != is_pdf {
            self.doc_is_pdf = is_pdf;
            self.doc_is_pdf_changed();
        }
        if !self.doc_drawer_open {
            self.doc_drawer_open = true;
            self.doc_drawer_open_changed();
        }
    }

    pub(crate) fn start_document_translation_impl(&mut self) {
        if self.pending_document_path.is_empty() {
            return;
        }
        self.doc_drawer_open = false;
        self.doc_drawer_open_changed();

        self.doc_progress_label = QString::from("Preparing file");
        self.doc_progress_label_changed();
        self.doc_text_fraction = 0.0;
        self.doc_text_fraction_changed();
        self.doc_show_pdf_phases = false;
        self.doc_show_pdf_phases_changed();
        self.doc_images_current = 0;
        self.doc_images_current_changed();
        self.doc_images_total = 0;
        self.doc_images_total_changed();
        self.doc_raster_current = 0;
        self.doc_raster_current_changed();
        self.doc_raster_total = 0;
        self.doc_raster_total_changed();
        self.doc_progress_open = true;
        self.doc_progress_open_changed();

        self.send_io(IoEvent::DocumentTranslationRequest {
            input_path: self.pending_document_path.clone(),
            from: self.source_language_code.clone(),
            to: self.target_language_code.clone(),
            translate_pdf_images: self.doc_is_pdf && self.doc_translate_images,
        });
    }

    pub(crate) fn cancel_document_translation_impl(&mut self) {
        self.send_io(IoEvent::CancelDocumentTranslation);
        self.doc_progress_open = false;
        self.doc_progress_open_changed();
    }

    pub(crate) fn apply_document_event(&mut self, event: DocumentEvent) {
        match event {
            DocumentEvent::Progress(progress) => self.apply_document_progress(progress),
            DocumentEvent::Done { output_path } => {
                self.doc_progress_open = false;
                self.doc_progress_open_changed();
                let name = std::path::Path::new(&output_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("translated")
                    .to_string();
                self.doc_output_name = QString::from(name);
                self.doc_output_name_changed();
                self.doc_output_url = QString::from(format!("file://{output_path}"));
                self.doc_output_url_changed();
                self.doc_error = QString::default();
                self.doc_error_changed();
                self.doc_done_open = true;
                self.doc_done_open_changed();
            }
            DocumentEvent::Failed { message } => {
                self.doc_progress_open = false;
                self.doc_progress_open_changed();
                self.doc_error = QString::from(message);
                self.doc_error_changed();
                self.doc_done_open = true;
                self.doc_done_open_changed();
            }
            DocumentEvent::Cancelled => {
                self.doc_progress_open = false;
                self.doc_progress_open_changed();
            }
        }
    }

    fn apply_document_progress(&mut self, progress: DocumentProgress) {
        match progress {
            DocumentProgress::Preparing => {
                self.doc_progress_label = QString::from("Preparing file");
                self.doc_progress_label_changed();
            }
            DocumentProgress::PdfPlan {
                image_xobjects,
                raster_pages,
                ..
            } => {
                self.doc_progress_label = QString::from("Translating");
                self.doc_progress_label_changed();
                self.doc_show_pdf_phases = true;
                self.doc_show_pdf_phases_changed();
                self.doc_images_total = image_xobjects as i32;
                self.doc_images_total_changed();
                self.doc_raster_total = raster_pages as i32;
                self.doc_raster_total_changed();
            }
            DocumentProgress::TranslatingText { fraction } => {
                self.doc_progress_label = QString::from("Translating");
                self.doc_progress_label_changed();
                self.doc_text_fraction = fraction;
                self.doc_text_fraction_changed();
            }
            DocumentProgress::TranslatingImages { current, total } => {
                self.doc_images_current = current as i32;
                self.doc_images_current_changed();
                self.doc_images_total = total as i32;
                self.doc_images_total_changed();
            }
            DocumentProgress::TranslatingRasterPages { current, total } => {
                self.doc_raster_current = current as i32;
                self.doc_raster_current_changed();
                self.doc_raster_total = total as i32;
                self.doc_raster_total_changed();
            }
            DocumentProgress::Writing => {
                self.doc_progress_label = QString::from("Writing file");
                self.doc_progress_label_changed();
            }
        }
    }
}
