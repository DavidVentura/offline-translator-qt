//! `LiveCameraItem`: the GPU live-camera viewfinder.
//!
//! A bare `QQuickItem` whose scene-graph node displays the present FBO color
//! texture produced by `live_gpu`. It hooks `QQuickWindow::afterRendering`
//! (DirectConnection → render thread, GL current) to drive the GPU composite
//! *after* qtvideo-node has latched the freshest camera frame; the composite
//! renders into an offscreen FBO rather than the screen. Because the node draws
//! at this item's z-order, the QML controls (language pills, buttons) compose on
//! top of the viewfinder — what a screen-wide blit in `afterRendering` clobbered.
//!
//! The node displays the *previous* pass's composite (the scene graph renders
//! before `afterRendering`); `frame_tick` schedules a pass per camera frame, so
//! the displayed frame trails the composite by one pass.
//!
//! QML drives repaints by binding `frame_tick` to a per-frame counter on the
//! app bridge; each change schedules an item update, which triggers a render
//! pass (and thus an `afterRendering` present).
//!
//! Distinct from `RenderedImageItem` (the CPU QImage display used by the
//! static translated-image view), which is intentionally left untouched.

use cpp::cpp;
use qmetaobject::scenegraph::{ContainerNode, SGNode};
use qmetaobject::*;

cpp! {{
    #include <QtGui/QOpenGLContext>
    #include <QtGui/QOpenGLFunctions>
    #include <QtQuick/QQuickWindow>
    #include <QtQuick/QQuickItem>
    #include <QtQuick/QSGSimpleTextureNode>
    #include <QtQuick/QSGTexture>
    #include <QtCore/QByteArray>
    #include <QtCore/QThread>
    #include <QtCore/QDebug>

    extern "C" void live_gpu_present_external(int vw, int vh);

    // Resolve GL procs for the Rust-side glow loaders. Render thread only.
    extern "C" const void *live_gl_get_proc(const char *name) {
        QOpenGLContext *c = QOpenGLContext::currentContext();
        if (!c) {
            return nullptr;
        }
        return reinterpret_cast<const void *>(c->getProcAddress(QByteArray(name)));
    }

    extern "C" unsigned live_gpu_present_texture(int *w, int *h);

    // QSGTexture over the live_gpu present FBO's color attachment (a plain
    // GL_TEXTURE_2D owned by the render-thread RenderState, not by this node).
    // The composite already ran in afterRendering, so bind() just binds it; the
    // wrap/filter params were set when the FBO texture was allocated. The id is
    // re-fetched each bind() so an FBO resize (new texture) is picked up. The
    // size is reported eagerly via setSize() in update_paint_node because the
    // renderer culls a node whose texture is 0x0 and would never call bind().
    class LivePresentTexture : public QSGTexture, protected QOpenGLFunctions {
    public:
        void setSize(QSize s) { m_size = s; }
        int textureId() const override { return static_cast<int>(m_id); }
        QSize textureSize() const override { return m_size; }
        bool hasAlphaChannel() const override { return false; }
        bool hasMipmaps() const override { return false; }
        void bind() override {
            initializeOpenGLFunctions();
            int w = 0, h = 0;
            m_id = live_gpu_present_texture(&w, &h);
            if (w > 0 && h > 0) {
                m_size = QSize(w, h);
            }
            glBindTexture(GL_TEXTURE_2D, m_id);
        }
    private:
        unsigned m_id = 0;
        QSize m_size;
    };
}}

#[derive(QObject, Default)]
pub struct LiveCameraItem {
    base: qt_base_class!(trait QQuickItem),
    /// Bound in QML to a per-frame counter; each write schedules a repaint.
    frame_tick: qt_property!(i32; WRITE set_frame_tick),
    /// Bound in QML to `LiveCameraScreen.screenActive`. Gates the GPU
    /// composite in `live_gpu::live_gpu_present_external` so the afterRendering
    /// hook doesn't burn cycles when the screen is hidden.
    screen_active: qt_property!(bool; WRITE set_screen_active),
    connected: bool,
}

