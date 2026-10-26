import QtQuick
import Bench 1.0

Item {
    Rectangle { anchors.fill: parent; color: "#dfe5ee" }
    Loader {
        anchors.fill: parent
        sourceComponent: bench.canvasImpl === "render" ? cr : bench.canvasImpl === "qml" ? cj : cp
    }
    Component { id: cp; CanvasItem { Component.onCompleted: bench.registerItem("canvas", this) } }
    Component { id: cr; CanvasRenderItem { Component.onCompleted: bench.registerItem("canvas", this) } }
    Component { id: cj; CanvasJs { } }
}
