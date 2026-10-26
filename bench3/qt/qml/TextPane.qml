import QtQuick
import Bench

Item {
    id: pane
    readonly property bool editable: App.scenario === "text-edit" || App.scenario === "soak"
    Flickable {
        id: fl
        anchors.fill: parent
        clip: true
        interactive: false
        contentWidth: width
        contentHeight: te.height
        Component.onCompleted: App.registerItem("flick", fl)
        TextEdit {
            id: te
            width: fl.width
            readOnly: !pane.editable
            textFormat: App.scenario === "text-rich" ? TextEdit.RichText : TextEdit.PlainText
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            font.pixelSize: 14
            color: "#1a202c"
            onTextChanged: if (pane.editable) App.editorChanged()
            onCursorRectangleChanged: if (pane.editable) { // keep the caret in view (idiom of the TextEdit docs: Flickable + cursorRectangle)
                if (fl.contentY >= cursorRectangle.y) fl.contentY = cursorRectangle.y
                else if (fl.contentY + fl.height <= cursorRectangle.y + cursorRectangle.height) fl.contentY = cursorRectangle.y + cursorRectangle.height - fl.height
            }
            Component.onCompleted: { App.registerItem("textedit", te); App.loadText(te, "") }
        }
    }
}
