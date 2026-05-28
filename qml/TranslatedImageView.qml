import QtQuick 2.15
import TranslatorUi 1.0

Item {
    id: root
    property var appBridge
    property int imageMargin: 0
    property bool interactive: false
    signal imageClicked()
    UiScale { id: ui }

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

    MouseArea {
        visible: root.interactive && paintedBounds.width > 0 && paintedBounds.height > 0
        x: paintedBounds.x
        y: paintedBounds.y
        width: paintedBounds.width
        height: paintedBounds.height
        onClicked: root.imageClicked()
    }
}
