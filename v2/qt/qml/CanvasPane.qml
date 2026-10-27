import QtQuick
import Bench

Item {
    Rectangle { anchors.fill: parent; color: "#dfe5ee" }
    Loader {
        anchors.fill: parent
        sourceComponent: App.canvasImpl === "render" ? cr : App.canvasImpl === "qml" ? cj : cp
    }
    Component { id: cp; CanvasItem { Component.onCompleted: App.registerItem("canvas", this) } }
    Component { id: cr; CanvasRenderItem { Component.onCompleted: App.registerItem("canvas", this) } }
    Component { id: cj; CanvasJs { } }
}
