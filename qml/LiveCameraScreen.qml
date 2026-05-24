import QtQuick 2.15
import QtQuick.Controls 2.15
import QtMultimedia 5.15
import TranslatorUi 1.0

Item {
    id: root
    property var appBridge

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    Camera {
        id: cam
        viewfinder.resolution: Qt.size(1280, 720)
    }

    // Frame pump only: the filter runs on every frame this VideoOutput receives,
    // feeding the Rust pipeline. We don't display it (the composited
    // RenderedImageItem below is the viewfinder), so it's 1x1 to avoid a second,
    // wasted full-screen GPU render of the live feed. Frame resolution comes from
    // the camera viewfinder, not this item's size, so the filter still gets full
    // 1280x720 frames.
    VideoOutput {
        width: 1
        height: 1
        source: cam
        filters: [ LiveOcrFilter {} ]
    }

    // The composited camera + translation overlay. The worker already rotates
    // the frame upright and crops it to this viewport's aspect, so it fills
    // without rotation or letterboxing here.
    RenderedImageItem {
        anchors.fill: parent
        image: root.appBridge.live_camera_image
    }

    RoundButton {
        text: "✕"
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 24
        width: 56
        height: 56
        onClicked: root.appBridge.close_live_camera()
    }

    Component.onCompleted: {
        root.appBridge.set_live_viewport(Math.round(width), Math.round(height))
        var cams = QtMultimedia.availableCameras
        for (var i = 0; i < cams.length; i++) {
            if (cams[i].position === Camera.BackFace) {
                cam.deviceId = cams[i].deviceId
                break
            }
        }
        cam.start()
    }
}
