//! Headless harness for the live-OCR pipeline. Feeds a still image through
//! `LiveTrackerPipeline::process_frame` repeatedly so we can measure acquire
//! (det+rec) and steady-state per-frame cost on a target device without any
//! camera/Qt plumbing — the per-frame compute is identical regardless of where
//! the bytes come from.

use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Instant;

use image::{GenericImageView, ImageReader, imageops::FilterType};
use translator::font_provider::{FontHandle, FontProvider, FontRequest};
use translator::live_frame::LiveFrame;
use translator::live_tracker_pipeline::LiveTrackerPipeline;
use translator::{Rect, TranslatorSession};

struct DejaVuFontProvider;

impl FontProvider for DejaVuFontProvider {
    fn locate(&self, _request: &FontRequest) -> Vec<FontHandle> {
        vec![FontHandle::from(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        )]
    }
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

    let pipeline = LiveTrackerPipeline::new(session, Arc::new(DejaVuFontProvider));
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

struct PendingFrame {
    rgbx: Vec<u8>,
    stride: usize,
    width: u32,
    height: u32,
}

/// A camera frame already rotated upright, cropped to the viewport aspect and
/// downscaled — ready to hand to the GPU compositor as both the camera source
/// and the composite output size.
pub(crate) struct ReadyFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

struct LiveState {
    pending: Mutex<Option<PendingFrame>>,
    cv: Condvar,
    viewport: Mutex<(u32, u32)>,
    // Latest transformed frame awaiting GPU present on the render thread.
    ready: Mutex<Option<ReadyFrame>>,
}

static LIVE_STATE: OnceLock<Arc<LiveState>> = OnceLock::new();
static PIPELINE: OnceLock<Arc<LiveTrackerPipeline>> = OnceLock::new();
static LIVE_FRAME_TICK: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

/// The render thread calls `process_frame` on this shared pipeline; it's created
/// in [`init_live_pipeline`] before any frame can arrive.
pub(crate) fn live_pipeline() -> Option<&'static Arc<LiveTrackerPipeline>> {
    PIPELINE.get()
}

/// Move the latest transformed frame out for the render thread to present.
/// Returns `None` when no new frame has arrived since the last call.
pub(crate) fn take_ready_frame() -> Option<ReadyFrame> {
    LIVE_STATE
        .get()?
        .ready
        .lock()
        .expect("ready poisoned")
        .take()
}

/// Called by the worker after staging a frame to ask the live item to repaint.
/// Carries no pixels — the render thread pulls the frame via [`take_ready_frame`].
pub fn set_live_frame_tick(tick: Arc<dyn Fn() + Send + Sync>) {
    let _ = LIVE_FRAME_TICK.set(tick);
}

pub fn set_live_viewport(width: u32, height: u32) {
    if let Some(state) = LIVE_STATE.get() {
        *state.viewport.lock().expect("viewport poisoned") = (width, height);
    }
}

/// Force a fresh acquire on the next frame (the user tapped the preview).
/// Clears tracker, overlay and session state so the next `process_frame`
/// re-detects from scratch instead of tracking the stale lock.
pub fn request_acquire() {
    if let Some(pipeline) = PIPELINE.get() {
        pipeline.reset();
    }
}

pub fn init_live_pipeline(session: Arc<TranslatorSession>) {
    let pipeline = LiveTrackerPipeline::new(session, Arc::new(DejaVuFontProvider));
    pipeline.set_languages("en", "nl", false);
    let _ = PIPELINE.set(pipeline);

    let state = Arc::new(LiveState {
        pending: Mutex::new(None),
        cv: Condvar::new(),
        viewport: Mutex::new((0, 0)),
        ready: Mutex::new(None),
    });
    let _ = LIVE_STATE.set(Arc::clone(&state));
    std::thread::Builder::new()
        .name("live-ocr".into())
        .spawn(move || live_worker(state))
        .expect("failed to spawn live-ocr worker");
}

