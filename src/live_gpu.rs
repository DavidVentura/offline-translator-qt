//! Render-thread GPU compositing for the live camera view (zero-copy).
//!
//! Runs on the QtQuick scene-graph render thread in `afterRendering`, where the
//! GL context is current and qtubuntu's qtvideo-node has just latched the
//! freshest camera frame into the external-OES texture. Per frame:
//!   1. `GlesRenderer::set_camera_external` — borrow the camera texture (no map,
//!      no upload), with the canonical (upright/crop/flip) uv transform.
//!   2. `read_camera_rgba` — GPU-render the canonical frame and read it back as
//!      the `LiveFrame` the tracker + det/rec work on (replaces the CPU `map` +
//!      `transform_frame`).
//!   3. `process_frame` (bundled) — tracker, then `ExternalPresentTarget`
//!      composites the full-res external camera + overlays into the owned
//!      present FBO, which the `LiveCameraItem` scene-graph node then displays
//!      (so the QML controls compose on top of it).
//!
//! `GlesRenderer` (`!Send`) lives in a render-thread `thread_local`.

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::Instant;

use glow::HasContext;
use translator::gl_renderer::GlesRenderer;
use translator::live_gpu_tick::{frame_from_camera_gray, run_tracker_with_acquire};
use translator::live_tracker_pipeline::LiveTrackerPipeline;

use crate::live_ocr::{fire_frame_tick, live_pipeline};

unsafe extern "C" {
    /// Resolves a GL proc address via the current `QOpenGLContext`. Defined in
    /// the `cpp!` block in `live_camera_item.rs`; render thread only.
    fn live_gl_get_proc(name: *const c_char) -> *const c_void;
}

fn load_proc(name: &str) -> *const c_void {
    let c = CString::new(name).expect("gl proc name has interior nul");
    unsafe { live_gl_get_proc(c.as_ptr()) }
}

// Canonical-frame orientation. Stored as a CW quadrant (0/1/2/3 = 0/90/180/270)
// so the render thread reads it as a single relaxed atomic. Default 1 = 90°,
// matching the previous hardcoded "landscape sensor on portrait screen" value;
// QML calls `set_camera_orientation_degrees(cam.orientation)` once the HAL
// reports the sensor mount angle so devices with a non-90° mount work too.
static ROT_QUADRANT: AtomicI32 = AtomicI32::new(1);

pub fn set_camera_orientation_degrees(degrees: i32) {
    let q = (((degrees % 360) + 360) % 360) / 90;
    ROT_QUADRANT.store(q, Ordering::Relaxed);
}
// f,t top-to-bottom bad
// f,f top-to-bottom bad
const FLIP_U: bool = true;
const FLIP_V: bool = false;
// Y direction when mapping the canonical frame to the screen. Tune if the
// composite is vertically mirrored vs the camera.
const DISPLAY_FLIP_Y: bool = false;
// Longest-side cap for the canonical (tracker/det/rec/overlay) frame.
const CANONICAL_MAX_SIDE: u32 = 1000;

static CAMERA_TEX: AtomicU32 = AtomicU32::new(0);
static CAMERA_W: AtomicU32 = AtomicU32::new(0);
static CAMERA_H: AtomicU32 = AtomicU32::new(0);

// The owned FBO color texture the composite renders into, published for the
// `LiveCameraItem` scene-graph node to sample. Both the composite and the node
// run on the render thread, so plain atomics suffice.
static PRESENT_TEX: AtomicU32 = AtomicU32::new(0);
static PRESENT_W: AtomicU32 = AtomicU32::new(0);
static PRESENT_H: AtomicU32 = AtomicU32::new(0);

