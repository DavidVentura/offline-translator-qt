import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: root
    required property var appBridge
    required property var theme

    UiScale { id: ui }

    visible: opacity > 0 || appBridge.dictionary_popup_open
    opacity: appBridge.dictionary_popup_open ? 1 : 0

    Behavior on opacity {
        NumberAnimation { duration: 150; easing.type: Easing.InOutQuad }
    }

    Rectangle {
        anchors.fill: parent
        color: "#80000000"
        opacity: root.opacity

        MouseArea {
            anchors.fill: parent
            onClicked: appBridge.close_dictionary_popup()
        }
    }

    Rectangle {
        id: sheet
        anchors.left: parent.left
        anchors.right: parent.right
        y: appBridge.dictionary_popup_open ? parent.height - height : parent.height
        readonly property real maxSheetHeight: parent.height * 0.5
        readonly property real chromeHeight: headerRow.implicitHeight + (subtitleLabel.visible ? subtitleLabel.implicitHeight + contentColumn.spacing : 0)
        readonly property real verticalPadding: ui.dp(40)
        readonly property real maxScrollHeight: Math.max(0, maxSheetHeight - chromeHeight - verticalPadding)
        height: Math.min(maxSheetHeight, chromeHeight + definitionScroll.height + verticalPadding)
        radius: ui.dp(18)
        color: theme.surfaceColor
        z: 1

        Behavior on y {
            NumberAnimation { duration: 150; easing.type: Easing.InQuad }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.radius
            color: parent.color
        }

        MouseArea {
            anchors.fill: parent
        }

        Column {
            id: contentColumn
            anchors.fill: parent
            anchors.margins: ui.dp(16)
            spacing: ui.dp(10)

            RowLayout {
                id: headerRow
                width: parent.width
                spacing: ui.dp(12)

                Label {
                    Layout.fillWidth: true
                    text: appBridge.dictionary_popup_word
                    color: theme.textPrimary
                    font.pixelSize: ui.dp(32)
                    font.bold: true
                    elide: Text.ElideRight
                }

                Row {
                    visible: appBridge.dictionary_popup_has_secondary
                    spacing: ui.dp(6)

                    Label {
                        text: appBridge.dictionary_popup_primary_label
                        color: appBridge.dictionary_popup_selected_entry_index === 0 ? theme.textPrimary : theme.textSecondary
                        font.pixelSize: ui.dp(21)
                        font.bold: appBridge.dictionary_popup_selected_entry_index === 0

                        MouseArea {
                            anchors.fill: parent
                            onClicked: appBridge.select_dictionary_popup_entry(0)
                        }
                    }

                    Label {
                        text: "|"
                        color: theme.textSecondary
                        font.pixelSize: ui.dp(21)
                    }

                    Label {
                        text: appBridge.dictionary_popup_secondary_label
                        color: appBridge.dictionary_popup_selected_entry_index === 1 ? theme.textPrimary : theme.textSecondary
                        font.pixelSize: ui.dp(21)
                        font.bold: appBridge.dictionary_popup_selected_entry_index === 1

                        MouseArea {
                            anchors.fill: parent
                            onClicked: appBridge.select_dictionary_popup_entry(1)
                        }
                    }
                }
            }

            Label {
                id: subtitleLabel
                visible: text.length > 0
                width: parent.width
                text: appBridge.dictionary_popup_subtitle
                color: theme.textPrimary
                opacity: 0.9
                font.pixelSize: ui.dp(20)
                wrapMode: Text.Wrap
            }

            ScrollView {
                id: definitionScroll
                width: parent.width
                height: Math.min(definitionColumn.implicitHeight, sheet.maxScrollHeight)
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical.policy: definitionColumn.implicitHeight > height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff

                Column {
                    id: definitionColumn
                    width: definitionScroll.availableWidth
                    spacing: ui.dp(6)

                    Repeater {
                        model: appBridge.dictionary_popup_rows_model

                        delegate: Item {
                            required property string kind
                            required property string text
                            width: parent.width
                            implicitHeight: rowText.implicitHeight

                            Label {
                                id: rowText
                                anchors.left: parent.left
                                anchors.right: parent.right
                                text: kind === "gloss" ? "• " + parent.text : parent.text
                                color: theme.textPrimary
                                font.pixelSize: kind === "pos" ? ui.dp(24) : ui.dp(20)
                                font.bold: kind === "pos"
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }
            }
        }
    }
}
