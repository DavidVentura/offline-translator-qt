#include "live_filter.h"

#include <QtMultimedia/QVideoFrame>
#include <QtMultimedia/QVideoSurfaceFormat>
#include <QtQml/qqml.h>
#include <QtCore/QByteArray>
#include <QtCore/QDebug>
#include <QtCore/QThread>

// Defined in Rust (src/live_gpu.rs). Hands the camera frame's external-OES
// texture id (+ sensor dims) to the afterRendering composite for zero-copy
// display, and drives a repaint. This is now the filter's *only* job — no
// map/copy; the whole frame stays on the GPU.
extern "C" void live_gpu_set_camera_texture(unsigned int id, unsigned int w, unsigned int h);

class LiveOcrRunnable : public QVideoFilterRunnable {
public:
    QVideoFrame run(QVideoFrame *input,
                    const QVideoSurfaceFormat & /*surfaceFormat*/,
                    RunFlags /*flags*/) override {
        if (!input || !input->isValid()) {
            return input ? *input : QVideoFrame();
        }

        // One-time probe: does the camera hand us a GL texture (handleType==1,
        // GLTextureHandle) or a CPU buffer that must be mapped (handleType==0,
        // NoHandle)? This decides whether zero-readback GPU compositing is on
        // the table. pixelFormat: see QVideoFrame::PixelFormat enum.
        if (!m_logged) {
            m_logged = true;
            qInfo("[livefilter] PROBE handleType=%d pixelFormat=%d size=%dx%d planes=%d thread=%p",
                  static_cast<int>(input->handleType()),
                  static_cast<int>(input->pixelFormat()),
                  input->width(), input->height(), input->planeCount(),
                  static_cast<void *>(QThread::currentThread()));
        }
        // The camera frame's handle() is the GL_TEXTURE_EXTERNAL_OES id, but it
        // starts at 0 until qtvideo-node mints the texture on the first rendered
        // frame (qtubuntu-camera onTextureCreated). Log on every *change* so we
        // see the 0 -> real-id transition; nonzero means we can sample it
        // directly (zero-copy).
        {
            unsigned int hdl = input->handle().toUInt();
            if (hdl != m_lastHandle) {
                m_lastHandle = hdl;
                qInfo("[livefilter] PROBE handle id now=%u", hdl);
            }
            live_gpu_set_camera_texture(hdl, static_cast<unsigned int>(input->width()),
                                        static_cast<unsigned int>(input->height()));
        }

        return *input;
    }

private:
    bool m_logged = false;
    unsigned int m_lastHandle = 0xFFFFFFFFu;
};

QVideoFilterRunnable *LiveOcrFilter::createFilterRunnable() {
    return new LiveOcrRunnable();
}

extern "C" void register_live_ocr_filter() {
    qmlRegisterType<LiveOcrFilter>("TranslatorUi", 1, 0, "LiveOcrFilter");
}
