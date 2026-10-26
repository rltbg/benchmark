import QtQuick
import QtQuick.Controls.Basic
import Bench

// Navigation with the official Qt Quick Controls idioms: StackView (pushItem / popCurrentItem, 300 ms slide
// transitions) and a modal Popup (200 ms fade enter/exit). Cycle: push, push, open modal, close modal, pop, pop.
Item {
    id: nav
    property int step: 0
    property bool inTransition: false

    function advance() {
        switch (step % 6) {
        case 0: inTransition = true; sv.pushItem(pageB); break
        case 1: inTransition = true; sv.pushItem(pageC); break
        case 2: modal.open(); break
        case 3: modal.close(); break
        case 4: inTransition = true; sv.popCurrentItem(); break
        case 5: inTransition = true; sv.popCurrentItem(); break
        }
        step++
    }
    Timer { id: kick; interval: 0; onTriggered: nav.advance() }

    StackView {
        id: sv
        anchors.fill: parent
        initialItem: pageA
        pushEnter: Transition { NumberAnimation { property: "x"; from: sv.width; to: 0; duration: 300; easing.type: Easing.OutCubic } }
        pushExit: Transition { NumberAnimation { property: "x"; from: 0; to: -sv.width / 3; duration: 300; easing.type: Easing.OutCubic } }
        popEnter: Transition { NumberAnimation { property: "x"; from: -sv.width / 3; to: 0; duration: 300; easing.type: Easing.OutCubic } }
        popExit: Transition { NumberAnimation { property: "x"; from: 0; to: sv.width; duration: 300; easing.type: Easing.OutCubic } }
        onBusyChanged: if (!busy && nav.inTransition) { nav.inTransition = false; App.transitionDone(); kick.start() }
        Component.onCompleted: kick.start()
    }

    Popup {
        id: modal
        modal: true
        dim: true
        closePolicy: Popup.NoAutoClose
        anchors.centerIn: Overlay.overlay
        width: 520; height: 320
        padding: 16
        enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200 } }
        exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 200 } }
        onOpened: { App.modalDone(); kick.start() }
        onClosed: { App.modalDone(); kick.start() }
        Column {
            spacing: 8
            Text { text: "Modal dialog"; font.pixelSize: 22; font.bold: true }
            Repeater { model: 6; Text { required property int index; text: "Line " + index + " of the dialog body text"; font.pixelSize: 14 } }
        }
    }
    Component { id: pageA; NavScreen { title: "A"; bg: "#fde8e8" } }
    Component { id: pageB; NavScreen { title: "B"; bg: "#e6f4ea" } }
    Component { id: pageC; NavScreen { title: "C"; bg: "#e8eefd" } }
}
