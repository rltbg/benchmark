package bench

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.ImageFilter
import org.jetbrains.skia.Paint
import org.jetbrains.skia.RRect
import org.jetbrains.skia.Rect
import kotlin.math.cos
import kotlin.math.sin

/** Simulation + objets de dessin pré-alloués (pinceaux en coordonnées locales, translatés au dessin). */
class Shapes(val n: Int, val areaW: Float, val areaH: Float, val withShadow: Boolean = true) {
    val x = FloatArray(n); val y = FloatArray(n); val vx = FloatArray(n); val vy = FloatArray(n)
    val w = FloatArray(n); val h = FloatArray(n)
    val brushes: Array<Brush>
    val shadowPaint: Paint = Paint().apply {
        color = 0xFF000000.toInt()
        imageFilter = ImageFilter.makeDropShadowOnly(2f, 2f, 4f, 4f, 0x99000000.toInt()) // sigma 4 ~ flou 8 px
    }
    init {
        brushes = Array(n) { i ->
            val f = fnv(i * 7 + 1)
            val s = 16f + (fnvU(i * 3 + 1) % 40).toInt()
            w[i] = if (i % 3 == 2) s * 1.5f else s; h[i] = s
            x[i] = (fnvU(i * 5 + 2) % 1280).toFloat(); y[i] = (fnvU(i * 5 + 3) % 720).toFloat()
            val sp = 40f + (fnvU(i * 11 + 4) % 160).toInt()
            val a = (fnvU(i * 13 + 5) % 360) * Math.PI / 180.0
            vx[i] = (sp * cos(a)).toFloat(); vy[i] = (sp * sin(a)).toFloat()
            val c1 = Color(f and 255, (f shr 8) and 255, (f shr 16) and 255)
            val c2 = Color((f shr 24) and 255, (f shr 4) and 255, (f shr 12) and 255)
            Brush.linearGradient(listOf(c1, c2), start = Offset.Zero, end = Offset(w[i], h[i]))
        }
    }
    fun step(dt: Float) {
        for (i in 0 until n) {
            var nx = x[i] + vx[i] * dt; var ny = y[i] + vy[i] * dt
            if (nx < 0f) { nx = -nx; vx[i] = -vx[i] } else if (nx + w[i] > areaW) { nx = 2 * (areaW - w[i]) - nx; vx[i] = -vx[i] }
            if (ny < 0f) { ny = -ny; vy[i] = -vy[i] } else if (ny + h[i] > areaH) { ny = 2 * (areaH - h[i]) - ny; vy[i] = -vy[i] }
            x[i] = nx; y[i] = ny
        }
    }
}

class Graph {
    val vals = FloatArray(10_000); var head = 0; var frame = 0
    fun push() {
        frame++
        vals[head] = 50f + 35f * sin(frame * 0.05f) + ((fnvU(frame) % 20).toInt() - 10)
        head = (head + 1) % vals.size
    }
}

class CanvasSc(val shapes: Shapes, val graph: Graph?, val tick: MutableLongState) : Sc {
    override suspend fun step(t: Double, dt: Double) {
        shapes.step(dt.toFloat().coerceAtMost(0.1f))
        graph?.push()
        tick.longValue++
    }
    var shapesN = shapes.n
    override fun finish(run: Run) {
        run.ops["shapes"] = shapesN
        run.notes = FRAME_NOTE + " Canvas/DrawScope: single Canvas, shapes drawn with pre-built gradient Brushes translated per shape, Skia drop-shadow (sigma 4) via nativeCanvas for 1/10, clipped to 1000x600 rounded rect; graph = Path of 10000 lineTo rebuilt per frame. Bounce area = window 1280x720."
    }
}

/** Dessine formes + (option) graphe dans le Canvas ; ox/oy/cw/ch = rect de clip. */
@Composable
fun ShapesCanvas(modifier: Modifier, shapes: Shapes, graph: Graph?, tick: MutableLongState, ox: Float, oy: Float, cw: Float, chh: Float) {
    val clip = remember { Path().apply { addRoundRect(RoundRect(ox, oy, ox + cw, oy + chh, CornerRadius(24f, 24f))) } }
    Canvas(modifier) {
        tick.longValue // invalidation de la phase dessin uniquement (pas de recomposition)
        val body: DrawScope.() -> Unit = {
            translate(ox, oy) {
                for (i in 0 until shapes.n) {
                    val s = shapes
                    translate(s.x[i], s.y[i]) {
                        val kind = i % 3
                        if (s.withShadow && i % 10 == 0) {
                            drawIntoCanvas { c ->
                                val nc = c.nativeCanvas
                                when (kind) {
                                    0 -> nc.drawRect(Rect.makeWH(s.w[i], s.h[i]), s.shadowPaint)
                                    1 -> nc.drawCircle(s.w[i] / 2, s.h[i] / 2, s.w[i] / 2, s.shadowPaint)
                                    else -> nc.drawRRect(RRect.makeXYWH(0f, 0f, s.w[i], s.h[i], 8f), s.shadowPaint)
                                }
                            }
                        }
                        when (kind) {
                            0 -> drawRect(s.brushes[i], size = Size(s.w[i], s.h[i]))
                            1 -> drawCircle(s.brushes[i], radius = s.w[i] / 2, center = Offset(s.w[i] / 2, s.h[i] / 2))
                            else -> drawRoundRect(s.brushes[i], size = Size(s.w[i], s.h[i]), cornerRadius = CornerRadius(8f, 8f))
                        }
                    }
                }
            }
        }
        clipPath(clip, block = body)
        if (graph != null) {
            val p = Path()
            val n = graph.vals.size
            val gx = 20f; val gy = 615f
            for (k in 0 until n) {
                val v = graph.vals[(graph.head + k) % n]
                val px = gx + k * (1000f / n); val py = gy + 100f - v
                if (k == 0) p.moveTo(px, py) else p.lineTo(px, py)
            }
            drawRect(Color(0xFF15151A), topLeft = Offset(gx, gy), size = Size(1000f, 100f))
            drawPath(p, Color(0xFF4FC3F7), style = Stroke(1f))
        }
    }
}

@Composable
fun CanvasScenario(run: Run) {
    val shapes = remember { Shapes(run.args.shapes, 1280f, 720f) }
    val graph = remember { Graph().also { g -> repeat(10_000) { g.push() } } }
    val tick = remember { mutableLongStateOf(0L) }
    val sc = remember { CanvasSc(shapes, graph, tick) }
    FrameLoop(run, sc)
    Box(Modifier.fillMaxSize().androidxBg()) {
        ShapesCanvas(Modifier.fillMaxSize(), shapes, graph, tick, 20f, 10f, 1000f, 600f)
    }
}

fun Modifier.androidxBg(): Modifier = this.then(Modifier.background(Color(0xFF1E1E24)))
