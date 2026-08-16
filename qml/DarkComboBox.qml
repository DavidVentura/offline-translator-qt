import QtQuick 2.15
import QtQuick.Controls 2.15

ComboBox {
    id: control
    property var theme
    property string iconSource
    leftInset: 0
    rightInset: 0
    topInset: 0
    bottomInset: 0
    leftPadding: 0
    rightPadding: 0
    topPadding: 0
    bottomPadding: 0
    UiScale { id: ui }

    contentItem: Label {
        leftPadding: ui.dp(6)
        rightPadding: control.indicator ? control.indicator.width + ui.dp(6) : ui.dp(6)
        text: control.displayText
        color: theme.textPrimary
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        font.pixelSize: ui.dp(20)
    }

    background: Rectangle {
        radius: ui.dp(8)
        color: theme.backgroundElevated
        border.width: 1
        border.color: theme.borderColor
    }

    indicator: Image {
        source: iconSource
        width: ui.dp(16); height: ui.dp(16)
        x: control.width - width - ui.dp(10)
        y: (control.height - height) / 2
    }

    popup: Popup {
        y: control.height
        width: control.width
        implicitHeight: contentItem.implicitHeight
        padding: ui.dp(1)

        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: parent.visible ? control.model : null

            delegate: Rectangle {
                width: control.width
                height: ui.dp(36)
                color: delegateMouseArea.containsMouse ? theme.surfaceAltColor : theme.surfaceColor

                Label {
                    anchors.fill: parent
                    leftPadding: ui.dp(6)
                    text: modelData
                    color: theme.textPrimary
                    verticalAlignment: Text.AlignVCenter
                    font.pixelSize: ui.dp(20)
                }

                MouseArea {
                    id: delegateMouseArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: {
                        control.currentIndex = index
                        control.activated(index)
                        control.popup.close()
                    }
                }
            }
        }

        background: Rectangle {
            color: theme.surfaceColor
            border.color: theme.borderColor
            radius: ui.dp(4)
        }
    }
}