impl LiveCameraItem {
    fn set_frame_tick(&mut self, tick: i32) {
        self.frame_tick = tick;
        (self as &dyn QQuickItem).update();
    }

    fn set_screen_active(&mut self, active: bool) {
        self.screen_active = active;
        crate::live_gpu::set_present_enabled(active);
    }

    fn connect_present(&mut self) {
        if self.connected {
            return;
        }
        let obj = (self as &dyn QQuickItem).get_cpp_object();
        let did = cpp!(unsafe [obj as "QQuickItem*"] -> bool as "bool" {
            if (!obj) {
                return false;
            }
            QQuickWindow *win = obj->window();
            if (!win) {
                return false;
            }
            // Composite the camera + overlays into the present FBO in
            // afterRendering — after qtvideo-node has latched the freshest frame
            // into the external texture (beforeRendering would sample the
            // previous pass's frame). The scene-graph node displays the FBO.
            QObject::connect(win, &QQuickWindow::afterRendering, win, [win]() {
                static bool logged = false;
                if (!logged) {
                    logged = true;
                    qInfo("[live_gpu] present thread=%p", static_cast<void *>(QThread::currentThread()));
                }
                const qreal dpr = win->effectiveDevicePixelRatio();
                live_gpu_present_external(static_cast<int>(win->width() * dpr),
                                          static_cast<int>(win->height() * dpr));
            }, Qt::DirectConnection);
            return true;
        });
        self.connected = did;
    }
}

impl QQuickItem for LiveCameraItem {
    fn component_complete(&mut self) {
        let obj = (self as &dyn QQuickItem).get_cpp_object();
        cpp!(unsafe [obj as "QQuickItem*"] {
            if (obj) {
                obj->setFlag(QQuickItem::ItemHasContents, true);
                obj->update();
            }
        });
    }

    fn update_paint_node(&mut self, node: SGNode<ContainerNode>) -> SGNode<ContainerNode> {
        // Ensure the afterRendering composite is connected; frame_tick →
        // update() schedules the passes that re-run the composite and re-sample
        // the present FBO.
        self.connect_present();
        let raw = node.into_raw();

        // No composite has allocated the FBO yet — nothing to show. Drop any node
        // (the renderer would cull a 0x0-texture node anyway).
        let (pw, ph) = crate::live_gpu::present_size();
        if pw == 0 || ph == 0 {
            let cleared = cpp!(unsafe [raw as "QSGNode*"] -> *mut std::os::raw::c_void as "QSGNode*" {
                delete raw;
                return nullptr;
            });
            return unsafe { SGNode::<ContainerNode>::from_raw(cleared) };
        }

        let rect = (self as &dyn QQuickItem).bounding_rect();
        let pw = pw as i32;
        let ph = ph as i32;
        let new_raw = cpp!(unsafe [
            raw as "QSGNode*",
            rect as "QRectF",
            pw as "int",
            ph as "int"
        ] -> *mut std::os::raw::c_void as "QSGNode*" {
            QSGSimpleTextureNode* n = static_cast<QSGSimpleTextureNode*>(raw);
            if (!n) {
                n = new QSGSimpleTextureNode();
                n->setOwnsTexture(true);
                n->setFiltering(QSGTexture::Linear);
                // The composite renders into the FBO bottom-up (GL clip space),
                // but the scene graph samples textures top-down; mirror so the
                // viewfinder isn't shown upside down.
                n->setTextureCoordinatesTransform(QSGSimpleTextureNode::MirrorVertically);
                n->setTexture(new LivePresentTexture());
            }
            LivePresentTexture* tex = static_cast<LivePresentTexture*>(n->texture());
            tex->setSize(QSize(pw, ph));
            n->setRect(rect);
            n->markDirty(QSGNode::DirtyMaterial);
            return n;
        });
        unsafe { SGNode::<ContainerNode>::from_raw(new_raw) }
    }
}
