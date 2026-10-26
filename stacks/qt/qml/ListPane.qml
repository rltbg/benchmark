import QtQuick
import Bench

Item {
    ListView {
        id: lv
        anchors.fill: parent
        model: App.model
        cacheBuffer: 600
        reuseItems: true
        clip: true
        interactive: false
        boundsBehavior: Flickable.StopAtBounds
        Component.onCompleted: App.registerItem("list", lv)
        delegate: Item {
            id: d
            required property int index
            required property string title
            required property string subtitle
            required property real price
            required property color dot
            required property int rowH
            width: ListView.view ? ListView.view.width : 0
            height: rowH
            Rectangle { x: 12; y: (d.height - 32) / 2; width: 32; height: 32; radius: 16; color: d.dot }
            Text { x: 60; y: 8; text: d.title; font.pixelSize: 16; font.bold: true; color: "#1a202c" }
            Text { x: 60; y: 32; width: d.width - 60 - 130; height: 16; clip: true; text: d.subtitle; font.pixelSize: 12; color: "#718096" }
            Text { anchors.right: parent.right; anchors.rightMargin: 16; y: (d.height - 18) / 2; width: 100
                   horizontalAlignment: Text.AlignRight; text: d.price.toFixed(2); font.pixelSize: 15; color: "#2b6cb0" }
            Rectangle { y: d.height - 1; width: d.width; height: 1; color: "#e2e8f0" }
        }
    }
}
