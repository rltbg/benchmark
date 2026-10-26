import QtQuick

Rectangle {
    property string title
    property color bg
    color: bg
    Rectangle { width: parent.width; height: 48; color: "#2d3748"
        Text { x: 16; anchors.verticalCenter: parent.verticalCenter; text: "Screen " + title; color: "white"; font.pixelSize: 18 } }
    Column {
        y: 60; x: 16; spacing: 6
        Repeater {
            model: 14
            Rectangle {
                required property int index
                width: 1240; height: 40; radius: 6; color: "white"; border.color: "#cbd5e0"
                Rectangle { x: 8; y: 6; width: 28; height: 28; radius: 14; color: Qt.hsla(index / 14, 0.6, 0.5, 1) }
                Text { x: 48; anchors.verticalCenter: parent.verticalCenter; text: "Screen " + title + " — row " + index + " — lorem ipsum dolor sit amet"; font.pixelSize: 14 }
            }
        }
    }
}
