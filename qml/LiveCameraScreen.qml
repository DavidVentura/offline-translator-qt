import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15
import QtMultimedia 5.15
import Qt.labs.settings 1.0
import TranslatorUi 1.0

Item {
    id: root
    property var appBridge
    property var theme

    UiScale { id: ui; desktopMode: appBridge.desktop_mode }

    // Set while a still capture is in flight, to dim the shutter and block
    // re-entry until the image is saved (or fails).
    property bool capturing: false

    // True when the live-camera screen is visible/in use. When false we
    // park the QCamera in UnloadedState so it doesn't sit on the HAL,
    // but the element itself stays alive — see the comment in Main.qml.
    property bool screenActive: false

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

    // The shutter "click" sound is played by qtubuntu-camera's
    // AalImageCaptureControl::shutter() (not by QtMultimedia or this app), which
    // reads QSettings.value("playShutterSound", true) on every capture. Because
    // the plugin lives in our process, writing this key from our own QSettings
    // mutes it — same trick lomiri-camera-app uses for its UI toggle.
    Settings {
        id: cameraSettings
        property bool playShutterSound: true
        Component.onCompleted: playShutterSound = false
    }

    function syncCombo(combo, value) {
        var index = combo.find(value)
        combo.currentIndex = index >= 0 ? index : 0
    }

    // Kick a one-shot autofocus at a normalized source-frame point. Returns
    // true if the HAL was actually asked to refocus. Shared between the tap
    // handler and the auto-fire on camera start.
    function focusAt(normPoint) {
        if (!cam.focus.isFocusPointModeSupported(Camera.FocusPointCustom))
            return false
        if (normPoint.x < 0.0 || normPoint.x > 1.0 || normPoint.y < 0.0 || normPoint.y > 1.0)
            return false
        cam.focus.focusMode = Camera.FocusAuto
        cam.focus.customFocusPoint = normPoint
        cam.focus.focusPointMode = Camera.FocusPointCustom
        autoFocusRevertTimer.restart()
        return true
    }

    Rectangle {
        anchors.fill: parent
        color: "black"
    }

    Camera {
        id: cam
        captureMode: Camera.CaptureStillImage
        // Park the camera when the screen isn't visible (same pattern as
        // lomiri-camera-app: never destroy the Camera, just toggle state).
        cameraState: root.screenActive ? Camera.ActiveState : Camera.UnloadedState
        focus.focusMode: Camera.FocusContinuous
        focus.focusPointMode: Camera.FocusPointAuto
        flash.mode: root.torchOn ? root.torchOnValue : Camera.FlashOff

        // Just log the HAL-reported sensor mount angle for now — the bridge
        // is wired (appBridge.set_camera_orientation) but pushing the value
        // produces a vertical-mirror on at least one device, so we leave the
        // compositor on its hardcoded default until the reported value can be
        // mapped to our quadrant/flip convention correctly.
        onOrientationChanged: console.warn("[cam] sensor orientation reported by HAL: " + cam.orientation)

        imageCapture {
            onImageSaved: {
                root.capturing = false
                appBridge.process_image_selection(path)
                appBridge.close_live_camera()
            }
            onCaptureFailed: {
                root.capturing = false
            }
        }

        onCameraStatusChanged: {
            if (cameraStatus === Camera.ActiveStatus)
                root.focusAt(Qt.point(0.5, 0.5))
        }
    }

    // Full-size so qtubuntu's qtvideo-node actually renders the camera — that's
    // what mints the preview GL_TEXTURE_EXTERNAL_OES and makes the filter's
    // handle() probe meaningful. It's covered by the composited LiveCameraItem
    // on top, so it's here to render + feed the OCR filter, not to be seen.
    VideoOutput {
        id: viewFinder
        objectName: "cameraVO"
        anchors.fill: parent
        source: cam
        filters: [ LiveOcrFilter {} ]
    }

    // Re-arms continuous AF a few seconds after a manual focus tap, matching
    // lomiri-camera-app's behaviour so the camera doesn't stay locked on the
    // tapped point forever.
    Timer {
        id: autoFocusRevertTimer
        interval: 5000
        onTriggered: {
            cam.focus.focusMode = Camera.FocusContinuous
            cam.focus.focusPointMode = Camera.FocusPointAuto
        }
    }

    // The composited camera + translation overlay, rendered on the GPU. The
    // worker rotates the frame upright and crops it to this viewport's aspect,
    // so it fills without rotation or letterboxing here. `frame_tick` advances
    // per camera frame to schedule a GPU present.
    LiveCameraItem {
        anchors.fill: parent
        frame_tick: root.appBridge.live_frame_tick
        screen_active: root.screenActive
    }

    // Covers the preview so a tap re-triggers OCR (when the live pipeline is on)
    // and, crucially, doesn't fall through to the TranslationScreen beneath this
    // overlay (which would focus a TextField and raise the keyboard). The
    // controls below are later siblings, so they stack above and get their taps.
    MouseArea {
        anchors.fill: parent
        onClicked: {
            focusRing.showAt(mouse.x, mouse.y)
            // setCustomFocusPoint is the only call that actually invokes
            // android_camera_start_autofocus() in qtubuntu-camera, and the HAL
            // only honours the focus region when focusMode is FocusAuto — so
            // drop out of continuous for the tap and let autoFocusRevertTimer
            // snap it back.
            root.focusAt(viewFinder.mapPointToSourceNormalized(Qt.point(mouse.x, mouse.y)))
            if (appBridge.live_ocr_active)
                appBridge.request_live_acquire()
        }
    }

    // Tap indicator: a ring that pops at the tap point and fades, mirroring the
    // Android viewfinder. The MouseArea above also kicks an actual autofocus at
    // this point — the ring is the visual half of that.
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
        onClicked: appBridge.close_live_camera()
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
