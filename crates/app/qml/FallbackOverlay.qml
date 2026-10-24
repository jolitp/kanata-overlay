import QtQuick
import QtQuick.Window

// The overlay on X11 and on Wayland without layer-shell: best effort with window
// hints (research R3). On X11 it bypasses the window manager, so it is
// undecorated, on every desktop, out of the task bar and never focused.
Window {
    id: win
    required property var device
    required property string deviceId
    required property var overlay
    required property var settings
    property bool first: false

    // This device's settings; touching revision re-reads them on every change.
    // Shown unless hidden globally or on its own (its tray icon, feature 007);
    // the settings window's preview shows every board.
    readonly property bool shownHere: overlay.overlayVisible
        && (overlay.inPreview || !JSON.parse(overlay.hiddenJson || "[]").includes(deviceId))
    readonly property var cfg: {
        void settings.revision
        return JSON.parse(settings.device(deviceId))
    }
    // The configured monitor, or the primary one (feature 001, research R8).
    readonly property var targetScreen: {
        for (const s of Qt.application.screens)
            if (s.name === cfg.screen)
                return s
        return Qt.application.screens[0]
    }
    readonly property var place: {
        // placement() reads the device's settings in Rust, where QML can't
        // see them: touch revision so the binding re-runs when they change.
        void settings.revision
        return JSON.parse(settings.placement(deviceId,
            Qt.rect(targetScreen.virtualX, targetScreen.virtualY, targetScreen.width, targetScreen.height),
            board.baseSize))
    }
    x: place.x
    y: place.y
    width: place.width
    height: place.height
    screen: targetScreen
    color: "transparent"
    visible: false
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput
           | Qt.WindowDoesNotAcceptFocus | Qt.Tool
           | (Qt.platform.pluginName === "xcb" ? Qt.X11BypassWindowManagerHint : 0)

    Board {
        id: board
        device: win.device
        overlay: win.overlay
        first: win.first
        compact: win.cfg.style === "compact"
        opacity: win.cfg.opacity
        scale: win.place.scale
        transformOrigin: Item.TopLeft
    }

    function checkScreen() {
        if (cfg.screen && targetScreen.name !== cfg.screen)
            overlay.addNotice(cfg.name + ": monitor " + cfg.screen + " not found, using " + targetScreen.name)
    }

    Component.onCompleted: {
        checkScreen()
        visible = Qt.binding(() => win.shownHere)
    }
    onTargetScreenChanged: checkScreen()
    onPlaceChanged: if (place.capped) overlay.addNotice(cfg.name + ": scale capped to fit the screen")
}
