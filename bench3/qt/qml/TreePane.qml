import QtQuick
import Bench

// U1: static tree of App.n nodes (containers of 10 children = GridLayout, leaves = rect + short text).
// QML forbids a type from instantiating itself statically, so the child delegate is a Component shared through App.nodeComp.
Item {
    readonly property bool pos: App.variant("tree") === "pos"
    Component { id: nodeComp; TreeNode { required property int modelData; nid: modelData } }
    Component { id: nodeCompP; TreeNodeP { required property int modelData; required property int index
        nid: modelData; width: parent ? parent.width / 5 : 0; height: parent ? parent.height / 2 : 0; x: (index % 5) * width; y: Math.floor(index / 5) * height } }
    Loader { anchors.fill: parent; sourceComponent: pos ? rootP : rootG }
    Component { id: rootG; TreeNode { nid: 0 } }
    Component { id: rootP; TreeNodeP { nid: 0 } }
    Component.onCompleted: { App.nodeComp = pos ? nodeCompP : nodeComp; App.setReady() }
}
