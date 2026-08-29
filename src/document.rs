//! Document (pdf/epub/odt/txt) translation: a port of the Android wrapper's
//! `translate_document_path_impl` over the same translator-rs pipelines, with
//! progress events delivered through a plain callback instead of uniffi.

#[cfg(feature = "pdf")]
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use translator::api::ScriptedLanguage;
use translator::{LanguageCode, TranslatorSession};

#[cfg(feature = "pdf")]
use font_provider::system_fonts;

#[derive(Debug, Clone, Copy)]
pub enum DocumentProgress {
    Preparing,
    /// PDF-only: emitted once after inventory so the UI can show the three
    /// labelled bars (text / images / raster pages) with totals up-front.
    /// `raster_pages` is an upper bound; the raster pass refines it.
    PdfPlan {
        text_pages: u32,
        image_xobjects: u32,
        raster_pages: u32,
    },
    /// Smooth completion fraction in `[0.0, 1.0]` for every text path.
    TranslatingText {
        fraction: f32,
    },
    TranslatingImages {
        current: u32,
        total: u32,
    },
    TranslatingRasterPages {
        current: u32,
        total: u32,
    },
    Writing,
}

#[derive(Debug)]
pub enum DocumentError {
    Cancelled,
    Other(String),
}

/// What the translation job reports back to the UI thread.
#[derive(Debug, Clone)]
pub enum DocumentEvent {
    Progress(DocumentProgress),
    Done { output_path: String },
    Failed { message: String },
    Cancelled,
}

#[cfg(feature = "pdf")]
const DOCUMENT_EXTENSIONS: &[&str] = &["pdf", "epub", "odt", "txt"];
#[cfg(not(feature = "pdf"))]
const DOCUMENT_EXTENSIONS: &[&str] = &["epub", "odt", "txt"];

pub fn supported_document_extension(path: &str) -> Option<String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())?
        .to_ascii_lowercase();
    DOCUMENT_EXTENSIONS.contains(&ext.as_str()).then_some(ext)
}

fn installed_languages(session: &TranslatorSession) -> Vec<ScriptedLanguage> {
    session
        .language_overview()
        .into_iter()
        .filter(|entry| entry.availability.translator_files() || entry.language.is_english())
        .map(|entry| entry.language.scripted())
        .collect()
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(feature = "pdf"), allow(unused_variables))]
pub fn translate_document(
    session: &TranslatorSession,
    input_path: &str,
    output_path: &str,
    source_code: &str,
    target_code: &str,
    translate_pdf_images: bool,
    cancel: &AtomicBool,
    on_progress: &(dyn Fn(DocumentProgress) + Sync),
) -> Result<(), DocumentError> {
    let is_cancelled = || cancel.load(Ordering::Relaxed);
    let check_cancelled = || {
        if is_cancelled() {
            Err(DocumentError::Cancelled)
        } else {
            Ok(())
        }
    };
    // The text translators report a fraction per sentence from slimt worker
    // threads; forward only when it advances by ≥0.1% so the UI thread isn't
    // flooded with queued events.
    let last_permille = AtomicUsize::new(0);
    let report_text = |fraction: f32| {
        let permille = (fraction * 1000.0) as usize;
        let prev = last_permille.fetch_max(permille, Ordering::Relaxed);
        if permille > prev || fraction >= 1.0 {
            on_progress(DocumentProgress::TranslatingText { fraction });
        }
    };

    check_cancelled()?;
    on_progress(DocumentProgress::Preparing);
    session.begin_document_translation();
    let extension = supported_document_extension(input_path)
        .ok_or_else(|| DocumentError::Other(format!("unsupported document type: {input_path}")))?;
    let target = session
        .scripted_language(&LanguageCode::from(target_code))
        .ok_or_else(|| DocumentError::Other(format!("unknown target language: {target_code}")))?;
    let available = installed_languages(session);
    let input_bytes = fs::read(input_path)
        .map_err(|error| DocumentError::Other(format!("failed to read document: {error}")))?;
    check_cancelled()?;

    let output_bytes = match extension.as_str() {
        "txt" => {
            let text = String::from_utf8(input_bytes)
                .map_err(|error| DocumentError::Other(format!("text is not UTF-8: {error}")))?;
            let translated = translator::txt::translate_txt_with_progress(
                session,
                &text,
                source_code,
                target_code,
                translator::txt::TxtLayout::Preserve,
                report_text,
            )
            .map_err(|error| match error {
                translator::txt::TxtTranslateError::Cancelled => DocumentError::Cancelled,
                translator::txt::TxtTranslateError::Translation(message) => {
                    DocumentError::Other(format!("failed to translate text: {message}"))
                }
            })?;
            translated.into_bytes()
        }
        "odt" => translator::odt::translate_odt_with_progress(
            session,
            &input_bytes,
            Some(source_code),
            target_code,
            &available,
            report_text,
        )
        .map_err(|error| match error {
            translator::odt::OdtTranslateError::Cancelled => DocumentError::Cancelled,
            other => DocumentError::Other(format!("failed to translate ODT: {other}")),
        })?,
        "epub" => translator::epub::translate_epub_with_progress(
            session,
            &input_bytes,
            Some(source_code),
            target_code,
            &available,
            report_text,
        )
        .map_err(|error| match error {
            translator::epub::EpubTranslateError::Cancelled => DocumentError::Cancelled,
            other => DocumentError::Other(format!("failed to translate EPUB: {other}")),
        })?,
        #[cfg(feature = "pdf")]
        "pdf" => translate_pdf(
            session,
            &input_bytes,
            source_code,
            &target,
            &available,
            translate_pdf_images,
            &is_cancelled,
            on_progress,
            &report_text,
        )?,
        _ => unreachable!("extension filtered by supported_document_extension"),
    };

    check_cancelled()?;
    on_progress(DocumentProgress::Writing);
    check_cancelled()?;
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DocumentError::Other(format!("failed to create output dir: {error}"))
        })?;
    }
    fs::write(output_path, output_bytes)
        .map_err(|error| DocumentError::Other(format!("failed to write document: {error}")))?;
    Ok(())
}

