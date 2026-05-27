//! Render-thread GPU compositing for the live camera view.
//!
//! Runs on the QtQuick scene-graph render thread, where the GL context is
//! current. Per new camera frame it drives `LiveTrackerPipeline::process_frame`
//! with translator's `PresentTarget`, rendering camera + translation overlays
//! into an app-owned FBO (no CPU readback). The composited FBO color texture is
//! handed to a `QSGSimpleTextureNode` for display (see `live_camera_item`).
//!
//! All GL here happens through two `glow` handles onto the *same* current
//! context: `GlesRenderer`'s own (inside translator) and ours (`gl`, for the
//! FBO + state save/restore). The whole thing is `!Send` and lives in a
//! `thread_local` so it never leaves the render thread.

use std::cell::RefCell;
use std::ffi::{CString, c_char, c_void};
use std::num::NonZeroU32;
use std::time::Instant;

use glow::HasContext;
use translator::Rect;
use translator::gl_renderer::{GlesRenderer, PresentTarget};

use crate::live_ocr::{ReadyFrame, live_pipeline, take_ready_frame};

unsafe extern "C" {
    /// Resolves a GL proc address via the current `QOpenGLContext`. Defined in
    /// the `cpp!` block in `live_camera_item.rs`; valid only on the render
    /// thread with a context current.
    fn live_gl_get_proc(name: *const c_char) -> *const c_void;
}

fn load_proc(name: &str) -> *const c_void {
    let c = CString::new(name).expect("gl proc name has interior nul");
    unsafe { live_gl_get_proc(c.as_ptr()) }
}

/// Maps composite-output pixels (top-left origin) to clip space, flipped so the
/// image's top row lands in the FBO texture's *first* row. translator's own
/// `ndc_from_viewport` flips the other way because it targets the window
/// framebuffer (scanned out directly); here Qt re-samples the FBO texture as a
/// quad, and `QSGSimpleTextureNode` treats texture row 0 as the top — so we
/// render "upside down" into the texture for it to come out upright.
fn ndc_for_fbo(w: f32, h: f32) -> [f32; 9] {
    [2.0 / w, 0.0, -1.0, 0.0, 2.0 / h, -1.0, 0.0, 0.0, 1.0]
}

struct RenderState {
    gl: glow::Context,
    gles: GlesRenderer,
    fbo: glow::Framebuffer,
    color: glow::Texture,
    size: (u32, u32),
    /// Dims of the most recently presented frame; `None` until the first
    /// present, which gates the display node.
    ready_dims: Option<(u32, u32)>,
    start: Instant,
}

impl RenderState {
    /// Build on the render thread with the GL context current. Returns `None`
    /// if the GPU compositor can't be constructed (shader/program failure),
    /// leaving the live view blank rather than crashing the UI.
    fn new() -> Option<Self> {
        let gles = match GlesRenderer::new(load_proc) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("live GPU: GlesRenderer init failed: {e:?}");
                return None;
            }
        };
        let gl = unsafe { glow::Context::from_loader_function(load_proc) };
        let (fbo, color) = unsafe {
            let fbo = gl.create_framebuffer().expect("create live FBO");
            let color = gl.create_texture().expect("create live FBO texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(color));
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
            (fbo, color)
        };
        Some(Self {
            gl,
            gles,
            fbo,
            color,
            size: (0, 0),
            ready_dims: None,
            start: Instant::now(),
        })
    }

    /// (Re)allocate the FBO color attachment when the frame size changes.
    unsafe fn ensure_size(&mut self, w: u32, h: u32) {
        if self.size == (w, h) {
            return;
        }
        let gl = &self.gl;
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.color));
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
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(self.color),
                0,
            );
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            assert_eq!(
                status,
                glow::FRAMEBUFFER_COMPLETE,
                "live FBO incomplete: status=0x{status:x}"
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        self.size = (w, h);
    }

    fn present(
        &mut self,
        pipeline: &translator::live_tracker_pipeline::LiveTrackerPipeline,
        rf: ReadyFrame,
    ) {
        let (w, h) = (rf.width, rf.height);
        if w == 0 || h == 0 {
            return;
        }
        unsafe { self.ensure_size(w, h) };

        // A fresh frame per present, never reused across presents. An in-flight
        // acquire holds its frame's state lock for the whole det+rec; reusing one
        // frame would make this present's reset_owned block on that lock, stalling
        // the render thread (and the whole UI) until recognition finishes.
        let frame = std::sync::Arc::new(translator::live_frame::LiveFrame::new(0));
        frame.reset_owned(rf.rgba, w, h, 0);

        let (prev_fbo, prev_vp) = unsafe {
            let prev_fbo = self.gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
            let mut vp = [0i32; 4];
            self.gl.get_parameter_i32_slice(glow::VIEWPORT, &mut vp);
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            self.gl.viewport(0, 0, w as i32, h as i32);
            (prev_fbo, vp)
        };

        let crop = Rect {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        let ts = self.start.elapsed().as_nanos() as u64;
        {
            let mut target = PresentTarget {
                renderer: &mut self.gles,
                display_xform: ndc_for_fbo(w as f32, h as f32),
            };
            if let Err(e) = pipeline.process_frame(&frame, crop, &mut target, w, h, w, h, w, h, ts)
            {
                eprintln!("live GPU process_frame failed: {e:?}");
            }
        }

        unsafe {
            match NonZeroU32::new(prev_fbo as u32) {
                Some(id) => self
                    .gl
                    .bind_framebuffer(glow::FRAMEBUFFER, Some(glow::NativeFramebuffer(id))),
                None => self.gl.bind_framebuffer(glow::FRAMEBUFFER, None),
            }
            self.gl
                .viewport(prev_vp[0], prev_vp[1], prev_vp[2], prev_vp[3]);
        }
        self.ready_dims = Some((w, h));
    }
}

thread_local! {
    static RT: RefCell<Option<RenderState>> = const { RefCell::new(None) };
}

/// Present the latest camera frame into the FBO. Called from the C++
/// `beforeRendering` handler on the render thread (GL context current). A no-op
/// when no new frame has arrived since the last present.
#[unsafe(no_mangle)]
pub extern "C" fn live_gpu_before_rendering() {
    let Some(rf) = take_ready_frame() else {
        return;
    };
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
            .present(pipeline, rf);
    });
}

/// Report the composited FBO color texture (GL name + size) for the display
/// node. Returns false until the first frame has been presented.
#[unsafe(no_mangle)]
pub extern "C" fn live_gpu_color_tex(out_id: *mut u32, out_w: *mut u32, out_h: *mut u32) -> bool {
    RT.with(|cell| {
        let slot = cell.borrow();
        let Some(state) = slot.as_ref() else {
            return false;
        };
        let Some((w, h)) = state.ready_dims else {
            return false;
        };
        unsafe {
            *out_id = state.color.0.get();
            *out_w = w;
            *out_h = h;
        }
        true
    })
}
