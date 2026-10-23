import QtQuick
import QtQuick.Controls

// Recent output of a managed Kanata (FR-010). In memory only; nothing here is
// saved (FR-011). Select and copy as usual.
//
// A plain TextEdit rather than Controls' TextArea: KDE's Breeze style attaches a
// mobile toolbar to TextArea that expects a TextInput and warns at load time.
ApplicationWindow {
    id: win
    required property var kanata

    title: qsTr("Kanata log")
    width: 760
    height: 420
    visible: false

    Flickable {
        id: flick
        anchors.fill: parent
        anchors.margins: 6
        clip: true
        contentWidth: log.contentWidth
        contentHeight: log.contentHeight
        ScrollBar.vertical: ScrollBar {}
        ScrollBar.horizontal: ScrollBar {}

        // Follow new output unless the user scrolled up to read.
        property bool atEnd: true
        onContentYChanged: atEnd = contentY >= contentHeight - height - 4
        onContentHeightChanged: if (atEnd) contentY = Math.max(0, contentHeight - height)

        TextEdit {
            id: log
            readOnly: true
            selectByMouse: true
            selectByKeyboard: true
            wrapMode: TextEdit.NoWrap
            textFormat: TextEdit.PlainText
            font.family: "monospace"
            font.pixelSize: 12
            color: palette.text
            selectionColor: palette.highlight
            selectedTextColor: palette.highlightedText
            text: win.kanata.logText
        }
    }
}
