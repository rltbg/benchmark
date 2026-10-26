import QtQuick
import Bench 1.0

Rectangle {
    id: root
    width: 1280; height: 720
    color: "#f4f5f7"
    readonly property string sc: bench.scenario
    Loader {
        anchors.fill: parent
        sourceComponent: {
            switch (root.sc) {
            case "idle": case "list": case "stream": return shellList
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
}
