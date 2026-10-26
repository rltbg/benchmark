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
            var im = App.variant("impl", "painted"); return im === "render" ? shapesRender : im === "items" ? shapesItems : shapes
        }
    }
    Component { id: shapes; ShapesItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: shapesItems
        Item { x: 140; y: 40; width: 1000; height: 610
            clip: App.variant("feature") === "clip" || App.variant("feature") === "all"
            Component.onCompleted: { App.registerItem("shapesItems", this); if (rep.count === App.n) App.shapesReady() }
            Repeater { id: rep; model: App.n
                delegate: App.variant("feature") === "gradient" || App.variant("feature") === "all" ? gradShape : solidShape
                onCountChanged: if (count === App.n) App.shapesReady() }
            Component { id: solidShape; Rectangle { required property int index; color: App.fnvColor(index) } }
            Component { id: gradShape; Rectangle { required property int index
                gradient: Gradient { GradientStop { position: 0; color: App.fnvColor(index) } GradientStop { position: 1; color: App.fnvColor(index + 5) } } } }
        } }
    Component { id: shapesRender; ShapesRenderItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: paths; PathsItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: chart; ChartItem { x: 140; y: 250; width: 1000; height: 310; Component.onCompleted: { App.registerItem("gfx", this); App.setReady() } } }
    Component { id: images; ImagesItem { x: 140; y: 40; width: 1000; height: 610; Component.onCompleted: App.registerItem("gfx", this) } }
}
