import QtQuick
import Bench

Item {
    clip: true
    Item {
        id: zoomer
        width: parent.width; height: parent.height
        transformOrigin: Item.Center
        Component.onCompleted: App.registerItem("zoomer", zoomer)
        GridView {
            id: gv
            anchors.fill: parent
            cellWidth: 128; cellHeight: 128
            cacheBuffer: 512
            interactive: false
            model: 600
            Component.onCompleted: App.registerItem("grid", gv)
            delegate: Image {
                required property int index
                width: 128; height: 128
                asynchronous: true
                cache: true
                sourceSize: Qt.size(128, 128)
                source: "image://bench/img" + String(index).padStart(3, "0")
            }
        }
    }
}
