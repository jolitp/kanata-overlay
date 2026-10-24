import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

// An ordinary window for the overlay settings: the device list, and for the
// selected device its files, address, placement and Kanata (feature 004, R8).
// Every change shows on the boards at once (live preview) and is saved when
// committed.
ApplicationWindow {
    id: win
    required property var settings
    required property var overlay
    /// Device id → DeviceState (from Main.qml).
    property var deviceStates: ({})
    /// The layout editor window (feature 005).
    property var layoutEditor: null
    /// DesktopState: Start at login and the permission check (feature 007).
    property var desktop: null
    readonly property var permissions: desktop && desktop.permissionsJson ? JSON.parse(desktop.permissionsJson) : null

    title: qsTr("Kanata Overlay Settings")
    width: 520
    height: form.implicitHeight + 32
    visible: false

    readonly property var edges: [
        { value: "top", text: qsTr("Top") },
        { value: "bottom", text: qsTr("Bottom") },
        { value: "left", text: qsTr("Left") },
        { value: "right", text: qsTr("Right") },
        { value: "top-left", text: qsTr("Top left") },
        { value: "top-right", text: qsTr("Top right") },
        { value: "bottom-left", text: qsTr("Bottom left") },
        { value: "bottom-right", text: qsTr("Bottom right") }
    ]
    readonly property var monitors: [{ value: "", text: qsTr("Primary") }].concat(
        Array.from(Qt.application.screens, s => ({ value: s.name, text: s.name })))

    // The device list and the selected device, re-read on every change.
    readonly property var ids: { void settings.revision; return JSON.parse(settings.deviceIds || "[]") }
    readonly property var deviceList: { void settings.revision; return ids.map(id => JSON.parse(settings.device(id))) }
    property string selectedId: ""
    readonly property var cfg: {
        void settings.revision
        return selectedId ? JSON.parse(settings.device(selectedId)) : ({})
    }
    readonly property var kanata: deviceStates[selectedId] || null
    // Another device at the same position on the same monitor (spec edge case).
    readonly property string overlapping: {
        const other = deviceList.find(d => d.id !== selectedId && d.edge === cfg.edge && d.screen === cfg.screen)
        return other ? other.name : ""
    }

    function open() {
        if (!ids.includes(selectedId))
            selectedId = ids.length > 0 ? ids[0] : ""
        show()
        raise()
        requestActivate()
    }

    function update(field, value) {
        if (selectedId)
            settings.updateDevice(selectedId, field, String(value))
    }

    // Load the selected device's values into the controls.
    // Reads the device itself: from onSelectedIdChanged, the cfg binding may
    // not have caught up yet and still hold the previous device.
    function refresh() {
        const cfg = selectedId ? JSON.parse(settings.device(selectedId)) : ({})
        deviceBox.currentIndex = Math.max(0, ids.indexOf(selectedId))
        nameField.text = cfg.name || ""
        configField.text = cfg.config || ""
        layoutField.text = cfg.layout || ""
        addressField.text = cfg.customAddress || ""
        addressField.placeholderText = cfg.address || ""
        edgeBox.currentIndex = edgeBox.indexOfValue(cfg.edge)
        marginBox.value = cfg.margin || 0
        opacitySlider.value = Math.round((cfg.opacity || 0) * 100)
        scaleSlider.value = cfg.scale || 100
        monitorBox.currentIndex = Math.max(0, monitorBox.indexOfValue(cfg.screen))
        highlightBox.currentIndex = Math.max(0, highlightBox.indexOfValue(cfg.highlight || "all"))
        styleBox.currentIndex = Math.max(0, styleBox.indexOfValue(cfg.style || "full"))
    }

    onSelectedIdChanged: refresh()
    onVisibleChanged: {
        if (visible) {
            refresh()
            if (desktop)
                desktop.checkPermissions()
        }
        overlay.setPreviewing(visible)
    }
    onClosing: settings.save()

    // Opens in the folder of the current file, if there is one.
    function browse(field, filters) {
        fileDialog.field = field
        fileDialog.nameFilters = filters
        const current = win.cfg[field] || ""
        if (current.includes("/"))
            fileDialog.currentFolder = "file://" + current.substring(0, current.lastIndexOf("/"))
        fileDialog.open()
    }

    FileDialog {
        id: fileDialog
        property string field
        onAccepted: {
            const path = decodeURIComponent(selectedFile.toString().replace(/^file:\/\//, ""))
            win.update(field, path)
            win.settings.save()
            win.refresh()
        }
    }

    GridLayout {
        id: form
        anchors.fill: parent
        anchors.margins: 16
        columns: 2
        rowSpacing: 10
        columnSpacing: 12

        Label { text: qsTr("Device") }
        RowLayout {
            ComboBox {
                // Focus by Tab only: after a click, typing on the keyboard must not
                // pick another option (letters jump to matching entries).
                focusPolicy: Qt.TabFocus
                id: deviceBox
                Layout.fillWidth: true
                model: win.deviceList
                // The list is rebuilt on every settings change, which puts the
                // box back on its first entry: reselect the device being edited.
                onModelChanged: Qt.callLater(() => currentIndex = Math.max(0, win.ids.indexOf(win.selectedId)))
                textRole: "name"
                valueRole: "id"
                // By index: currentValue can still hold the previous entry here.
                onActivated: index => win.selectedId = win.ids[index]
            }
            Button {
                text: qsTr("Add")
                onClicked: {
                    win.selectedId = win.settings.addDevice(qsTr("New device"))
                    win.refresh()
                }
            }
            Button {
                text: qsTr("Remove")
                enabled: win.ids.length > 0
                onClicked: removeDialog.open()
            }
        }

        Label { text: qsTr("Name") }
        TextField {
            id: nameField
            Layout.fillWidth: true
            onEditingFinished: { win.update("name", text); win.settings.save() }
        }

        Label { text: qsTr("Kanata config") }
        RowLayout {
            TextField {
                id: configField
                Layout.fillWidth: true
                placeholderText: qsTr("none: printed legends only")
                onEditingFinished: { win.update("config", text); win.settings.save() }
            }
            Button {
                text: qsTr("Browse…")
                onClicked: win.browse("config", [qsTr("Kanata config (*.kbd)"), qsTr("All files (*)")])
            }
        }

        Label { text: qsTr("Layout") }
        RowLayout {
            TextField {
                id: layoutField
                Layout.fillWidth: true
                placeholderText: qsTr("none: a grid of the config's keys")
                onEditingFinished: { win.update("layout", text); win.settings.save() }
            }
            Button {
                text: qsTr("Browse…")
                onClicked: win.browse("layout", [qsTr("Layouts (*.json)"), qsTr("All files (*)")])
            }
            Button {
                text: qsTr("Edit layout…")
                enabled: !!win.kanata && !!win.layoutEditor
                onClicked: win.layoutEditor.whenSaved(() => {
                    win.layoutEditor.endSession()
                    win.layoutEditor.openDevice(win.selectedId)
                })
            }
        }

        Label { text: qsTr("Address") }
        TextField {
            id: addressField
            Layout.fillWidth: true
            onEditingFinished: { win.update("address", text); win.settings.save() }
        }

        Label { text: qsTr("Position") }
        RowLayout {
            ComboBox {
                // Focus by Tab only: after a click, typing on the keyboard must not
                // pick another option (letters jump to matching entries).
                focusPolicy: Qt.TabFocus
                id: edgeBox
                Layout.fillWidth: true
                model: win.edges
                textRole: "text"
                valueRole: "value"
                onActivated: { win.update("edge", currentValue); win.settings.save() }
            }
            Label {
                visible: win.overlapping.length > 0
                color: palette.placeholderText
                text: qsTr("overlaps %1").arg(win.overlapping)
            }
        }

        Label { text: qsTr("Margin") }
        SpinBox {
            id: marginBox
            Layout.fillWidth: true
            from: 0
            to: 500
            editable: true
            onValueModified: { win.update("margin", value); win.settings.save() }
        }

        Label { text: qsTr("Opacity") }
        RowLayout {
            Slider {
                id: opacitySlider
                Layout.fillWidth: true
                from: 10
                to: 100
                stepSize: 1
                onMoved: win.update("opacity", value)
                onPressedChanged: if (!pressed) win.settings.save()
            }
            Label { text: Math.round(opacitySlider.value) + "%" }
        }

        Label { text: qsTr("Scale") }
        RowLayout {
            Slider {
                id: scaleSlider
                Layout.fillWidth: true
                from: 50
                to: 300
                stepSize: 5
                onMoved: win.update("scale", value)
                onPressedChanged: if (!pressed) win.settings.save()
            }
            Label { text: Math.round(scaleSlider.value) + "%" }
        }

        Label { text: qsTr("Monitor") }
        ComboBox {
            // Focus by Tab only: after a click, typing on the keyboard must not
            // pick another option (letters jump to matching entries).
            focusPolicy: Qt.TabFocus
            id: monitorBox
            Layout.fillWidth: true
            model: win.monitors
            textRole: "text"
            valueRole: "value"
            onActivated: { win.update("screen", currentValue); win.settings.save() }
        }

        // The whole keyboard, or only the device name and layer (feature 007).
        Label { text: qsTr("Board style") }
        ComboBox {
            id: styleBox
            // Focus by Tab only: after a click, typing on the keyboard must not
            // pick another option (letters jump to matching entries).
            focusPolicy: Qt.TabFocus
            Layout.fillWidth: true
            model: [
                { value: "full", text: qsTr("Full keyboard") },
                { value: "compact", text: qsTr("Compact: name and layer") }
            ]
            textRole: "text"
            valueRole: "value"
            onActivated: { win.update("style", currentValue); win.settings.save() }
        }

        // Pressed keys on the board (feature 006). "No typing keys" never shows
        // letters, digits or symbols, e.g. while screen sharing.
        Label { text: qsTr("Highlight") }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2
            ComboBox {
                // Focus by Tab only: after a click, typing on the keyboard must not
                // pick another option (letters jump to matching entries).
                focusPolicy: Qt.TabFocus
                id: highlightBox
                Layout.fillWidth: true
                model: [
                    { value: "all", text: qsTr("All keys") },
                    { value: "no-typing", text: qsTr("No typing keys") },
                    { value: "off", text: qsTr("Off") }
                ]
                textRole: "text"
                valueRole: "value"
                onActivated: { win.update("highlight", currentValue); win.settings.save() }
            }
            Label {
                Layout.fillWidth: true
                visible: !!win.kanata && win.kanata.pressSource === "tcp"
                wrapMode: Text.Wrap
                color: palette.placeholderText
                text: qsTr("Pressed keys: only for a Kanata started by the overlay. Tap and hold still show.")
            }
        }

        Label { text: qsTr("Kanata") }
        RowLayout {
            Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: !win.kanata ? "" : win.kanata.mode === "none" ? qsTr("not managed")
                      : win.kanata.mode + (win.kanata.stateText && win.kanata.stateText !== win.kanata.mode
                                           ? " · " + win.kanata.stateText : "")
            }
            Button {
                text: qsTr("Restart")
                visible: !!win.kanata && win.kanata.canRestart
                onClicked: win.kanata.restart()
            }
            Button {
                text: qsTr("Start Kanata")
                visible: !!win.kanata && win.kanata.canStart
                onClicked: win.kanata.startManaged()
            }
            Button {
                text: qsTr("Show log")
                visible: !!win.kanata && win.kanata.mode === "managed"
                onClicked: { logWindow.show(); logWindow.raise() }
            }
        }

        // The header is short and can't be clicked; read messages in full here.
        Label {
            text: qsTr("Messages")
            Layout.alignment: Qt.AlignTop
            visible: messages.text.length > 0
        }
        Label {
            id: messages
            Layout.fillWidth: true
            visible: text.length > 0
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            text: !win.kanata ? "" : [
                    win.kanata.mode === "managed" && !win.kanata.connected ? win.kanata.stateText : "",
                    win.kanata.notices.split(" · ").join("\n"),
                    win.overlay.notices.split(" · ").join("\n")
                ].filter(s => s.length > 0).join("\n")
        }

        // Can Kanata run without root? Explained, never fixed for you (FR-010–012).
        Label { text: qsTr("Permissions"); Layout.alignment: Qt.AlignTop }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            readonly property bool ready: !!win.permissions && win.permissions.fix === ""
            Label {
                visible: parent.ready
                text: qsTr("✓ Kanata can run without root")
            }
            Repeater {
                model: win.permissions && win.permissions.fix !== "" ? win.permissions.items : []
                delegate: Label {
                    required property var modelData
                    text: (modelData.status === "ok" ? "✓ " : "✗ ") + modelData.name
                          + (modelData.status === "needs-login" ? qsTr(" (log out and back in)")
                             : modelData.status === "missing" ? qsTr(" (missing)") : "")
                    color: modelData.status === "ok" ? palette.text : "#e57373"
                }
            }
            RowLayout {
                visible: !!win.permissions && win.permissions.fix !== ""
                Layout.fillWidth: true
                // Plain TextEdit: Breeze's TextArea warns at load (see LogWindow).
                TextEdit {
                    id: fixText
                    Layout.fillWidth: true
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    font.family: "monospace"
                    font.pixelSize: 11
                    color: palette.text
                    text: win.permissions ? win.permissions.fix : ""
                }
                Button {
                    Layout.alignment: Qt.AlignTop
                    text: qsTr("Copy")
                    onClicked: { fixText.selectAll(); fixText.copy(); fixText.deselect() }
                }
            }
        }

        Label { text: qsTr("Start at login") }
        CheckBox {
            checked: !!win.desktop && win.desktop.startAtLogin
            enabled: !!win.desktop
            onToggled: win.desktop.setLogin(checked)
        }

        Item { Layout.columnSpan: 2; Layout.fillWidth: true; implicitHeight: 4 }
        Button {
            Layout.columnSpan: 2
            Layout.alignment: Qt.AlignRight
            text: qsTr("Quit Kanata Overlay")
            onClicked: Qt.quit()
        }

        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: palette.placeholderText
            text: qsTr("Saved to %1").arg(win.settings.path || qsTr("(nowhere: no config directory)"))
        }
    }

    Dialog {
        id: removeDialog
        anchors.centerIn: parent
        title: qsTr("Remove %1?").arg(win.cfg.name || "")
        standardButtons: Dialog.Ok | Dialog.Cancel
        Label { text: qsTr("Its board disappears and a Kanata it started is stopped.") }
        onAccepted: {
            win.settings.removeDevice(win.selectedId)
            win.selectedId = win.ids.length > 0 ? win.ids[0] : ""
        }
    }

    LogWindow {
        id: logWindow
        kanata: win.kanata
    }
}
