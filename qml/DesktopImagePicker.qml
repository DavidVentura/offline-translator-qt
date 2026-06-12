import QtQuick 2.15
import QtQuick.Dialogs 1.3

Item {
    property var appBridge

    function open() {
        picker.open()
    }

    FileDialog {
        id: picker
        title: "Choose an image or document"
        nameFilters: [
            "Images and documents (*.png *.jpg *.jpeg *.webp *.bmp *.gif *.tif *.tiff *.pdf *.epub *.odt *.txt)",
            "Documents (*.pdf *.epub *.odt *.txt)",
            "Images (*.png *.jpg *.jpeg *.webp *.bmp *.gif *.tif *.tiff)"
        ]
        selectExisting: true
        selectMultiple: false
        onAccepted: appBridge.process_file_selection(fileUrl.toString())
    }
}
