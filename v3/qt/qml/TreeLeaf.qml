import QtQuick
Rectangle {
    required property int nid
    color: Qt.hsla((nid * 0.0618) % 1, 0.45, 0.82, 1)
    Text { anchors.centerIn: parent; text: parent.nid; textFormat: Text.PlainText; font.pixelSize: 8; color: "#1a202c" }
}
