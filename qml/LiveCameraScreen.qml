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
        focus.focusMode: Camera.FocusContinuous
        focus.focusPointMode: Camera.FocusPointAuto
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

    // The composited camera + translation overlay, rendered on the GPU. The
    // worker rotates the frame upright and crops it to this viewport's aspect,
    // so it fills without rotation or letterboxing here. `frame_tick` advances
    // per camera frame to schedule a GPU present.
    LiveCameraItem {
        anchors.fill: parent
        frame_tick: root.appBridge.live_frame_tick
    }

    // Covers the preview so a tap re-triggers OCR and, crucially, doesn't fall
    // through to the TranslationScreen beneath this overlay (which would focus a
    // TextField and raise the keyboard). The close button is a later sibling, so
    // it stacks above this and still receives its own taps.
    MouseArea {
        anchors.fill: parent
        onClicked: {
            focusRing.showAt(mouse.x, mouse.y)
            root.appBridge.request_live_acquire()
        }
    }

    // Tap indicator only: focus stays continuous (this device's manual-focus
    // controls are no-ops), so the ring is purely a hint that the tap landed.
    // Pops in slightly large and shrinks/fades, mirroring the Android viewfinder.
    Rectangle {
        id: focusRing
        width: Math.round(Math.min(root.width, root.height) * 0.16)
        height: width
        radius: width / 2
        color: "transparent"
        border.color: "white"
        border.width: 2
        antialiasing: true
        opacity: 0
        visible: opacity > 0

        function showAt(px, py) {
            x = px - width / 2
            y = py - height / 2
            ringAnim.restart()
        }

        ParallelAnimation {
            id: ringAnim
            NumberAnimation {
                target: focusRing
                property: "scale"
                from: 1.5
                to: 1.0
                duration: 280
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: focusRing
                property: "opacity"
                from: 1.0
                to: 0.0
                duration: 420
            }
        }
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
