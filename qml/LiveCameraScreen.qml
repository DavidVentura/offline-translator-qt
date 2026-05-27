import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtMultimedia 5.15
import TranslatorUi 1.0

Item {
    id: root
    property var appBridge
    property var theme

    UiScale { id: ui; desktopMode: appBridge.desktop_mode }

    // Set while a still capture is in flight, to dim the shutter and block
    // re-entry until the image is saved (or fails).
    property bool capturing: false

    property bool torchOn: false
    // The flash mode that means "continuous light" on this device, picked from
    // what the backend actually advertises (some report FlashTorch, some
    // FlashVideoLight). FlashOff when neither is offered — i.e. no torch.
    readonly property int torchOnValue: {
        var modes = cam.flash.supportedModes
        for (var i = 0; i < modes.length; i++) {
            if (modes[i] === Camera.FlashTorch || modes[i] === Camera.FlashVideoLight)
                return modes[i]
        }
        return Camera.FlashOff
    }
    readonly property bool hasTorch: torchOnValue !== Camera.FlashOff

    function syncCombo(combo, value) {
        var index = combo.find(value)
        combo.currentIndex = index >= 0 ? index : 0
    }

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    Camera {
        id: cam
        captureMode: Camera.CaptureStillImage
        viewfinder.resolution: Qt.size(1280, 720)
        focus.focusMode: Camera.FocusContinuous
        focus.focusPointMode: Camera.FocusPointAuto
        flash.mode: root.torchOn ? root.torchOnValue : Camera.FlashOff

        imageCapture {
            onImageSaved: {
                root.capturing = false
                appBridge.process_image_selection(path)
                // Defer the close: it deactivates this Loader and destroys the
                // Camera whose signal we're inside, which is unsafe to do inline.
                Qt.callLater(appBridge.close_live_camera)
            }
            onCaptureFailed: {
                root.capturing = false
            }
        }

        // The 1280x720 request above is silently ignored on this backend (the
        // sensor handed back 1920x1440 = ~11MB/frame, which the filter reads
        // back + copies every frame). Once loaded, pick the smallest *supported*
        // resolution with width >= 960 (so recognizer strips stay legible) to
        // shrink the per-frame readback proportionally.
        onCameraStatusChanged: {
            if (cameraStatus !== Camera.LoadedStatus)
                return
            if (typeof cam.supportedViewfinderResolutions !== "function")
                return
            var res = cam.supportedViewfinderResolutions()
            var names = []
            for (var i = 0; i < res.length; i++)
                names.push(res[i].width + "x" + res[i].height)
            console.warn("[cam] supported viewfinder: " + names.join(", "))
            var best = null
            for (var j = 0; j < res.length; j++) {
                var r = res[j]
                if (r.width < 960)
                    continue
                if (best === null || r.width * r.height < best.width * best.height)
                    best = r
            }
            if (best === null)
                for (var k = 0; k < res.length; k++) {
                    var r2 = res[k]
                    if (best === null || r2.width * r2.height < best.width * best.height)
                        best = r2
                }
            if (best !== null) {
                cam.viewfinder.resolution = Qt.size(best.width, best.height)
                console.warn("[cam] set viewfinder " + best.width + "x" + best.height)
            }
        }
    }

    // Full-size so qtubuntu's qtvideo-node actually renders the camera — that's
    // what mints the preview GL_TEXTURE_EXTERNAL_OES and makes the filter's
    // handle() probe meaningful. It's covered by the composited LiveCameraItem
    // on top, so it's here to render + feed the OCR filter, not to be seen.
    VideoOutput {
        objectName: "cameraVO"
        anchors.fill: parent
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

    // Covers the preview so a tap re-triggers OCR (when the live pipeline is on)
    // and, crucially, doesn't fall through to the TranslationScreen beneath this
    // overlay (which would focus a TextField and raise the keyboard). The
    // controls below are later siblings, so they stack above and get their taps.
    MouseArea {
        anchors.fill: parent
        onClicked: {
            focusRing.showAt(mouse.x, mouse.y)
            if (appBridge.live_ocr_active)
                appBridge.request_live_acquire()
        }
    }

    // Tap indicator: a ring that pops at the tap point and fades, mirroring the
    // Android viewfinder. Focus stays continuous; this is purely a hint.
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

    // From/to language pills with swap, bound to the same bridge API the main
    // TopBar uses, so language changes drive both the live pipeline and a
    // captured still.
    RowLayout {
        id: languageBar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: closeButton.left
        anchors.margins: ui.dp(12)
        height: ui.dp(40)
        spacing: ui.dp(8)

        Component.onCompleted: {
            root.syncCombo(fromCombo, appBridge.source_language_name)
            root.syncCombo(toCombo, appBridge.target_language_name)
        }

        Connections {
            target: appBridge
            function onSource_language_name_changed() { root.syncCombo(fromCombo, appBridge.source_language_name) }
            function onTarget_language_name_changed() { root.syncCombo(toCombo, appBridge.target_language_name) }
            function onInstalled_from_language_names_changed() { root.syncCombo(fromCombo, appBridge.source_language_name) }
            function onInstalled_to_language_names_changed() { root.syncCombo(toCombo, appBridge.target_language_name) }
        }

        DarkComboBox {
            id: fromCombo
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            desktopMode: appBridge.desktop_mode
            theme: root.theme
            iconSource: appBridge.asset_url("expand_more.svg")
            model: appBridge.installed_from_language_names
            onActivated: appBridge.set_from(currentText)
        }

        FeedbackIconButton {
            Layout.preferredWidth: ui.dp(36)
            Layout.fillHeight: true
            iconSize: ui.dp(20)
            iconSource: appBridge.asset_url("swap.svg")
            enabled: appBridge.swap_enabled
            onClicked: appBridge.swap_languages()
        }

        DarkComboBox {
            id: toCombo
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            desktopMode: appBridge.desktop_mode
            theme: root.theme
            iconSource: appBridge.asset_url("expand_more.svg")
            model: appBridge.installed_to_language_names
            onActivated: appBridge.set_to(currentText)
        }
    }

    RoundButton {
        id: closeButton
        text: "✕"
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: ui.dp(12)
        width: ui.dp(40)
        height: ui.dp(40)
        onClicked: root.appBridge.close_live_camera()
    }

    // Bottom controls: torch (left), shutter (centre), live-OCR toggle (right).
    Item {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: ui.dp(24)
        height: ui.dp(72)

        FeedbackIconButton {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: ui.dp(48)
            height: ui.dp(48)
            iconSize: ui.dp(28)
            iconSource: appBridge.asset_url(root.torchOn ? "flash_on.svg" : "flash_off.svg")
            enabled: root.hasTorch
            onClicked: root.torchOn = !root.torchOn
        }

        // Shutter: white ring with a filled inner disc, dimmed while capturing.
        Rectangle {
            anchors.centerIn: parent
            width: ui.dp(72)
            height: ui.dp(72)
            radius: width / 2
            color: "transparent"
            border.color: "white"
            border.width: ui.dp(4)
            antialiasing: true

            Rectangle {
                anchors.centerIn: parent
                width: parent.width - ui.dp(20)
                height: width
                radius: width / 2
                antialiasing: true
                color: root.capturing ? Qt.rgba(1, 1, 1, 0.5) : "white"
            }

            MouseArea {
                anchors.fill: parent
                enabled: !root.capturing && cam.imageCapture.ready
                onClicked: {
                    root.capturing = true
                    cam.imageCapture.captureToLocation(appBridge.capture_dir())
                }
            }
        }

        FeedbackIconButton {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: ui.dp(48)
            height: ui.dp(48)
            iconSize: ui.dp(28)
            iconSource: appBridge.asset_url("auto_awesome.svg")
            enabled: !appBridge.disable_ocr
            opacity: (!appBridge.disable_ocr && !appBridge.live_ocr_active) ? 0.5 : 1.0
            onClicked: appBridge.set_live_ocr_active(!appBridge.live_ocr_active)
        }
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
