import QtQuick
import Spike

Item {
    ShapesItem { anchors.fill: parent }
    Rectangle {                                        // compteur de FPS superposé
        anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 8
        width: fpsText.implicitWidth + 16; height: fpsText.implicitHeight + 8
        radius: 6; color: "#cc000000"
        Text { id: fpsText; anchors.centerIn: parent; text: Spike.fps + " fps"; color: "#ffffff"; font.pixelSize: 18; font.bold: true }
    }
}
