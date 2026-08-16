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
        anchors.bottomMargin: ui.dp(12)
        spacing: ui.dp(10)

        PageHeader {
            Layout.fillWidth: true
            appBridge: root.appBridge
            theme: root.theme
            title: "Manage languages"
            onBackRequested: appBridge.back_from_manage_languages()
        }

        LanguageCatalogBrowser {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: ui.dp(12)
            Layout.rightMargin: ui.dp(12)
            appBridge: root.appBridge
            theme: root.theme
        }
    }
}
