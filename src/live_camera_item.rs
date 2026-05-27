//! `LiveCameraItem`: the GPU live-camera viewfinder.
//!
//! A bare `QQuickItem` whose scene-graph node displays the FBO color texture
//! produced by `live_gpu`. It hooks `QQuickWindow::beforeRendering`
//! (DirectConnection → render thread, GL current) to drive the GPU composite
//! *before* the scene graph renders its nodes, so the node's `bind()` is a
//! trivial texture bind with no mid-pass GL-state pollution.
//!
//! QML drives repaints by binding `frame_tick` to a per-frame counter on the
//! app bridge; each change schedules an item update, which triggers a render
//! pass (and thus a `beforeRendering` present).
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

    extern "C" void live_gpu_before_rendering();
    extern "C" bool live_gpu_color_tex(unsigned *id, unsigned *w, unsigned *h);

    // Resolve GL procs for the Rust-side glow loaders. Render thread only.
    extern "C" const void *live_gl_get_proc(const char *name) {
        QOpenGLContext *c = QOpenGLContext::currentContext();
        if (!c) {
            return nullptr;
        }
        return reinterpret_cast<const void *>(c->getProcAddress(QByteArray(name)));
    }

    // QSGTexture over a GL texture owned elsewhere (the live_gpu FBO color
    // attachment). bind() just binds it — the composite already ran in
    // beforeRendering, so there is no rendering here.
    class LiveExternalTexture : public QSGTexture, protected QOpenGLFunctions {
    public:
        void set(unsigned id, QSize size) { m_id = id; m_size = size; }
        int textureId() const override { return static_cast<int>(m_id); }
        QSize textureSize() const override { return m_size; }
        bool hasAlphaChannel() const override { return false; }
        bool hasMipmaps() const override { return false; }
        void bind() override {
            initializeOpenGLFunctions();
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
    connected: bool,
}

impl LiveCameraItem {
    fn set_frame_tick(&mut self, tick: i32) {
        self.frame_tick = tick;
        (self as &dyn QQuickItem).update();
    }

    fn connect_before_rendering(&mut self) {
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
            QObject::connect(win, &QQuickWindow::beforeRendering, win, []() {
                live_gpu_before_rendering();
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
        self.connect_before_rendering();

        let raw = node.into_raw();
        let mut id: u32 = 0;
        let mut w: u32 = 0;
        let mut h: u32 = 0;
        let ready = crate::live_gpu::live_gpu_color_tex(&mut id, &mut w, &mut h);
        if !ready || w == 0 || h == 0 {
            let cleared = cpp!(unsafe [raw as "QSGNode*"] -> *mut std::os::raw::c_void as "QSGNode*" {
                delete raw;
                return nullptr;
            });
            return unsafe { SGNode::<ContainerNode>::from_raw(cleared) };
        }

        // The worker already crops to the viewport aspect, so fill bounds.
        let target = (self as &dyn QQuickItem).bounding_rect();
        let new_raw = cpp!(unsafe [
            raw as "QSGNode*",
            id as "unsigned",
            w as "unsigned",
            h as "unsigned",
            target as "QRectF"
        ] -> *mut std::os::raw::c_void as "QSGNode*" {
            QSGSimpleTextureNode *n = static_cast<QSGSimpleTextureNode *>(raw);
            if (!n) {
                n = new QSGSimpleTextureNode();
                n->setOwnsTexture(true);
                n->setFiltering(QSGTexture::Linear);
                n->setTexture(new LiveExternalTexture());
            }
            LiveExternalTexture *tex = static_cast<LiveExternalTexture *>(n->texture());
            tex->set(id, QSize(static_cast<int>(w), static_cast<int>(h)));
            n->setRect(target);
            n->markDirty(QSGNode::DirtyMaterial);
            return n;
        });
        unsafe { SGNode::<ContainerNode>::from_raw(new_raw) }
    }
}
