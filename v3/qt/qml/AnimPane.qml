import QtQuick
import Bench

// U3: N elements animated by the QML animation engine (position + opacity, interpolated, infinite loops)
Item {
    Repeater {
        model: App.n
        delegate: Rectangle {
            id: e
            required property int index
            y: 40 + (index * 29) % 640
            width: 24; height: 24; radius: 6
            color: Qt.hsla((index * 0.0618) % 1, 0.7, 0.5, 1)
            NumberAnimation on x { from: 0; to: 1250; duration: 3000 + (e.index % 13) * 250; loops: Animation.Infinite }
            SequentialAnimation on opacity {
                loops: Animation.Infinite
                NumberAnimation { from: 0.2; to: 1; duration: 700 + (e.index % 5) * 100 }
                NumberAnimation { from: 1; to: 0.2; duration: 700 + (e.index % 5) * 100 }
            }
        }
    }
    Component.onCompleted: App.setReady()
}
