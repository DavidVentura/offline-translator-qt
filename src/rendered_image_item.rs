use cpp::cpp;
use qmetaobject::scenegraph::{ContainerNode, SGNode};
use qmetaobject::*;

cpp! {{
    #include <QtGui/QImage>
    #include <QtGui/QGuiApplication>
    #include <QtGui/QOpenGLFunctions>
    #include <QtQuick/QQuickWindow>
    #include <QtQuick/QQuickItem>
    #include <QtQuick/QSGSimpleTextureNode>
    #include <QtQuick/QSGTexture>
    #include <QtGui/QScreen>
    #include <QtGui/QPixmap>
    #include <QtCore/QElapsedTimer>
    #include <QtCore/QThread>
    #include <QtCore/QDebug>
    #include <cstring>

    // A QSGTexture backed by a single GL texture that we reuse across frames:
    // first frame does glTexImage2D, every later frame glTexSubImage2D into the
    // same texture object. This avoids allocating/freeing a GL texture every
    // frame (which stalls the render thread). The pixel upload happens in bind()
    // because that's guaranteed to run on the render thread with the GL context
    // current; update_paint_node only stashes the latest frame.
    class RustVideoTexture : public QSGTexture, protected QOpenGLFunctions {
    public:
        RustVideoTexture() {}
        ~RustVideoTexture() override {
            // Runs on the render thread (QSGSimpleTextureNode owns us) with the
            // context current, so deleting the GL texture here is safe.
            if (m_id) {
                glDeleteTextures(1, &m_id);
            }
        }

        // Called from update_paint_node (render thread, GUI blocked). Holds a
        // ref to the frame's buffer until bind() uploads it. Report the size
        // eagerly: the renderer culls a node whose texture is 0x0 and would
        // never call bind() (so the upload would never happen).
        void setImage(const QImage &img) {
            m_pending = img;
            m_dirty = true;
            m_size = img.size();
        }

        int textureId() const override { return static_cast<int>(m_id); }
        QSize textureSize() const override { return m_size; }
        bool hasAlphaChannel() const override { return false; }
        bool hasMipmaps() const override { return false; }

        void bind() override {
            initializeOpenGLFunctions();
            if (m_id == 0) {
                glGenTextures(1, &m_id);
                glBindTexture(GL_TEXTURE_2D, m_id);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            } else {
                glBindTexture(GL_TEXTURE_2D, m_id);
            }

            if (m_dirty && !m_pending.isNull()) {
                QElapsedTimer t;
                t.start();
                // Format_RGBX8888 is byte order R,G,B,X; GL_RGBA reads R,G,B,A.
                // The X byte lands in A but the node uses an opaque material
                // (hasAlphaChannel() == false), so it's ignored.
                if (m_uploadedSize != m_pending.size()) {
                    m_uploadedSize = m_pending.size();
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA,
                                 m_pending.width(), m_pending.height(), 0,
                                 GL_RGBA, GL_UNSIGNED_BYTE, m_pending.constBits());
                } else {
                    glTexSubImage2D(GL_TEXTURE_2D, 0, 0, 0,
                                    m_pending.width(), m_pending.height(),
                                    GL_RGBA, GL_UNSIGNED_BYTE, m_pending.constBits());
                }
                m_dirty = false;
                m_pending = QImage(); // release the frame buffer post-upload

                s_uploadSum += t.nsecsElapsed() / 1e6;
                if (++s_uploads >= 30) {
                    qInfo("[vidtex] render_thread=%p %d uploads avg=%.2fms (%dx%d)",
                          (void *)QThread::currentThread(), s_uploads,
                          s_uploadSum / s_uploads, m_size.width(), m_size.height());
                    s_uploads = 0;
                    s_uploadSum = 0.0;
                }
            }
        }

    private:
        GLuint m_id = 0;
        QSize m_size;         // reported via textureSize(), set in setImage()
        QSize m_uploadedSize; // size last glTexImage2D'd, drives alloc vs sub-image
        QImage m_pending;
        bool m_dirty = false;
        static int s_uploads;
        static double s_uploadSum;
    };
    int RustVideoTexture::s_uploads = 0;
    double RustVideoTexture::s_uploadSum = 0.0;
}}

#[derive(QObject, Default)]
pub struct RenderedImageItem {
    base: qt_base_class!(trait QQuickItem),
    image: qt_property!(QImage; WRITE set_image NOTIFY image_changed),
    image_changed: qt_signal!(),
    preserve_aspect: qt_property!(bool),
}

impl RenderedImageItem {
    fn set_image(&mut self, image: QImage) {
        self.image = image;
        self.image_changed();
        (self as &dyn QQuickItem).update();
    }

    fn target_rect(&self) -> QRectF {
        let bounds = (self as &dyn QQuickItem).bounding_rect();
        let size = self.image.size();
        if !self.preserve_aspect || size.width == 0 || size.height == 0 {
            return bounds;
        }
        let iw = size.width as f64;
        let ih = size.height as f64;
        let scale = (bounds.width / iw).min(bounds.height / ih);
        let tw = iw * scale;
        let th = ih * scale;
        QRectF {
            x: bounds.x + (bounds.width - tw) / 2.0,
            y: bounds.y + (bounds.height - th) / 2.0,
            width: tw,
            height: th,
        }
    }
}

