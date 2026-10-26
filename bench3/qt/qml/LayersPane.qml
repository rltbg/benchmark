import QtQuick
import QtQuick.Effects
import Bench

// G5: N superposed groups, each a layer (group opacity 0.5 on the composited result), moved by native QML animations
Item {
    Repeater {
        model: App.n
        delegate: Item {
            id: grp
            required property int index
            readonly property real x0: 60 + (index * 137) % 700
            width: 400; height: 260
            y: 20 + (index * 53) % 420
            opacity: 0.5
            layer.enabled: true
            layer.effect: App.layerBlur ? blurFx : null
            Rectangle { x: 0; y: 0; width: 260; height: 170; radius: 14; color: Qt.hsla((grp.index * 0.137) % 1, 0.7, 0.55, 1) }
            Rectangle { x: 120; y: 70; width: 260; height: 170; radius: 14; color: Qt.hsla((grp.index * 0.137 + 0.4) % 1, 0.7, 0.45, 1) }
            Rectangle { x: 70; y: 40; width: 150; height: 150; radius: 75; color: Qt.hsla((grp.index * 0.137 + 0.7) % 1, 0.8, 0.5, 1) }
            SequentialAnimation on x {
                loops: Animation.Infinite
                NumberAnimation { from: grp.x0; to: grp.x0 + 160; duration: 2000 + (grp.index % 7) * 250; easing.type: Easing.InOutQuad }
                NumberAnimation { from: grp.x0 + 160; to: grp.x0; duration: 2000 + (grp.index % 7) * 250; easing.type: Easing.InOutQuad }
            }
        }
    }
    Component { id: blurFx; MultiEffect { blurEnabled: true; blurMax: 4; blur: 1.0 } }
    Component.onCompleted: App.setReady()
}
