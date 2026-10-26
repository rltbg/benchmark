import QtQuick
import QtQuick.Layouts
import Bench

GridLayout {
    id: node
    required property int nid
    readonly property int firstKid: 10 * nid + 1
    readonly property int kids: Math.max(0, Math.min(10, App.n - firstKid))
    // 0: all children are leaves, 1: all are containers, 2: mixed (the single boundary node)
    readonly property int kidMode: firstKid >= App.leafStart ? 0 : (firstKid + 9 < App.leafStart ? 1 : 2)
    columns: 5; rowSpacing: 1; columnSpacing: 1
    Repeater { model: node.kidMode === 0 ? node.kids : 0
        TreeLeaf { required property int index; nid: node.firstKid + index; Layout.fillWidth: true; Layout.fillHeight: true } }
    Repeater { model: node.kidMode === 1 ? node.kids : 0
        TreeNode { required property int index; nid: node.firstKid + index; Layout.fillWidth: true; Layout.fillHeight: true } }
    Repeater { model: node.kidMode === 2 ? node.kids : 0
        Loader { required property int index; readonly property int cid: node.firstKid + index; Layout.fillWidth: true; Layout.fillHeight: true
                 Component.onCompleted: setSource(cid >= App.leafStart ? "TreeLeaf.qml" : "TreeNode.qml", {nid: cid}) } }
}
