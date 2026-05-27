#include "live_filter.h"

#include <QtMultimedia/QVideoFrame>
#include <QtMultimedia/QVideoSurfaceFormat>
#include <QtQml/qqml.h>
#include <QtCore/QByteArray>
#include <QtCore/QDebug>
#include <QtCore/QElapsedTimer>
#include <QtCore/QThread>

#include <cstring>

// Defined in Rust (src/live_ocr.rs). Reads a Format_RGB32 (BGRX) frame, runs
// the pipeline, and pushes the composited result to the UI itself.
extern "C" void live_ocr_process_frame(const unsigned char *in_ptr,
                                       int stride,
                                       int width,
                                       int height);

class LiveOcrRunnable : public QVideoFilterRunnable {
public:
    QVideoFrame run(QVideoFrame *input,
                    const QVideoSurfaceFormat & /*surfaceFormat*/,
                    RunFlags /*flags*/) override {
        if (!input || !input->isValid()) {
            return input ? *input : QVideoFrame();
        }

        QElapsedTimer t;
        t.start();
        if (!input->map(QAbstractVideoBuffer::ReadOnly)) {
            warnOnce("LiveOcrFilter: map(ReadOnly) failed (GPU-only frame?)");
            return *input;
        }
        const double mapMs = t.nsecsElapsed() / 1e6;

        const int w = input->width();
        const int h = input->height();
        const int stride = input->bytesPerLine();
        const unsigned char *bits = input->bits();
        double copyMs = 0.0;
        if (bits && w > 0 && h > 0) {
            QElapsedTimer t2;
            t2.start();
            live_ocr_process_frame(bits, stride, w, h);
            copyMs = t2.nsecsElapsed() / 1e6;
        } else {
            warnOnce("LiveOcrFilter: mapped but bits() null");
        }
        input->unmap();

        // Attribution: is this on the SG render thread, and how much does the
        // camera-buffer map (potential GPU->CPU readback) + the frame copy cost?
        m_mapSum += mapMs;
        m_copySum += copyMs;
        if (++m_frames >= 30) {
            qInfo("[livefilter] thread=%p %d frames map=%.2fms copy=%.2fms",
                  (void *)QThread::currentThread(), m_frames,
                  m_mapSum / m_frames, m_copySum / m_frames);
            m_frames = 0;
            m_mapSum = 0.0;
            m_copySum = 0.0;
        }
        return *input;
    }

private:
    void warnOnce(const char *msg) {
        if (!m_warned) {
            m_warned = true;
            qWarning("%s", msg);
        }
    }

    bool m_warned = false;
    int m_frames = 0;
    double m_mapSum = 0.0;
    double m_copySum = 0.0;
};

QVideoFilterRunnable *LiveOcrFilter::createFilterRunnable() {
    return new LiveOcrRunnable();
}

extern "C" void register_live_ocr_filter() {
    qmlRegisterType<LiveOcrFilter>("TranslatorUi", 1, 0, "LiveOcrFilter");
}
