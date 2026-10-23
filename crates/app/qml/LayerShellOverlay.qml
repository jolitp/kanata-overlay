import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell

// The overlay on Wayland compositors with layer-shell (KDE Plasma): on the
// overlay layer above full-screen windows, never focused, and all input passes
// through to the windows below (research R1, R2).
Window {
    id: win
    required property var overlay
    required property var settings
    required property var kanata

    // The configured monitor, or the primary one (research R8).
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
    // Which screen edges the overlay touches: one for an edge, two for a corner.
    readonly property bool atTop: settings.edge.startsWith("top")
    readonly property bool atBottom: settings.edge.startsWith("bottom")
    readonly property bool atLeft: settings.edge.endsWith("left")
    readonly property bool atRight: settings.edge.endsWith("right")

    // QMargins can't be built in QML, so the margin is transparent padding on
    // the anchored edges; it takes no input anyway.
    width: place.width + (atLeft || atRight ? settings.margin : 0)
    height: place.height + (atTop || atBottom ? settings.margin : 0)
    screen: targetScreen
    color: "transparent"
    visible: false // shown once the layer-shell properties are in place
    flags: Qt.FramelessWindowHint | Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus

    LayerShell.Window.scope: "kanata-overlay"
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.activateOnShow: false
    LayerShell.Window.exclusionZone: 0 // avoid panels, reserve nothing
    // One anchor centres the surface along that edge; two put it in a corner.
    LayerShell.Window.anchors: (atTop ? LayerShell.Window.AnchorTop : 0)
                               | (atBottom ? LayerShell.Window.AnchorBottom : 0)
                               | (atLeft ? LayerShell.Window.AnchorLeft : 0)
                               | (atRight ? LayerShell.Window.AnchorRight : 0)

    Board {
        id: board
        overlay: win.overlay
        kanata: win.kanata
        x: win.atLeft ? win.settings.margin : 0
        y: win.atTop ? win.settings.margin : 0
        opacity: win.settings.opacity
        scale: win.place.scale
        transformOrigin: Item.TopLeft
    }

    function bindVisible() {
        visible = Qt.binding(() => overlay.overlayVisible)
    }

    function checkScreen() {
        if (settings.screenName !== "" && targetScreen.name !== settings.screenName)
            overlay.addNotice("monitor " + settings.screenName + " not found, using " + targetScreen.name)
    }

    Component.onCompleted: {
        checkScreen()
        bindVisible()
    }

    // A layer surface is bound to one output: re-create it on the new screen.
    onTargetScreenChanged: {
        checkScreen()
        visible = false
        bindVisible()
    }

    onPlaceChanged: if (place.capped) overlay.addNotice("scale capped to fit the screen")
}
