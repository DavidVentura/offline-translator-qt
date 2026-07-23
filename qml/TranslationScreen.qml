import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: root
    property var appBridge
    property var theme
    property bool speechLongPressTriggered: false
    UiScale { id: ui; desktopMode: root.appBridge && root.appBridge.desktop_mode }
    readonly property real clipboardButtonSize: ui.dp(24)
    readonly property real clipboardIconSize: ui.dp(22)
    readonly property bool imageSelectionMode: appBridge.image_mode
                                              && appBridge.image_error.length === 0
    readonly property real imageOverlayButtonSize: appBridge.desktop_mode ? ui.dp(36) : ui.dp(40)
    readonly property real imageOverlayIconSize: appBridge.desktop_mode ? ui.dp(18) : ui.dp(20)
    readonly property real fullscreenOverlayButtonSize: ui.dp(40)
    readonly property real fullscreenOverlayIconSize: ui.dp(20)

    function shareCurrentImage() {
        if (imageShareLoader.item) {
            imageShareLoader.item.share(appBridge.share_image_url || appBridge.selected_image_url)
        }
    }

    function isLookupWordChar(ch) {
        return /[0-9A-Za-zÀ-ÖØ-öø-ÿĀ-ɏͰ-ϿЀ-ӿא-ת؀-ۿݐ-ݿऀ-ॿ়-৾਀-੿઀-૿଀-୿ா-௿ఀ-౿ಀ-೿ം-ൿก-๿ກ-໿ༀ-࿿က-႟Ḁ-ỿⰀ-⳿々-〇ぁ-ヿ一-鿿가-힯'\-]/.test(ch)
    }

    function wordAt(text, index) {
        if (index < 0 || index >= text.length) {
            return ""
        }
        if (!isLookupWordChar(text.charAt(index))) {
            return ""
        }
        var start = index
        var end = index + 1
        while (start > 0 && isLookupWordChar(text.charAt(start - 1))) start--
        while (end < text.length && isLookupWordChar(text.charAt(end))) end++
        return text.slice(start, end)
    }

    Loader {
        id: imagePickerLoader
        active: true
        parent: appBridge.desktop_mode ? root : Overlay.overlay
        anchors.fill: parent
        z: 30
        source: appBridge.desktop_mode ? "DesktopImagePicker.qml" : "UbportsImagePicker.qml"

        onLoaded: {
            if (item) {
                item.appBridge = appBridge
            }
        }
    }

    Loader {
        id: imageShareLoader
        active: true
        anchors.fill: parent
        z: 40
        source: appBridge.desktop_mode ? "DesktopImageShare.qml" : "UbportsImageShare.qml"

        onLoaded: {
            if (item) {
                item.appBridge = appBridge
            }
        }
    }

    Loader {
        id: documentShareLoader
        active: !appBridge.desktop_mode
        parent: Overlay.overlay
        anchors.fill: parent
        z: 41
        source: "UbportsDocumentShare.qml"

        onLoaded: {
            if (item) {
                item.appBridge = appBridge
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.leftMargin: ui.dp(16)
        anchors.rightMargin: ui.dp(16)
        anchors.topMargin: 0
        anchors.bottomMargin: ui.dp(16)
        spacing: ui.dp(12)

        ScrollView {
            id: inputScroll
            visible: !appBridge.image_mode
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredHeight: Math.max(ui.dp(180), root.height * 0.34)
            Layout.minimumHeight: ui.dp(120)
            clip: true
            contentWidth: availableWidth
            contentHeight: inputPane.height
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            Rectangle {
                id: inputPane
                width: inputScroll.availableWidth
                height: Math.max(inputScroll.availableHeight, inputContent.height + ui.dp(24))
                color: theme.backgroundColor
                border.color: theme.borderColor
                border.width: 1

                TapHandler {
                    onTapped: inputArea.forceActiveFocus()
                }

                Item {
                    id: inputContent
                    x: ui.dp(12)
                    y: ui.dp(12)
                    width: Math.max(0, parent.width - ui.dp(24))
                    height: inputArea.height + (inputTransliteration.visible ? ui.dp(6) + inputTransliteration.implicitHeight : 0)

                    TextArea {
                        id: inputArea
                        width: Math.max(0, parent.width - (inputActionButton.visible ? root.clipboardButtonSize + ui.dp(12) : 0))
                        height: Math.max(ui.dp(28), contentHeight + topPadding + bottomPadding)
                        topInset: 0
                        leftInset: 0
                        rightInset: 0
                        bottomInset: 0
                        topPadding: 0
                        leftPadding: 0
                        rightPadding: 0
                        bottomPadding: 0
                        text: appBridge.input_text
                        color: theme.textPrimary
                        font.pointSize: ui.pt(16)
                        wrapMode: TextEdit.Wrap
                        verticalAlignment: TextEdit.AlignTop
                        activeFocusOnPress: true
                        selectByMouse: appBridge.desktop_mode
                        background: Item {}
                        onTextChanged: if (text !== appBridge.input_text) appBridge.process_text(text)
                    }

                    PlaceholderText {
                        target: inputArea
                        placeholderText: "Enter text"
                        placeholderColor: theme.textSecondary
                    }

                    Text {
                        id: inputTransliteration
                        visible: appBridge.input_transliteration.length > 0
                        y: inputArea.height + (visible ? ui.dp(6) : 0)
                        width: parent.width
                        text: appBridge.input_transliteration
                        wrapMode: Text.Wrap
                        color: theme.textSecondary
                        font.pointSize: ui.pt(13)
                    }
                }

                FeedbackIconButton {
                    id: inputActionButton
                    visible: !appBridge.image_mode
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.topMargin: ui.dp(12)
                    anchors.rightMargin: ui.dp(12)
                    width: root.clipboardButtonSize
                    height: root.clipboardButtonSize
                    iconSize: root.clipboardIconSize
                    iconSource: appBridge.asset_url(inputArea.text.length > 0 ? "clear.svg" : "paste.svg")

                    onClicked: {
                        inputArea.forceActiveFocus()
                        if (inputArea.text.length > 0) {
                            inputArea.text = ""
                        } else {
                            inputArea.paste()
                        }
                    }
                }
            }
        }

        Item {
            visible: appBridge.image_mode
            Layout.fillWidth: true
            Layout.fillHeight: root.imageSelectionMode
            Layout.preferredHeight: root.imageSelectionMode
                                    ? Math.max(ui.dp(320), root.height * 0.72)
                                    : Math.min(ui.dp(380), Math.max(ui.dp(220), root.height * 0.42))
            // Clipped, but with no frame of its own: the picture is the surface, not a card on it.
            clip: true

            TranslatedImageView {
                id: inlineImageView
                anchors.fill: parent
                appBridge: root.appBridge
                imageMargin: 0
                interactive: true
            }

            // Floating bar over the selection, mirroring the phone app's Copy / Share / Web search.
            Rectangle {
                id: selectionActions
                visible: root.imageSelectionMode && appBridge.selection_active
                z: 5
                radius: height / 2
                color: theme.backgroundElevated
                border.color: theme.borderColor
                border.width: 1
                width: selectionActionRow.width + ui.dp(24)
                height: selectionActionRow.height + ui.dp(12)
                x: Math.max(ui.dp(8),
                            Math.min(parent.width - width - ui.dp(8),
                                     inlineImageView.mapX((appBridge.selection_left + appBridge.selection_right) / 2) - width / 2))
                y: Math.max(ui.dp(8),
                            inlineImageView.mapY(appBridge.selection_top) - height - ui.dp(10))

                Row {
                    id: selectionActionRow
                    anchors.centerIn: parent
                    spacing: ui.dp(18)

                    Text {
                        text: qsTr("Copy")
                        color: theme.textPrimary
                        font.pixelSize: ui.dp(14)
                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -ui.dp(6)
                            onClicked: {
                                inlineImageView.copySelection()
                                inlineImageView.clearSelection()
                            }
                        }
                    }

                    Text {
                        text: qsTr("Clear")
                        color: theme.textSecondary
                        font.pixelSize: ui.dp(14)
                        MouseArea {
                            anchors.fill: parent
                            anchors.margins: -ui.dp(6)
                            onClicked: inlineImageView.clearSelection()
                        }
                    }
                }
            }

            Row {
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.margins: ui.dp(12)
                spacing: ui.dp(8)

                Rectangle {
                    visible: root.imageSelectionMode
                    width: root.imageOverlayButtonSize
                    height: root.imageOverlayButtonSize
                    radius: width / 2
                    color: flipOverlayMouse.pressed ? "#99000000"
                                                    : (appBridge.image_show_original ? "#B33B82F6" : "#80000000")

                    Image {
                        anchors.centerIn: parent
                        width: root.imageOverlayIconSize
                        height: root.imageOverlayIconSize
                        source: appBridge.asset_url("swap.svg")
                        sourceSize.width: root.imageOverlayIconSize
                        sourceSize.height: root.imageOverlayIconSize
                    }

                    MouseArea {
                        id: flipOverlayMouse
                        anchors.fill: parent
                        onClicked: appBridge.toggle_image_original()
                    }
                }

                Rectangle {
                    width: root.imageOverlayButtonSize
                    height: root.imageOverlayButtonSize
                    radius: width / 2
                    color: shareOverlayMouse.pressed ? "#99000000" : "#80000000"

                    Image {
                        anchors.centerIn: parent
                        width: root.imageOverlayIconSize
                        height: root.imageOverlayIconSize
                        source: appBridge.asset_url("share.svg")
                        sourceSize.width: root.imageOverlayIconSize
                        sourceSize.height: root.imageOverlayIconSize
                    }

                    MouseArea {
                        id: shareOverlayMouse
                        anchors.fill: parent
                        onClicked: root.shareCurrentImage()
                    }
                }

                Rectangle {
                    width: root.imageOverlayButtonSize
                    height: root.imageOverlayButtonSize
                    radius: width / 2
                    color: closeOverlayMouse.pressed ? "#99000000" : "#80000000"

                    Image {
                        anchors.centerIn: parent
                        width: root.imageOverlayIconSize
                        height: root.imageOverlayIconSize
                        source: appBridge.asset_url("close.svg")
                        sourceSize.width: root.imageOverlayIconSize
                        sourceSize.height: root.imageOverlayIconSize
                    }

                    MouseArea {
                        id: closeOverlayMouse
                        anchors.fill: parent
                        onClicked: appBridge.clear_selected_image()
                    }
                }
            }
        }

        Rectangle {
            visible: appBridge.show_missing_card
            Layout.fillWidth: true
            Layout.topMargin: ui.dp(4)
            Layout.bottomMargin: ui.dp(4)
            color: theme.surfaceColor
            radius: ui.dp(8)
            implicitHeight: ui.dp(60)

            Column {
                anchors.left: parent.left
                anchors.leftMargin: ui.dp(16)
                anchors.verticalCenter: parent.verticalCenter
                spacing: ui.dp(2)

                Label {
                    text: "Translate from"
                    color: theme.textSecondary
                    font.pointSize: ui.pt(13)
                }

                Label {
                    text: appBridge.detected_language_name
                    color: theme.textPrimary
                    font.pointSize: ui.pt(16)
                    font.bold: true
                }
            }

            CircularProgress {
                visible: appBridge.detected_language_progress > 0 && appBridge.detected_language_progress < 1
                anchors.right: parent.right
                anchors.rightMargin: ui.dp(16)
                anchors.verticalCenter: parent.verticalCenter
                progress: appBridge.detected_language_progress
                progressColor: theme.accentColor
            }

            Item {
                visible: appBridge.detected_language_progress <= 0 || appBridge.detected_language_progress >= 1
                anchors.right: parent.right
                anchors.rightMargin: ui.dp(8)
                anchors.verticalCenter: parent.verticalCenter
                width: ui.dp(40); height: ui.dp(40)

                Image {
                    anchors.centerIn: parent
                    width: ui.dp(24); height: ui.dp(24)
                    source: appBridge.asset_url(appBridge.detected_language_installed ? "forward.svg" : "download.svg")
                    sourceSize.width: ui.dp(24); sourceSize.height: ui.dp(24)
                }

                MouseArea {
                    anchors.fill: parent
                    onClicked: appBridge.missing_language_action()
                }
            }
        }

        Rectangle {
            visible: !appBridge.show_missing_card && !root.imageSelectionMode
            Layout.fillWidth: true
            color: "transparent"
            implicitHeight: ui.dp(8)

            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width / 2
                height: ui.dp(4)
                radius: ui.dp(2)
                color: theme.borderColor
            }
        }

        Item {
            visible: !root.imageSelectionMode
            Layout.fillWidth: true
            Layout.fillHeight: !root.imageSelectionMode
            Layout.preferredHeight: Math.max(ui.dp(180), root.height * 0.34)
            Layout.minimumHeight: ui.dp(140)

            ScrollView {
                id: outputScroll
                anchors.fill: parent
                clip: true
                contentWidth: availableWidth
                contentHeight: outputPane.height
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                Rectangle {
                    id: outputPane
                    width: outputScroll.availableWidth
                    height: Math.max(outputScroll.availableHeight, outputContent.height + ui.dp(24))
                    color: theme.backgroundColor
                    border.color: theme.borderColor
                    border.width: 1

                    Item {
                        id: outputContent
                        x: ui.dp(12)
                        y: ui.dp(12)
                        width: Math.max(0, parent.width - ui.dp(24))
                        height: outputArea.height + (outputTransliteration.visible ? ui.dp(6) + outputTransliteration.implicitHeight : 0)

                        TextArea {
                            id: outputArea
                            width: Math.max(0, parent.width - ((copyButton.visible || speechButton.visible) ? root.clipboardButtonSize + ui.dp(12) : 0))
                            height: Math.max(ui.dp(28), contentHeight + topPadding + bottomPadding)
                            topInset: 0
                            leftInset: 0
                            rightInset: 0
                            bottomInset: 0
                            topPadding: 0
                            leftPadding: 0
                            rightPadding: 0
                            bottomPadding: 0
                            text: appBridge.output_text
                            readOnly: true
                            wrapMode: TextEdit.Wrap
                            activeFocusOnPress: appBridge.desktop_mode
                            selectByMouse: appBridge.desktop_mode
                            color: theme.textPrimary
                            font.pointSize: ui.pt(16)
                            background: Item {}
                        }

                        MouseArea {
                            id: lookupOutputArea
                            property bool holdTriggered: false
                            anchors.fill: outputArea
                            acceptedButtons: Qt.LeftButton
                            pressAndHoldInterval: 450
                            z: 2

                            onPressed: holdTriggered = false
                            onCanceled: holdTriggered = false
                            onReleased: {
                                if (!holdTriggered) {
                                    mouse.accepted = false
                                }
                            }
                            onClicked: {
                                if (!holdTriggered) {
                                    mouse.accepted = false
                                }
                            }
                            onDoubleClicked: mouse.accepted = false
                            onPositionChanged: if (!pressed) mouse.accepted = false
                            onPressAndHold: {
                                holdTriggered = true
                                const index = outputArea.positionAt(mouse.x, mouse.y)
                                const word = root.wordAt(outputArea.text, index)
                                if (word.length > 0) {
                                    appBridge.lookup_output_dictionary(word)
                                }
                            }
                        }

                        Text {
                            id: outputTransliteration
                            visible: appBridge.output_transliteration.length > 0
                            y: outputArea.height + (visible ? ui.dp(6) : 0)
                            width: parent.width
                            text: appBridge.output_transliteration
                            wrapMode: Text.Wrap
                            color: theme.textSecondary
                            font.pointSize: ui.pt(13)
                        }
                    }
                }
            }

            FeedbackIconButton {
                id: copyButton
                visible: appBridge.output_text.length > 0
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.topMargin: ui.dp(12)
                anchors.rightMargin: ui.dp(12)
                width: root.clipboardButtonSize
                height: root.clipboardButtonSize
                iconSize: root.clipboardIconSize
                iconSource: appBridge.asset_url("copy.svg")

                onClicked: {
                    outputArea.selectAll()
                    outputArea.copy()
                    outputArea.deselect()
                }
            }

            Item {
                id: speechButton
                visible: (appBridge.tts_available || appBridge.tts_loading || appBridge.tts_playing)
                         && appBridge.output_text.length > 0
                anchors.top: copyButton.visible ? copyButton.bottom : parent.top
                anchors.right: parent.right
                anchors.topMargin: copyButton.visible ? ui.dp(8) : ui.dp(12)
                anchors.rightMargin: ui.dp(12)
                width: ui.dp(24)
                height: ui.dp(24)

                Image {
                    anchors.centerIn: parent
                    width: ui.dp(22)
                    height: ui.dp(22)
                    source: appBridge.asset_url((appBridge.tts_loading || appBridge.tts_playing) ? "close.svg" : "tts.svg")
                    sourceSize.width: ui.dp(22)
                    sourceSize.height: ui.dp(22)
                }

                MouseArea {
                    anchors.fill: parent
                    pressAndHoldInterval: 450
                    onPressed: root.speechLongPressTriggered = false
                    onPressAndHold: {
                        root.speechLongPressTriggered = true
                        appBridge.prepare_tts_options()
                        speechOptionsPopup.open()
                    }
                    onClicked: {
                        if (root.speechLongPressTriggered) {
                            root.speechLongPressTriggered = false
                            return
                        }
                        appBridge.toggle_speak_output()
                    }
                }
            }

            Popup {
                id: speechOptionsPopup
                property bool voicePickerExpanded: false
                x: Math.max(0, speechButton.x - width + speechButton.width)
                y: speechButton.y + speechButton.height + ui.dp(8)
                width: Math.min(ui.dp(220), parent.width - ui.dp(24))
                height: popupContent.implicitHeight
                contentWidth: availableWidth
                modal: false
                closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
                padding: 0
                onClosed: voicePickerExpanded = false

                background: Rectangle {
                    radius: ui.dp(8)
                    color: theme.surfaceColor
                    border.color: theme.borderColor
                    border.width: 1
                }

                contentItem: Item {
                    id: popupContent
                    implicitHeight: popupColumn.implicitHeight + ui.dp(24)

                    Column {
                        id: popupColumn
                        x: ui.dp(12)
                        y: ui.dp(12)
                        width: Math.max(0, parent.width - ui.dp(24))
                        spacing: ui.dp(12)

                        Label {
                            text: "Playback speed"
                            color: theme.textPrimary
                            font.pointSize: ui.pt(16)
                            font.bold: true
                        }

                        Row {
                            width: parent.width
                            spacing: ui.dp(10)

                            Rectangle {
                                width: ui.dp(28)
                                height: ui.dp(28)
                                radius: ui.dp(8)
                                color: theme.backgroundElevated

                                Label {
                                    anchors.centerIn: parent
                                    text: "-"
                                    color: theme.textPrimary
                                    font.pointSize: ui.pt(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: appBridge.set_tts_playback_speed_value(appBridge.tts_playback_speed - 0.1)
                                }
                            }

                            Label {
                                width: parent.width - ui.dp(76)
                                horizontalAlignment: Text.AlignHCenter
                                verticalAlignment: Text.AlignVCenter
                                text: appBridge.tts_playback_speed.toFixed(2) + "x"
                                color: theme.textPrimary
                                font.pointSize: ui.pt(16)
                            }

                            Rectangle {
                                width: ui.dp(28)
                                height: ui.dp(28)
                                radius: ui.dp(8)
                                color: theme.backgroundElevated

                                Label {
                                    anchors.centerIn: parent
                                    text: "+"
                                    color: theme.textPrimary
                                    font.pointSize: ui.pt(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: appBridge.set_tts_playback_speed_value(appBridge.tts_playback_speed + 0.1)
                                }
                            }
                        }

                        Rectangle {
                            width: parent.width
                            height: 1
                            color: theme.borderColor
                            opacity: 0.7
                        }

                        Label {
                            text: "Voice"
                            color: theme.textPrimary
                            font.pointSize: ui.pt(16)
                            font.bold: true
                        }

                        Column {
                            width: parent.width
                            spacing: ui.dp(6)

                            Rectangle {
                                width: parent.width
                                height: ui.dp(40)
                                radius: ui.dp(8)
                                color: theme.backgroundElevated
                                border.color: theme.borderColor
                                border.width: 1

                                Label {
                                    anchors.left: parent.left
                                    anchors.leftMargin: ui.dp(12)
                                    anchors.right: voiceExpandIndicator.left
                                    anchors.rightMargin: ui.dp(8)
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: appBridge.tts_selected_voice_display_name
                                    color: theme.textPrimary
                                    verticalAlignment: Text.AlignVCenter
                                    elide: Text.ElideRight
                                    font.pointSize: ui.pt(15)
                                }

                                Image {
                                    id: voiceExpandIndicator
                                    anchors.right: parent.right
                                    anchors.rightMargin: ui.dp(10)
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: appBridge.tts_voice_option_count > 1
                                    source: appBridge.asset_url("expand_more.svg")
                                    width: ui.dp(18)
                                    height: ui.dp(18)
                                    rotation: speechOptionsPopup.voicePickerExpanded ? 180 : 0
                                    sourceSize.width: ui.dp(18)
                                    sourceSize.height: ui.dp(18)
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    enabled: appBridge.tts_voice_option_count > 1
                                    onClicked: speechOptionsPopup.voicePickerExpanded = !speechOptionsPopup.voicePickerExpanded
                                }
                            }

                            Rectangle {
                                visible: speechOptionsPopup.voicePickerExpanded && appBridge.tts_voice_option_count > 1
                                width: parent.width
                                height: visible ? Math.min(ui.dp(222), voiceListView.contentHeight + ui.dp(2)) : 0
                                radius: ui.dp(8)
                                color: theme.surfaceColor
                                border.color: theme.borderColor
                                border.width: 1
                                clip: true

                                ListView {
                                    id: voiceListView
                                    anchors.fill: parent
                                    anchors.margins: ui.dp(1)
                                    clip: true
                                    model: speechOptionsPopup.voicePickerExpanded ? appBridge.tts_voice_options_model : null

                                    ScrollIndicator.vertical: ScrollIndicator { }

                                    delegate: ItemDelegate {
                                        required property string name
                                        required property string display_name

                                        width: voiceListView.width
                                        text: display_name
                                        highlighted: appBridge.tts_selected_voice_name === name
                                        onClicked: {
                                            appBridge.set_tts_voice_name(name)
                                            speechOptionsPopup.voicePickerExpanded = false
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    RoundButton {
        visible: !appBridge.disable_ocr
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: ui.dp(24)
        width: ui.dp(64)
        height: ui.dp(64)
        display: AbstractButton.IconOnly
        icon.source: appBridge.asset_url("attach_file.svg")
        icon.width: ui.dp(28)
        icon.height: ui.dp(28)
        text: "Attach"
        background: Rectangle {
            radius: width / 2
            color: parent.down ? Qt.darker(theme.accentColor, 1.15) : theme.accentColor
            border.width: 0
        }
        onClicked: if (imagePickerLoader.item) imagePickerLoader.item.open()
    }

    RoundButton {
        visible: !appBridge.disable_ocr
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.rightMargin: ui.dp(24)
        anchors.bottomMargin: ui.dp(100)
        width: ui.dp(64)
        height: ui.dp(64)
        display: AbstractButton.IconOnly
        icon.source: appBridge.asset_url("camera.svg")
        icon.width: ui.dp(28)
        icon.height: ui.dp(28)
        text: "Live"
        onClicked: appBridge.open_live_camera()
        background: Rectangle {
            radius: width / 2
            color: parent.down ? Qt.darker(theme.accentColor, 1.15) : theme.accentColor
            border.width: 0
        }
    }

    // Document-translate drawer: file info + options + start, shown after
    // picking a pdf/epub/odt/txt.
    MouseArea {
        visible: appBridge.doc_drawer_open
        anchors.fill: parent
        z: 15
        onClicked: appBridge.close_document_drawer()

        Rectangle {
            anchors.fill: parent
            color: "#80000000"
        }
    }

    Rectangle {
        id: documentDrawer
        visible: appBridge.doc_drawer_open
        z: 16
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: drawerColumn.implicitHeight + ui.dp(40)
        radius: ui.dp(16)
        color: "#222530"
        border.color: theme.borderColor
        border.width: 1

        // Square off the bottom corners.
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
            id: drawerColumn
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: ui.dp(20)
            spacing: ui.dp(14)

            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                width: ui.dp(36)
                height: ui.dp(4)
                radius: height / 2
                color: theme.surfaceAltColor
            }

            Column {
                width: parent.width
                spacing: ui.dp(2)

                Label {
                    width: parent.width
                    text: appBridge.doc_file_name
                    color: theme.textPrimary
                    font.pixelSize: ui.dp(16)
                    elide: Text.ElideMiddle
                }
                Label {
                    text: appBridge.doc_file_size
                    color: theme.textSecondary
                    font.pixelSize: ui.dp(13)
                }
            }

            DarkSwitch {
                visible: appBridge.doc_is_pdf
                width: parent.width
                label: "Translate images in PDF"
                theme: root.theme
                desktopMode: appBridge.desktop_mode
                checked: appBridge.doc_translate_images
                onToggled: appBridge.doc_translate_images = checked
            }

            Button {
                width: parent.width
                height: ui.dp(48)
                text: "Translate"
                font.pixelSize: ui.dp(16)
                onClicked: appBridge.start_document_translation()
                contentItem: Label {
                    text: parent.text
                    font: parent.font
                    color: "#1E1E2E"
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                }
                background: Rectangle {
                    radius: height / 2
                    color: parent.down ? Qt.darker(theme.accentColor, 1.15) : theme.accentColor
                }
            }
        }
    }

    // Progress modal while a document translation runs.
    Rectangle {
        visible: appBridge.doc_progress_open
        anchors.fill: parent
        z: 17
        color: "#80000000"

        MouseArea {
            anchors.fill: parent
        }

        Rectangle {
            anchors.centerIn: parent
            width: Math.min(parent.width - ui.dp(48), ui.dp(360))
            height: progressColumn.implicitHeight + ui.dp(40)
            radius: ui.dp(16)
            color: "#222530"
            border.color: theme.borderColor
            border.width: 1

            Column {
                id: progressColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: ui.dp(20)
                spacing: ui.dp(12)

                Item {
                    width: parent.width
                    height: ui.dp(28)

                    Label {
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        text: appBridge.doc_progress_label
                        color: theme.textPrimary
                        font.pixelSize: ui.dp(19)
                    }

                    FeedbackIconButton {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: ui.dp(24)
                        height: ui.dp(24)
                        iconSize: ui.dp(22)
                        iconSource: appBridge.asset_url("close.svg")
                        onClicked: appBridge.cancel_document_translation()
                    }
                }

                Label {
                    width: parent.width
                    text: appBridge.doc_file_name
                    color: theme.textSecondary
                    font.pixelSize: ui.dp(14)
                    elide: Text.ElideMiddle
                }

                Column {
                    width: parent.width
                    spacing: ui.dp(4)

                    Item {
                        width: parent.width
                        height: ui.dp(18)
                        Label {
                            anchors.left: parent.left
                            text: "Text"
                            color: theme.textPrimary
                            font.pixelSize: ui.dp(14)
                        }
                        Label {
                            anchors.right: parent.right
                            text: Math.round(appBridge.doc_text_fraction * 100) + "%"
                            color: theme.textSecondary
                            font.pixelSize: ui.dp(14)
                        }
                    }
                    Rectangle {
                        width: parent.width
                        height: ui.dp(4)
                        radius: height / 2
                        color: theme.surfaceAltColor
                        Rectangle {
                            width: parent.width * Math.min(1, appBridge.doc_text_fraction)
                            height: parent.height
                            radius: parent.radius
                            color: theme.accentColor
                        }
                    }
                }

                Column {
                    visible: appBridge.doc_show_pdf_phases
                    width: parent.width
                    spacing: ui.dp(4)

                    Item {
                        width: parent.width
                        height: ui.dp(18)
                        Label {
                            anchors.left: parent.left
                            text: "Images"
                            color: theme.textPrimary
                            font.pixelSize: ui.dp(14)
                        }
                        Label {
                            anchors.right: parent.right
                            text: appBridge.doc_images_current + "/" + appBridge.doc_images_total
                            color: theme.textSecondary
                            font.pixelSize: ui.dp(14)
                        }
                    }
                    Rectangle {
                        width: parent.width
                        height: ui.dp(4)
                        radius: height / 2
                        color: theme.surfaceAltColor
                        Rectangle {
                            width: appBridge.doc_images_total > 0
                                   ? parent.width * Math.min(1, appBridge.doc_images_current / appBridge.doc_images_total)
                                   : 0
                            height: parent.height
                            radius: parent.radius
                            color: theme.accentColor
                        }
                    }
                }

                Column {
                    visible: appBridge.doc_show_pdf_phases
                    width: parent.width
                    spacing: ui.dp(4)

                    Item {
                        width: parent.width
                        height: ui.dp(18)
                        Label {
                            anchors.left: parent.left
                            text: "Bitmap pages"
                            color: theme.textPrimary
                            font.pixelSize: ui.dp(14)
                        }
                        Label {
                            anchors.right: parent.right
                            text: appBridge.doc_raster_current + "/" + appBridge.doc_raster_total
                            color: theme.textSecondary
                            font.pixelSize: ui.dp(14)
                        }
                    }
                    Rectangle {
                        width: parent.width
                        height: ui.dp(4)
                        radius: height / 2
                        color: theme.surfaceAltColor
                        Rectangle {
                            width: appBridge.doc_raster_total > 0
                                   ? parent.width * Math.min(1, appBridge.doc_raster_current / appBridge.doc_raster_total)
                                   : 0
                            height: parent.height
                            radius: parent.radius
                            color: theme.accentColor
                        }
                    }
                }
            }
        }
    }

    // Done / error modal after a document translation finishes.
    Rectangle {
        visible: appBridge.doc_done_open
        anchors.fill: parent
        z: 18
        color: "#80000000"

        MouseArea {
            anchors.fill: parent
            onClicked: appBridge.close_document_done()
        }

        Rectangle {
            anchors.centerIn: parent
            width: Math.min(parent.width - ui.dp(48), ui.dp(360))
            height: doneColumn.implicitHeight + ui.dp(40)
            radius: ui.dp(16)
            color: "#222530"
            border.color: theme.borderColor
            border.width: 1

            MouseArea {
                anchors.fill: parent
            }

            Column {
                id: doneColumn
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: ui.dp(20)
                spacing: ui.dp(12)

                Label {
                    text: appBridge.doc_error.length > 0 ? "Translation failed" : "Translated file"
                    color: theme.textPrimary
                    font.pixelSize: ui.dp(19)
                }

                Label {
                    width: parent.width
                    text: appBridge.doc_file_name
                    color: theme.textSecondary
                    font.pixelSize: ui.dp(14)
                    elide: Text.ElideMiddle
                }

                Label {
                    visible: appBridge.doc_error.length === 0
                    width: parent.width
                    text: "Translated file:\n" + appBridge.doc_output_name
                    color: theme.textPrimary
                    font.pixelSize: ui.dp(14)
                    wrapMode: Text.Wrap
                }

                Label {
                    visible: appBridge.doc_error.length > 0
                    width: parent.width
                    text: appBridge.doc_error
                    color: theme.textPrimary
                    font.pixelSize: ui.dp(14)
                    wrapMode: Text.Wrap
                }

                // Desktop has no content-hub to share into, so hand the file to whatever the
                // system has registered for it.
                Label {
                    visible: appBridge.doc_error.length === 0 && appBridge.desktop_mode
                    anchors.right: parent.right
                    text: "Open"
                    color: theme.accentColor
                    font.pixelSize: ui.dp(15)
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -ui.dp(8)
                        onClicked: Qt.openUrlExternally(appBridge.doc_output_url)
                    }
                }

                Label {
                    visible: appBridge.doc_error.length === 0 && !appBridge.desktop_mode
                    anchors.right: parent.right
                    text: "Save"
                    color: theme.accentColor
                    font.pixelSize: ui.dp(15)
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -ui.dp(8)
                        onClicked: {
                            if (documentShareLoader.item) {
                                documentShareLoader.item.share(appBridge.doc_output_url)
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: toastBubble
        visible: appBridge.toast_visible && appBridge.toast_message.length > 0
        z: 14
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: ui.dp(24)
        width: Math.min(parent.width - ui.dp(32), toastLabel.implicitWidth + ui.dp(28))
        height: toastLabel.implicitHeight + ui.dp(18)
        radius: ui.dp(12)
        color: "#E6222530"
        border.color: theme.borderColor
        border.width: 1

        Label {
            id: toastLabel
            anchors.centerIn: parent
            width: Math.min(parent.width - ui.dp(28), implicitWidth)
            text: appBridge.toast_message
            color: theme.textPrimary
            wrapMode: Text.Wrap
            horizontalAlignment: Text.AlignHCenter
            font.pointSize: ui.pt(14)
        }

        MouseArea {
            anchors.fill: parent
            onClicked: appBridge.clear_toast()
        }

        Timer {
            id: toastTimer
            interval: 2200
            repeat: false
            onTriggered: appBridge.clear_toast()
        }

        Connections {
            target: appBridge

            function onToast_visible_changed() {
                if (appBridge.toast_visible) {
                    toastTimer.restart()
                } else {
                    toastTimer.stop()
                }
            }

            function onToast_message_changed() {
                if (appBridge.toast_visible) {
                    toastTimer.restart()
                }
            }
        }
    }

    DictionaryPopup {
        anchors.fill: parent
        appBridge: root.appBridge
        theme: root.theme
        z: 15
    }

    Rectangle {
        anchors.fill: parent
        visible: appBridge.image_viewer_open
        color: "#000000"
        z: 20

        TranslatedImageView {
            anchors.fill: parent
            appBridge: root.appBridge
            imageMargin: 0
            interactive: false
        }

        Rectangle {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.margins: ui.dp(16)
            width: root.fullscreenOverlayButtonSize
            height: root.fullscreenOverlayButtonSize
            radius: width / 2
            color: fullscreenBackMouse.pressed ? "#99000000" : "#80000000"

            Image {
                anchors.centerIn: parent
                width: root.fullscreenOverlayIconSize
                height: root.fullscreenOverlayIconSize
                source: appBridge.asset_url("back.svg")
                sourceSize.width: root.fullscreenOverlayIconSize
                sourceSize.height: root.fullscreenOverlayIconSize
            }

            MouseArea {
                id: fullscreenBackMouse
                anchors.fill: parent
                onClicked: appBridge.close_image_viewer()
            }
        }

        Rectangle {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: ui.dp(16)
            width: root.fullscreenOverlayButtonSize
            height: root.fullscreenOverlayButtonSize
            radius: width / 2
            color: fullscreenShareMouse.pressed ? "#99000000" : "#80000000"

            Image {
                anchors.centerIn: parent
                width: root.fullscreenOverlayIconSize
                height: root.fullscreenOverlayIconSize
                source: appBridge.asset_url("share.svg")
                sourceSize.width: root.fullscreenOverlayIconSize
                sourceSize.height: root.fullscreenOverlayIconSize
            }

            MouseArea {
                id: fullscreenShareMouse
                anchors.fill: parent
                onClicked: root.shareCurrentImage()
            }
        }
    }
}
