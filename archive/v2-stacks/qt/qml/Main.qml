import QtQuick
import Bench

Rectangle {
    id: root
    width: 1280; height: 720
    color: "#f4f5f7"
    readonly property string sc: App.scenario
    Loader {
        anchors.fill: parent
        sourceComponent: {
            switch (root.sc) {
            case "idle": case "list": case "stream": case "input": return shellList
            case "text": return shellText
            case "images": return shellImages
            case "nav": return navPane
            case "canvas": case "core": return canvasPane
            case "stress": return stressPane
            }
            return shellList
        }
    }
    Component { id: shellList; Shell { ListPane { anchors.fill: parent } } }
    Component { id: shellText; Shell { TextPane { anchors.fill: parent } } }
    Component { id: shellImages; Shell { ImagesPane { anchors.fill: parent } } }
    Component { id: navPane; NavPane { } }
    Component { id: canvasPane; CanvasPane { } }
    Component { id: stressPane
        Item {
            ListPane { x: 0; y: 0; width: 880; height: 720 }
            CanvasPane { x: 880; y: 0; width: 400; height: 720 }
        }
    }
    // input scenario: 200x200 square at (100,100) above the scrolling list, toggled black/white on each Space key press
    Rectangle {
        id: square
        visible: root.sc === "input"
        x: 100; y: 100; width: 200; height: 200
        property bool white: false
        color: white ? "#ffffff" : "#000000"
        focus: visible
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space && !event.isAutoRepeat) { square.white = !square.white; App.keyEvent(); event.accepted = true }
        }
        Component.onCompleted: if (visible) forceActiveFocus()
    }
}
