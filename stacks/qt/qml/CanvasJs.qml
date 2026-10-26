import QtQuick

// QML Canvas (JS Context2D) - smoke comparison only. Shapes + graph drawn from JS each frame.
Canvas {
    id: cv
    renderStrategy: Canvas.Cooperative
    renderTarget: Canvas.Image
    property int n: bench.shapes
    property var px: []; property var py: []; property var pvx: []; property var pvy: []; property var pw: []
    property var gr: []; property int gh: 0
    property double last: Date.now()
    function fnv(i) { var h = 2166136261; for (var k = 0; k < 4; k++) { h ^= (i >>> (8 * k)) & 255; h = Math.imul(h, 16777619) >>> 0 } return h >>> 0 }
    Component.onCompleted: {
        for (var i = 0; i < n; i++) {
            var s = 16 + 8 * (fnv(i + 3) % 8); pw.push(s)
            px.push(fnv(i + 11) % (1000 - s)); py.push(fnv(i + 13) % (600 - s))
            var sp = 40 + fnv(i + 17) % 160, a = (fnv(i + 19) % 360) * Math.PI / 180
            pvx.push(Math.cos(a) * sp); pvy.push(Math.sin(a) * sp)
        }
        for (var g = 0; g < 10000; g++) gr.push(50)
        bench.registerItem("qmlcanvas", cv)
    }
    Connections { target: bench; function onFrame() { cv.requestPaint() } }
    onPaint: {
        var now = Date.now(), dt = Math.min(0.1, (now - last) / 1000); last = now
        var ctx = getContext("2d")
        ctx.reset()
        var cw = 1000, ch = 600, ox = (width - cw) / 2, oy = 5
        ctx.fillStyle = "#ffffff"; ctx.beginPath(); ctx.roundedRect(ox, oy, cw, ch, 24, 24); ctx.fill()
        ctx.save(); ctx.beginPath(); ctx.roundedRect(ox, oy, cw, ch, 24, 24); ctx.clip()
        for (var i = 0; i < n; i++) {
            var x = px[i] + pvx[i] * dt, y = py[i] + pvy[i] * dt, w = pw[i]
            if (x < 0 || x > cw - w) { pvx[i] = -pvx[i]; x = Math.max(0, Math.min(cw - w, x)) }
            if (y < 0 || y > ch - w) { pvy[i] = -pvy[i]; y = Math.max(0, Math.min(ch - w, y)) }
            px[i] = x; py[i] = y
            var f1 = fnv(i), f2 = fnv(i + 5)
            var gd = ctx.createLinearGradient(0, oy + y, 0, oy + y + w)
            gd.addColorStop(0, "rgb(" + (f1 & 255) + "," + ((f1 >> 8) & 255) + "," + ((f1 >> 16) & 255) + ")")
            gd.addColorStop(1, "rgb(" + (f2 & 255) + "," + ((f2 >> 8) & 255) + "," + ((f2 >> 16) & 255) + ")")
            if (i % 10 === 0) { ctx.shadowColor = "rgba(0,0,0,0.43)"; ctx.shadowBlur = 8; ctx.shadowOffsetX = 2; ctx.shadowOffsetY = 2 }
            else { ctx.shadowColor = "transparent"; ctx.shadowBlur = 0; ctx.shadowOffsetX = 0; ctx.shadowOffsetY = 0 }
            ctx.fillStyle = gd; ctx.beginPath()
            var t = i % 3
            if (t === 0) ctx.rect(ox + x, oy + y, w, w * 0.75)
            else if (t === 1) ctx.arc(ox + x + w / 2, oy + y + w / 2, w / 2, 0, 2 * Math.PI)
            else ctx.roundedRect(ox + x, oy + y, w, w, 8, 8)
            ctx.fill()
        }
        ctx.restore()
        ctx.shadowColor = "transparent"; ctx.shadowBlur = 0
        ctx.fillStyle = "#181c28"; ctx.fillRect(ox, oy + ch + 10, cw, 100)
        gr[gh] = 50 + 30 * Math.sin(now / 300) + Math.random() * 20 - 10; gh = (gh + 1) % 10000
        ctx.strokeStyle = "#50dc78"; ctx.lineWidth = 1; ctx.beginPath()
        for (var k = 0; k < 10000; k++) { var yy = oy + ch + 110 - gr[(gh + k) % 10000] * 0.9; if (k === 0) ctx.moveTo(ox, yy); else ctx.lineTo(ox + k * 0.1, yy) }
        ctx.stroke()
    }
}
