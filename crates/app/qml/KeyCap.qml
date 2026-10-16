import QtQuick

// One physical key. `key` is an entry from the layout JSON
// ({x, y, width, height, rotation?, rx?, ry?, kanata?, legend?}) in keyboard units.
Item {
    id: cap
    required property var key
    required property real unit
    readonly property real gap: 3

    x: key.x * unit
    y: key.y * unit
    width: key.width * unit
    height: key.height * unit

    // KLE rotates around (rx, ry), given in absolute layout coordinates.
    transform: Rotation {
        origin.x: ((cap.key.rx || 0) - cap.key.x) * cap.unit
        origin.y: ((cap.key.ry || 0) - cap.key.y) * cap.unit
        angle: cap.key.rotation || 0
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: cap.gap
        radius: 6
        color: "#2b2d31"
        border.width: 1
        border.color: cap.key.kanata ? "#4a4d55" : "#a0522d" // unbound keys stand out

        Text {
            anchors.centerIn: parent
            width: parent.width - 6
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            color: "#e8e8e8"
            font.pixelSize: 13
            // Later: the action from the active deflayer for this key.
            text: cap.key.kanata || cap.key.legend || ""
        }
    }
}
