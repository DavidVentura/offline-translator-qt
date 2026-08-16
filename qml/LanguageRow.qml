import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Rectangle {
    UiScale { id: ui }
    id: root
    required property string code
    required property string name
    required property string size
    required property real download_progress
    property bool built_in: false
    property bool installed: false
    property var appBridge
    property var theme

    Layout.fillWidth: true
    color: theme.surfaceColor
    radius: ui.dp(12)
    border.color: theme.borderColor
    implicitHeight: ui.dp(72)

    RowLayout {
        anchors.fill: parent
        anchors.margins: ui.dp(16)
        spacing: ui.dp(12)

        ColumnLayout {
            Layout.fillWidth: true

            Label {
                text: parent.parent.name
                color: parent.parent.parent.theme.textPrimary
                font.pixelSize: ui.dp(21)
            }

            Label {
                text: parent.parent.parent.built_in ? "Built in" : parent.parent.parent.size
                color: parent.parent.parent.theme.textSecondary
                font.pixelSize: ui.dp(17)
            }
        }

        ProgressBar {
            visible: parent.parent.download_progress > 0
            value: parent.parent.download_progress
            from: 0
            to: 1
            Layout.preferredWidth: ui.dp(96)
        }

        Button {
            visible: parent.parent.download_progress <= 0
            enabled: !parent.parent.built_in || !parent.parent.installed
            text: parent.parent.installed ? "Delete" : "Download"
            onClicked: {
                if (parent.parent.installed) {
                    parent.parent.appBridge.delete_language(parent.parent.code)
                } else {
                    parent.parent.appBridge.download_language(parent.parent.code)
                }
            }
        }
    }
}
