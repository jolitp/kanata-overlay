import QtQuick
import QtQuick.Window
import io.github.jolitp.kanataoverlay

// Scaffold window: draws the physical layout and shows the active Kanata layer.
// Turning this into a click-through, always-on-top layer-shell overlay is the
// first Spec Kit feature (see README).
Window {
    id: root
    visible: true
    color: "#1e1f22"
    title: "kanata-overlay"

    readonly property real unit: 54 // pixels per keyboard unit (1u)
    readonly property real pad: 16
    property var layout: overlay.layoutJson.length > 0 ? JSON.parse(overlay.layoutJson) : ({ name: "", keys: [] })
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

    width: Math.max(360, bounds.w * unit + 2 * pad)
    height: bounds.h * unit + 2 * pad + header.height + 8

    OverlayState {
        id: overlay
        Component.onCompleted: start()
    }

    Row {
        id: header
        x: root.pad
        y: 8
        height: 32
        spacing: 12

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 10; height: 10; radius: 5
            color: overlay.connected ? "#4caf50" : "#9e9e9e"
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: "#e8e8e8"
            font.pixelSize: 16
            font.bold: true
            text: overlay.connected ? (overlay.currentLayer || "…") : "kanata not connected"
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            color: "#8a8a8a"
            font.pixelSize: 12
            text: overlay.status
        }
    }

    Item {
        id: board
        x: root.pad
        y: header.y + header.height + 8

        Repeater {
            model: root.layout.keys
            delegate: KeyCap {
                required property var modelData
                key: modelData
                unit: root.unit
            }
        }
    }
}
