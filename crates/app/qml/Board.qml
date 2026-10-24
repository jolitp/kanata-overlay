import QtQuick

// The picture of the keyboard: a header with the connection state and layer,
// and one KeyCap per physical key. Sized at 100% scale; the overlay windows
// scale it.
Item {
    id: board
    // DeviceState: connection, layer, labels, Kanata session of this device.
    required property var device
    // OverlayState: global notices (shown on the first board only).
    required property var overlay
    property bool first: false
    property real unit: 54 // pixels per keyboard unit (1u)
    readonly property real pad: 16

    readonly property var layout: device.layoutJson.length > 0 ? JSON.parse(device.layoutJson) : ({ name: "", keys: [] })
    // Labels for the active layer, keyed by canonical Kanata key name; {} without a config.
    readonly property var labels: JSON.parse(device.labelsJson)
    readonly property bool hasConfig: Object.keys(labels).length > 0
    readonly property var bounds: {
        // Bounding box of all key corners, after KLE rotation around (rx, ry).
        // Keys may sit left of or above 0 while the layout editor previews.
        if (layout.keys.length === 0)
            return { x: 0, y: 0, w: 0, h: 0 }
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity
        for (const k of layout.keys) {
            const a = (k.rotation || 0) * Math.PI / 180
            const rx = k.rx || 0, ry = k.ry || 0
            for (const [cx, cy] of [[k.x, k.y], [k.x + k.width, k.y],
                                    [k.x, k.y + k.height], [k.x + k.width, k.y + k.height]]) {
                const dx = cx - rx, dy = cy - ry
                const px = rx + dx * Math.cos(a) - dy * Math.sin(a)
                const py = ry + dx * Math.sin(a) + dy * Math.cos(a)
                x0 = Math.min(x0, px); y0 = Math.min(y0, py)
                x1 = Math.max(x1, px); y1 = Math.max(y1, py)
            }
        }
        // Saved layouts start at (0, 0); keep that origin unless keys go past it.
        x0 = Math.min(x0, 0); y0 = Math.min(y0, 0)
        return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }

    // Size at 100% scale.
    readonly property size baseSize: Qt.size(Math.max(360, bounds.w * unit + 2 * pad),
                                             bounds.h * unit + 2 * pad + header.height + 8)
    width: baseSize.width
    height: baseSize.height

    Rectangle {
        anchors.fill: parent
        radius: 10
        color: "#1e1f22"
    }

    Row {
        id: header
        x: board.pad
        y: 8
        height: 32
        spacing: 12

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 10; height: 10; radius: 5
            color: board.device.connected ? "#4caf50" : "#9e9e9e"
        }
        Text {
            id: layerName
            anchors.verticalCenter: parent.verticalCenter
            color: "#e8e8e8"
            font.pixelSize: 16
            font.bold: true
            text: (board.device.name ? board.device.name + " · " : "")
                  + (board.device.connected ? (board.device.currentLayer || "…") : "not connected")
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: board.width - 2 * board.pad - layerName.width - 10 - 2 * header.spacing
            elide: Text.ElideRight
            color: "#8a8a8a"
            font.pixelSize: 12
            // While a managed Kanata is down, its state says why; "connecting to …" adds nothing.
            readonly property bool managedDown: board.device.mode === "managed" && !board.device.connected
            text: [board.device.stateText,
                   managedDown ? "" : board.device.status,
                   board.device.notices,
                   board.first ? board.overlay.notices : ""]
                  .filter(s => s.length > 0).join(" · ")
        }
    }

    Item {
        x: board.pad - board.bounds.x * board.unit
        y: header.y + header.height + 8 - board.bounds.y * board.unit

        Repeater {
            model: board.layout.keys
            delegate: KeyCap {
                required property var modelData
                key: modelData
                unit: board.unit
                label: modelData.kanataCanonical ? board.labels[modelData.kanataCanonical] : undefined
                hasConfig: board.hasConfig
            }
        }
    }
}
