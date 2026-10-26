import QtQuick
import Bench

// App shell: title bar (40) + content + status bar (28)
Item {
    default property alias content: slot.data
    Rectangle { id: bar; width: parent.width; height: 40; color: "#2d3748"
        Text { x: 16; anchors.verticalCenter: parent.verticalCenter; text: "Bench — " + App.scenario; color: "white"; font.pixelSize: 16 }
        Text { anchors.right: parent.right; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter; text: "Qt Quick"; color: "#cbd5e0"; font.pixelSize: 13 }
    }
    Item { id: slot; y: 40; width: parent.width; height: parent.height - 68; clip: true }
    Rectangle { y: parent.height - 28; width: parent.width; height: 28; color: "#e2e8f0"
        Text { x: 16; anchors.verticalCenter: parent.verticalCenter; text: "100000 rows"; color: "#4a5568"; font.pixelSize: 12 }
    }
}
