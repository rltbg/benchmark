import QtQuick
import Bench

// N text labels in a grid covering the window (T2 labels fed by a QAbstractListModel, U2 labels bound to QObject properties)
Item {
    id: g
    readonly property int cols: Math.ceil(Math.sqrt(App.n * width / height))
    readonly property int rows: Math.ceil(App.n / cols)
    readonly property real cw: width / cols
    readonly property real ch: height / rows
    readonly property real fs: Math.max(3, Math.min(14, Math.min(cw / 8.6, ch - 1)))
    Repeater {
        model: App.scenario === "text-labels" ? App.labels : null
        delegate: Text {
            required property int index
            required property string label
            x: (index % g.cols) * g.cw; y: Math.floor(index / g.cols) * g.ch
            text: label; textFormat: Text.PlainText; font.pixelSize: g.fs; color: "#1a202c"
        }
    }
    Repeater {
        model: App.scenario === "ui-bind" ? App.cells : null
        delegate: Text {
            required property int index
            required property var cell
            x: (index % g.cols) * g.cw; y: Math.floor(index / g.cols) * g.ch
            text: cell.value; textFormat: Text.PlainText; font.pixelSize: g.fs; color: "#1a202c"
        }
    }
    Component.onCompleted: App.setReady()
}
