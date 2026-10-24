import QtQuick
import QtQuick.Window
import io.github.jolitp.kanataoverlay

// Root object: the global state, one DeviceView per device, the settings
// window and the two global shortcuts. Not an Item: since Qt 6.7 a Window
// inside an Item becomes a child window of that item's window.
QtObject {
    id: root
    /// Device id → its DeviceState, for the settings window.
    property var deviceStates: ({})

    property list<QtObject> objects: [
        OverlayState {
            id: overlayState
        },
        SettingsState {
            id: settingsState
            onWarning: text => overlayState.addNotice(text)
            onDeviceIdsChanged: root.syncDevices()
        },
        // Updated incrementally, so adding or removing one device never
        // recreates (and restarts) the others.
        ListModel {
            id: deviceModel
        },
        Instantiator {
            id: devices
            active: false
            model: deviceModel
            // DeviceView's own required deviceId is filled from the model role.
            delegate: DeviceView {
                required property int index
                first: index === 0
                editedId: layoutEditor.deviceId
                settings: settingsState
                overlay: overlayState
            }
            onObjectAdded: root.updateDeviceStates()
            onObjectRemoved: root.updateDeviceStates()
        },
        SettingsWindow {
            id: settingsWindow
            settings: settingsState
            overlay: overlayState
            deviceStates: root.deviceStates
            layoutEditor: layoutEditorWindow
        },
        // One layout editor for all devices (feature 005).
        LayoutEditor {
            id: layoutEditor
            // A new file becomes the device's layout; every board showing that
            // file rereads it (two devices may share one).
            onSaved: (deviceId, path, isNew) => {
                if (isNew) {
                    settingsState.updateDevice(deviceId, "layout", path)
                    settingsState.save()
                    settingsWindow.refresh()
                }
                for (const id in root.deviceStates)
                    root.deviceStates[id].reloadLayout(path)
            }
        },
        LayoutEditorWindow {
            id: layoutEditorWindow
            editor: layoutEditor
            settings: settingsState
            deviceStates: root.deviceStates
        },
        // Configs can be edited and devices plugged in at any time: re-run the
        // device check now and then (cheap: a few small files and /proc).
        Timer {
            interval: 2000
            repeat: true
            running: devices.active
            onTriggered: settingsState.recheck()
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

    function syncDevices() {
        const ids = JSON.parse(settingsState.deviceIds || "[]")
        for (let i = deviceModel.count - 1; i >= 0; i--)
            if (!ids.includes(deviceModel.get(i).deviceId))
                deviceModel.remove(i)
        const present = Array.from({ length: deviceModel.count }, (_, i) => deviceModel.get(i).deviceId)
        for (const id of ids)
            if (!present.includes(id))
                deviceModel.append({ deviceId: id })
    }

    function updateDeviceStates() {
        const map = {}
        for (let i = 0; i < devices.count; i++) {
            const view = devices.objectAt(i)
            if (view)
                map[view.deviceId] = view.device
        }
        deviceStates = map
    }

    Component.onCompleted: {
        overlayState.start()
        settingsState.load()
        syncDevices()
        devices.active = true

        toggleShortcut.registerShortcut()
        if (!toggleShortcut.registered)
            overlayState.addNotice("show/hide shortcut unavailable — set one in System Settings › Shortcuts")
        settingsShortcut.registerShortcut()
        if (!settingsShortcut.registered)
            overlayState.addNotice("settings shortcut unavailable — edit " + settingsState.path)
    }
}
