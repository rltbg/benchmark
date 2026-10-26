package bench

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.delay
import java.awt.EventQueue
import java.lang.management.ManagementFactory
import kotlin.system.exitProcess

/** Un scénario piloté par la boucle de frames. t = secondes depuis le début de la phase (warm-up puis mesure). */
interface Sc {
    suspend fun reset() {}
    suspend fun step(t: Double, dt: Double) {}
    fun finish(run: Run) {}
}

const val WARMUP_NS = 2_000_000_000L

const val FRAME_NOTE = "frame_ms = interval between consecutive post-present timestamps: in each withFrameNanos callback " +
    "(Compose FrameRecomposer.performFrame, which runs at the START of the frame, before measure/layout/draw) the app posts " +
    "java.awt.EventQueue.invokeLater; Skiko's FrameDispatcher runs update+draw+finishFrame (SOFTWARE_FAST: raster surface -> " +
    "X11 put image) inside the same EDT event, so the posted runnable runs right after finishFrame returned = the latest " +
    "public post-render signal. Redraw is continuous (each frame requests the next one and changes content). Pacing (ops.pacing): " +
    "'60' (default) = Skiko's native limiter kept on (public property skiko.vsync.enabled=true, Skiko default: software redrawer " +
    "awaits FrameLimiter = 1000/displayRefreshRate) BUT under Xvfb it does not cap (measured ~170 fps on list: the refresh rate " +
    "Skiko reads natively from X11 is not 60), so the app paces with a public API: at the start of every frame (withFrameNanos " +
    "callback, EDT, runs before measure/layout/draw of that frame, including frames driven by Compose animations) it parks " +
    "(LockSupport.parkNanos) until an absolute deadline grid of 16.667 ms, re-anchored after a missed deadline; " +
    "'uncapped' = skiko.vsync.enabled=false and no app pacing."

/** Cadence 60 Hz par API publique (voir FRAME_NOTE). */
object Pacer {
    @Volatile var enabled = true
    private const val PERIOD = 16_666_667L
    private var deadline = 0L
    fun waitFrame() {
        if (!enabled) return
        var now = System.nanoTime()
        if (deadline == 0L || now - deadline > PERIOD) deadline = now   // 1re frame ou échéance manquée : ré-ancrage
        while (now < deadline) { java.util.concurrent.locks.LockSupport.parkNanos(deadline - now); now = System.nanoTime() }
        deadline += PERIOD
    }
}

fun finishAndExit(run: Run, sc: Sc?) {
    run.markEnd()
    sc?.finish(run)
    run.write()
    System.out.flush()
    exitProcess(0)
}

/** Horodatage post-présentation (voir FRAME_NOTE). */
class PresentClock(val run: Run) {
    @Volatile var measuring = false
    private var last = 0L
    val cb = Runnable {
        val now = System.nanoTime()
        if (measuring) { if (last != 0L) run.frames.add((now - last) / 1e6); last = now }
    }
}

/** Boucle continue withFrameNanos : chaque frame demande la suivante et fait avancer le scénario. */
@Composable
fun FrameLoop(run: Run, sc: Sc) {
    LaunchedEffect(Unit) {
        val pc = PresentClock(run)
        var t0 = 0L; var prev = 0L; var measuring = false
        var first = true
        while (true) {
            val now = withFrameNanos { Pacer.waitFrame(); EventQueue.invokeLater(pc.cb); System.nanoTime() }
            if (first) {
                first = false
                run.ops["startup_to_first_frame_ms"] = System.currentTimeMillis() - ManagementFactory.getRuntimeMXBean().startTime
                t0 = now; prev = now
            }
            if (!measuring && now - t0 >= WARMUP_NS) {
                measuring = true; sc.reset(); t0 = now; prev = now
                run.markStart(); pc.measuring = true
            }
            if (measuring && (now - t0) / 1e9 >= run.args.duration) { pc.measuring = false; finishAndExit(run, sc) }
            sc.step((now - t0) / 1e9, (now - prev) / 1e9)
            prev = now
        }
    }
}

@Composable
fun Shell(content: @Composable () -> Unit) {
    Column(Modifier.fillMaxSize().background(Color(0xFF1E1E24))) {
        Box(Modifier.fillMaxWidth().height(28.dp).background(Color(0xFF2D2D36)), contentAlignment = Alignment.CenterStart) {
            BasicText("  Bench App", style = TextStyle(color = Color.White, fontSize = 13.sp))
        }
        Box(Modifier.fillMaxWidth().weight(1f)) { content() }
        Box(Modifier.fillMaxWidth().height(24.dp).background(Color(0xFF2D2D36)), contentAlignment = Alignment.CenterStart) {
            BasicText("  Ready - 100000 items", style = TextStyle(color = Color(0xFFBBBBBB), fontSize = 12.sp))
        }
    }
}

