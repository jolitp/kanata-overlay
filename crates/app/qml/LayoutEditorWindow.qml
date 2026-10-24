import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

// The layout editor (feature 005): a canvas with the device's keys, a toolbar
// and an inspector for the selection. Every change goes through `editor`
// (LayoutEditor, Rust); while a drag is in progress this window only shows a
// transient offset, angle or edge change and commits it on release (R4).
ApplicationWindow {
    id: win
    required property var editor
    required property var settings
    /// Device id → DeviceState (from Main.qml).
    property var deviceStates: ({})

    readonly property var device: deviceStates[editor.deviceId] || null
    readonly property var layout: editor.layoutJson.length > 0 ? JSON.parse(editor.layoutJson) : ({ keys: [] })
    readonly property var flags: JSON.parse(editor.flagsJson || '{"keys": [], "counts": {}}')
    readonly property var selection: JSON.parse(editor.selectionJson || "[]")
    readonly property var labels: device ? JSON.parse(device.labelsJson) : ({})
    // Held keys and tap-hold results of the edited device (feature 006).
    readonly property var pressedKeys: device ? JSON.parse(device.pressedJson || "[]") : []
    readonly property var tapHolds: device ? JSON.parse(device.tapHoldJson || "{}") : ({})

    // ── press to bind (feature 006) ──
    property int captureLeft: 0 // seconds left; 0 = not waiting
    readonly property bool canCapture: !!device && device.pressSource === "debug" && !!single
    function startCapture() {
        if (canCapture && device.capture())
            captureLeft = 10
    }
    function stopCapture() {
        if (captureLeft > 0 && device)
            device.cancelCapture()
        captureLeft = 0
    }
    Timer {
        interval: 1000
        repeat: true
        running: win.captureLeft > 0
        onTriggered: if (--win.captureLeft === 0 && win.device) win.device.cancelCapture()
    }
    Connections {
        target: win.device
        function onCaptured(key) {
            win.captureLeft = 0
            win.editor.bind(win.editor.nameFor(key))
        }
    }
    onSelectionChanged: stopCapture()
    readonly property var single: selection.length === 1 ? layout.keys[selection[0]] : null
    property real unit: 54
    readonly property real minUnit: 15
    readonly property real maxUnit: 160

    title: editor.deviceId
           ? qsTr("Layout — %1 — %2%3").arg(editor.deviceName).arg(editor.savePath.split("/").pop()).arg(editor.dirty ? " •" : "")
           : qsTr("Layout editor")
    width: 1100
    height: 640
    visible: false

    // ── transient gesture state (units, degrees) ──
    property real moveX: 0
    property real moveY: 0
    property real extraAngle: 0
    property point pivot: Qt.point(0, 0)
    property string growEdge: ""
    property real grow: 0
    // The reference image while Alt+dragging or Alt+scrolling it.
    readonly property var refImage: layout.background || null
    property real bgMoveX: 0
    property real bgMoveY: 0
    property real bgScale: 1
    property point bgAnchor: Qt.point(0, 0)
    readonly property var bgShown: {
        const b = refImage
        if (!b) return null
        const w = b.width * bgScale
        return { x: bgAnchor.x - (bgAnchor.x - b.x) * bgScale + bgMoveX,
                 y: bgAnchor.y - (bgAnchor.y - b.y) * bgScale + bgMoveY,
                 width: w, opacity: b.opacity }
    }
    function commitBackground() {
        const b = bgShown
        bgMoveX = 0; bgMoveY = 0; bgScale = 1
        if (b)
            editor.setBackgroundGeometry(b.x, b.y, b.width, b.opacity)
    }
    function fileUrl(path) { return "file://" + path.split("/").map(encodeURIComponent).join("/") }

    function snapped(v, step) { return step > 0 ? Math.round(v / step) * step : v }

    // ── sessions ──

    /// Open the editor on a device (from the settings window or the selector).
    function openDevice(id) {
        const state = deviceStates[id]
        if (!state)
            return
        editor.open(settings.device(id), state.layoutJson, state.configKeysJson(), settings.path)
        show()
        raise()
        requestActivate()
    }

    property var pendingAction: null
    /// Run `action` now, or after asking about unsaved changes (FR-016).
    function whenSaved(action) {
        if (!editor.dirty) {
            action()
            return
        }
        pendingAction = action
        unsavedDialog.open()
    }

    function endSession() {
        stopCapture()
        if (device)
            device.endPreview()
        editor.close()
    }

    function finishClose() {
        editor.close()
        hide()
    }

    onClosing: close => {
        if (editor.dirty) {
            close.accepted = false
            whenSaved(finishClose)
        } else {
            editor.close()
        }
    }

    // Every change shows on the device's board (FR-013).
    Connections {
        target: win.editor
        function onLayoutJsonChanged() {
            if (win.device && win.editor.deviceId && win.editor.layoutJson.length > 0)
                win.device.previewLayout(win.editor.layoutJson)
        }
    }

    // The device was removed while editing (edge case): offer to keep the work.
    Connections {
        target: win.settings
        function onDeviceIdsChanged() {
            const ids = JSON.parse(win.settings.deviceIds || "[]")
            if (!win.editor.deviceId || ids.includes(win.editor.deviceId))
                return
            if (win.editor.dirty)
                saveAsDialog.open()
            else
                win.finishClose()
        }
    }

    // ── geometry helpers (units) ──

    function corners(k) {
        const a = (k.rotation || 0) * Math.PI / 180, rx = k.rx || 0, ry = k.ry || 0
        return [[k.x, k.y], [k.x + k.width, k.y], [k.x + k.width, k.y + k.height], [k.x, k.y + k.height]]
            .map(([x, y]) => {
                const dx = x - rx, dy = y - ry
                return [rx + dx * Math.cos(a) - dy * Math.sin(a), ry + dx * Math.sin(a) + dy * Math.cos(a)]
            })
    }
    function boundsOf(keys) {
        if (keys.length === 0)
            return { x: 0, y: 0, w: 0, h: 0 }
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
        for (const k of keys)
            for (const [x, y] of corners(k)) {
                x0 = Math.min(x0, x); y0 = Math.min(y0, y)
                x1 = Math.max(x1, x); y1 = Math.max(y1, y)
            }
        return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
    function centre(k) {
        const c = corners(k)
        return Qt.point((c[0][0] + c[2][0]) / 2, (c[0][1] + c[2][1]) / 2)
    }
    readonly property var layoutBounds: boundsOf(layout.keys)
    // What the canvas must show: the keys and the reference image.
    // Uses the image's committed place, not the one being dragged: the canvas
    // growing under the pointer would otherwise feed back into the drag.
    readonly property var viewBounds: {
        const b = layoutBounds, i = refImage
        if (!i || bgImage.implicitWidth <= 0)
            return b
        const ih = i.width * bgImage.implicitHeight / bgImage.implicitWidth
        const empty = layout.keys.length === 0
        const x0 = empty ? i.x : Math.min(b.x, i.x), y0 = empty ? i.y : Math.min(b.y, i.y)
        const x1 = Math.max(empty ? i.x : b.x + b.w, i.x + i.width), y1 = Math.max(empty ? i.y : b.y + b.h, i.y + ih)
        return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
    readonly property var selectionBounds: boundsOf(selection.map(i => layout.keys[i]).filter(k => !!k))
    readonly property point selectionCentre: single ? centre(single)
        : Qt.point(selectionBounds.x + selectionBounds.w / 2, selectionBounds.y + selectionBounds.h / 2)

    /// The key under a point (units), topmost first; -1 if none.
    function keyAt(px, py) {
        for (let i = layout.keys.length - 1; i >= 0; i--) {
            const k = layout.keys[i]
            const a = -(k.rotation || 0) * Math.PI / 180, rx = k.rx || 0, ry = k.ry || 0
            const dx = px - rx, dy = py - ry
            const x = rx + dx * Math.cos(a) - dy * Math.sin(a), y = ry + dx * Math.sin(a) + dy * Math.cos(a)
            if (x >= k.x && x <= k.x + k.width && y >= k.y && y <= k.y + k.height)
                return i
        }
        return -1
    }

    function nudge(dx, dy) {
        if (selection.length > 0)
            editor.moveSelection(dx, dy)
    }

    // Shortcuts stay out of the way while a text field has focus.
    readonly property bool typing: !!(activeFocusItem && activeFocusItem.hasOwnProperty("cursorPosition"))
    Shortcut { sequences: [StandardKey.Undo]; onActivated: win.editor.undo() }
    Shortcut { sequences: [StandardKey.Redo, "Ctrl+Shift+Z"]; onActivated: win.editor.redo() }
    Shortcut { sequences: [StandardKey.Save]; onActivated: win.save() }
    Shortcut { sequence: "Ctrl+A"; enabled: !win.typing; onActivated: win.editor.selectAll() }
    Shortcut { sequence: "Ctrl+D"; enabled: !win.typing; onActivated: win.editor.duplicateSelection() }
    Shortcut { sequences: ["Delete", "Backspace"]; enabled: !win.typing; onActivated: win.editor.deleteSelection() }
    Shortcut { sequence: "Escape"; enabled: !win.typing; onActivated: win.captureLeft > 0 ? win.stopCapture() : win.editor.selectNone() }
    readonly property real step: editor.snapUnit > 0 ? editor.snapUnit : 0.25
    Shortcut { sequence: "Left"; enabled: !win.typing; onActivated: win.nudge(-win.step, 0) }
    Shortcut { sequence: "Right"; enabled: !win.typing; onActivated: win.nudge(win.step, 0) }
    Shortcut { sequence: "Up"; enabled: !win.typing; onActivated: win.nudge(0, -win.step) }
    Shortcut { sequence: "Down"; enabled: !win.typing; onActivated: win.nudge(0, win.step) }
    Shortcut { sequence: "Shift+Left"; enabled: !win.typing; onActivated: win.nudge(-1, 0) }
    Shortcut { sequence: "Shift+Right"; enabled: !win.typing; onActivated: win.nudge(1, 0) }
    Shortcut { sequence: "Shift+Up"; enabled: !win.typing; onActivated: win.nudge(0, -1) }
    Shortcut { sequence: "Shift+Down"; enabled: !win.typing; onActivated: win.nudge(0, 1) }

    function save() {
        if (!editor.deviceId)
            return false
        return editor.save(false).length > 0
    }

    // ── layout ──

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 6
            ComboBox {
                // Focus by Tab only: after a click, typing on the keyboard must not
                // pick another option (letters jump to matching entries).
                focusPolicy: Qt.TabFocus
                id: deviceBox
                Layout.preferredWidth: 180
                readonly property var ids: { void win.settings.revision; return JSON.parse(win.settings.deviceIds || "[]") }
                model: { void win.settings.revision; return ids.map(id => ({ id: id, name: JSON.parse(win.settings.device(id)).name })) }
                textRole: "name"
                valueRole: "id"
                currentIndex: Math.max(0, ids.indexOf(win.editor.deviceId))
                onActivated: index => {
                    const id = ids[index]
                    currentIndex = Qt.binding(() => Math.max(0, ids.indexOf(win.editor.deviceId)))
                    if (id !== win.editor.deviceId)
                        win.whenSaved(() => { win.endSession(); win.openDevice(id) })
                }
            }
            ToolSeparator {}
            ToolButton { text: qsTr("Add"); onClicked: win.editor.addKey(canvas.centreUnits().x, canvas.centreUnits().y) }
            ToolButton { text: qsTr("Duplicate"); enabled: win.selection.length > 0; onClicked: win.editor.duplicateSelection() }
            ToolButton { text: qsTr("Delete"); enabled: win.selection.length > 0; onClicked: win.editor.deleteSelection() }
            ToolSeparator {}
            ToolButton { text: qsTr("Undo"); enabled: win.editor.canUndo; onClicked: win.editor.undo() }
            ToolButton { text: qsTr("Redo"); enabled: win.editor.canRedo; onClicked: win.editor.redo() }
            ToolSeparator {}
            CheckBox {
                text: qsTr("Snap")
                checked: win.editor.snap
                onToggled: win.editor.setSnapping(checked)
            }
            Label { text: qsTr("Zoom") }
            Slider {
                id: zoomSlider
                Layout.preferredWidth: 120
                from: win.minUnit; to: win.maxUnit; value: win.unit
                onMoved: win.unit = value
                // Moving the slider breaks its binding; follow wheel zooms too.
                Connections {
                    target: win
                    function onUnitChanged() { zoomSlider.value = win.unit }
                }
            }
            Item { Layout.fillWidth: true }
            ToolButton { text: qsTr("Save"); enabled: win.editor.dirty; onClicked: win.save() }
        }
    }

    footer: Pane {
        padding: 6
        ColumnLayout {
            anchors.fill: parent
            // Save errors; "changed" means someone else edited the file (FR-017).
            RowLayout {
                visible: win.editor.error.length > 0
                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    color: "#e57373"
                    text: win.editor.error === "changed"
                          ? qsTr("%1 changed on disk since it was opened.").arg(win.editor.savePath)
                          : win.editor.error
                }
                Button { visible: win.editor.error === "changed"; text: qsTr("Overwrite"); onClicked: win.editor.save(true) }
                Button { visible: win.editor.error === "changed"; text: qsTr("Reload"); onClicked: win.editor.reload() }
            }
            Label {
                Layout.fillWidth: true
                elide: Text.ElideMiddle
                color: palette.placeholderText
                readonly property var c: win.flags.counts || {}
                text: qsTr("%1 keys · %2 unbound · %3 duplicate · %4 not in config · %5 overlapping · saves to %6")
                      .arg(win.layout.keys.length).arg(c.unbound || 0).arg(c.duplicate || 0)
                      .arg(c.notInConfig || 0).arg(c.overlapping || 0).arg(win.editor.savePath)
            }
        }
    }

    SplitView {
        anchors.fill: parent

        // ── canvas ──
        Flickable {
            id: canvas
            SplitView.fillWidth: true
            clip: true
            readonly property real margin: 3 // units around the keys
            contentWidth: Math.max(width, (win.viewBounds.w + Math.max(0, win.viewBounds.x) + 2 * margin) * win.unit)
            contentHeight: Math.max(height, (win.viewBounds.h + Math.max(0, win.viewBounds.y) + 2 * margin) * win.unit)
            interactive: false
            ScrollBar.vertical: ScrollBar {}
            ScrollBar.horizontal: ScrollBar {}

            function clampView() {
                contentX = Math.max(0, Math.min(contentWidth - width, contentX))
                contentY = Math.max(0, Math.min(contentHeight - height, contentY))
            }

            // Zoom to `unit`, keeping the layout point under the viewport point
            // (vx, vy) where it is.
            property bool zooming: false
            function zoomAt(newUnit, vx, vy) {
                newUnit = Math.max(win.minUnit, Math.min(win.maxUnit, newUnit))
                if (newUnit === win.unit)
                    return
                const ux = (contentX + vx - keys.x) / win.unit
                const uy = (contentY + vy - keys.y) / win.unit
                zooming = true
                win.unit = newUnit
                zooming = false
                contentX = keys.x + ux * newUnit - vx
                contentY = keys.y + uy * newUnit - vy
                clampView()
            }

            // Mouse wheel zooms around the pointer.
            WheelHandler {
                target: null
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onWheel: event => {
                    // Alt+wheel may arrive as horizontal scrolling.
                    const delta = event.angleDelta.y || event.angleDelta.x
                    if ((event.modifiers & Qt.AltModifier) && win.refImage) {
                        // Scale the reference image around the pointer; one undo
                        // step once the wheel stops.
                        const k = keys.mapFromItem(canvas.contentItem, point.position.x, point.position.y)
                        if (!bgWheelDone.running)
                            win.bgAnchor = Qt.point(k.x / win.unit, k.y / win.unit)
                        win.bgScale *= Math.pow(1.0015, delta)
                        bgWheelDone.restart()
                    } else {
                        const p = canvas.mapFromItem(canvas.contentItem, point.position.x, point.position.y)
                        canvas.zoomAt(win.unit * Math.pow(1.0015, delta), p.x, p.y)
                    }
                    event.accepted = true
                }
            }
            Timer {
                id: bgWheelDone
                interval: 400
                onTriggered: win.commitBackground()
            }

            function centreUnits() {
                const p = keys.mapFromItem(canvas, canvas.width / 2, canvas.height / 2)
                return Qt.point(p.x / win.unit - 0.5, p.y / win.unit - 0.5)
            }

            Rectangle {
                width: canvas.contentWidth
                height: canvas.contentHeight
                color: "#17181b"
            }

            // Background interaction: select, move, rubber band (R11).
            MouseArea {
                id: area
                width: canvas.contentWidth
                height: canvas.contentHeight
                // Left: select, move, box. Right or middle: pan.
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                property string mode: "" // "move" | "band" | "pan"
                property point panFrom
                property point panContent
                property point start
                property point now
                property bool moved: false
                function units(mouse) {
                    const p = keys.mapFromItem(area, mouse.x, mouse.y)
                    return Qt.point(p.x / win.unit, p.y / win.unit)
                }
                onPressed: mouse => {
                    win.stopCapture()
                    if (mouse.button !== Qt.LeftButton) {
                        mode = "pan"
                        panFrom = area.mapToItem(canvas, mouse.x, mouse.y)
                        panContent = Qt.point(canvas.contentX, canvas.contentY)
                        return
                    }
                    start = units(mouse); now = start; moved = false
                    if ((mouse.modifiers & Qt.AltModifier) && win.refImage) {
                        mode = "bg"
                        return
                    }
                    const hit = win.keyAt(start.x, start.y)
                    const modifier = mouse.modifiers & (Qt.ShiftModifier | Qt.ControlModifier)
                    if (hit >= 0) {
                        if (modifier)
                            win.editor.select(JSON.stringify([hit]), "toggle")
                        else if (!win.selection.includes(hit))
                            win.editor.select(JSON.stringify([hit]), "replace")
                        mode = win.selection.includes(hit) ? "move" : ""
                    } else {
                        mode = "band"
                    }
                }
                onPositionChanged: mouse => {
                    if (mode === "pan") {
                        const p = area.mapToItem(canvas, mouse.x, mouse.y)
                        canvas.contentX = panContent.x - (p.x - panFrom.x)
                        canvas.contentY = panContent.y - (p.y - panFrom.y)
                        canvas.clampView()
                        return
                    }
                    now = units(mouse)
                    const dx = now.x - start.x, dy = now.y - start.y
                    if (mode === "bg") {
                        win.bgMoveX = dx; win.bgMoveY = dy
                        return
                    }
                    if (Math.abs(dx) + Math.abs(dy) > 0.05)
                        moved = true
                    if (mode === "move" && moved) {
                        win.moveX = win.snapped(dx, win.editor.snapUnit)
                        win.moveY = win.snapped(dy, win.editor.snapUnit)
                    }
                }
                onReleased: mouse => {
                    if (mode === "pan") {
                        mode = ""
                        return
                    }
                    if (mode === "bg") {
                        mode = ""
                        win.commitBackground()
                        return
                    }
                    const dx = now.x - start.x, dy = now.y - start.y
                    if (mode === "move" && moved) {
                        win.moveX = 0; win.moveY = 0
                        win.editor.moveSelection(dx, dy)
                    } else if (mode === "band") {
                        const add = mouse.modifiers & (Qt.ShiftModifier | Qt.ControlModifier)
                        if (moved)
                            win.editor.selectRect(start.x, start.y, dx, dy, add ? "add" : "replace")
                        else if (!add)
                            win.editor.selectNone()
                    }
                    mode = ""
                }
            }

            Item {
                id: keys
                x: (canvas.margin - Math.min(0, win.viewBounds.x)) * win.unit
                y: (canvas.margin - Math.min(0, win.viewBounds.y)) * win.unit
                // When the canvas grows left or up, scroll by as much, so what
                // is on screen stays put (zoomAt places the view itself).
                property real lastX: x
                property real lastY: y
                onXChanged: {
                    if (!canvas.zooming)
                        canvas.contentX += x - lastX
                    lastX = x
                    Qt.callLater(canvas.clampView)
                }
                onYChanged: {
                    if (!canvas.zooming)
                        canvas.contentY += y - lastY
                    lastY = y
                    Qt.callLater(canvas.clampView)
                }

                // The reference image, under the keys (editor only).
                Image {
                    id: bgImage
                    visible: !!win.bgShown
                    source: win.refImage ? win.fileUrl(win.refImage.path) : ""
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit
                    x: win.bgShown ? win.bgShown.x * win.unit : 0
                    y: win.bgShown ? win.bgShown.y * win.unit : 0
                    width: win.bgShown ? win.bgShown.width * win.unit : 0
                    height: implicitWidth > 0 ? width * implicitHeight / implicitWidth : 0
                    opacity: win.bgShown ? win.bgShown.opacity : 1
                }

                Repeater {
                    model: win.layout.keys
                    delegate: EditorKey {
                        required property var modelData
                        required property int index
                        readonly property bool isSelected: win.selection.includes(index)
                        key: modelData
                        unit: win.unit
                        canvas: keys
                        flags: (win.flags.keys || [])[index] || ({})
                        label: modelData.kanataCanonical ? win.labels[modelData.kanataCanonical] : undefined
                        selected: isSelected
                        pressed: !!modelData.kanataCanonical && win.pressedKeys.includes(modelData.kanataCanonical)
                        tapHold: (modelData.kanataCanonical && win.tapHolds[modelData.kanataCanonical]) || ""
                        handles: isSelected && win.selection.length === 1
                        moveX: isSelected ? win.moveX : 0
                        moveY: isSelected ? win.moveY : 0
                        extraAngle: isSelected ? win.extraAngle : 0
                        pivot: win.pivot
                        growEdge: isSelected ? win.growEdge : ""
                        grow: isSelected ? win.grow : 0
                        onResizeDragged: (edge, delta) => { win.growEdge = edge; win.grow = win.snapped(delta, win.editor.snapUnit) }
                        onResizeDone: (edge, delta) => { win.growEdge = ""; win.grow = 0; win.editor.resizeSelection(edge, delta) }
                        onRotateDragged: degrees => { win.pivot = win.selectionCentre; win.extraAngle = win.snapped(degrees, win.editor.snapAngle) }
                        onRotateDone: degrees => { win.extraAngle = 0; win.editor.rotateSelection(degrees) }
                    }
                }

                // Rubber band.
                Rectangle {
                    visible: area.mode === "band" && area.moved
                    x: Math.min(area.start.x, area.now.x) * win.unit
                    y: Math.min(area.start.y, area.now.y) * win.unit
                    width: Math.abs(area.now.x - area.start.x) * win.unit
                    height: Math.abs(area.now.y - area.start.y) * win.unit
                    color: "#226aa0ff"
                    border.color: "#6aa0ff"
                }

                // Several keys: one rotate handle above the selection's box.
                Rectangle {
                    id: groupRotor
                    visible: win.selection.length > 1
                    width: 12; height: 12; radius: 6
                    color: "#1e1f22"
                    border.color: "#6aa0ff"
                    border.width: 2
                    x: (win.selectionBounds.x + win.selectionBounds.w / 2) * win.unit - width / 2
                    y: win.selectionBounds.y * win.unit - 28
                    property real startAngle: 0
                    function angle(mouse) {
                        const p = groupRotor.mapToItem(keys, mouse.x, mouse.y)
                        return Math.atan2(p.y / win.unit - win.pivot.y, p.x / win.unit - win.pivot.x) * 180 / Math.PI
                    }
                    function delta(mouse) {
                        let d = angle(mouse) - startAngle
                        while (d > 180) d -= 360
                        while (d <= -180) d += 360
                        return d
                    }
                    MouseArea {
                        anchors.fill: parent
                        anchors.margins: -4
                        cursorShape: Qt.CrossCursor
                        preventStealing: true
                        onPressed: mouse => { win.pivot = win.selectionCentre; groupRotor.startAngle = groupRotor.angle(mouse) }
                        onPositionChanged: mouse => win.extraAngle = win.snapped(groupRotor.delta(mouse), win.editor.snapAngle)
                        onReleased: mouse => { win.extraAngle = 0; win.editor.rotateSelection(groupRotor.delta(mouse)) }
                    }
                }
            }
        }

        // ── inspector ──
        ScrollView {
            SplitView.preferredWidth: 300
            SplitView.minimumWidth: 240
            contentWidth: availableWidth

            ColumnLayout {
                width: parent.width
                spacing: 8

                Label {
                    Layout.margins: 10
                    font.bold: true
                    text: win.selection.length === 0 ? qsTr("Nothing selected")
                          : win.selection.length === 1 ? qsTr("1 key") : qsTr("%1 keys").arg(win.selection.length)
                }

                GridLayout {
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Layout.fillWidth: true
                    columns: 2
                    columnSpacing: 8
                    rowSpacing: 6
                    enabled: win.selection.length > 0

                    // Binding (FR-006): the config's unbound keys first.
                    Label { text: qsTr("Key") }
                    RowLayout {
                        Layout.fillWidth: true
                        enabled: !!win.single
                        TextField {
                            id: bindField
                            Layout.fillWidth: true
                            placeholderText: win.single ? qsTr("unbound: pick or type") : ""
                            text: win.single ? (win.single.kanata || "") : ""
                            onTextEdited: picker.open()
                            onActiveFocusChanged: if (activeFocus) picker.open()
                            onAccepted: {
                                if (win.editor.bind(text))
                                    picker.close()
                            }
                            Popup {
                                id: picker
                                y: bindField.height
                                width: bindField.width + 60
                                height: 260
                                padding: 2
                                ListView {
                                    id: names
                                    anchors.fill: parent
                                    clip: true
                                    model: picker.visible ? JSON.parse(win.editor.keyNames(bindField.text === (win.single ? win.single.kanata || "" : "") ? "" : bindField.text)) : []
                                    ScrollBar.vertical: ScrollBar {}
                                    // A header above the first entry of each group.
                                    delegate: Column {
                                        id: entry
                                        required property var modelData
                                        required property int index
                                        width: names.width
                                        Label {
                                            visible: entry.index === 0 || names.model[entry.index - 1].group !== entry.modelData.group
                                            padding: 4
                                            font.pixelSize: 11
                                            color: palette.placeholderText
                                            text: entry.modelData.group === "unbound" ? qsTr("In the config, not on the layout")
                                                  : entry.modelData.group === "config" ? qsTr("In the config") : qsTr("Other Kanata keys")
                                        }
                                        ItemDelegate {
                                            width: names.width
                                            text: entry.modelData.name + (entry.modelData.aliases.length ? "   " + entry.modelData.aliases.join(" ") : "")
                                            onClicked: {
                                                win.editor.bind(entry.modelData.name)
                                                picker.close()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Button {
                            text: qsTr("Clear")
                            enabled: !!(win.single && win.single.kanata)
                            onClicked: win.editor.bind("")
                        }
                    }
                    Item { width: 1; height: 1 }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Button {
                            Layout.fillWidth: true
                            enabled: win.canCapture
                            highlighted: win.captureLeft > 0
                            text: win.captureLeft > 0 ? qsTr("Press a key… (%1 s)").arg(win.captureLeft) : qsTr("Press to bind")
                            onClicked: win.captureLeft > 0 ? win.stopCapture() : win.startCapture()
                        }
                        Label {
                            Layout.fillWidth: true
                            visible: !!win.single && !win.canCapture
                            wrapMode: Text.Wrap
                            font.pixelSize: 11
                            color: palette.placeholderText
                            text: qsTr("Needs a Kanata started by the overlay, with highlighting on.")
                        }
                    }

                    Label { text: qsTr("Legend") }
                    TextField {
                        Layout.fillWidth: true
                        enabled: !!win.single
                        text: win.single ? (win.single.legend || "") : ""
                        maximumLength: 32
                        onEditingFinished: if (win.single && text !== (win.single.legend || "")) win.editor.setLegend(text)
                    }

                    component GeometryField: TextField {
                        required property string field
                        Layout.fillWidth: true
                        // One value for one key; for several, x/y of their box and
                        // a shared size or angle, else "—".
                        readonly property var keysNow: win.selection.map(i => win.layout.keys[i]).filter(k => !!k)
                        readonly property var value: {
                            if (keysNow.length === 0) return ""
                            if (field === "x" && keysNow.length > 1) return win.selectionBounds.x
                            if (field === "y" && keysNow.length > 1) return win.selectionBounds.y
                            const v = keysNow.map(k => field === "rotation" ? (k.rotation || 0) : k[field])
                            return v.every(x => Math.abs(x - v[0]) < 1e-6) ? v[0] : "—"
                        }
                        text: value === "" || value === "—" ? String(value) : String(Math.round(value * 1000) / 1000)
                        validator: RegularExpressionValidator { regularExpression: /^(-?\d*\.?\d*|—)$/ }
                        onEditingFinished: {
                            const v = parseFloat(text)
                            if (!isNaN(v) && String(v) !== String(Math.round(value * 1000) / 1000))
                                win.editor.setGeometry(field, v)
                        }
                    }
                    Label { text: qsTr("X") }
                    GeometryField { field: "x" }
                    Label { text: qsTr("Y") }
                    GeometryField { field: "y" }
                    Label { text: qsTr("Width") }
                    GeometryField { field: "width" }
                    Label { text: qsTr("Height") }
                    GeometryField { field: "height" }
                    Label { text: qsTr("Rotation °") }
                    GeometryField { field: "rotation" }
                }

                // Reference image (e.g. a screenshot of the vendor's configurator).
                MenuSeparator { Layout.fillWidth: true }
                Label { Layout.leftMargin: 10; font.bold: true; text: qsTr("Reference image") }
                RowLayout {
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Button { text: win.refImage ? qsTr("Replace…") : qsTr("Choose image…"); onClicked: imageDialog.open() }
                    Button { text: qsTr("Remove"); visible: !!win.refImage; onClicked: win.editor.clearBackground() }
                }
                Label {
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Layout.fillWidth: true
                    visible: !!win.refImage && bgImage.status === Image.Error
                    wrapMode: Text.Wrap
                    color: "#e57373"
                    text: qsTr("Cannot load %1").arg(win.refImage ? win.refImage.path : "")
                }
                GridLayout {
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Layout.fillWidth: true
                    visible: !!win.refImage
                    columns: 2
                    columnSpacing: 8
                    rowSpacing: 6
                    component BackgroundField: TextField {
                        required property string field
                        Layout.fillWidth: true
                        text: win.refImage ? String(Math.round(win.refImage[field] * 1000) / 1000) : ""
                        validator: DoubleValidator {}
                        onEditingFinished: {
                            const b = win.refImage, v = parseFloat(text)
                            if (!b || isNaN(v) || Math.abs(v - b[field]) < 1e-9)
                                return
                            win.editor.setBackgroundGeometry(field === "x" ? v : b.x, field === "y" ? v : b.y,
                                                             field === "width" ? v : b.width, b.opacity)
                        }
                    }
                    Label { text: qsTr("X") }
                    BackgroundField { field: "x" }
                    Label { text: qsTr("Y") }
                    BackgroundField { field: "y" }
                    Label { text: qsTr("Width") }
                    BackgroundField { field: "width" }
                    Label { text: qsTr("Opacity") }
                    Slider {
                        Layout.fillWidth: true
                        from: 0.05; to: 1
                        value: win.refImage ? win.refImage.opacity : 0.5
                        onPressedChanged: if (!pressed && win.refImage)
                            win.editor.setBackgroundGeometry(win.refImage.x, win.refImage.y, win.refImage.width, value)
                    }
                }
                Label {
                    Layout.leftMargin: 10
                    Layout.rightMargin: 10
                    Layout.fillWidth: true
                    visible: !!win.refImage
                    wrapMode: Text.Wrap
                    font.pixelSize: 11
                    color: palette.placeholderText
                    text: qsTr("Alt+drag moves the image, Alt+wheel scales it. Match one key in the picture to one key on the grid, then place the keys over it.")
                }
                MenuSeparator { Layout.fillWidth: true }

                // What the selected key does now (FR-004).
                Label {
                    Layout.margins: 10
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    visible: !!win.single
                    color: palette.placeholderText
                    readonly property var f: win.single ? (win.flags.keys || [])[win.selection[0]] || {} : {}
                    readonly property var l: win.single && win.single.kanataCanonical ? win.labels[win.single.kanataCanonical] : undefined
                    text: !win.single ? ""
                          : f.unbound ? qsTr("Unbound: the board can't show what this key does.")
                          : f.notInConfig ? qsTr("“%1” is not in the config.").arg(win.single.kanata)
                          : l ? qsTr("Does: %1").arg(l.text) + (f.duplicate ? qsTr(" (also on another key)") : "")
                          : ""
                }

                Label {
                    Layout.margins: 10
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: 11
                    color: palette.placeholderText
                    text: qsTr("Click to select, Shift/Ctrl+click to add, drag on the background for a box. Drag keys to move; handles resize and rotate. Arrows nudge (Shift: 1u). Wheel zooms, right-drag pans.")
                }
            }
        }
    }

    // ── dialogs ──

    Dialog {
        id: unsavedDialog
        anchors.centerIn: parent
        title: qsTr("Save the layout of %1?").arg(win.editor.deviceName)
        standardButtons: Dialog.Save | Dialog.Discard | Dialog.Cancel
        Label { text: qsTr("It has unsaved changes.") }
        onAccepted: {
            if (win.save() && win.pendingAction)
                win.pendingAction()
            win.pendingAction = null
        }
        onDiscarded: {
            if (win.device)
                win.device.endPreview()
            const action = win.pendingAction
            win.pendingAction = null
            win.editor.close()
            close()
            if (action)
                action()
        }
        onRejected: win.pendingAction = null
    }

    FileDialog {
        id: imageDialog
        title: qsTr("Reference image")
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg *.webp *.bmp)"), qsTr("All files (*)")]
        onAccepted: win.editor.setBackgroundImage(decodeURIComponent(selectedFile.toString().replace(/^file:\/\//, "")))
    }

    FileDialog {
        id: saveAsDialog
        title: qsTr("The device was removed: save its layout as…")
        fileMode: FileDialog.SaveFile
        nameFilters: [qsTr("Layouts (*.json)")]
        onAccepted: {
            const path = decodeURIComponent(selectedFile.toString().replace(/^file:\/\//, ""))
            if (win.editor.saveAs(path).length > 0)
                win.finishClose()
        }
        onRejected: win.finishClose()
    }
}