/// Called from the video filter each frame: the camera frame's external-OES
/// texture id (0 until qtvideo-node mints it) + sensor dims.
#[unsafe(no_mangle)]
pub extern "C" fn live_gpu_set_camera_texture(id: u32, w: u32, h: u32) {
    CAMERA_TEX.store(id, Ordering::Relaxed);
    CAMERA_W.store(w, Ordering::Relaxed);
    CAMERA_H.store(h, Ordering::Relaxed);
    // Drive a repaint per camera frame → afterRendering → present. (The
    // VideoOutput also re-renders per frame, but this guarantees the cadence.)
    fire_frame_tick();
}

/// Canonical frame dims: the screen aspect, capped to `CANONICAL_MAX_SIDE` on
/// the long side (matches what the old CPU `transform_frame` produced).
fn canonical_dims(screen_w: u32, screen_h: u32) -> (u32, u32) {
    if screen_w == 0 || screen_h == 0 {
        return (0, 0);
    }
    let long = screen_w.max(screen_h) as f32;
    let s = (CANONICAL_MAX_SIDE as f32 / long).min(1.0);
    (
        ((screen_w as f32 * s) as u32).max(1),
        ((screen_h as f32 * s) as u32).max(1),
    )
}

/// Row-major canonical uv transform: output (unit quad) → external-texture uv,
/// rotating the sensor upright (`ROT_QUADRANT`) and aspect-fill covering the
/// output. `GlesRenderer` transposes it for GL.
fn compute_uv_mat(cam_w: f32, cam_h: f32, fw: f32, fh: f32) -> [f32; 9] {
    let q = ROT_QUADRANT.load(Ordering::Relaxed).rem_euclid(4);
    let odd = q == 1 || q == 3;
    let displayed_aspect = if odd { cam_h / cam_w } else { cam_w / cam_h };
    let out_aspect = fw / fh;
    let (vis_w, vis_h) = if out_aspect <= displayed_aspect {
        (out_aspect / displayed_aspect, 1.0)
    } else {
        (1.0, displayed_aspect / out_aspect)
    };
    let (frac_u, frac_v) = if odd { (vis_h, vis_w) } else { (vis_w, vis_h) };
    let (r00, r01, r10, r11) = match q {
        0 => (1.0, 0.0, 0.0, 1.0),
        1 => (0.0, 1.0, -1.0, 0.0),
        2 => (-1.0, 0.0, 0.0, -1.0),
        _ => (0.0, -1.0, 1.0, 0.0),
    };
    let (mut l00, mut l01) = (frac_u * r00, frac_u * r01);
    let (mut l10, mut l11) = (frac_v * r10, frac_v * r11);
    let mut t0 = 0.5 - 0.5 * (l00 + l01);
    let mut t1 = 0.5 - 0.5 * (l10 + l11);
    if FLIP_U {
        l00 = -l00;
        l01 = -l01;
        t0 = 1.0 - t0;
    }
    if FLIP_V {
        l10 = -l10;
        l11 = -l11;
        t1 = 1.0 - t1;
    }
    [l00, l01, t0, l10, l11, t1, 0.0, 0.0, 1.0]
}

/// Row-major dst→clip transform filling the screen with the canonical frame.
fn display_xform(fw: f32, fh: f32) -> [f32; 9] {
    let (e, f) = if DISPLAY_FLIP_Y {
        (-2.0 / fh, 1.0)
    } else {
        (2.0 / fh, -1.0)
    };
    [2.0 / fw, 0.0, -1.0, 0.0, e, f, 0.0, 0.0, 1.0]
}

struct RenderState {
    gl: glow::Context,
    gles: GlesRenderer,
    /// Screen-sized RGBA FBO the composite renders into (instead of the screen),
    /// so the scene graph can show it under the QML controls via a texture node.
    present_fbo: Option<glow::Framebuffer>,
    present_tex: Option<glow::Texture>,
    present_size: Option<(u32, u32)>,
    start: Instant,
    frames: u32,
    gap_ms_sum: f32,
    readback_ms_sum: f32,
    present_ms_sum: f32,
    last_present: Option<Instant>,
}

