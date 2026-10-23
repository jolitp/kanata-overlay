import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// An ordinary window for the overlay settings. Every change shows on the
// overlay at once (live preview) and is saved when committed.
ApplicationWindow {
    id: win
    required property var settings
    required property var overlay

    title: qsTr("Kanata Overlay Settings")
    width: 420
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

    function open() {
        show()
        raise()
        requestActivate()
    }

    // Load the current values into the controls each time the window opens.
    function refresh() {
        edgeBox.currentIndex = edgeBox.indexOfValue(settings.edge)
        marginBox.value = settings.margin
        opacitySlider.value = Math.round(settings.opacity * 100)
        scaleSlider.value = settings.scale
        monitorBox.currentIndex = Math.max(0, monitorBox.indexOfValue(settings.screenName))
    }

    onVisibleChanged: {
        if (visible)
            refresh()
        overlay.setPreviewing(visible)
    }
    onClosing: settings.save()

    GridLayout {
        id: form
        anchors.fill: parent
        anchors.margins: 16
        columns: 2
        rowSpacing: 10
        columnSpacing: 12

        Label { text: qsTr("Position") }
        ComboBox {
            id: edgeBox
            Layout.fillWidth: true
            model: win.edges
            textRole: "text"
            valueRole: "value"
            onActivated: {
                win.settings.updateEdge(currentValue)
                win.settings.save()
            }
        }

        Label { text: qsTr("Margin") }
        SpinBox {
            id: marginBox
            Layout.fillWidth: true
            from: 0
            to: 500
            editable: true
            onValueModified: {
                win.settings.updateMargin(value)
                win.settings.save()
            }
        }

        Label { text: qsTr("Opacity") }
        RowLayout {
            Slider {
                id: opacitySlider
                Layout.fillWidth: true
                from: 10
                to: 100
                stepSize: 1
                onMoved: win.settings.updateOpacityPercent(value)
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
                onMoved: win.settings.updateScalePercent(value)
                onPressedChanged: if (!pressed) win.settings.save()
            }
            Label { text: Math.round(scaleSlider.value) + "%" }
        }

        Label { text: qsTr("Monitor") }
        ComboBox {
            id: monitorBox
            Layout.fillWidth: true
            model: win.monitors
            textRole: "text"
            valueRole: "value"
            onActivated: {
                win.settings.updateScreenName(currentValue)
                win.settings.save()
            }
        }

        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: palette.placeholderText
            text: qsTr("Saved to %1").arg(win.settings.path || qsTr("(nowhere: no config directory)"))
        }
    }
}
