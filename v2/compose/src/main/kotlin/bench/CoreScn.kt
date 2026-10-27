package bench

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.*
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/** Threads de fond du cœur C-ABI (Dispatchers.Default) : boucle serrée + core_heavy(100) chaque seconde. */
class CoreWork(val scope: CoroutineScope) {
    val calls = AtomicLong(); val heavy = AtomicLong(); val stop = AtomicBoolean(false)
    var t0 = 0L; var tight: Job? = null; var heavyJob: Job? = null
    fun start() {
        t0 = System.nanoTime()
        tight = scope.launch(Dispatchers.Default) { Core.tightLoop({ stop.get() }, calls) }
        heavyJob = scope.launch(Dispatchers.Default) {
            while (isActive) { delay(1000); Core.heavy.invoke(100); heavy.incrementAndGet() }
        }
    }
    fun finish(run: Run) {
        val secs = (System.nanoTime() - t0) / 1e9
        stop.set(true)
        run.ops["calls_total"] = calls.get(); run.ops["calls_per_s"] = Math.round(calls.get() / secs)
        run.ops["heavy_calls"] = heavy.get()
        run.ops["ffi"] = "java.lang.foreign (FFM, final API, JDK 25) downcall MethodHandle.invoke; validate/parse/hash linked with Linker.Option.critical(false)"
        run.ops["published_sink"] = Core.published.get()
        run.ops["core_lib"] = Core.LIB
    }
}

class CoreSc(val canvas: CanvasSc, val work: CoreWork) : Sc {
    override suspend fun reset() { work.start() }
    override suspend fun step(t: Double, dt: Double) = canvas.step(t, dt)
    override fun finish(run: Run) {
        canvas.finish(run); work.finish(run)
        var mx = 0.0; for (i in 0 until run.frames.n) mx = maxOf(mx, run.frames.a[i])
        run.ops["max_frame_stall_ms"] = mx
        run.ops["shapes"] = canvas.shapes.n
    }
}

@Composable
fun CoreScenario(run: Run) {
    val shapes = remember { Shapes(1000, 1000f, 600f, withShadow = false) }
    val tick = remember { mutableLongStateOf(0L) }
    val scope = rememberCoroutineScope()
    val sc = remember { CoreSc(CanvasSc(shapes, null, tick), CoreWork(scope)) }
    FrameLoop(run, sc)
    Box(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Color(0xFF1E1E24))) { ShapesCanvas(Modifier.fillMaxSize(), shapes, null, tick, 20f, 10f, 1000f, 600f) }
}

class StressSc(val list: ListSc, val canvas: CanvasSc, val work: CoreWork) : Sc {
    var nextSample = 10.0
    val samples = ArrayList<String>()
    override suspend fun reset() { list.reset(); work.start(); nextSample = 10.0; samples.clear() }
    override suspend fun step(t: Double, dt: Double) {
        list.step(t, dt); canvas.step(t, dt)
        if (t >= nextSample) {
            val rt = Runtime.getRuntime()
            samples.add("{\"t\":${nextSample.toInt()},\"managed_heap_kb\":${(rt.totalMemory() - rt.freeMemory()) / 1024}}")
            nextSample += 10.0
        }
    }
    override fun finish(run: Run) {
        list.finish(run); work.finish(run)
        run.ops["shapes"] = canvas.shapes.n
        run.ops["heap_samples"] = RawJson(samples.joinToString(",", "[", "]"))
        var mx = 0.0; for (i in 0 until run.frames.n) mx = maxOf(mx, run.frames.a[i])
        run.ops["max_frame_stall_ms"] = mx
        run.notes = FRAME_NOTE + " stress = LazyColumn(880px)+stream+Canvas panel 400x720 (5000 shapes, bounce in panel)+FFM core threads; managed_heap_kb = JVM used heap (total-free) every 10 s."
    }
}

@Composable
fun StressScenario(run: Run) {
    val state = rememberLazyListState()
    val prices = remember { PriceStore() }
    val counter = remember { Counter() }
    val scope = rememberCoroutineScope()
    val stream = remember { UpdateStream(run.args.rate, scope) }
    val shapes = remember { Shapes(run.args.shapes, 400f, 720f) }
    val tick = remember { mutableLongStateOf(0L) }
    val sc = remember { StressSc(ListSc(state, prices, counter, stream), CanvasSc(shapes, null, tick), CoreWork(scope)) }
    FrameLoop(run, sc)
    Row(Modifier.fillMaxSize().background(androidx.compose.ui.graphics.Color(0xFF1E1E24))) {
        Box(Modifier.width(880.dp).fillMaxHeight()) { ListView(Modifier.fillMaxSize(), state, prices, counter) }
        ShapesCanvas(Modifier.width(400.dp).fillMaxHeight(), shapes, null, tick, 0f, 0f, 400f, 720f)
    }
}
