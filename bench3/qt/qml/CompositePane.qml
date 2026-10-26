import QtQuick
import Bench

// M1 soak: list (L3 20k/s) | list (F2 50k/s) | shapes (500, gradient) over an editable text (T3).  M2 mem-min: list (L1 100k) | text (T1, 1 Mo)
Item {
    readonly property bool soak: App.scenario === "soak"
    ListPane { x: 0; y: 0; width: soak ? 420 : 640; height: parent.height; role: "list"; mdl: App.model }
    ListPane { x: 420; y: 0; width: 420; height: parent.height; role: "list2"; mdl: App.model2; visible: soak }
    Loader { active: soak; x: 840; y: 0; width: 440; height: 360; sourceComponent: soakShapes }
    Component { id: soakShapes; ShapesItem { Component.onCompleted: App.registerItem("gfx", this) } }
    TextPane { x: soak ? 840 : 640; y: soak ? 360 : 0; width: soak ? 440 : 640; height: soak ? 360 : parent.height }
    Component.onCompleted: if (!soak) { } 
}
