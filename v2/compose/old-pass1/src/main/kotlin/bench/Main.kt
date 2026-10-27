package bench

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.text.TextStyle
import androidx.compose.foundation.text.BasicText
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.delay
import kotlin.system.exitProcess

/** Un scénario piloté par la boucle de frames. t = secondes depuis le début de la phase (warm-up puis mesure). */
interface Sc {
    suspend fun reset() {}
    suspend fun step(t: Double, dt: Double) {}
    fun finish(run: Run) {}
}

const val WARMUP_NS = 2_000_000_000L

fun finishAndExit(run: Run, sc: Sc?) {
    sc?.finish(run)
    run.write()
    System.out.flush()
    exitProcess(0)
}

/** Boucle continue withFrameNanos : chaque frame demande la suivante immédiatement. */
@Composable
fun FrameLoop(run: Run, sc: Sc) {
    LaunchedEffect(Unit) {
        var t0 = 0L; var prev = 0L; var measuring = false; var measureStart = 0L
        var first = true
        while (true) {
            val now = withFrameNanos { System.nanoTime() }
            if (first) { first = false; run.ops["startup_to_first_frame_ms"] = System.currentTimeMillis() - java.lang.management.ManagementFactory.getRuntimeMXBean().startTime; t0 = now; prev = now; measureStart = now }
            if (!measuring && now - t0 >= WARMUP_NS) {
                measuring = true; sc.reset(); t0 = now; prev = now; measureStart = now
            }
            if (measuring) {
                run.frames.add((now - prev) / 1e6)
                if ((now - t0) / 1e9 >= run.args.duration) finishAndExit(run, sc)
            }
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

fun main(argv: Array<String>) {
    val args = parseArgs(argv)
    val run = Run(args)
    run.ops["jvm_start_to_main_ms"] = (System.currentTimeMillis() - java.lang.management.ManagementFactory.getRuntimeMXBean().startTime)
    application {
        val state = rememberWindowState(size = DpSize(1280.dp, 720.dp), position = WindowPosition(0.dp, 0.dp))
        Window(onCloseRequest = ::exitApplication, state = state, title = "bench", undecorated = true, resizable = false) {
            Root(run)
        }
    }
}

@Composable
fun Root(run: Run) {
    val a = run.args
    when (a.scenario) {
        "idle" -> {
            LaunchedEffect(Unit) {
                delay(1000)
                run.ops["sum_heights"] = SUM_H
                run.notes = "idle: shell + first page, no animation, no redraw loop; JVM sleeps duration after first composition (+1s settle). frame_ms empty."
                delay((a.duration * 1000).toLong())
                finishAndExit(run, null)
            }
            Shell { ListView(Modifier.fillMaxSize(), androidx.compose.foundation.lazy.rememberLazyListState(), PriceStore(), Counter()) }
        }
        "list" -> ListScenario(run, 0)
        "stream" -> ListScenario(run, a.rate)
        "canvas" -> CanvasScenario(run)
        "core" -> CoreScenario(run)
        "stress" -> StressScenario(run)
        "text" -> TextScenario(run)
        "images" -> ImagesScenario(run)
        "nav" -> NavScenario(run)
        else -> { run.unsupported = "unknown scenario"; LaunchedEffect(Unit) { finishAndExit(run, null) } }
    }
}
