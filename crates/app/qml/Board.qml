import QtQuick

// The picture of the keyboard: a header with the connection state and layer,
// and one KeyCap per physical key. Sized at 100% scale; the overlay windows
// scale it.
Item {
    id: board
    required property var overlay
    property real unit: 54 // pixels per keyboard unit (1u)
    readonly property real pad: 16

    readonly property var layout: overlay.layoutJson.length > 0 ? JSON.parse(overlay.layoutJson) : ({ name: "", keys: [] })
    readonly property var bounds: {
        // Bounding box of all key corners, after KLE rotation around (rx, ry).
        let w = 0, h = 0
        for (const k of layout.keys) {
            const a = (k.rotation || 0) * Math.PI / 180
            const rx = k.rx || 0, ry = k.ry || 0
            for (const [cx, cy] of [[k.x, k.y], [k.x + k.width, k.y],
                                    [k.x, k.y + k.height], [k.x + k.width, k.y + k.height]]) {
                const dx = cx - rx, dy = cy - ry
                w = Math.max(w, rx + dx * Math.cos(a) - dy * Math.sin(a))
                h = Math.max(h, ry + dx * Math.sin(a) + dy * Math.cos(a))
            }
        }
        return { w: w, h: h }
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
            color: board.overlay.connected ? "#4caf50" : "#9e9e9e"
        }
        Text {
            id: layerName
            anchors.verticalCenter: parent.verticalCenter
            color: "#e8e8e8"
            font.pixelSize: 16
            font.bold: true
            text: board.overlay.connected ? (board.overlay.currentLayer || "…") : "kanata not connected"
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: board.width - 2 * board.pad - layerName.width - 10 - 2 * header.spacing
            elide: Text.ElideRight
            color: "#8a8a8a"
            font.pixelSize: 12
            text: board.overlay.notices.length > 0
                  ? board.overlay.status + " · " + board.overlay.notices
                  : board.overlay.status
        }
    }

    Item {
        x: board.pad
        y: header.y + header.height + 8

        Repeater {
            model: board.layout.keys
            delegate: KeyCap {
                required property var modelData
                key: modelData
                unit: board.unit
            }
        }
    }
}
