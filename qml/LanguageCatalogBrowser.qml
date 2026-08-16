import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    property var appBridge
    property var theme

    UiScale { id: ui }
    function actionIcon(installed) {
        return installed ? appBridge.asset_url("delete.svg") : appBridge.asset_url("download.svg")
    }

    function featureAction(code, feature, installed) {
        if (installed) {
            appBridge.delete_feature(code, feature)
        } else {
            appBridge.download_feature(code, feature)
        }
    }

    function handleTtsFeatureAction(code, installed) {
        if (installed) {
            appBridge.delete_feature(code, 2)
        } else {
            appBridge.open_tts_download_picker(code)
        }
    }

    function toggleLanguage(code) {
        appBridge.toggle_manage_language(code)
    }

    function isBusy(progress) {
        return progress > 0 && progress < 1
    }

    Connections {
        target: appBridge

        function onManage_tts_picker_openChanged() {
            if (appBridge.manage_tts_picker_open) {
                ttsPickerPopup.open()
            } else {
                ttsPickerPopup.close()
            }
        }
    }

    Popup {
        id: ttsPickerPopup
        parent: Overlay.overlay
        modal: true
        focus: true
        dim: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        x: Math.round((parent.width - width) / 2)
        y: Math.round((parent.height - height) / 2)
        width: Math.min(parent.width - ui.dp(24), ui.dp(420))
        height: Math.min(parent.height - ui.dp(80), ui.dp(520))
        padding: 0

        background: Rectangle {
            radius: ui.dp(28)
            color: "#2B2D36"
        }

        onClosed: {
            if (appBridge.manage_tts_picker_open) {
                appBridge.close_tts_download_picker()
            }
        }

        contentItem: ColumnLayout {
            spacing: 0

            Label {
                Layout.fillWidth: true
                Layout.topMargin: ui.dp(18)
                Layout.leftMargin: ui.dp(20)
                Layout.rightMargin: ui.dp(20)
                text: "Pick a voice"
                color: "white"
                font.pixelSize: ui.dp(29)
                font.bold: true
            }

            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.topMargin: ui.dp(14)
                Layout.leftMargin: ui.dp(14)
                Layout.rightMargin: ui.dp(14)
                clip: true
                spacing: ui.dp(2)
                model: appBridge.manage_tts_picker_model
                section.property: "region_display_name"
                section.criteria: ViewSection.FullString

                section.delegate: Label {
                    width: ListView.view.width
                    text: section
                    color: "#E4E6F2"
                    font.pixelSize: ui.dp(19)
                    font.bold: true
                    leftPadding: ui.dp(4)
                    topPadding: ui.dp(8)
                    bottomPadding: ui.dp(4)
                }

                delegate: Item {
                    required property string pack_id
                    required property string voice_display_name
                    required property string quality_text
                    required property string size_text
                    required property bool installed

                    width: ListView.view.width
                    height: ui.dp(46)

                    Column {
                        anchors.left: parent.left
                        anchors.leftMargin: ui.dp(12)
                        anchors.right: actionItem.left
                        anchors.rightMargin: ui.dp(10)
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: ui.dp(1)

                        Label {
                            width: parent.width
                            text: voice_display_name
                            color: installed ? "#8A8E9F" : "#F1F3FB"
                            font.pixelSize: ui.dp(21)
                            elide: Text.ElideRight
                        }

                        Label {
                            width: parent.width
                            text: size_text
                            color: "#A9ADBC"
                            font.pixelSize: ui.dp(17)
                            horizontalAlignment: Text.AlignLeft
                            elide: Text.ElideRight
                        }
                    }

                    Item {
                        id: actionItem
                        anchors.right: parent.right
                        anchors.rightMargin: ui.dp(10)
                        anchors.verticalCenter: parent.verticalCenter
                        width: installed ? ui.dp(62) : ui.dp(28)
                        height: ui.dp(28)

                        Label {
                            anchors.centerIn: parent
                            visible: installed
                            text: "Installed"
                            color: "#8A8E9F"
                            font.pixelSize: ui.dp(16)
                        }

                        Image {
                            anchors.centerIn: parent
                            visible: !installed
                            width: ui.dp(18)
                            height: ui.dp(18)
                            source: appBridge.asset_url("download.svg")
                            sourceSize.width: ui.dp(18)
                            sourceSize.height: ui.dp(18)
                        }

                        MouseArea {
                            anchors.fill: parent
                            enabled: !installed
                            onClicked: appBridge.download_tts_pack(pack_id)
                        }
                    }
                }
            }

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: ui.dp(64)

                Button {
                    anchors.right: parent.right
                    anchors.rightMargin: ui.dp(18)
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Cancel"
                    flat: true
                    onClicked: appBridge.close_tts_download_picker()

                    contentItem: Text {
                        text: parent.text
                        color: theme.accentColor
                        font.pixelSize: ui.dp(21)
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }

                    background: Item {}
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: ui.dp(10)

        TextField {
            Layout.fillWidth: true
            placeholderText: "Filter languages"
            text: appBridge.manage_filter_text
            color: theme.textPrimary
            placeholderTextColor: theme.textSecondary
            font.pixelSize: ui.dp(19)
            onTextChanged: appBridge.set_manage_filter(text)

            background: Rectangle {
                radius: ui.dp(4)
                color: "#181922"
                border.width: 1
                border.color: "#343646"
            }
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 0
            model: appBridge.manage_languages_model

            delegate: Item {
                required property string code
                required property string name
                required property string total_size
                required property bool built_in
                required property bool expanded
                required property bool core_available
                required property bool core_installed
                required property string core_size
                required property real core_progress
                required property bool dictionary_available
                required property bool dictionary_installed
                required property string dictionary_size
                required property real dictionary_progress
                required property bool tts_available
                required property bool tts_installed
                required property string tts_size
                required property real tts_progress

                readonly property int installedCount:
                    (core_available && !built_in && core_installed ? 1 : 0) +
                    (dictionary_available && dictionary_installed ? 1 : 0) +
                    (tts_available && tts_installed ? 1 : 0)
                readonly property int availableCount:
                    (core_available && !built_in ? 1 : 0) +
                    (dictionary_available ? 1 : 0) +
                    (tts_available ? 1 : 0)
                readonly property bool allInstalled: availableCount > 0 && installedCount === availableCount
                readonly property bool noneInstalled: installedCount === 0
                readonly property bool someInstalled: !allInstalled && !noneInstalled
                readonly property bool anyBusy:
                    isBusy(core_progress) || isBusy(dictionary_progress) || isBusy(tts_progress)

                width: ListView.view.width
                height: delegateLayout.implicitHeight

                ColumnLayout {
                    id: delegateLayout
                    width: parent.width
                    spacing: 0

                    Item {
                        Layout.fillWidth: true
                        implicitHeight: ui.dp(52)

                        MouseArea {
                            anchors.fill: parent
                            onClicked: toggleLanguage(code)
                        }

                        ToolButton {
                            id: chevronBtn
                            anchors.left: parent.left
                            anchors.leftMargin: ui.dp(4)
                            anchors.verticalCenter: parent.verticalCenter
                            z: 1
                            display: AbstractButton.IconOnly
                            icon.source: expanded ? appBridge.asset_url("expand_less.svg") : appBridge.asset_url("expand_more.svg")
                            icon.width: ui.dp(16); icon.height: ui.dp(16)
                            icon.color: theme.textSecondary
                            background: Item {}
                            onClicked: toggleLanguage(code)
                        }

                        Column {
                            anchors.left: chevronBtn.right
                            anchors.right: actionArea.left
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.rightMargin: ui.dp(8)
                            spacing: ui.dp(1)

                            Label {
                                text: name
                                width: parent.width
                                color: theme.textPrimary
                                font.pixelSize: ui.listPrimaryPx
                                font.bold: true
                                elide: Text.ElideRight
                            }

                            Label {
                                text: total_size
                                width: parent.width
                                color: theme.textSecondary
                                font.pixelSize: ui.listSecondaryPx
                                horizontalAlignment: Text.AlignLeft
                            }
                        }

                        Row {
                            id: actionArea
                            anchors.right: parent.right
                            anchors.rightMargin: ui.dp(12)
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: ui.dp(4)

                            // T D S icons: when expanded (always) or collapsed with partial install, and not busy
                            Row {
                                visible: (expanded || someInstalled) && !anyBusy
                                spacing: ui.dp(2)
                                anchors.verticalCenter: parent.verticalCenter

                                Image {
                                    width: ui.dp(20); height: ui.dp(20)
                                    source: appBridge.asset_url("translate.svg")
                                    sourceSize.width: ui.dp(20); sourceSize.height: ui.dp(20)
                                    opacity: (core_available && !built_in) ? (core_installed ? 1.0 : 0.3) : 0
                                }

                                Image {
                                    width: ui.dp(20); height: ui.dp(20)
                                    source: appBridge.asset_url("dictionary.svg")
                                    sourceSize.width: ui.dp(20); sourceSize.height: ui.dp(20)
                                    opacity: dictionary_available ? (dictionary_installed ? 1.0 : 0.3) : 0
                                }

                                Image {
                                    width: ui.dp(20); height: ui.dp(20)
                                    source: appBridge.asset_url("tts.svg")
                                    sourceSize.width: ui.dp(20); sourceSize.height: ui.dp(20)
                                    opacity: tts_available ? (tts_installed ? 1.0 : 0.3) : 0
                                }
                            }

                            // Download button: collapsed + nothing installed + not busy
                            Item {
                                visible: !expanded && noneInstalled && !anyBusy
                                width: ui.dp(24); height: ui.dp(24)
                                anchors.verticalCenter: parent.verticalCenter

                                Image {
                                    anchors.centerIn: parent
                                    width: ui.dp(20); height: ui.dp(20)
                                    source: appBridge.asset_url("download.svg")
                                    sourceSize.width: ui.dp(20); sourceSize.height: ui.dp(20)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    z: 1
                                    onClicked: appBridge.download_all_features(code)
                                }
                            }

                            // Delete button: collapsed + everything installed + not busy
                            FeedbackIconButton {
                                visible: !expanded && allInstalled && !anyBusy
                                width: ui.dp(24); height: ui.dp(24)
                                anchors.verticalCenter: parent.verticalCenter
                                iconSize: ui.dp(20)
                                iconSource: appBridge.asset_url("delete.svg")
                                onClicked: appBridge.delete_all_features(code)
                            }

                            // Indeterminate spinner: collapsed + any download active
                            CircularProgress {
                                visible: !expanded && anyBusy
                                indeterminate: true
                                progressColor: theme.accentColor
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        height: 1
                        color: "#2A2D3A"
                    }

                    ColumnLayout {
                        visible: expanded
                        Layout.fillWidth: true
                        Layout.leftMargin: ui.dp(20)
                        Layout.rightMargin: ui.dp(8)
                        Layout.bottomMargin: ui.dp(8)
                        spacing: ui.dp(2)

                        // Translation feature (hidden for built-in languages like English)
                        Item {
                            visible: core_available && !built_in
                            Layout.fillWidth: true
                            implicitHeight: ui.dp(28)

                            Label {
                                id: coreTitleLabel
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                text: "Translation"
                                color: theme.textPrimary
                                font.pixelSize: ui.sectionTitlePx
                            }

                            Label {
                                anchors.left: coreTitleLabel.right
                                anchors.leftMargin: ui.dp(8)
                                anchors.verticalCenter: parent.verticalCenter
                                text: core_size
                                color: theme.textSecondary
                                font.pixelSize: ui.listSecondaryPx
                                horizontalAlignment: Text.AlignLeft
                            }

                            // Circular progress when downloading
                            CircularProgress {
                                visible: isBusy(core_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                progress: core_progress
                                progressColor: theme.accentColor
                            }

                            // Action icon when not downloading
                            Item {
                                visible: !isBusy(core_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                width: ui.dp(24); height: ui.dp(24)

                                Image {
                                    anchors.centerIn: parent
                                    width: ui.dp(18); height: ui.dp(18)
                                    source: actionIcon(core_installed)
                                    sourceSize.width: ui.dp(18); sourceSize.height: ui.dp(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: featureAction(code, 0, core_installed)
                                }
                            }
                        }

                        // Dictionary feature
                        Item {
                            visible: dictionary_available
                            Layout.fillWidth: true
                            implicitHeight: ui.dp(28)

                            Label {
                                id: dictionaryTitleLabel
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                text: "Dictionary"
                                color: theme.textPrimary
                                font.pixelSize: ui.sectionTitlePx
                            }

                            Label {
                                anchors.left: dictionaryTitleLabel.right
                                anchors.leftMargin: ui.dp(8)
                                anchors.verticalCenter: parent.verticalCenter
                                text: dictionary_size
                                color: theme.textSecondary
                                font.pixelSize: ui.listSecondaryPx
                                horizontalAlignment: Text.AlignLeft
                            }

                            CircularProgress {
                                visible: isBusy(dictionary_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                progress: dictionary_progress
                                progressColor: theme.accentColor
                            }

                            Item {
                                visible: !isBusy(dictionary_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                width: ui.dp(24); height: ui.dp(24)

                                Image {
                                    anchors.centerIn: parent
                                    width: ui.dp(18); height: ui.dp(18)
                                    source: actionIcon(dictionary_installed)
                                    sourceSize.width: ui.dp(18); sourceSize.height: ui.dp(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: featureAction(code, 1, dictionary_installed)
                                }
                            }
                        }

                        // TTS feature
                        Item {
                            visible: tts_available
                            Layout.fillWidth: true
                            implicitHeight: ui.dp(28)

                            Label {
                                id: ttsTitleLabel
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                text: "Text-to-speech"
                                color: theme.textPrimary
                                font.pixelSize: ui.sectionTitlePx
                            }

                            Label {
                                anchors.left: ttsTitleLabel.right
                                anchors.leftMargin: ui.dp(8)
                                anchors.verticalCenter: parent.verticalCenter
                                text: tts_size
                                color: theme.textSecondary
                                font.pixelSize: ui.listSecondaryPx
                                horizontalAlignment: Text.AlignLeft
                            }

                            CircularProgress {
                                visible: isBusy(tts_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                progress: tts_progress
                                progressColor: theme.accentColor
                            }

                            Item {
                                visible: !isBusy(tts_progress)
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                width: ui.dp(24); height: ui.dp(24)

                                Image {
                                    anchors.centerIn: parent
                                    width: ui.dp(18); height: ui.dp(18)
                                    source: actionIcon(tts_installed)
                                    sourceSize.width: ui.dp(18); sourceSize.height: ui.dp(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: handleTtsFeatureAction(code, tts_installed)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
