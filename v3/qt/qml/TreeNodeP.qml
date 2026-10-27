import QtQuick
import Bench

// variant tree=pos: same tree, children positioned by plain property bindings (5 columns x 2 rows, 1 px gaps) instead of QtQuick.Layouts
Item {
    id: node
    property int nid: -1
    property int slot: 0
    readonly property int firstKid: 10 * nid + 1
    readonly property int kids: Math.max(0, Math.min(10, App.n - firstKid))
    readonly property var childIds: { var a = []; for (var i = 0; i < kids; ++i) a.push(firstKid + i); return a }
    Repeater { model: node.kids > 0 ? node.childIds : []; delegate: App.nodeComp }
    Rectangle { visible: node.kids === 0; anchors.fill: parent; color: Qt.hsla((node.nid * 0.0618) % 1, 0.45, 0.82, 1)
        Text { anchors.centerIn: parent; text: node.nid; textFormat: Text.PlainText; font.pixelSize: 8; color: "#1a202c" } }
}
