//! Live-OCR pipeline ownership + a headless benchmark harness.
//!
//! The live camera path is fully GPU now: the video filter taps the camera's
//! external-OES texture id (no CPU map), and `live_gpu` runs `process_frame` on
//! the render thread — canonical frame from a GPU readback, fused external-camera
//! composite to screen. This module owns the shared `LiveTrackerPipeline`,
//! exposes it to `live_gpu`, drives a per-frame repaint tick, and carries the
//! UI controls (focus re-acquire, language, active/suppressed). `run_benchmark`
//! is an unrelated headless harness that feeds a still image through
//! `process_frame`.

use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use image::{GenericImageView, ImageReader, imageops::FilterType};
use translator::live_frame::LiveFrame;
use translator::live_tracker_pipeline::{LiveTrackerPipeline, TargetMode};
use translator::{Rect, TranslatorSession};

use crate::fonts;

static PIPELINE: OnceLock<Arc<LiveTrackerPipeline>> = OnceLock::new();
static LIVE_FRAME_TICK: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

/// The render thread (`live_gpu`) calls `process_frame` on this shared pipeline.
pub(crate) fn live_pipeline() -> Option<&'static Arc<LiveTrackerPipeline>> {
    PIPELINE.get()
}

/// Set the GUI-thread callback that schedules a repaint of the live view.
pub fn set_live_frame_tick(tick: Arc<dyn Fn() + Send + Sync>) {
    let _ = LIVE_FRAME_TICK.set(tick);
}

/// Fire the repaint tick — called per camera frame from the filter (via
/// `live_gpu_set_camera_texture`) to drive an afterRendering present.
pub(crate) fn fire_frame_tick() {
    if let Some(tick) = LIVE_FRAME_TICK.get() {
        tick();
    }
}

/// Kept for QML compatibility (`appBridge.set_live_viewport`). The canonical
/// frame is sized from the render-target dimensions in `live_gpu`, so this is
/// currently a no-op.
pub fn set_live_viewport(_width: u32, _height: u32) {}

/// Force a fresh acquire on the next frame (the user tapped the preview).
/// Clears tracker/overlay/session state so the next `process_frame` re-detects
/// from scratch instead of tracking the stale lock.
pub fn request_acquire() {
    if let Some(pipeline) = PIPELINE.get() {
        pipeline.reset();
    }
}

/// Point the live pipeline at the languages currently selected in the UI.
/// Resets so the change takes effect on the next acquire rather than waiting
/// for the current lock to drop.
pub fn set_live_languages(from: &str, to: &str) {
    if let Some(pipeline) = PIPELINE.get() {
        pipeline.set_languages(from, to, false);
        pipeline.reset();
    }
}

/// Toggle whether the live pipeline detects/translates (`Active`) or just
/// composites the camera as a plain viewfinder (`Suppressed`).
pub fn set_live_active(active: bool) {
    if let Some(pipeline) = PIPELINE.get() {
        let mode = if active {
            TargetMode::Active
        } else {
            TargetMode::Suppressed
        };
        pipeline.set_target_mode(mode);
    }
}

pub fn init_live_pipeline(session: Arc<TranslatorSession>) {
    let pipeline = LiveTrackerPipeline::new(session, fonts::provider());
    pipeline.set_languages("en", "nl", false);
    let _ = PIPELINE.set(pipeline);
}

fn load_rgba(path: &Path, max_side: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let img = ImageReader::open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("decode {}: {e}", path.display()))?;
    let (w0, h0) = img.dimensions();
    let img = if w0.max(h0) > max_side {
        let scale = max_side as f32 / w0.max(h0) as f32;
        let nw = ((w0 as f32 * scale) as u32).max(1);
        let nh = ((h0 as f32 * scale) as u32).max(1);
        img.resize_exact(nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Ok((rgba.into_raw(), w, h))
}

pub fn run_benchmark(
    session: Arc<TranslatorSession>,
    image_path: &str,
    from: &str,
    to: &str,
    max_side: u32,
    frames: u32,
) {
    let (rgba, w, h) = match load_rgba(Path::new(image_path), max_side) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bench: {e}");
            return;
        }
    };
    eprintln!("bench: image {w}x{h} (max_side={max_side}) from={from} to={to} frames={frames}");

    let pipeline = LiveTrackerPipeline::new(session, fonts::provider());
    pipeline.set_languages(from, to, false);

    let frame = Arc::new(LiveFrame::new((w * h * 4) as usize));
    let crop = Rect {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
    };
    let mut dst = vec![0u8; (w * h * 4) as usize];
    let mut timestamp_ns: u64 = 0;

    for i in 0..frames {
        frame.reset_owned(rgba.clone(), w, h, 0);
        let started = Instant::now();
        let mut target = translator::live_compositor::SliceTarget { dst: &mut dst[..] };
        let result =
            pipeline.process_frame(&frame, crop, &mut target, w, h, w, h, w, h, timestamp_ns);
        let frame_ms = started.elapsed().as_secs_f32() * 1000.0;
        match result {
            Ok(r) => eprintln!(
                "bench frame {i}: {frame_ms:.1}ms state={:?} inliers={} composite_bytes={} acquire={} refresh={}",
                r.state, r.inliers, r.composite_bytes, r.started_acquire, r.started_refresh
            ),
            Err(e) => eprintln!("bench frame {i}: {frame_ms:.1}ms ERR {}", e.message),
        }
        if let Some(t) = pipeline.last_acquire_telemetry() {
            eprintln!(
                "bench telemetry: total_ms={:.1} detected={} rec_ok={} rec_empty={} canceled={} is_refresh={} err={:?}",
                t.total_ms,
                t.detected_count,
                t.rec_ok_count,
                t.rec_empty_count,
                t.canceled,
                t.is_refresh,
                t.error
            );
        }
        timestamp_ns += 33_000_000;
    }

    let out = format!("/tmp/live_bench_{w}x{h}.png");
    match image::save_buffer(&out, &dst, w, h, image::ExtendedColorType::Rgba8) {
        Ok(()) => eprintln!("bench: wrote composite to {out}"),
        Err(e) => eprintln!("bench: failed to write {out}: {e}"),
    }
}
