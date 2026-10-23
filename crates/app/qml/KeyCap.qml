import QtQuick

// One physical key. `key` is an entry from the layout JSON
// ({x, y, width, height, rotation?, rx?, ry?, kanata?, kanataCanonical?, legend?})
// in keyboard units. `label` is its label on the active layer
// ({text, style, hold, alias}, contracts/labels.md), or undefined.
Item {
    id: cap
    required property var key
    required property real unit
    property var label
    property bool hasConfig: false
    readonly property real gap: 3

    // Unbound: no Kanata name, or a config is loaded and this key isn't in defsrc (FR-008).
    readonly property bool unbound: !key.kanata || (hasConfig && !label)

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
        border.color: cap.unbound ? "#a0522d" : "#4a4d55" // unbound keys stand out

        Column {
            anchors.centerIn: parent
            width: parent.width - 6

            Text {
                width: parent.width
                height: cap.label && cap.label.alias ? (cap.height - 2 * cap.gap) * 0.6 : cap.height - 2 * cap.gap - 6
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                wrapMode: Text.NoWrap
                elide: Text.ElideRight
                fontSizeMode: Text.Fit
                font.pixelSize: 13
                minimumPixelSize: 7
                color: !cap.label ? "#e8e8e8"
                      : cap.label.style === "inherited" ? "#8a8a8a"
                      : cap.label.style === "disabled" ? "#555555"
                      : "#e8e8e8"
                text: cap.label ? cap.label.text : (cap.key.kanata || cap.key.legend || "")
            }

            // The alias the action came from (US2 scenario 3).
            Text {
                visible: !!(cap.label && cap.label.alias)
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                font.pixelSize: 9
                color: "#6a6a6a"
                text: cap.label && cap.label.alias ? "@" + cap.label.alias : ""
            }
        }

        // Tap-hold: the key also does something when held (clarification Q2).
        Rectangle {
            visible: !!(cap.label && cap.label.hold)
            width: 5
            height: 5
            radius: 2.5
            color: "#6aa0ff"
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: 4
        }
    }
}