impl QQuickItem for RenderedImageItem {
    // QQuickItem doesn't render anything unless ItemHasContents is set; without
    // it the scene graph never calls update_paint_node. QQuickPaintedItem sets
    // this for us, a bare QQuickItem does not.
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
        let raw = node.into_raw();
        let size = self.image.size();
        if size.width == 0 || size.height == 0 {
            let cleared = cpp!(unsafe [raw as "QSGNode*"] -> *mut std::os::raw::c_void as "QSGNode*" {
                delete raw;
                return nullptr;
            });
            return unsafe { SGNode::<ContainerNode>::from_raw(cleared) };
        }

        let target = self.target_rect();
        let image_ref = &self.image;
        // Stash the latest frame on a persistent texture; the actual GPU upload
        // happens in RustVideoTexture::bind() on the render thread. No per-frame
        // texture allocation.
        let new_raw = cpp!(unsafe [
            raw as "QSGNode*",
            image_ref as "QImage*",
            target as "QRectF"
        ] -> *mut std::os::raw::c_void as "QSGNode*" {
            QSGSimpleTextureNode* n = static_cast<QSGSimpleTextureNode*>(raw);
            if (!n) {
                n = new QSGSimpleTextureNode();
                n->setOwnsTexture(true);
                n->setFiltering(QSGTexture::Linear);
                n->setTexture(new RustVideoTexture());
            }
            RustVideoTexture* tex = static_cast<RustVideoTexture*>(n->texture());
            tex->setImage(*image_ref);
            n->setRect(target);
            n->markDirty(QSGNode::DirtyMaterial);
            return n;
        });
        unsafe { SGNode::<ContainerNode>::from_raw(new_raw) }
    }
}

/// Wrap an owned RGBA/RGBX buffer in a `QImage` without copying: the QImage
/// borrows the buffer and a cleanup callback frees it when the last copy of the
/// QImage is destroyed (after the scene graph has uploaded it to a texture).
/// Used on the live path so the compositor writes straight into the buffer the
/// QImage shows.
pub fn qimage_from_owned_rgba(width: u32, height: u32, bytes: Vec<u8>) -> QImage {
    let expected_len = width.saturating_mul(height).saturating_mul(4) as usize;
    if width == 0 || height == 0 || bytes.len() != expected_len {
        return QImage::default();
    }
    let mut boxed = Box::new(bytes);
    let data_ptr = boxed.as_mut_ptr();
    let info = Box::into_raw(boxed) as *mut std::os::raw::c_void;
    let w = width as i32;
    let h = height as i32;
    cpp!(unsafe [
        w as "int",
        h as "int",
        data_ptr as "unsigned char *",
        info as "void *"
    ] -> QImage as "QImage" {
        return QImage(data_ptr, w, h, w * 4, QImage::Format_RGBX8888,
            [](void *p) {
                rust!(RenderedImageItem_free_owned_buf [p: *mut std::os::raw::c_void as "void *"] {
                    drop(unsafe { Box::from_raw(p as *mut Vec<u8>) });
                });
            },
            info);
    })
}

pub fn qimage_from_rgba_bytes(width: u32, height: u32, rgba_bytes: &[u8]) -> QImage {
    let expected_len = width.saturating_mul(height).saturating_mul(4) as usize;
    if width == 0 || height == 0 || rgba_bytes.len() != expected_len {
        return QImage::default();
    }

    let bytes_ptr = rgba_bytes.as_ptr();
    let bytes_len = rgba_bytes.len();
    cpp!(unsafe [
        width as "int",
        height as "int",
        bytes_ptr as "const unsigned char *",
        bytes_len as "size_t"
    ] -> QImage as "QImage" {
        QImage image(QSize(width, height), QImage::Format_RGBA8888);
        if (!image.isNull()) {
            std::memcpy(image.bits(), bytes_ptr, bytes_len);
        }
        return image;
    })
}

pub fn save_window_screenshot(path: &str) -> bool {
    let path_ptr = path.as_ptr();
    let path_len = path.len();
    cpp!(unsafe [path_ptr as "const char *", path_len as "size_t"] -> bool as "bool" {
        const QString path = QString::fromUtf8(path_ptr, static_cast<int>(path_len));
        QPixmap shot;
        const auto windows = QGuiApplication::topLevelWindows();
        for (QWindow* window : windows) {
            auto quickWindow = qobject_cast<QQuickWindow*>(window);
            if (!quickWindow || !quickWindow->isVisible()) {
                continue;
            }
            QScreen* screen = quickWindow->screen();
            if (!screen) {
                screen = QGuiApplication::primaryScreen();
            }
            if (!screen) {
                continue;
            }
            shot = screen->grabWindow(quickWindow->winId());
            if (!shot.isNull()) {
                break;
            }
        }
        if (shot.isNull()) {
            return false;
        }
        return shot.save(path);
    })
}
