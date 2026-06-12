import QtQuick 2.15
import Lomiri.Content 1.1

Item {
    id: root
    property var appBridge
    property var shareContentType: ContentType.Pictures
    property var shareHandler: ContentHandler.Share
    property string pendingUrl: ""
    property var activeTransfer: null
    property var sharedItem: null

    function share(url) {
        if (!url) {
            return
        }

        pendingUrl = ("" + url).trim()
        if (!pendingUrl.length) {
            return
        }
        picker.visible = true
    }

    function destroySharedItem() {
        if (sharedItem) {
            sharedItem.destroy()
            sharedItem = null
        }
    }

    function createSharedItem() {
        destroySharedItem()
        sharedItem = shareItemComponent.createObject(root, { "url": pendingUrl })
        return sharedItem
    }

    function cleanupTransfer() {
        activeTransfer = null
        pendingUrl = ""
        destroySharedItem()
    }

    ContentPeerPicker {
        id: picker
        anchors.fill: parent
        visible: false
        showTitle: false
        contentType: root.shareContentType
        handler: root.shareHandler
        onPeerSelected: {
            visible = false
            if (!pendingUrl.length) {
                cleanupTransfer()
                return
            }

            if (!createSharedItem()) {
                cleanupTransfer()
                return
            }

            peer.selectionType = ContentTransfer.Single
            activeTransfer = peer.request()
            if (!activeTransfer) {
                cleanupTransfer()
                return
            }

            activeTransfer.items = [sharedItem]
            activeTransfer.state = ContentTransfer.Charged
        }
        onCancelPressed: {
            visible = false
            cleanupTransfer()
        }
    }

    Component {
        id: shareItemComponent

        ContentItem {}
    }

    Connections {
        target: activeTransfer
        ignoreUnknownSignals: true

        function onStateChanged() {
            if (!activeTransfer) {
                return
            }

            if (activeTransfer.state === ContentTransfer.Aborted ||
                    activeTransfer.state === ContentTransfer.Finalized) {
                cleanupTransfer()
            }
        }
    }
}
