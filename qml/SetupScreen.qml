import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Layouts 1.15

Item {
    id: root
    property var appBridge
    property var theme
    UiScale { id: ui }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: ui.dp(16)
        spacing: ui.dp(12)

        RowLayout {
            Layout.fillWidth: true

            Label {
                text: "Language Setup"
                color: theme.textPrimary
                font.pixelSize: ui.dp(29)
                Layout.fillWidth: true
            }

            FeedbackIconButton {
                width: ui.dp(24)
                height: ui.dp(24)
                iconSize: ui.dp(24)
                iconSource: appBridge.asset_url("settings.svg")
                onClicked: appBridge.show_settings()
            }
        }

        Label {
            Layout.fillWidth: true
            text: "Download language packs to start translating"
            color: theme.textSecondary
            wrapMode: Text.WordWrap
        }

        LanguageCatalogBrowser {
            Layout.fillWidth: true
            Layout.fillHeight: true
            appBridge: root.appBridge
            theme: root.theme
        }

        Button {
            Layout.fillWidth: true
            enabled: appBridge.has_languages
            text: "Done"
            onClicked: appBridge.finish_language_setup()
        }
    }
}
