import QtQuick
import QtQuick.Window
import org.kde.layershell as LayerShell

// The overlay on Wayland compositors with layer-shell (KDE Plasma): on the
// overlay layer above full-screen windows, never focused, and all input passes
// through to the windows below (research R1, R2).
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
    // Which screen edges the overlay touches: one for an edge, two for a corner.
    readonly property bool atTop: cfg.edge.startsWith("top")
    readonly property bool atBottom: cfg.edge.startsWith("bottom")
    readonly property bool atLeft: cfg.edge.endsWith("left")
    readonly property bool atRight: cfg.edge.endsWith("right")

    // QMargins can't be built in QML, so the margin is transparent padding on
    // the anchored edges; it takes no input anyway.
    width: place.width + (atLeft || atRight ? cfg.margin : 0)
    height: place.height + (atTop || atBottom ? cfg.margin : 0)
    screen: targetScreen
    color: "transparent"
    visible: false // shown once the layer-shell properties are in place
    flags: Qt.FramelessWindowHint | Qt.WindowTransparentForInput | Qt.WindowDoesNotAcceptFocus

    LayerShell.Window.scope: "kanata-overlay-" + deviceId
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
        device: win.device
        overlay: win.overlay
        first: win.first
        compact: win.cfg.style === "compact"
        x: win.atLeft ? win.cfg.margin : 0
        y: win.atTop ? win.cfg.margin : 0
        opacity: win.cfg.opacity
        scale: win.place.scale
        transformOrigin: Item.TopLeft
    }

    function bindVisible() {
        visible = Qt.binding(() => win.shownHere)
    }

    function checkScreen() {
        if (cfg.screen && targetScreen.name !== cfg.screen)
            overlay.addNotice(cfg.name + ": monitor " + cfg.screen + " not found, using " + targetScreen.name)
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

    onPlaceChanged: if (place.capped) overlay.addNotice(cfg.name + ": scale capped to fit the screen")
}