impl RenderState {
    fn new() -> Option<Self> {
        let gles = match GlesRenderer::new(load_proc) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("live GPU: GlesRenderer init failed: {e:?}");
                return None;
            }
        };
        Some(Self {
            gl: unsafe { glow::Context::from_loader_function(load_proc) },
            gles,
            present_fbo: None,
            present_tex: None,
            present_size: None,
            start: Instant::now(),
            frames: 0,
            gap_ms_sum: 0.0,
            readback_ms_sum: 0.0,
            present_ms_sum: 0.0,
            last_present: None,
        })
    }

    /// (Re)create the screen-sized RGBA FBO + color texture the composite draws
    /// into, and publish the texture id for the scene-graph node. The texture is
    /// a plain `GL_TEXTURE_2D` (unlike the external-OES camera source), so a
    /// stock `QSGSimpleTextureNode` can display it.
    fn ensure_present_fbo(&mut self, w: u32, h: u32) {
        if self.present_size == Some((w, h)) {
            return;
        }
        let gl = &self.gl;
        unsafe {
            if let Some(t) = self.present_tex.take() {
                gl.delete_texture(t);
            }
            let tex = gl.create_texture().expect("create present fbo texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                w as i32,
                h as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            let fbo = match self.present_fbo {
                Some(f) => f,
                None => {
                    let f = gl.create_framebuffer().expect("create present framebuffer");
                    self.present_fbo = Some(f);
                    f
                }
            };
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(tex),
                0,
            );
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            assert_eq!(
                status,
                glow::FRAMEBUFFER_COMPLETE,
                "present FBO incomplete: status=0x{status:x}"
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.present_tex = Some(tex);
            self.present_size = Some((w, h));
            PRESENT_TEX.store(tex.0.get(), Ordering::Relaxed);
            PRESENT_W.store(w, Ordering::Relaxed);
            PRESENT_H.store(h, Ordering::Relaxed);
        }
    }

    fn present(&mut self, pipeline: &LiveTrackerPipeline, screen_w: i32, screen_h: i32) {
        let t_enter = Instant::now();
        let id = CAMERA_TEX.load(Ordering::Relaxed);
        let cam_w = CAMERA_W.load(Ordering::Relaxed);
        let cam_h = CAMERA_H.load(Ordering::Relaxed);
        if id == 0 || cam_w == 0 || cam_h == 0 || screen_w <= 0 || screen_h <= 0 {
            return;
        }
        let (fw, fh) = canonical_dims(screen_w as u32, screen_h as u32);
        if fw == 0 || fh == 0 {
            return;
        }

        // The framebuffer afterRendering has bound is the scene graph's target —
        // NOT necessarily 0 on Qt's render thread. Save it so we can hand it back
        // for the buffer swap (read_camera_gray and the composite bind our own
        // FBOs in between).
        let sg_fbo = unsafe { self.gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING) };

        let uv = compute_uv_mat(cam_w as f32, cam_h as f32, fw as f32, fh as f32);
        // One transform for both the OCR readback and the present, so the frame
        // the OCR sees is exactly what's displayed (no divergent orientation).
        let dx = display_xform(fw as f32, fh as f32);

        // Per-frame: a one-channel luma readback feeds the tracker (no RGBA
        // transfer, no CPU RGBA→luma). The expensive full-res RGBA readback only
        // happens on acquire/refresh, inside run_tracker_with_acquire. The
        // glReadPixels stall is billed outside the pipeline's `[lt]` total, so
        // it's timed separately.
        let t_read = Instant::now();
        let Some(frame) = frame_from_camera_gray(&mut self.gles, id, fw, fh, uv, dx) else {
            return;
        };
        let readback_ms = t_read.elapsed().as_secs_f32() * 1000.0;

        // Composite into the owned present FBO, not the screen: the scene graph
        // already rendered this pass (the LiveCameraItem node shows the previous
        // present, the QML controls compose on top of it). read_camera_gray left
        // its gray FBO bound; point at the present FBO and size the viewport to
        // the screen so the composite fills it.
        self.ensure_present_fbo(screen_w as u32, screen_h as u32);
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, self.present_fbo);
            self.gl.viewport(0, 0, screen_w, screen_h);
        }

        let ts = self.start.elapsed().as_nanos() as u64;
        if let Err(e) = run_tracker_with_acquire(pipeline, &mut self.gles, &frame, fw, fh, dx, ts) {
            eprintln!("live GPU process_frame failed: {e:?}");
        }

        // Hand the scene-graph target back to Qt for the buffer swap.
        unsafe {
            match NonZeroU32::new(sg_fbo as u32) {
                Some(fbo) => self
                    .gl
                    .bind_framebuffer(glow::FRAMEBUFFER, Some(glow::NativeFramebuffer(fbo))),
                None => self.gl.bind_framebuffer(glow::FRAMEBUFFER, None),
            }
        }

        // Render-thread budget the pipeline's `[lt]` line can't see: `gap` is the
        // wall period between presents (render/swap/vsync/camera cadence on top
        // of our work), `present` is our on-thread time (readback + pipeline).
        if let Some(prev) = self.last_present {
            self.gap_ms_sum += (t_enter - prev).as_secs_f32() * 1000.0;
        }
        self.last_present = Some(t_enter);
        self.readback_ms_sum += readback_ms;
        self.present_ms_sum += t_enter.elapsed().as_secs_f32() * 1000.0;
        self.frames += 1;
        if self.frames >= 30 {
            let n = self.frames as f32;
            eprintln!(
                "[lt-gpu] {} frames gap={:.1}ms present={:.1}ms readback={:.1}ms",
                self.frames,
                self.gap_ms_sum / n,
                self.present_ms_sum / n,
                self.readback_ms_sum / n,
            );
            self.frames = 0;
            self.gap_ms_sum = 0.0;
            self.readback_ms_sum = 0.0;
            self.present_ms_sum = 0.0;
        }
    }
}