/** Scénario input : état du carré (noir/blanc) basculé par le chemin d'événements clavier normal (Window.onKeyEvent). */
class InputState { val white = mutableStateOf(false); var keyEvents = 0L; var refocus = 0 }

fun main(argv: Array<String>) {
    // Cadence (spec v2.1) : BENCH_PACING=60 (défaut) = limiteur natif de Skiko (skiko.vsync.enabled=true, défaut Skiko :
    // en rendu logiciel, FrameLimiter au taux de rafraîchissement de l'écran) ; BENCH_PACING=uncapped = skiko.vsync.enabled=false.
    val pacing = when (System.getenv("BENCH_PACING") ?: "60") { "uncapped" -> "uncapped"; else -> "60" }
    System.setProperty("skiko.vsync.enabled", if (pacing == "uncapped") "false" else "true")
    val args = parseArgs(argv)
    val run = Run(args)
    run.ops["pacing"] = pacing
    Pacer.enabled = pacing == "60"
    run.ops["jvm_start_to_main_ms"] = (System.currentTimeMillis() - ManagementFactory.getRuntimeMXBean().startTime)
    val input = InputState()
    if (System.getenv("BENCH_DEBUG_KEYS") == "1") java.awt.Toolkit.getDefaultToolkit().addAWTEventListener({ e ->
        System.err.println("AWT ${System.nanoTime() / 1000000} ${e.id} ${e.javaClass.simpleName} ${(e as? java.awt.event.KeyEvent)?.keyCode} src=${e.source.javaClass.simpleName}")
    }, java.awt.AWTEvent.KEY_EVENT_MASK or java.awt.AWTEvent.FOCUS_EVENT_MASK or java.awt.AWTEvent.WINDOW_FOCUS_EVENT_MASK)
    application {
        val state = rememberWindowState(size = DpSize(1280.dp, 720.dp), position = WindowPosition(0.dp, 0.dp))
        Window(
            onCloseRequest = ::exitApplication, state = state, title = "bench", undecorated = true, resizable = false,
            onKeyEvent = {
                if (args.scenario == "input" && it.key == Key.Spacebar && it.type == KeyEventType.KeyDown) {
                    input.keyEvents++; input.white.value = !input.white.value; true
                } else false
            }
        ) {
            LaunchedEffect(Unit) {
                run.ops["render_api"] = window.renderApi.name
                run.ops["vsync_enabled"] = System.getProperty("skiko.vsync.enabled")
                if (args.scenario == "input") {
                    // Sans WM, XToolkit garde le focus clavier X sur sa fenêtre FocusProxy ; un XSetInputFocus externe sur la
                    // toplevel (harnais : xdotool windowfocus) est vu comme une perte de focus et AWT ignore ensuite les touches.
                    // Ré-demande publique du focus (Window.toFront + Component.requestFocus sur le dernier propriétaire) quand le focus part hors de l'app.
                    window.addWindowFocusListener(object : java.awt.event.WindowAdapter() {
                        override fun windowLostFocus(e: java.awt.event.WindowEvent) {
                            if (e.oppositeWindow != null) return
                            val owner = window.mostRecentFocusOwner   // composant Compose qui avait le focus
                            java.awt.EventQueue.invokeLater { input.refocus++; window.toFront(); (owner ?: window).requestFocus() }
                        }
                    })
                }
            }
            Root(run, input)
        }
    }
}

@Composable
fun Root(run: Run, input: InputState) {
    val a = run.args
    when (a.scenario) {
        "idle" -> {
            LaunchedEffect(Unit) {
                delay(WARMUP_NS / 1_000_000)
                run.ops["sum_heights"] = SUM_H
                run.notes = "idle: shell + first page, no animation, no redraw loop; the app sleeps `duration` between the markers. frame_ms empty."
                run.markStart()
                delay((a.duration * 1000).toLong())
                finishAndExit(run, null)
            }
            Shell { ListView(Modifier.fillMaxSize(), androidx.compose.foundation.lazy.rememberLazyListState(), remember { PriceStore() }, remember { Counter() }) }
        }
        "list" -> ListScenario(run, 0)
        "stream" -> ListScenario(run, a.rate)
        "input" -> {
            Box(Modifier.fillMaxSize()) {
                ListScenario(run, 0, input)
                // carré 200x200 en (100,100) (coordonnées fenêtre, échelle 1) ; lecture de l'état en phase dessin uniquement
                Box(Modifier.offset(100.dp, 100.dp).size(200.dp).drawBehind {
                    drawRect(if (input.white.value) Color.White else Color.Black)
                })
            }
        }
        "canvas" -> CanvasScenario(run)
        "core" -> CoreScenario(run)
        "stress" -> StressScenario(run)
        "text" -> TextScenario(run)
        "images" -> ImagesScenario(run)
        "nav" -> NavScenario(run)
        else -> { run.unsupported = "unknown scenario"; LaunchedEffect(Unit) { run.markStart(); finishAndExit(run, null) } }
    }
}
