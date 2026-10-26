import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import Spike

Item {
    ColumnLayout {
        anchors.fill: parent; anchors.margins: 16; spacing: 10
        Text { text: "Saisie (trame [0xA5][0x01][n][UTF-8 ≤ 64])"; font.pixelSize: 14; color: "#52606d" }
        TextField {
            Layout.fillWidth: true
            placeholderText: "tapez du texte"
            text: Spike.inputText
            inputMethodHints: Qt.ImhNone
            font.pixelSize: 18
            onTextEdited: Spike.inputText = text
        }
        Text { Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; font.pixelSize: 14; text: "trame = " + Spike.frameHex }
        Text { font.pixelSize: 18; text: "validate = " + Spike.validText }
        Text { Layout.fillWidth: true; wrapMode: Text.Wrap; font.pixelSize: 18; text: "payload = " + Spike.payloadText }
        Text { font.pixelSize: 18; text: "hash = 0x" + Spike.hashHex }
        Button {
            text: "charge 100 ms"
            enabled: !Spike.busy
            onClicked: Spike.heavy()
        }
        Text { font.pixelSize: 14; color: "#52606d"; text: Spike.heavyText }
        Item { Layout.fillHeight: true }
    }
}