thread_local! {
    static RT: RefCell<Option<RenderState>> = const { RefCell::new(None) };
}

/// Composite the live camera + overlays into the owned present FBO. Called from
/// the C++ `afterRendering` handler (render thread, GL current) with the
/// framebuffer size in device pixels. The `LiveCameraItem` scene-graph node
/// samples the FBO texture so the QML controls compose on top of it.
#[unsafe(no_mangle)]
pub extern "C" fn live_gpu_present_external(screen_w: i32, screen_h: i32) {
    let Some(pipeline) = live_pipeline() else {
        return;
    };
    RT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            match RenderState::new() {
                Some(s) => *slot = Some(s),
                None => return,
            }
        }
        slot.as_mut()
            .expect("render state present")
            .present(pipeline, screen_w, screen_h);
    });
}

/// The present FBO color texture id, writing its size into `out_w`/`out_h`.
/// Called from the `LiveCameraItem` scene-graph texture node (render thread).
/// Returns 0 before the first composite has allocated the FBO.
#[unsafe(no_mangle)]
pub extern "C" fn live_gpu_present_texture(out_w: *mut i32, out_h: *mut i32) -> u32 {
    let w = PRESENT_W.load(Ordering::Relaxed);
    let h = PRESENT_H.load(Ordering::Relaxed);
    unsafe {
        if !out_w.is_null() {
            *out_w = w as i32;
        }
        if !out_h.is_null() {
            *out_h = h as i32;
        }
    }
    PRESENT_TEX.load(Ordering::Relaxed)
}

/// The present FBO size, or `(0, 0)` before the first composite. Lets the node
/// skip building geometry (and report a non-zero texture size) before there is
/// anything to show.
pub fn present_size() -> (u32, u32) {
    (
        PRESENT_W.load(Ordering::Relaxed),
        PRESENT_H.load(Ordering::Relaxed),
    )
}