/// Pipeline order: text translation first, then image-XObject translation,
/// then page-raster overlay. Each later pass must not see its own output:
/// text surgery after the overlay would re-process the overlay's `Tj` ops,
/// and XObject re-encoding after the raster pass would bake duplicate text.
#[cfg(feature = "pdf")]
#[allow(clippy::too_many_arguments)]
fn translate_pdf(
    session: &TranslatorSession,
    input_bytes: &[u8],
    source_code: &str,
    target: &ScriptedLanguage,
    available: &[ScriptedLanguage],
    translate_pdf_images: bool,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    on_progress: &(dyn Fn(DocumentProgress) + Sync),
    report_text: &(dyn Fn(f32) + Sync),
) -> Result<Vec<u8>, DocumentError> {
    let overlay_pages: HashSet<usize> = if translate_pdf_images {
        translator::pdf_image_translate::log_page_inventory(input_bytes);
        let pages = translator::pdf_image_translate::pages_without_extractable_text(input_bytes);
        if let Some(inv) = translator::pdf_image_translate::pdf_translation_inventory(input_bytes) {
            on_progress(DocumentProgress::PdfPlan {
                text_pages: inv.total_pages,
                image_xobjects: inv.image_xobjects,
                raster_pages: inv.raster_pages,
            });
        }
        pages
    } else {
        HashSet::new()
    };

    let translations = match translator::pdf_translate::translate_pdf_with_progress(
        session,
        input_bytes,
        Some(source_code),
        target,
        available,
        report_text,
    ) {
        Ok(t) => t,
        // No native text, but image translation may still add overlay
        // content; the writer round-trips the bytes for an empty set.
        Err(translator::pdf_translate::PdfTranslateError::NoTextFound) => Vec::new(),
        Err(translator::pdf_translate::PdfTranslateError::Cancelled) => {
            return Err(DocumentError::Cancelled);
        }
        Err(error) => {
            return Err(DocumentError::Other(format!(
                "failed to translate PDF: {error}"
            )));
        }
    };
    let fonts = system_fonts();
    let after_text =
        translator::pdf_write::write_translated_pdf(input_bytes, &translations, &*fonts)
            .map_err(|error| DocumentError::Other(format!("failed to write PDF: {error}")))?;

    if !translate_pdf_images {
        return Ok(after_text);
    }

    let xobject_progress = |current: usize, total: usize| {
        on_progress(DocumentProgress::TranslatingImages {
            current: current as u32,
            total: total as u32,
        });
    };
    let xobject_output = translator::pdf_image_translate::translate_pdf_images_in_place(
        &after_text,
        session,
        source_code,
        target.as_str(),
        &*fonts,
        &overlay_pages,
        is_cancelled,
        xobject_progress,
    )
    .map_err(|error| DocumentError::Other(format!("failed to translate PDF images: {error}")))?;
    if is_cancelled() {
        return Err(DocumentError::Cancelled);
    }

    // Pages whose visible content was already translated via image XObjects
    // don't need a raster overlay stamped on top of the translated bitmap.
    let raster_pages: HashSet<usize> = overlay_pages
        .difference(&xobject_output.translated_pages)
        .copied()
        .collect();
    let page_progress = |current: usize, total: usize| {
        on_progress(DocumentProgress::TranslatingRasterPages {
            current: current as u32,
            total: total as u32,
        });
    };
    let final_bytes = translator::pdf_image_translate::translate_pdf_pages_as_raster_in_place(
        &xobject_output.bytes,
        session,
        source_code,
        target,
        &*fonts,
        &raster_pages,
        is_cancelled,
        page_progress,
    )
    .map_err(|error| DocumentError::Other(format!("failed to rasterize PDF pages: {error}")))?;
    if is_cancelled() {
        return Err(DocumentError::Cancelled);
    }
    Ok(final_bytes)
}
