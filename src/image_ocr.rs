use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{GenericImageView, ImageDecoder, ImageReader, imageops::FilterType};
use translator::{BackgroundMode, OcrSourceSelection, PreferredOcrEngine, TranslatorSession};

#[derive(Debug, Clone)]
pub struct ImageOverlayLine {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub foreground_argb: u32,
}

#[derive(Debug, Clone)]
pub struct ImageOverlayBlock {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub suggested_font_size_px: f32,
    pub lines: Vec<ImageOverlayLine>,
    pub translated_text: String,
    pub background_argb: u32,
    pub foreground_argb: u32,
}

pub struct ImageTranslation {
    pub extracted_text: String,
    pub translated_text: String,
    pub image_width: u32,
    pub image_height: u32,
    pub cleaned_rgba_bytes: Vec<u8>,
    pub overlay_blocks: Vec<ImageOverlayBlock>,
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
) -> Result<ImageTranslation, String> {
    let total_start = Instant::now();
    let load_start = Instant::now();
    let loaded = load_image_rgba(image_path, max_image_size)?;
    let load_elapsed = load_start.elapsed();
    let background_mode = map_background_mode(background_mode_label);
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
            translator::ReadingOrder::LeftToRight,
            background_mode,
            PreferredOcrEngine::Paddle,
        )
        .map_err(|err| {
            if err.is_missing_asset() {
                format!("Missing installed language pair {source_code}->{target_code}")
            } else {
                err.message
            }
        })?;
    let process_elapsed = process_start.elapsed();

    let overlay_blocks = prepared
        .blocks
        .into_iter()
        .map(|block| ImageOverlayBlock {
            suggested_font_size_px: block.layout_hints.suggested_font_size_px,
            lines: block
                .lines
                .iter()
                .map(|line| ImageOverlayLine {
                    x: line.bounding_box.left,
                    y: line.bounding_box.top,
                    width: line.bounding_box.width(),
                    height: line.bounding_box.height(),
                    foreground_argb: line.foreground_argb,
                })
                .collect(),
            x: block.bounding_box.left,
            y: block.bounding_box.top,
            width: block.bounding_box.width(),
            height: block.bounding_box.height(),
            translated_text: block.translated_text,
            background_argb: block.background_argb,
            foreground_argb: block.foreground_argb,
        })
        .collect::<Vec<_>>();

    println!(
        "image_ocr timings load={:?} process={:?} total={:?}",
        load_elapsed,
        process_elapsed,
        total_start.elapsed()
    );

    Ok(ImageTranslation {
        extracted_text: prepared.extracted_text,
        translated_text: prepared.translated_text,
        image_width: loaded.width,
        image_height: loaded.height,
        cleaned_rgba_bytes: prepared.rgba_bytes,
        overlay_blocks,
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
