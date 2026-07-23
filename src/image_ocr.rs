use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{GenericImageView, ImageDecoder, ImageReader, imageops::FilterType};
use translator::image_render::{RenderOptions, render_overlay};
use translator::ocr::{DetectedTextBox, PositionedWord};
use translator::{BackgroundMode, OcrSourceSelection, TranslatorSession};

use crate::fonts;

pub struct ImageTranslation {
    /// Only feeds language detection now; the translated text is read off the image.
    pub extracted_text: String,
    pub image_width: u32,
    pub image_height: u32,
    pub rendered_rgba_bytes: Vec<u8>,
    /// The loaded image before erasing and overlay, for the original/translated flip. Same
    /// dimensions as the rendered image, so both share one layout.
    pub original_rgba_bytes: Vec<u8>,
    pub source_words: Vec<PositionedWord>,
    pub translated_words: Vec<PositionedWord>,
}

struct LoadedImage {
    rgba_bytes: Vec<u8>,
    width: u32,
    height: u32,
}

pub(crate) fn load_preview_rgba(
    path: &Path,
    max_image_size: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let loaded = load_image_rgba(path, max_image_size)?;
    Ok((loaded.rgba_bytes, loaded.width, loaded.height))
}

pub fn resolve_local_path(input: &str) -> Option<PathBuf> {
    if input.is_empty() {
        return None;
    }

    if let Some(path) = input.strip_prefix("file://") {
        let decoded = percent_decode(path);
        let local = PathBuf::from(decoded);
        if !local.as_os_str().is_empty() {
            return Some(local);
        }
    }

    let direct = PathBuf::from(percent_decode(input));
    if direct.exists() {
        return Some(direct);
    }
    Some(direct)
}

pub fn translate_image_with_session(
    session: &TranslatorSession,
    image_path: &Path,
    source_code: &str,
    target_code: &str,
    min_confidence: u32,
    max_image_size: u32,
    background_mode_label: &str,
    // Fired with the detector's boxes before recognition starts, so the UI can mark the regions
    // it is about to scan. Detection is then handed to the pipeline rather than redone.
    on_detected: &(dyn Fn(&[DetectedTextBox], u32, u32) + Sync),
) -> Result<ImageTranslation, String> {
    let total_start = Instant::now();
    let load_start = Instant::now();
    let loaded = load_image_rgba(image_path, max_image_size)?;
    let load_elapsed = load_start.elapsed();
    let background_mode = map_background_mode(background_mode_label);

    let detect_start = Instant::now();
    let detection = session
        .detect_image_boxes(
            &loaded.rgba_bytes,
            loaded.width,
            loaded.height,
            max_image_size,
        )
        .map_err(|err| err.message)?;
    on_detected(&detection, loaded.width, loaded.height);
    let detect_elapsed = detect_start.elapsed();

    let process_start = Instant::now();
    let prepared = session
        .translate_image_rgba(
            &loaded.rgba_bytes,
            loaded.width,
            loaded.height,
            max_image_size,
            OcrSourceSelection::specific(source_code),
            target_code,
            min_confidence,
            None,
            background_mode,
            Some(detection),
        )
        .map_err(|err| {
            if err.is_missing_asset() {
                format!("Missing installed language pair {source_code}->{target_code}")
            } else {
                err.message
            }
        })?;
    let process_elapsed = process_start.elapsed();

    let render_start = Instant::now();
    let opts = RenderOptions {
        language: target_code.to_string(),
        min_font_size_px: 8.0,
    };
    let provider = fonts::provider();
    let (rendered_rgba_bytes, translated_words) = match render_overlay(&prepared, &*provider, &opts)
    {
        Ok(rendered) => (rendered.rgba_bytes, rendered.translated_words),
        Err(err) => {
            eprintln!("image_render render_overlay failed: {err}; showing erased background");
            (prepared.rgba_bytes.clone(), Vec::new())
        }
    };
    let render_elapsed = render_start.elapsed();

    println!(
        "image_ocr timings load={:?} detect={:?} process={:?} render={:?} total={:?}",
        load_elapsed,
        detect_elapsed,
        process_elapsed,
        render_elapsed,
        total_start.elapsed()
    );

    if prepared.width != loaded.width || prepared.height != loaded.height {
        return Err(format!(
            "overlay is {}x{} but the source is {}x{}; word boxes would not line up",
            prepared.width, prepared.height, loaded.width, loaded.height
        ));
    }

    Ok(ImageTranslation {
        extracted_text: prepared.extracted_text,
        image_width: prepared.width,
        image_height: prepared.height,
        rendered_rgba_bytes,
        original_rgba_bytes: loaded.rgba_bytes,
        source_words: prepared.source_words,
        translated_words,
    })
}

fn load_image_rgba(path: &Path, max_image_size: u32) -> Result<LoadedImage, String> {
    let mut decoder = ImageReader::open(path)
        .map_err(|err| format!("Failed to open image {}: {err}", path.display()))?
        .into_decoder()
        .map_err(|err| {
            format!(
                "Failed to create decoder for image {}: {err}",
                path.display()
            )
        })?;
    let orientation = decoder.orientation().map_err(|err| {
        format!(
            "Failed to read orientation for image {}: {err}",
            path.display()
        )
    })?;
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|err| format!("Failed to decode image {}: {err}", path.display()))?;
    image.apply_orientation(orientation);

    let (source_width, source_height) = image.dimensions();
    println!(
        "image load orientation path={} orientation={orientation:?} size={}x{} max_image_size={}",
        path.display(),
        source_width,
        source_height,
        max_image_size
    );
    if source_width == 0 || source_height == 0 {
        return Err(format!("Failed to load image: {}", path.display()));
    }

    let (width, height) = scaled_dimensions(source_width, source_height, max_image_size);
    let rgba = if width == source_width && height == source_height {
        image.to_rgba8()
    } else {
        image
            .resize_exact(width, height, FilterType::Triangle)
            .to_rgba8()
    };

    Ok(LoadedImage {
        rgba_bytes: rgba.into_raw(),
        width,
        height,
    })
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3])
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            decoded.push(value);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn scaled_dimensions(width: u32, height: u32, max_image_size: u32) -> (u32, u32) {
    if max_image_size == 0 {
        return (width.max(1), height.max(1));
    }

    let largest = width.max(height);
    if largest <= max_image_size {
        return (width.max(1), height.max(1));
    }

    if width >= height {
        let scaled_height = ((height as u64 * max_image_size as u64) / width as u64).max(1);
        (max_image_size, scaled_height as u32)
    } else {
        let scaled_width = ((width as u64 * max_image_size as u64) / height as u64).max(1);
        (scaled_width as u32, max_image_size)
    }
}

fn map_background_mode(label: &str) -> BackgroundMode {
    match label {
        "Light Background" => BackgroundMode::BlackOnWhite,
        "Dark Background" => BackgroundMode::WhiteOnBlack,
        _ => BackgroundMode::AutoDetect,
    }
}
