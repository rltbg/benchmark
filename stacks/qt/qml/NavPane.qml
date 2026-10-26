import QtQuick

// Navigation without QtQuick.Controls (the QtQuick.Templates QML module is not installed): hand-written
// push/pop stack with 300 ms slide transitions and a modal overlay with a 200 ms fade.
Item {
    id: nav
    property int step: 0
    property var stack: []        // pages below the current one
    property Item current: null
    property Item incoming: null
    property bool popping: false

    function newPage(comp) { return comp.createObject(nav, { x: nav.width, width: nav.width, height: nav.height }) }
    function push(comp) {
        incoming = newPage(comp); popping = false
        inA.target = incoming; inA.from = nav.width; inA.to = 0
        outA.target = current; outA.from = 0; outA.to = -nav.width / 3
        slide.start()
    }
    function pop() {
        incoming = stack.pop(); incoming.visible = true; popping = true
        inA.target = incoming; inA.from = -nav.width / 3; inA.to = 0
        outA.target = current; outA.from = 0; outA.to = nav.width
        slide.start()
    }
    function advance() {
        switch (step % 6) {
        case 0: push(pageB); break
        case 1: push(pageC); break
        case 2: modal.show(); break
        case 3: modal.hide(); break
        case 4: pop(); break
        case 5: pop(); break
        }
        step++
    }
    ParallelAnimation {
        id: slide
        NumberAnimation { id: inA; property: "x"; duration: 300 }
        NumberAnimation { id: outA; property: "x"; duration: 300 }
        onFinished: {
            if (nav.popping) { nav.current.destroy(); }
            else { nav.current.visible = false; nav.stack.push(nav.current) }
            nav.current = nav.incoming; nav.incoming = null
            bench.transitionDone(); kick.start()
        }
    }
    Timer { id: kick; interval: 0; onTriggered: nav.advance() }
    Component.onCompleted: { current = newPage(pageA); current.x = 0; kick.start() }

    Rectangle {
        id: modal
        anchors.fill: parent
        color: "#80000000"
        opacity: 0
        visible: opacity > 0
        function show() { fade.from = 0; fade.to = 1; fade.start() }
        function hide() { fade.from = 1; fade.to = 0; fade.start() }
        NumberAnimation { id: fade; target: modal; property: "opacity"; duration: 200
            onFinished: { bench.modalDone(); kick.start() } }
        MouseArea { anchors.fill: parent }
        Rectangle {
            anchors.centerIn: parent; width: 520; height: 320; radius: 8; color: "white"
            Column { x: 16; y: 16; spacing: 8
                Text { text: "Modal dialog"; font.pixelSize: 22; font.bold: true }
                Repeater { model: 6; Text { required property int index; text: "Line " + index + " of the dialog body text"; font.pixelSize: 14 } }
            }
        }
    }
    Component { id: pageA; Page { title: "A"; bg: "#fde8e8" } }
    Component { id: pageB; Page { title: "B"; bg: "#e6f4ea" } }
    Component { id: pageC; Page { title: "C"; bg: "#e8eefd" } }
}
