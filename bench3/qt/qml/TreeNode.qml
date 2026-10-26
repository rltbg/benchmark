import QtQuick
import QtQuick.Layouts
import Bench

GridLayout {
    id: node
    property int nid: -1
    readonly property int firstKid: 10 * nid + 1
    readonly property int kids: Math.max(0, Math.min(10, App.n - firstKid))
    // 0: all children are leaves, 1: all are containers, 2: mixed (the single boundary node: children rendered by TreeNode, leaf body when kids === 0)
    readonly property int kidMode: firstKid >= App.leafStart ? 0 : (firstKid + 9 < App.leafStart ? 1 : 2)
    readonly property var childIds: { var a = []; for (var i = 0; i < kids; ++i) a.push(firstKid + i); return a }
    Layout.fillWidth: true; Layout.fillHeight: true
    columns: 5; rowSpacing: 1; columnSpacing: 1
    Repeater { model: node.kidMode === 0 ? node.childIds : []
        TreeLeaf { required property int modelData; nid: modelData; Layout.fillWidth: true; Layout.fillHeight: true } }
    Repeater { model: node.kidMode !== 0 ? node.childIds : []; delegate: App.nodeComp }
    Loader { active: node.kids === 0; visible: active; Layout.fillWidth: true; Layout.fillHeight: true; sourceComponent: TreeLeaf { nid: node.nid } }
}
