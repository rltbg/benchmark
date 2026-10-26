import QtQuick
import Spike

Item {
    id: tab
    PriceModel { id: prices; Component.onCompleted: startFeed() }
    ListView {
        id: list
        anchors.fill: parent
        clip: true
        model: prices
        boundsBehavior: Flickable.StopAtBounds
        delegate: Rectangle {
            required property string title
            required property string price
            required property int rowHeight
            required property int index
            width: ListView.view.width
            height: rowHeight
            color: index % 2 ? "#ffffff" : "#eef2f5"
            Text { text: title; anchors.left: parent.left; anchors.leftMargin: 12; anchors.verticalCenter: parent.verticalCenter; font.pixelSize: 16 }
            Text { text: price; anchors.right: parent.right; anchors.rightMargin: 12; anchors.verticalCenter: parent.verticalCenter
                   font.pixelSize: 16; font.bold: true; color: "#0b6e4f" }
        }
        // défilement simulé du parcours automatique (3 s)
        NumberAnimation { id: demo; target: list; property: "contentY"; from: 0; to: 9000; duration: 2900; easing.type: Easing.InOutQuad }
    }
    Connections { target: Spike; function onScrollDemo() { demo.restart() } }
}
