#pragma once

#include <QtMultimedia/QAbstractVideoFilter>
#include <QtMultimedia/QVideoFilterRunnable>

// A QtMultimedia video filter that taps each camera frame. For now it only
// logs the incoming frame format (to learn what the device delivers) and
// passes the frame through unchanged. Registered as a QML type so a
// VideoOutput can attach it via `filters: [ LiveOcrFilter {} ]`.
class LiveOcrFilter : public QAbstractVideoFilter {
    Q_OBJECT
public:
    QVideoFilterRunnable *createFilterRunnable() override;
};
