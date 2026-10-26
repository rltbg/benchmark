import QtQuick

Item {
    Flickable {
        id: fl
        anchors.fill: parent
        clip: true
        interactive: false
        contentWidth: width
        contentHeight: te.height
        Component.onCompleted: bench.registerItem("flick", fl)
        TextEdit {
            id: te
            width: fl.width
            readOnly: true
            selectByMouse: true
            wrapMode: TextEdit.Wrap
            font.pixelSize: 14
            color: "#1a202c"
            Component.onCompleted: { bench.registerItem("textedit", te); bench.loadText(te) }
        }
    }
}
