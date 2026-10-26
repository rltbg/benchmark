import QtQuick
import Bench

Item {
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
            readOnly: true
            textFormat: TextEdit.PlainText
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            font.pixelSize: 14
            color: "#1a202c"
            Component.onCompleted: { App.registerItem("textedit", te); App.loadText(te) }
        }
    }
}
