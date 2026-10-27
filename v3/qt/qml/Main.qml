import QtQuick
import Bench

Rectangle {
    id: root
    width: 1280; height: 720
    color: "#f4f5f7"
    Loader {
        anchors.fill: parent
        sourceComponent: {
            switch (App.pane) {
            case "text": return shellText
            case "labels": return labelsPane
            case "gfx": return gfxPane
            case "layers": return layersPane
            case "tree": return treePane
            case "anim": return animPane
            case "composite": return compositePane
            }
            return shellList
        }
    }
    Component { id: shellList; Shell { ListPane { anchors.fill: parent; role: "list"; mdl: App.model; interactive: App.interactive } } }
    Component { id: shellText; Shell { TextPane { anchors.fill: parent } } }
    Component { id: labelsPane; LabelsPane { } }
    Component { id: gfxPane; GfxPane { } }
    Component { id: layersPane; LayersPane { } }
    Component { id: treePane; TreePane { } }
    Component { id: animPane; AnimPane { } }
    Component { id: compositePane; CompositePane { } }

    // Permanent latency square (all scenarios except idle): 200x200 at (100,100), above everything; black/white toggled by Space
    // through the normal Qt Quick key-event path (text-edit: toggled by the editor once it has applied the typed character).
    Rectangle {
        id: square
        visible: App.scenario !== "idle"
        x: 100; y: 100; width: 200; height: 200; z: 1000
        color: App.squareWhite ? "#ffffff" : "#000000"
        focus: visible
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Space && !event.isAutoRepeat) { App.toggleSquare(); event.accepted = true }
        }
        Component.onCompleted: if (visible && App.scenario !== "text-edit" && App.scenario !== "soak") forceActiveFocus()
    }
}
