import QtQuick
import Bench

Item {
    id: pane
    property string role: "list"
    property var mdl: App.model
    property bool interactive: false

    function chipColor(s, j) { var m = (s ^ Math.imul(j + 1, 0x9E3779B1)) >>> 0; return Qt.rgba((m & 255) / 255, ((m >> 8) & 255) / 255, ((m >> 16) & 255) / 255, 1) }
    function chipText(s, j) { return ((s ^ Math.imul(j + 7, 0x85EBCA6B)) >>> 0).toString(36).substring(0, 2) }

    ListView {
        id: lv
        anchors.fill: parent
        model: pane.mdl
        cacheBuffer: parseInt(App.variant("cache", "600"))   // explicit setting (default 320 px)
        reuseItems: App.variant("reuse", "1") !== "0"          // explicit setting (Qt default: false)
        clip: true
        interactive: pane.interactive
        boundsBehavior: Flickable.StopAtBounds
        Component.onCompleted: App.registerItem(pane.role, lv)
        delegate: App.richK > 0 ? richDelegate : stdDelegate
    }
    Component {
        id: stdDelegate
        Item {
            id: d
            required property int index
            required property string title
            required property string subtitle
            required property real price
            required property color dot
            required property int rowH
            width: ListView.view ? ListView.view.width : 0
            height: rowH
            Rectangle { x: 12; y: (d.height - 32) / 2; width: 32; height: 32; radius: 16; color: d.dot }
            Text { x: 60; y: 8; text: d.title; textFormat: Text.PlainText; font.pixelSize: 16; font.bold: true; color: "#1a202c" }
            Text { x: 60; y: 32; width: d.width - 60 - 130; height: 16; clip: true; text: d.subtitle; textFormat: Text.PlainText; font.pixelSize: 12; color: "#718096" }
            Text { anchors.right: parent.right; anchors.rightMargin: 16; y: (d.height - 18) / 2; width: 100
                   horizontalAlignment: Text.AlignRight; text: d.price.toFixed(2); textFormat: Text.PlainText; font.pixelSize: 15; color: "#2b6cb0" }
            Rectangle { y: d.height - 1; width: d.width; height: 1; color: "#e2e8f0" }
        }
    }
    // L2: dense row (24..44 px): 6 base elements (dot, title, 2-line subtitle with ellipsis, progress bar, toggle, price) + (K-6)/2 chips
    Component {
        id: richDelegate
        Item {
            id: r
            required property int index
            required property string title
            required property string subtitle
            required property real price
            required property color dot
            required property int rowH
            required property real seed
            readonly property int chips: (App.richK - 6) / 2
            readonly property real step: chips > 0 ? Math.min(44, 650 / chips) : 0
            width: ListView.view ? ListView.view.width : 0
            height: rowH
            Rectangle { x: 8; y: (r.height - 16) / 2; width: 16; height: 16; radius: 8; color: r.dot }
            Text { x: 30; y: (r.height - 14) / 2; width: 130; text: r.title; textFormat: Text.PlainText; font.pixelSize: 11; font.bold: true; color: "#1a202c"; elide: Text.ElideRight }
            Text { x: 170; y: 2; width: 200; height: r.height - 4; text: r.subtitle; textFormat: Text.PlainText; font.pixelSize: 9; color: "#718096"; wrapMode: Text.WordWrap; maximumLineCount: 2; elide: Text.ElideRight }
            Rectangle { x: 380; y: (r.height - 6) / 2; width: 120; height: 6; radius: 3; color: "#e2e8f0"
                Rectangle { width: parent.width * ((r.seed % 100) / 100); height: 6; radius: 3; color: "#38a169" } }
            Rectangle { x: 510; y: (r.height - 18) / 2; width: 34; height: 18; radius: 9; color: (r.seed & 1) ? "#3182ce" : "#a0aec0"
                Rectangle { x: (r.seed & 1) ? 18 : 2; y: 2; width: 14; height: 14; radius: 7; color: "white" } }
            Text { x: 548; y: (r.height - 14) / 2; width: 46; horizontalAlignment: Text.AlignRight; text: r.price.toFixed(2); textFormat: Text.PlainText; font.pixelSize: 11; color: "#2b6cb0" }
            Repeater {
                model: r.chips
                Rectangle {
                    required property int index
                    x: 600 + index * r.step; y: (r.height - 16) / 2; width: Math.max(10, r.step - 2); height: 16; radius: 4
                    color: pane.chipColor(r.seed, index)
                    Text { anchors.centerIn: parent; text: pane.chipText(r.seed, index); textFormat: Text.PlainText; font.pixelSize: 8; color: "white" }
                }
            }
            Rectangle { y: r.height - 1; width: r.width; height: 1; color: "#e2e8f0" }
        }
    }
}
