import QtQuick
import QtQuick.Window

// The overlay on X11 and on Wayland without layer-shell: best effort with window
// hints (research R3). On X11 it bypasses the window manager, so it is
// undecorated, on every desktop, out of the task bar and never focused.
Window {
    id: win
    required property var overlay
    required property var settings

    readonly property var targetScreen: {
        for (const s of Qt.application.screens)
            if (s.name === settings.screenName)
                return s
        return Qt.application.screens[0]
    }
    readonly property var place: {
        // placement() reads these in Rust, where QML can't see them: touch
        // them here so the binding re-runs when they change.
        void settings.edge, settings.margin, settings.scale
        return JSON.parse(settings.placement(
            targetScreen.virtualX, targetScreen.virtualY, targetScreen.width, targetScreen.height,
            board.baseSize.width, board.baseSize.height))
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
        overlay: win.overlay
        opacity: win.settings.opacity
        scale: win.place.scale
        transformOrigin: Item.TopLeft
    }

    function checkScreen() {
        if (settings.screenName !== "" && targetScreen.name !== settings.screenName)
            overlay.addNotice("monitor " + settings.screenName + " not found, using " + targetScreen.name)
    }

    Component.onCompleted: {
        checkScreen()
        visible = Qt.binding(() => overlay.overlayVisible)
    }
    onTargetScreenChanged: checkScreen()
    onPlaceChanged: if (place.capped) overlay.addNotice("scale capped to fit the screen")
}
