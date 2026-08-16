import QtQuick 2.15

Item {
    id: root
    visible: false
    width: 0
    height: 0

    // Device pixels per grid unit / 8, resolved once in Rust: the shell's GRID_UNIT_PX on Ubuntu
    // Touch, 1.0 everywhere else (where Qt's own high-DPI scaling has already been applied).
    readonly property real scaleFactor: app.ui_scale

    readonly property real pageTitlePx: dp(27)
    readonly property real listPrimaryPx: dp(19)
    readonly property real listSecondaryPx: dp(15)
    readonly property real sectionTitlePx: dp(17)

    function dp(value) {
        return value * scaleFactor
    }
}
