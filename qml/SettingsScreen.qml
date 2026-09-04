import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: root
    property var appBridge
    property var theme
    UiScale { id: ui }

    property bool advancedExpanded: false
    property string expandMoreIcon: appBridge.asset_url("expand_more.svg")

    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: content.implicitHeight
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: content
            width: parent.width
            spacing: 0

            PageHeader {
                Layout.fillWidth: true
                appBridge: root.appBridge
                theme: root.theme
                title: "Settings"
                onBackRequested: appBridge.back_from_settings()
            }

            Item { Layout.preferredHeight: ui.dp(12) }

            // ── Languages ──
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: ui.dp(16); Layout.rightMargin: ui.dp(16)
                implicitHeight: langCol.implicitHeight + ui.dp(32)
                radius: ui.dp(12); color: theme.surfaceColor

                ColumnLayout {
                    id: langCol
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: ui.dp(16) }
                    spacing: ui.dp(12)

                    Label { text: "Languages"; color: theme.accentColor; font.pixelSize: ui.dp(24); font.bold: true }

                    Item {
                        Layout.fillWidth: true; implicitHeight: ui.dp(28)

                        Label {
                            anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
                            text: "Language Packs"; color: theme.textPrimary; font.pixelSize: ui.dp(20)
                        }
                        Label {
                            anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                            text: "Manage"; color: theme.accentColor; font.pixelSize: ui.dp(20)
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: appBridge.show_manage_languages() }
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: ui.dp(16) }

            // ── General ──
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: ui.dp(16); Layout.rightMargin: ui.dp(16)
                implicitHeight: generalCol.implicitHeight + ui.dp(32)
                radius: ui.dp(12); color: theme.surfaceColor

                ColumnLayout {
                    id: generalCol
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: ui.dp(16) }
                    spacing: ui.dp(16)

                    Label { text: "General"; color: theme.accentColor; font.pixelSize: ui.dp(24); font.bold: true }

                    ColumnLayout {
                        Layout.fillWidth: true; spacing: ui.dp(6)
                        Label { text: "Default 'from' language"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                        DarkComboBox {
                            Layout.fillWidth: true; Layout.preferredHeight: ui.dp(40)
                            theme: root.theme; iconSource: expandMoreIcon
                            model: appBridge.installed_from_language_names
                            Component.onCompleted: { var idx = find(appBridge.source_language_name); if (idx >= 0) currentIndex = idx }
                            onActivated: appBridge.set_from(currentText)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true; spacing: ui.dp(6)
                        Label { text: "Default 'to' language"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                        DarkComboBox {
                            Layout.fillWidth: true; Layout.preferredHeight: ui.dp(40)
                            theme: root.theme; iconSource: expandMoreIcon
                            model: appBridge.installed_to_language_names
                            Component.onCompleted: { var idx = find(appBridge.target_language_name); if (idx >= 0) currentIndex = idx }
                            onActivated: appBridge.set_to(currentText)
                        }
                    }

                }
            }

            Item { Layout.preferredHeight: ui.dp(16) }

            // ── OCR ──
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: ui.dp(16); Layout.rightMargin: ui.dp(16)
                implicitHeight: ocrCol.implicitHeight + ui.dp(32)
                radius: ui.dp(12); color: theme.surfaceColor

                ColumnLayout {
                    id: ocrCol
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: ui.dp(16) }
                    spacing: ui.dp(16)

                    Label { text: "OCR"; color: theme.accentColor; font.pixelSize: ui.dp(24); font.bold: true }

                    ColumnLayout {
                        Layout.fillWidth: true; spacing: ui.dp(6)
                        Label { text: "Background Mode"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                        DarkComboBox {
                            Layout.fillWidth: true; Layout.preferredHeight: ui.dp(40)
                            theme: root.theme; iconSource: expandMoreIcon
                            model: ["Auto-detect Colors", "Light Background", "Dark Background"]
                            Component.onCompleted: { var idx = find(appBridge.ocr_background_mode); if (idx >= 0) currentIndex = idx }
                            onActivated: appBridge.set_ocr_background_mode_value(currentText)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true; spacing: ui.dp(6)
                        Label { text: "Min Confidence: " + appBridge.ocr_min_confidence + "%"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                        DarkSlider {
                            Layout.fillWidth: true; Layout.preferredHeight: ui.dp(28)
                            theme: root.theme
                            from: 0; to: 100; stepSize: 5
                            value: appBridge.ocr_min_confidence
                            onMoved: appBridge.set_ocr_min_confidence_value(value)
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true; spacing: ui.dp(6)
                        Label { text: "Max Image Size: " + appBridge.ocr_max_image_size + "px"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                        DarkSlider {
                            Layout.fillWidth: true; Layout.preferredHeight: ui.dp(28)
                            theme: root.theme
                            from: 700; to: 1400; stepSize: 100
                            value: appBridge.ocr_max_image_size
                            onMoved: appBridge.set_ocr_max_image_size_value(value)
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: ui.dp(16) }

            // ── API Server ──
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: ui.dp(16); Layout.rightMargin: ui.dp(16)
                implicitHeight: apiCol.implicitHeight + ui.dp(32)
                radius: ui.dp(12); color: theme.surfaceColor

                ColumnLayout {
                    id: apiCol
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: ui.dp(16) }
                    spacing: ui.dp(16)

                    Label { text: "API Server"; color: theme.accentColor; font.pixelSize: ui.dp(24); font.bold: true }

                    DarkSwitch {
                        Layout.fillWidth: true; theme: root.theme
                        label: "Enable LibreTranslate compatible HTTP server"
                        checked: appBridge.http_server_enabled
                        onToggled: appBridge.set_http_server_enabled_value(checked)
                    }
                    Label {
                        Layout.fillWidth: true; wrapMode: Text.WordWrap
                        text: "Allows other apps to programmatically request translations, and serves a web page for translating from a browser"
                        color: theme.textSecondary; font.pixelSize: ui.dp(15)
                    }

                    ColumnLayout {
                        visible: appBridge.http_server_enabled
                        Layout.fillWidth: true
                        spacing: ui.dp(16)

                        ColumnLayout {
                            Layout.fillWidth: true; spacing: ui.dp(6)
                            Label { text: "Port"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                            TextField {
                                Layout.fillWidth: true
                                text: appBridge.http_server_port
                                inputMethodHints: Qt.ImhDigitsOnly
                                validator: IntValidator { bottom: 1; top: 65535 }
                                color: theme.textPrimary
                                placeholderTextColor: theme.textSecondary
                                font.pixelSize: ui.dp(19)
                                onEditingFinished: appBridge.set_http_server_port_value(parseInt(text))
                                background: Rectangle { radius: ui.dp(8); color: theme.backgroundElevated; border.width: 1; border.color: theme.borderColor }
                            }
                        }

                        DarkSwitch {
                            Layout.fillWidth: true; theme: root.theme
                            label: "Accept connections from other devices"
                            checked: appBridge.http_server_bind_all
                            onToggled: appBridge.set_http_server_bind_all_value(checked)
                        }

                        Label {
                            Layout.fillWidth: true; wrapMode: Text.WordWrap
                            visible: text.length > 0
                            text: appBridge.http_server_status
                            color: theme.textSecondary; font.pixelSize: ui.dp(15)
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: ui.dp(16) }

            // ── Advanced Settings ──
            Rectangle {
                Layout.fillWidth: true
                Layout.leftMargin: ui.dp(16); Layout.rightMargin: ui.dp(16)
                implicitHeight: advCol.implicitHeight + ui.dp(32)
                radius: ui.dp(12); color: theme.surfaceColor

                ColumnLayout {
                    id: advCol
                    anchors { left: parent.left; right: parent.right; top: parent.top; margins: ui.dp(16) }
                    spacing: ui.dp(16)

                    Item {
                        Layout.fillWidth: true; implicitHeight: ui.dp(28)

                        Label {
                            anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
                            text: "Advanced Settings"; color: theme.accentColor; font.pixelSize: ui.dp(24); font.bold: true
                        }
                        Image {
                            anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                            width: ui.dp(20); height: ui.dp(20)
                            source: advancedExpanded ? appBridge.asset_url("expand_less.svg") : expandMoreIcon
                            sourceSize.width: ui.dp(20); sourceSize.height: ui.dp(20)
                        }
                        MouseArea { anchors.fill: parent; onClicked: advancedExpanded = !advancedExpanded }
                    }

                    ColumnLayout {
                        visible: advancedExpanded
                        Layout.fillWidth: true
                        spacing: ui.dp(16)

                        ColumnLayout {
                            Layout.fillWidth: true; spacing: ui.dp(6)
                            Label { text: "Catalog Index URL"; color: theme.textSecondary; font.pixelSize: ui.dp(17) }
                            TextField {
                                Layout.fillWidth: true
                                text: appBridge.catalog_index_url
                                color: theme.textPrimary
                                placeholderTextColor: theme.textSecondary
                                font.pixelSize: ui.dp(19)
                                onEditingFinished: appBridge.set_catalog_index_url_value(text)
                                background: Rectangle { radius: ui.dp(8); color: theme.backgroundElevated; border.width: 1; border.color: theme.borderColor }
                            }
                        }

                        DarkSwitch {
                            Layout.fillWidth: true; theme: root.theme
                            label: "Disable OCR"
                            checked: appBridge.disable_ocr
                            onToggled: appBridge.set_disable_ocr_value(checked)
                        }

                        DarkSwitch {
                            Layout.fillWidth: true; theme: root.theme
                            label: "Disable automatic language detection"
                            checked: appBridge.disable_auto_detect
                            onToggled: appBridge.set_disable_auto_detect_value(checked)
                        }

                        DarkSwitch {
                            Layout.fillWidth: true; theme: root.theme
                            label: "Show transliteration for output"
                            checked: appBridge.show_transliteration_output
                            onToggled: appBridge.set_show_transliteration_output_value(checked)
                        }

                        DarkSwitch {
                            Layout.fillWidth: true; theme: root.theme
                            label: "Show transliteration for input"
                            checked: appBridge.show_transliteration_input
                            onToggled: appBridge.set_show_transliteration_input_value(checked)
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: ui.dp(32) }
        }
    }
}
