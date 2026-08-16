import QtQuick 2.15
import QtQuick.Controls 2.15

Item {
    id: control
    UiScale { id: ui }
    property string label
    property bool checked
    property var theme
    signal toggled(bool checked)

    implicitHeight: ui.dp(36)

    Label {
        anchors.left: parent.left
        anchors.right: sw.left
        anchors.rightMargin: ui.dp(12)
        anchors.verticalCenter: parent.verticalCenter
        text: control.label
        color: theme.textPrimary
        font.pixelSize: ui.dp(20)
        wrapMode: Text.WordWrap
    }

    Switch {
        id: sw
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        checked: control.checked
        onToggled: control.toggled(checked)

        indicator: Rectangle {
            implicitWidth: ui.dp(48); implicitHeight: ui.dp(26)
            x: sw.leftPadding
            y: sw.height / 2 - height / 2
            radius: ui.dp(13)
            color: sw.checked ? theme.accentColor : "#555"

            Rectangle {
                x: sw.checked ? parent.width - width - ui.dp(3) : ui.dp(3)
                y: (parent.height - height) / 2
                width: ui.dp(20); height: ui.dp(20); radius: ui.dp(10)
                color: "white"
                Behavior on x { NumberAnimation { duration: 150 } }
            }
        }
    }
}
