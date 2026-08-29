import QtQuick 2.15
import QtQuick.Dialogs 1.3

Item {
    property var appBridge

    readonly property var imageExtensions: ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.bmp", "*.gif", "*.tif", "*.tiff"]
    readonly property var documentExtensions: (appBridge.pdf_available ? ["*.pdf"] : []).concat(["*.epub", "*.odt", "*.txt"])

    function open() {
        picker.open()
    }

    FileDialog {
        id: picker
        title: "Choose an image or document"
        nameFilters: [
            "Images and documents (" + imageExtensions.concat(documentExtensions).join(" ") + ")",
            "Documents (" + documentExtensions.join(" ") + ")",
            "Images (" + imageExtensions.join(" ") + ")"
        ]
        selectExisting: true
        selectMultiple: false
        onAccepted: appBridge.process_file_selection(fileUrl.toString())
    }
}
