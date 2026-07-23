import QtQuick 2.15
import TranslatorUi 1.0

Item {
    id: root
    property var appBridge
    property int imageMargin: 0
    property bool interactive: false
    // On-image word selection replaces the text output pane, so it is only offered once the
    // translation actually produced selectable words.
    property bool selectable: interactive && appBridge && appBridge.image_words_ready
    signal imageClicked()
    UiScale { id: ui }

    // Selection endpoints as word indices into the layer currently shown. Rust owns the geometry
    // and the text; these two ints are the whole of the view's selection state.
    property int selStart: -1
    property int selEnd: -1
    property int dragAnchor: -1

    // Zoom lives on a wrapper around the image and every overlay, so pointer coordinates reaching
    // the selection layer stay in un-zoomed image space and the map/unmap helpers below need no
    // knowledge of it.
    property real zoom: 1.0
    readonly property real minZoom: 1.0
    readonly property real maxZoom: 4.0
    readonly property real maxPanX: Math.max(0, paintedBounds.width * (zoom - 1) / 2)
    readonly property real maxPanY: Math.max(0, paintedBounds.height * (zoom - 1) / 2)

    function clampPan() {
        content.x = Math.max(-maxPanX, Math.min(maxPanX, content.x))
        content.y = Math.max(-maxPanY, Math.min(maxPanY, content.y))
    }

    function setZoom(value) {
        zoom = Math.max(minZoom, Math.min(maxZoom, value))
        if (zoom === minZoom) {
            content.x = 0
            content.y = 0
        } else {
            clampPan()
        }
    }

    function resetZoom() { setZoom(minZoom) }

    // Only a newly picked image invalidates the framing. Flipping to the original and
    // re-translating both swap the pixels in place at identical dimensions, so they keep it.
    Connections {
        target: root.appBridge
        function onSelected_image_url_changed() { root.resetZoom() }
    }

    function mapX(imageX) { return paintedBounds.x + imageX * paintedBounds.scaleFactor }
    function mapY(imageY) { return paintedBounds.y + imageY * paintedBounds.scaleFactor }
    function unmapX(viewX) { return (viewX - paintedBounds.x) / paintedBounds.scaleFactor }
    function unmapY(viewY) { return (viewY - paintedBounds.y) / paintedBounds.scaleFactor }

    function clearSelection() {
        selStart = -1
        selEnd = -1
        dragAnchor = -1
        if (appBridge)
            appBridge.clear_image_selection()
    }

    onSelectableChanged: if (!selectable) clearSelection()

    Item {
        id: content
        width: root.width
        height: root.height
        scale: root.zoom
        transformOrigin: Item.Center

    Item {
        id: paintedBounds
        property real sourceWidth: root.appBridge && root.appBridge.processed_image_width > 0
                                   ? root.appBridge.processed_image_width
                                   : Math.max(selectedImage.sourceSize.width, selectedImage.implicitWidth)
        property real sourceHeight: root.appBridge && root.appBridge.processed_image_height > 0
                                    ? root.appBridge.processed_image_height
                                    : Math.max(selectedImage.sourceSize.height, selectedImage.implicitHeight)
        property real availableWidth: Math.max(0, root.width - root.imageMargin * 2)
        property real availableHeight: Math.max(0, root.height - root.imageMargin * 2)
        property real scaleFactor: sourceWidth > 0 && sourceHeight > 0
                                   ? Math.min(availableWidth / sourceWidth, availableHeight / sourceHeight)
                                   : 0
        x: root.imageMargin + (availableWidth - width) / 2
        y: root.imageMargin + (availableHeight - height) / 2
        width: sourceWidth > 0 && sourceHeight > 0 ? sourceWidth * scaleFactor : 0
        height: sourceWidth > 0 && sourceHeight > 0 ? sourceHeight * scaleFactor : 0
    }

    Image {
        id: selectedImage
        x: paintedBounds.x
        y: paintedBounds.y
        width: paintedBounds.width
        height: paintedBounds.height
        source: root.appBridge ? root.appBridge.selected_image_url : ""
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        cache: false
        smooth: true
        opacity: root.appBridge && root.appBridge.processed_image_width > 0 && root.appBridge.processed_image_height > 0 ? 0 : 1
    }

    RenderedImageItem {
        id: processedImage
        x: paintedBounds.x
        y: paintedBounds.y
        width: paintedBounds.width
        height: paintedBounds.height
        visible: root.appBridge
                 && root.appBridge.processed_image_width > 0
                 && root.appBridge.processed_image_height > 0
        image: root.appBridge ? root.appBridge.processed_image : undefined
    }

    // Detector output while recognition runs: a light sweep down the image brightens each box as
    // it passes, then every box breathes until results arrive. Mirrors the phone app's overlay.
    Item {
        id: scanLayer
        anchors.fill: parent
        visible: root.appBridge && root.appBridge.scan_active
        property real sweep: 0
        property real breathe: 0
        property bool sweepDone: false

        onVisibleChanged: {
            if (visible) {
                sweepDone = false
                breathe = 0
                sweep = 0
                sweepAnim.restart()
            } else {
                sweepAnim.stop()
                breatheAnim.stop()
            }
        }

        NumberAnimation {
            id: sweepAnim
            target: scanLayer
            property: "sweep"
            from: 0; to: 1
            duration: 1300
            easing.type: Easing.Linear
            onFinished: { scanLayer.sweepDone = true; breatheAnim.restart() }
        }

        // Starts from 0 so the pulse begins at the sweep's resting opacity rather than mid-phase,
        // then reverses rather than snapping back — the phone app's RepeatMode.Reverse.
        SequentialAnimation {
            id: breatheAnim
            loops: Animation.Infinite
            NumberAnimation {
                target: scanLayer; property: "breathe"
                from: 0; to: 1; duration: 1100; easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: scanLayer; property: "breathe"
                from: 1; to: 0; duration: 1100; easing.type: Easing.InOutQuad
            }
        }

        Repeater {
            model: root.appBridge ? root.appBridge.scan_boxes_model : null
            delegate: Rectangle {
                readonly property real centerY: root.mapY(model.cy)
                readonly property real halfBand: scanLayer.height * 0.06
                readonly property real falloff: scanLayer.sweepDone
                    ? 0
                    : Math.max(0, 1 - Math.abs(centerY - scanLayer.sweep * scanLayer.height) / halfBand)
                width: model.width * paintedBounds.scaleFactor
                height: model.height * paintedBounds.scaleFactor
                x: root.mapX(model.cx) - width / 2
                y: centerY - height / 2
                radius: height / 2
                rotation: model.angle_degrees
                transformOrigin: Item.Center
                antialiasing: true
                color: "white"
                opacity: scanLayer.sweepDone
                         ? 0.14 + 0.12 * scanLayer.breathe
                         : 0.14 + 0.30 * falloff
            }
        }
    }

    // One merged pill per selected line, already gap-clamped by translator-rs.
    Repeater {
        model: root.appBridge ? root.appBridge.selection_pills_model : null
        delegate: Rectangle {
            width: model.width * paintedBounds.scaleFactor
            height: model.height * paintedBounds.scaleFactor
            x: root.mapX(model.cx) - width / 2
            y: root.mapY(model.cy) - height / 2
            radius: height / 2
            rotation: model.angle_degrees
            transformOrigin: Item.Center
            color: "#553B82F6"
            antialiasing: true
        }
    }

    Component {
        id: handleMarker
        Rectangle {
            property real imageX: 0
            property real imageY: 0
            width: ui.dp(14)
            height: width
            radius: width / 2
            x: root.mapX(imageX) - width / 2
            y: root.mapY(imageY)
            color: "#3B82F6"
            border.width: ui.dp(2)
            border.color: "#FFFFFF"
            antialiasing: true
            visible: root.selectable && root.appBridge.selection_active
        }
    }

    Loader {
        sourceComponent: handleMarker
        onLoaded: {
            item.imageX = Qt.binding(function() { return root.appBridge ? root.appBridge.selection_start_x : 0 })
            item.imageY = Qt.binding(function() { return root.appBridge ? root.appBridge.selection_start_y : 0 })
        }
    }

    Loader {
        sourceComponent: handleMarker
        onLoaded: {
            item.imageX = Qt.binding(function() { return root.appBridge ? root.appBridge.selection_end_x : 0 })
            item.imageY = Qt.binding(function() { return root.appBridge ? root.appBridge.selection_end_y : 0 })
        }
    }

    // Off-screen carrier: QML has no clipboard API, so copying goes through a TextEdit the way the
    // output pane's copy button already does.
    TextEdit {
        id: clipboardCarrier
        visible: false
        text: root.appBridge ? root.appBridge.selection_text : ""
    }

    function copySelection() {
        clipboardCarrier.selectAll()
        clipboardCarrier.copy()
        clipboardCarrier.deselect()
    }

    MouseArea {
        id: selectionArea
        visible: root.interactive && paintedBounds.width > 0 && paintedBounds.height > 0
        enabled: visible
        anchors.fill: parent
        // Panning is a plain drag of the wrapper; Qt suppresses `clicked` once the drag threshold
        // is crossed, which is exactly the tap-vs-pan split we want.
        drag.target: (handleDrag || root.zoom <= root.minZoom) ? null : content
        drag.axis: Drag.XAndYAxis
        drag.minimumX: -root.maxPanX
        drag.maximumX: root.maxPanX
        drag.minimumY: -root.maxPanY
        drag.maximumY: root.maxPanY
        drag.threshold: ui.dp(8)

        property real handleRadius: ui.dp(22)
        property bool handleDrag: false

        function distanceToHandle(mx, my, imageX, imageY) {
            const dx = mx - root.mapX(imageX)
            const dy = my - root.mapY(imageY)
            return Math.sqrt(dx * dx + dy * dy)
        }

        onPressed: function(mouse) {
            handleDrag = false
            if (!root.selectable || !root.appBridge.selection_active)
                return
            const toStart = distanceToHandle(mouse.x, mouse.y,
                                             root.appBridge.selection_start_x,
                                             root.appBridge.selection_start_y)
            const toEnd = distanceToHandle(mouse.x, mouse.y,
                                           root.appBridge.selection_end_x,
                                           root.appBridge.selection_end_y)
            if (Math.min(toStart, toEnd) <= handleRadius) {
                handleDrag = true
                root.dragAnchor = toStart <= toEnd ? root.selEnd : root.selStart
            }
        }

        onPositionChanged: function(mouse) {
            if (!handleDrag || !pressed || root.dragAnchor < 0)
                return
            const word = root.appBridge.image_nearest_word(root.unmapX(mouse.x),
                                                           root.unmapY(mouse.y),
                                                           root.dragAnchor)
            if (word < 0)
                return
            root.selStart = Math.min(root.dragAnchor, word)
            root.selEnd = Math.max(root.dragAnchor, word)
            root.appBridge.set_image_selection(root.selStart, root.selEnd)
        }

        onReleased: {
            handleDrag = false
            root.dragAnchor = -1
        }

        // Only fires when the press did not turn into a drag, so a pan never selects.
        onClicked: function(mouse) {
            if (!root.selectable)
                return
            const word = root.appBridge.image_word_at(root.unmapX(mouse.x), root.unmapY(mouse.y))
            if (word < 0) {
                root.clearSelection()
                return
            }
            root.selStart = word
            root.selEnd = word
            root.appBridge.set_image_selection(word, word)
        }
    }
    }

    // Two-finger pinch. Single-touch falls through to the selection/pan area above.
    PinchArea {
        anchors.fill: parent
        enabled: root.interactive
        property real startZoom: 1.0
        onPinchStarted: startZoom = root.zoom
        onPinchUpdated: function(pinch) { root.setZoom(startZoom * pinch.scale) }
    }

    // Desktop: ctrl+wheel and ctrl+plus/minus, matching every other image viewer.
    WheelHandler {
        acceptedModifiers: Qt.ControlModifier
        onWheel: function(event) {
            root.setZoom(root.zoom * (event.angleDelta.y > 0 ? 1.15 : 1 / 1.15))
        }
    }

    Shortcut { sequences: ["Ctrl++", "Ctrl+="]; onActivated: root.setZoom(root.zoom * 1.25) }
    Shortcut { sequence: "Ctrl+-"; onActivated: root.setZoom(root.zoom / 1.25) }
    Shortcut { sequence: "Ctrl+0"; onActivated: root.resetZoom() }
}
