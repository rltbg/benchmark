import QtQuick
import Bench

// U1: static tree of App.n nodes (containers of 10 children = GridLayout, leaves = rect + short text)
Item {
    TreeNode { anchors.fill: parent; nid: 0; Component.onCompleted: App.setReady() }
}
