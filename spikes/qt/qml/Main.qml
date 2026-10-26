import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import Spike

ApplicationWindow {
    id: win
    visible: true
    width: Spike.mobile ? 390 : 960
    height: Spike.mobile ? 800 : 640
    title: "Qt spike"
    color: "#f4f6f8"

    // Barre d'onglets : en bas sur mobile, en haut sur bureau / web
    component Tabs: TabBar {
        id: bar
        Layout.fillWidth: true
        TabButton { text: "Liste" }
        TabButton { text: "Canvas" }
        TabButton { text: "Cœur" }
        onCurrentIndexChanged: Spike.currentTab = currentIndex
        Binding { target: bar; property: "currentIndex"; value: Spike.currentTab; restoreMode: Binding.RestoreNone }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.topMargin: SafeArea.margins.top
        anchors.bottomMargin: SafeArea.margins.bottom
        anchors.leftMargin: SafeArea.margins.left
        anchors.rightMargin: SafeArea.margins.right
        spacing: 0

        Rectangle {                                        // bandeau permanent
            Layout.fillWidth: true
            implicitHeight: banner.implicitHeight + 12
            color: "#1f2933"
            Text {
                id: banner
                anchors.fill: parent; anchors.margins: 6
                text: Spike.banner
                color: "#ffffff"; font.pixelSize: 14; wrapMode: Text.Wrap
                verticalAlignment: Text.AlignVCenter
            }
        }
        Loader { Layout.fillWidth: true; active: !Spike.mobile; visible: active; sourceComponent: Tabs {} }
        StackLayout {
            Layout.fillWidth: true; Layout.fillHeight: true
            currentIndex: Spike.currentTab
            ListTab {}
            CanvasTab {}
            CoreTab {}
        }
        Loader { Layout.fillWidth: true; active: Spike.mobile; visible: active; sourceComponent: Tabs {} }
    }
}
