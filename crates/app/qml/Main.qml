import QtQuick
import QtQuick.Window
import io.github.jolitp.kanataoverlay

// Root object: the state, the overlay window (layer-shell or fallback), the
// settings window and the two global shortcuts. Not an Item: since Qt 6.7 a
// Window inside an Item becomes a child window of that item's window.
QtObject {
    id: root
    property Window overlayWindow

    property list<QtObject> objects: [
        OverlayState {
            id: overlayState
        },
        SettingsState {
            id: settingsState
            onWarning: text => overlayState.addNotice(text)
        },
        KanataState {
            id: kanataState
        },
        Connections {
            target: overlayState
            function onConnectedChanged() { kanataState.setConnectedState(overlayState.connected) }
        },
        // A managed Kanata stops with the overlay (constitution III); crashes and
        // signals are covered by the parent-death signal.
        Connections {
            target: Qt.application
            function onAboutToQuit() { kanataState.shutdown() }
        },
        SettingsWindow {
            id: settingsWindow
            settings: settingsState
            overlay: overlayState
            kanata: kanataState
        },
        GlobalShortcut {
            id: toggleShortcut
            shortcutId: "toggle-overlay"
            text: "Show/hide overlay"
            defaultKeys: "Meta+Ctrl+K"
            onActivated: overlayState.toggleOverlay()
        },
        GlobalShortcut {
            id: settingsShortcut
            shortcutId: "open-settings"
            text: "Open overlay settings"
            defaultKeys: "Meta+Alt+Shift+K"
            onActivated: settingsWindow.open()
        }
    ]

    Component.onCompleted: {
        overlayState.start()
        kanataState.start()
        settingsState.load()

        // Created imperatively so its required properties are set at creation.
        const variant = Qt.createComponent(overlayState.useLayerShell ? "LayerShellOverlay.qml" : "FallbackOverlay.qml")
        overlayWindow = variant.createObject(root, { overlay: overlayState, settings: settingsState, kanata: kanataState })
        if (!overlayWindow)
            console.error(variant.errorString())

        toggleShortcut.registerShortcut()
        if (!toggleShortcut.registered)
            overlayState.addNotice("show/hide shortcut unavailable — set one in System Settings › Shortcuts")
        settingsShortcut.registerShortcut()
        if (!settingsShortcut.registered)
            overlayState.addNotice("settings shortcut unavailable — edit " + settingsState.path)
    }
}
