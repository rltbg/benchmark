import QtQuick
import Bench

// U1: static tree of App.n nodes (containers of 10 children = GridLayout, leaves = rect + short text).
// QML forbids a type from instantiating itself statically, so the child delegate is a Component shared through App.nodeComp.
Item {
    Component { id: nodeComp; TreeNode { required property int modelData; nid: modelData } }
    TreeNode { anchors.fill: parent; nid: 0 }
    Component.onCompleted: { App.nodeComp = nodeComp; App.setReady() }
}
