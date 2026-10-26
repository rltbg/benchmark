import QtQuick
import Bench

Item {
    id: pane
    Loader {
        anchors.fill: parent
        sourceComponent: {
            switch (App.scenario) {
            case "gfx-paths": return paths
            case "gfx-chart": return chart
            case "gfx-images": return images
            }
            return App.variant("impl", "painted") === "render" ? shapesRender : shapes
        }
    }
    Component { id: shapes; ShapesItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: shapesRender; ShapesRenderItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: paths; PathsItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: chart; ChartItem { x: 140; y: 250; width: 1000; height: 310; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: images; ImagesItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: App.registerItem("gfx", this) } }
}
