import QtQuick 2.15
import QtQuick.Controls 2.15
import QtQuick.Window 2.15

ApplicationWindow {
    id: root
    visible: true
    visibility: app.desktop_mode ? Window.Windowed : Window.Maximized
    width: app.desktop_mode ? 600 : 720
    height: app.desktop_mode ? 1024 : 1280
    minimumWidth: app.desktop_mode ? 600 : 360
    minimumHeight: app.desktop_mode ? 1024 : 640
    title: "Offline Translator"

    AppTheme { id: theme }
    color: theme.backgroundColor

    Item {
        id: appChrome
        anchors.fill: parent
        property bool automationPending: app.automation_enabled
        property string lastAutomationOutput: ""

        function maybeStartAutomation() {
            app.automation_log("start pending=" + automationPending + " from=" + app.automation_from + " to=" + app.automation_to + " text_len=" + app.automation_text.length)
            if (!automationPending) {
                return
            }
            if (app.automation_from.length > 0) {
                app.set_from(app.automation_from)
            }
            if (app.automation_to.length > 0) {
                app.set_to(app.automation_to)
            }
            if (app.automation_text.length > 0) {
                app.process_text(app.automation_text)
            }
            maybeScheduleAutomationScreenshot()
        }

        function maybeScheduleAutomationScreenshot() {
            if (!automationPending) {
                return
            }
            if (app.automation_screenshot_path.length === 0) {
                return
            }
            app.automation_log("watch output='" + app.output_text + "'")
            if (app.output_text.length === 0 || app.output_text === "Running OCR...") {
                return
            }
            automationPending = false
            app.automation_log("scheduling screenshot path=" + app.automation_screenshot_path)
            automationScreenshotTimer.restart()
        }

        Column {
            anchors.fill: parent
            spacing: 0

            TopBar {
                id: topBar
                width: parent.width
                visible: app.current_screen === 1 && !app.image_viewer_open
                appBridge: app
                theme: theme
            }

            Item {
                id: screenHost
                width: parent.width
                height: parent.height - (topBar.visible ? topBar.height : 0)

                SetupScreen {
                    anchors.fill: parent
                    visible: app.current_screen === 0
                    appBridge: app
                    theme: theme
                }

                TranslationScreen {
                    anchors.fill: parent
                    visible: app.current_screen === 1
                    appBridge: app
                    theme: theme
                }

                SettingsScreen {
                    anchors.fill: parent
                    visible: app.current_screen === 2
                    appBridge: app
                    theme: theme
                }

                ManageLanguagesScreen {
                    anchors.fill: parent
                    visible: app.current_screen === 3
                    appBridge: app
                    theme: theme
                }
            }
        }

        Loader {
            id: liveCameraLoader
            anchors.fill: parent
            z: 100
            active: app.live_camera_open
            sourceComponent: active ? liveCameraComponent : null
        }

        Component {
            id: liveCameraComponent
            LiveCameraScreen {
                appBridge: app
            }
        }

        Timer {
            id: automationStartupTimer
            interval: 250
            running: app.automation_enabled
            repeat: false
            onTriggered: appChrome.maybeStartAutomation()
        }

        Timer {
            id: automationWatchTimer
            interval: 100
            running: app.automation_enabled
            repeat: true
            onTriggered: appChrome.maybeScheduleAutomationScreenshot()
        }

        Timer {
            id: automationScreenshotTimer
            interval: 400
            repeat: false
            onTriggered: {
                var ok = app.save_automation_screenshot(app.automation_screenshot_path)
                app.automation_log("screenshot ok=" + ok + " path=" + app.automation_screenshot_path)
                if (app.automation_quit_after_screenshot) {
                    Qt.quit()
                }
            }
        }
    }
}
