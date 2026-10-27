package bench

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.shadow.DropShadowPainter
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.platform.LocalGraphicsContext
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import kotlin.math.cos
import kotlin.math.sin

/** Simulation + objets de dessin pré-alloués (pinceaux en coordonnées locales, translatés au dessin). Rebond dans areaW x areaH. */
class Shapes(val n: Int, val areaW: Float, val areaH: Float, val withShadow: Boolean = true) {
    val x = FloatArray(n); val y = FloatArray(n); val vx = FloatArray(n); val vy = FloatArray(n)
    val w = FloatArray(n); val h = FloatArray(n); val sz = IntArray(n)
    val brushes: Array<Brush>
    init {
        brushes = Array(n) { i ->
            val f = fnv(i * 7 + 1)
            val s = 16 + (fnvU(i * 3 + 1) % 40).toInt()
            sz[i] = s
            w[i] = if (i % 3 == 2) s * 1.5f else s.toFloat(); h[i] = s.toFloat()
            x[i] = (fnvU(i * 5 + 2) % (areaW - w[i]).toLong()).toFloat(); y[i] = (fnvU(i * 5 + 3) % (areaH - h[i]).toLong()).toFloat()
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
    override fun finish(run: Run) {
        run.ops["shapes"] = shapes.n
        run.ops["not_rendered"] = emptyList<String>()
        run.notes = FRAME_NOTE + " Canvas: one Modifier.drawWithCache node (clip Path + graph Path allocated once, graph Path rewind()+10000 lineTo " +
            "per frame), per-shape pre-built linear-gradient Brush (local coords + translate), drop shadow for 1/10 via the public " +
            "ShadowContext.createDropShadowPainter(shape, Shadow(radius=8.dp, offset=2,2)) (blurred shadow bitmap cached per size), all " +
            "clipped by clipPath(rounded rect 1000x600, r=24), shapes bounce inside that rect; tick state read in onDrawBehind => draw-phase " +
            "invalidation only (no recomposition)."
    }
}

/** Dessine formes + (option) graphe ; ox/oy/cw/chh = rect de clip (et zone de rebond, translatée). */
@Composable
fun ShapesCanvas(modifier: Modifier, shapes: Shapes, graph: Graph?, tick: MutableLongState, ox: Float, oy: Float, cw: Float, chh: Float) {
    val shadowCtx = LocalGraphicsContext.current.shadowContext
    // une DropShadowPainter par (forme, taille) : le bitmap flouté est mis en cache par le painter
    val shadowPainters = remember(shapes) {
        val m = HashMap<Int, DropShadowPainter>()
        val sh = Shadow(radius = 8.dp, color = Color(0x99000000), offset = DpOffset(2.dp, 2.dp))
        if (shapes.withShadow) for (i in 0 until shapes.n step 10) {
            val kind = i % 3; val key = kind * 1000 + shapes.sz[i]
            m.getOrPut(key) {
                shadowCtx.createDropShadowPainter(when (kind) { 0 -> RectangleShape; 1 -> CircleShape; else -> RoundedCornerShape(8.dp) }, sh)
            }
        }
        m
    }
    Box(modifier.drawWithCache {
        val clip = Path().apply { addRoundRect(RoundRect(ox, oy, ox + cw, oy + chh, CornerRadius(24f, 24f))) }
        val graphPath = Path()
        val graphBg = Color(0xFF15151A); val graphFg = Color(0xFF4FC3F7); val stroke = Stroke(1f)
        onDrawBehind {
            tick.longValue // invalidation de la phase dessin uniquement
            val s = shapes
            clipPath(clip) {
                translate(ox, oy) {
                    for (i in 0 until s.n) {
                        val kind = i % 3
                        translate(s.x[i], s.y[i]) {
                            if (s.withShadow && i % 10 == 0) {
                                val p = shadowPainters[kind * 1000 + s.sz[i]]!!
                                with(p) { draw(Size(s.w[i], s.h[i])) }
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
            if (graph != null) {
                graphPath.rewind()
                val n = graph.vals.size
                val gx = 20f; val gy = 615f
                for (k in 0 until n) {
                    val v = graph.vals[(graph.head + k) % n]
                    val px = gx + k * (1000f / n); val py = gy + 100f - v
                    if (k == 0) graphPath.moveTo(px, py) else graphPath.lineTo(px, py)
                }
                drawRect(graphBg, topLeft = Offset(gx, gy), size = Size(1000f, 100f))
                drawPath(graphPath, graphFg, style = stroke)
            }
        }
    })
}

@Composable
fun CanvasScenario(run: Run) {
    val shapes = remember { Shapes(run.args.shapes, 1000f, 600f) }
    val graph = remember { Graph().also { g -> repeat(10_000) { g.push() } } }
    val tick = remember { mutableLongStateOf(0L) }
    val sc = remember { CanvasSc(shapes, graph, tick) }
    FrameLoop(run, sc)
    Box(Modifier.fillMaxSize().background(Color(0xFF1E1E24))) {
        ShapesCanvas(Modifier.fillMaxSize(), shapes, graph, tick, 20f, 10f, 1000f, 600f)
    }
}
