import QtQuick
import QtQuick.Window
import io.github.jolitp.kanataoverlay

// One device: its DeviceState (Kanata session, connection, labels) and its
// overlay window. Created and destroyed by Main.qml's Instantiator as devices
// are added and removed (feature 004, research R2). Not an Item, so its window
// stays a top-level window.
QtObject {
    id: view
    required property string deviceId
    required property var settings
    required property var overlay
    /// The first board also shows the global notices.
    property bool first: false
    /// The layout editor's device, "" when closed (feature 006: it shows presses too).
    property string editedId: ""
    // Presses are read only while something shows them (FR-004).
    // A compact board has no keys to light (feature 007, FR-006).
    readonly property bool compact: { void settings.revision; return JSON.parse(settings.device(deviceId)).style === "compact" }
    readonly property bool boardShown: overlay.overlayVisible
        && (overlay.inPreview || !JSON.parse(overlay.hiddenJson || "[]").includes(deviceId))
    readonly property bool watched: (boardShown && !compact) || editedId === deviceId
    onWatchedChanged: deviceState.setWatching(watched)
    property Window window
    readonly property var device: deviceState

    /// Something the tray shows changed (feature 007).
    signal trayChanged()
    onCompactChanged: trayChanged()

    property list<QtObject> objects: [
        DeviceState {
            id: deviceState
            onNameChanged: view.trayChanged()
            onCurrentLayerChanged: view.trayChanged()
            onStateTextChanged: view.trayChanged()
            onStatusChanged: view.trayChanged()
            onConnectedChanged: view.trayChanged()
            onModeChanged: view.trayChanged()
        },
        // Settings changed: restart this device's session if its config,
        // layout or address changed (FR-001a).
        Connections {
            target: view.settings
            function onRevisionChanged() {
                deviceState.apply(view.settings.device(view.deviceId), view.settings.deviceCheck(view.deviceId))
            }
        },
        // The orderly stop of feature 003 on a normal quit; onDestruction may
        // only run after the event loop has ended (constitution III).
        Connections {
            target: Qt.application
            function onAboutToQuit() { deviceState.shutdown() }
        }
    ]

    Component.onCompleted: {
        deviceState.start(settings.device(deviceId), settings.deviceCheck(deviceId))
        deviceState.setWatching(watched)
        const variant = Qt.createComponent(overlay.useLayerShell ? "LayerShellOverlay.qml" : "FallbackOverlay.qml")
        window = variant.createObject(view, {
            device: deviceState,
            deviceId: Qt.binding(() => view.deviceId),
            first: Qt.binding(() => view.first),
            overlay: overlay,
            settings: settings
        })
        if (!window)
            console.error(variant.errorString())
    }

    // The device was removed: its managed Kanata stops and its board goes.
    Component.onDestruction: {
        deviceState.shutdown()
        if (window)
            window.destroy()
    }
}