// Transforms the most recent camera frame upright off the render thread,
// dropping any that arrived while it was busy, then stages it and pokes the
// render thread to present. The heavy tracker + GPU compose runs in
// `process_frame` on the render thread (see `live_gpu`).
fn live_worker(state: Arc<LiveState>) {
    loop {
        let pending = {
            let mut guard = state.pending.lock().expect("live pending poisoned");
            while guard.is_none() {
                guard = state.cv.wait(guard).expect("live cv poisoned");
            }
            guard.take().expect("pending present")
        };

        let viewport = *state.viewport.lock().expect("viewport poisoned");
        let (rgba, width, height) = transform_frame(&pending, viewport);
        *state.ready.lock().expect("ready poisoned") = Some(ReadyFrame {
            rgba,
            width,
            height,
        });
        if let Some(tick) = LIVE_FRAME_TICK.get() {
            tick();
        }
    }
}

const LIVE_MAX_SIDE: usize = 1000;

// Single fused pass: rotate the (landscape) sensor frame 90° to upright,
// center-crop to the viewport aspect, and downscale the longest side to
// LIVE_MAX_SIDE — sampling the source directly into one output buffer
// (nearest-neighbour), so there are no intermediate rotate/crop/resize copies.
fn transform_frame(pending: &PendingFrame, viewport: (u32, u32)) -> (Vec<u8>, u32, u32) {
    let sw = pending.width as usize;
    let sh = pending.height as usize;
    let stride = pending.stride;

    // Upright (post-90° rotation) dimensions.
    let uw = sh;
    let uh = sw;

    let (vw, vh) = (viewport.0 as usize, viewport.1 as usize);
    let (cw, ch) = if vw == 0 || vh == 0 {
        (uw, uh)
    } else if vw * uh < uw * vh {
        // viewport narrower than the frame -> crop width
        ((uh * vw / vh).clamp(1, uw), uh)
    } else {
        (uw, (uw * vh / vw).clamp(1, uh))
    };
    let (cx, cy) = ((uw - cw) / 2, (uh - ch) / 2);

    let longest = cw.max(ch);
    let (fw, fh) = if longest > LIVE_MAX_SIDE {
        let scale = LIVE_MAX_SIDE as f64 / longest as f64;
        (
            ((cw as f64 * scale) as usize).max(1),
            ((ch as f64 * scale) as usize).max(1),
        )
    } else {
        (cw, ch)
    };

    let mut rgba = vec![0u8; fw * fh * 4];
    for oy in 0..fh {
        let uy = cy + oy * ch / fh;
        for ox in 0..fw {
            let ux = cx + ox * cw / fw;
            // Inverse of a 90° rotation: upright(ux,uy) <- source(uy, sh-1-ux).
            let sx = uy;
            let sy = sh - 1 - ux;
            let p = sy * stride + sx * 4;
            let o = (oy * fw + ox) * 4;
            rgba[o] = pending.rgbx[p];
            rgba[o + 1] = pending.rgbx[p + 1];
            rgba[o + 2] = pending.rgbx[p + 2];
            rgba[o + 3] = 255;
        }
    }
    (rgba, fw as u32, fh as u32)
}

/// Called from the C++ video filter on the QtQuick render thread. `in_ptr`
/// points to `stride * height` bytes of `Format_RGB32` (RGBX byte order on
/// this device). Copies the frame into the latest-frame slot and wakes the
/// worker, then returns immediately — no compute on the render thread.
///
/// # Safety
/// `in_ptr` must be valid for `stride * height` bytes for the call duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn live_ocr_process_frame(
    in_ptr: *const u8,
    stride: i32,
    width: i32,
    height: i32,
) {
    let Some(state) = LIVE_STATE.get() else {
        return;
    };
    let stride = stride as usize;
    let len = stride * height as usize;
    let rgbx = unsafe { std::slice::from_raw_parts(in_ptr, len) }.to_vec();
    let mut guard = state.pending.lock().expect("live pending poisoned");
    *guard = Some(PendingFrame {
        rgbx,
        stride,
        width: width as u32,
        height: height as u32,
    });
    state.cv.notify_one();
}
