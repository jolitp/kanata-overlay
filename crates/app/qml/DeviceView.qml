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
    property Window window
    readonly property var device: deviceState

    property list<QtObject> objects: [
        DeviceState {
            id: deviceState
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
