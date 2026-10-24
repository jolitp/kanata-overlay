import QtQuick

// One key on the layout editor's canvas (feature 005). Drawn like KeyCap, plus
// its flags, a selection outline and its bound name. While a gesture is in
// progress the window passes a transient offset, angle or edge change, which is
// shown here only; the change itself is committed in Rust on release (R4).
//
// Flags differ from the board's single "unbound" border: here a key without a
// name and a key whose name isn't in the config are told apart (FR-005).
Item {
    id: cap
    required property var key
    required property real unit
    property var flags: ({})
    property var label
    property bool selected: false
    /// Held on the device right now, and its tap-hold result (feature 006).
    property bool pressed: false
    property string tapHold: ""
    /// Only a single selected key gets resize and rotate handles.
    property bool handles: false
    /// The canvas content item, to map pointer positions into units.
    property Item canvas

    // Transient gesture state (units, degrees) from the window.
    property real moveX: 0
    property real moveY: 0
    property real extraAngle: 0
    property point pivot: Qt.point(0, 0)
    property string growEdge: ""
    property real grow: 0

    signal resizeDragged(string edge, real delta)
    signal resizeDone(string edge, real delta)
    signal rotateDragged(real degrees)
    signal rotateDone(real degrees)

    readonly property real gap: 3
    readonly property real kx: key.x + moveX - (growEdge === "left" ? grow : 0)
    readonly property real ky: key.y + moveY - (growEdge === "top" ? grow : 0)
    readonly property real kw: Math.max(0.25, key.width + (growEdge === "left" || growEdge === "right" ? grow : 0))
    readonly property real kh: Math.max(0.25, key.height + (growEdge === "top" || growEdge === "bottom" ? grow : 0))

    x: kx * unit
    y: ky * unit
    width: kw * unit
    height: kh * unit

    transform: [
        // KLE rotates around (rx, ry), absolute layout coordinates.
        Rotation {
            origin.x: ((cap.key.rx || 0) + cap.moveX - cap.kx) * cap.unit
            origin.y: ((cap.key.ry || 0) + cap.moveY - cap.ky) * cap.unit
            angle: cap.key.rotation || 0
        },
        // A rotation being dragged, around the selection's centre.
        Rotation {
            origin.x: (cap.pivot.x - cap.kx) * cap.unit
            origin.y: (cap.pivot.y - cap.ky) * cap.unit
            angle: cap.extraAngle
        }
    ]

    Rectangle {
        anchors.fill: parent
        anchors.margins: cap.gap
        radius: 6
        color: cap.pressed ? "#3d4048" : cap.flags.overlapping ? "#4a2b2b" : "#2b2d31"
        border.width: cap.selected ? 2 : 1
        border.color: cap.selected || cap.pressed ? "#6aa0ff"
                    : cap.flags.unbound ? "#d2691e"
                    : cap.flags.notInConfig ? "#b8a040"
                    : "#4a4d55"

        Column {
            anchors.centerIn: parent
            width: parent.width - 6

            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                fontSizeMode: Text.Fit
                font.pixelSize: 13
                minimumPixelSize: 7
                height: Math.max(10, (cap.height - 2 * cap.gap) * 0.55)
                verticalAlignment: Text.AlignVCenter
                color: cap.label ? "#e8e8e8" : "#a8a8a8"
                text: cap.label ? cap.label.text : (cap.key.legend || cap.key.kanata || "")
            }
            // What the key is bound to; "?" when unbound.
            Text {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                font.pixelSize: 9
                color: cap.flags.unbound ? "#d2691e" : cap.flags.notInConfig ? "#b8a040" : "#7a7d85"
                text: cap.key.kanata ? cap.key.kanata + (cap.flags.notInConfig ? " ∉" : "") : "?"
            }
        }

        // Tap-hold result.
        Rectangle {
            visible: cap.tapHold.length > 0
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.margins: 3
            width: thBadge.implicitWidth + 6
            height: thBadge.implicitHeight + 2
            radius: 3
            color: cap.tapHold === "hold" ? "#b8860b" : "#2e7d32"
            Text { id: thBadge; anchors.centerIn: parent; font.pixelSize: 8; font.bold: true; color: "#ffffff"; text: cap.tapHold }
        }

        // Duplicate binding.
        Text {
            visible: !!cap.flags.duplicate
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: 3
            font.pixelSize: 9
            color: "#e0a050"
            text: "2×"
        }
    }

    // ── handles (single selection) ──

    component EdgeHandle: Rectangle {
        id: handle
        required property string edge
        width: 9; height: 9; radius: 2
        color: "#6aa0ff"
        visible: cap.handles
        property point start
        // The pointer's travel along the key's axis for this edge, in units.
        function along(mouse) {
            const p = handle.mapToItem(cap.canvas, mouse.x, mouse.y)
            const a = (cap.key.rotation || 0) * Math.PI / 180
            const dx = (p.x - start.x) / cap.unit, dy = (p.y - start.y) / cap.unit
            const horizontal = edge === "left" || edge === "right"
            const d = horizontal ? dx * Math.cos(a) + dy * Math.sin(a) : -dx * Math.sin(a) + dy * Math.cos(a)
            return edge === "left" || edge === "top" ? -d : d
        }
        MouseArea {
            anchors.fill: parent
            anchors.margins: -4
            cursorShape: handle.edge === "left" || handle.edge === "right" ? Qt.SizeHorCursor : Qt.SizeVerCursor
            preventStealing: true
            onPressed: mouse => handle.start = handle.mapToItem(cap.canvas, mouse.x, mouse.y)
            onPositionChanged: mouse => cap.resizeDragged(handle.edge, handle.along(mouse))
            onReleased: mouse => cap.resizeDone(handle.edge, handle.along(mouse))
        }
    }

    EdgeHandle { edge: "left"; x: -width / 2; y: (cap.height - height) / 2 }
    EdgeHandle { edge: "right"; x: cap.width - width / 2; y: (cap.height - height) / 2 }
    EdgeHandle { edge: "top"; x: (cap.width - width) / 2; y: -height / 2 }
    EdgeHandle { edge: "bottom"; x: (cap.width - width) / 2; y: cap.height - height / 2 }

    // Rotate handle above the key, rotating around its centre.
    Rectangle {
        id: rotor
        visible: cap.handles
        width: 12; height: 12; radius: 6
        color: "#1e1f22"
        border.color: "#6aa0ff"
        border.width: 2
        x: (cap.width - width) / 2
        y: -24
        Rectangle { x: parent.width / 2 - 1; y: parent.height; width: 2; height: 12; color: "#6aa0ff" }
        property real startAngle: 0
        function angle(mouse) {
            const p = rotor.mapToItem(cap.canvas, mouse.x, mouse.y)
            return Math.atan2(p.y / cap.unit - cap.pivot.y, p.x / cap.unit - cap.pivot.x) * 180 / Math.PI
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
            onPressed: mouse => rotor.startAngle = rotor.angle(mouse)
            onPositionChanged: mouse => cap.rotateDragged(rotor.delta(mouse))
            onReleased: mouse => cap.rotateDone(rotor.delta(mouse))
        }
    }
}
