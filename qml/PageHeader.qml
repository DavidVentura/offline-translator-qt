import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: root
    required property var appBridge
    required property var theme
    required property string title
    signal backRequested()

    UiScale { id: ui }

    implicitHeight: ui.dp(56)

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: ui.dp(12)
        anchors.rightMargin: ui.dp(12)
        anchors.topMargin: ui.dp(4)
        anchors.bottomMargin: ui.dp(4)
        spacing: ui.dp(8)

        FeedbackIconButton {
            Layout.preferredWidth: ui.dp(36)
            Layout.preferredHeight: ui.dp(36)
            Layout.alignment: Qt.AlignVCenter
            iconSize: ui.dp(20)
            iconVerticalOffset: -ui.dp(1)
            iconSource: appBridge.asset_url("back.svg")
            onClicked: root.backRequested()
        }

        Label {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            text: root.title
            color: theme.textPrimary
            font.pixelSize: ui.pageTitlePx
            font.bold: true
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }
    }
}
