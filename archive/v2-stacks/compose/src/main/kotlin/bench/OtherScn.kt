package bench

import androidx.compose.animation.*
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.text.selection.SelectionState
import androidx.compose.foundation.text.selection.rememberSelectionState
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.asComposeImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.*
import org.jetbrains.skia.Bitmap
import org.jetbrains.skia.ColorAlphaType
import org.jetbrains.skia.ColorType
import org.jetbrains.skia.Image as SkImage
import org.jetbrains.skia.ImageInfo
import org.jetbrains.skia.SamplingMode
import java.io.File
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicLong

// ---------------- TEXT ----------------
class TextSc(val state: LazyListState, val sel: SelectionState, val lines: List<String>, val loadMs: Double) : Sc {
    var offset = 0.0
    var selected = false
    var selectAllMs = -1.0
    override suspend fun reset() { state.scrollToItem(0); offset = 0.0; selected = false; sel.clear() }
    override suspend fun step(t: Double, dt: Double) {
        // aller-retour 3000 px/s, demi-période 5 s
        val dir = if (((t / 5.0).toInt()) % 2 == 0) 1f else -1f
        offset += state.scrollPx(dir * 3000f * dt.toFloat())
        if (!selected && t >= 5.0) {
            val s = System.nanoTime()
            sel.selectAll()     // SelectionState.selectAll() : API publique (Compose 1.12)
            selectAllMs = (System.nanoTime() - s) / 1e6
            selected = true
        }
    }
    override fun finish(run: Run) {
        run.ops["lines"] = lines.size
        run.ops["final_offset"] = Math.round(offset)
        run.ops["load_ms"] = loadMs
        val selChars = sel.selectedTexts.sumOf { it.length }
        run.ops["select_all"] = if (selected) "partial: SelectionState.selectAll() at t=5s selects only composed LazyColumn items (documented)" else "not reached (duration < 5 s)"
        run.ops["select_all_selected_chars"] = selChars
        run.ops["select_all_call_ms"] = selectAllMs
        run.notes = FRAME_NOTE + " text: 1 MB split into ${lines.size} lines (paragraphs) in a LazyColumn (key=index) inside " +
            "SelectionContainer(rememberSelectionState()); scripted scroll via scroll{scrollBy}. Select-all at t=5 s via the public " +
            "SelectionState.selectAll(): per the API doc, texts of non-composed lazy items are not included, so only the composed " +
            "lines get selected (select_all_selected_chars). A single read-only BasicTextField holding the whole 1 MB was tried first: " +
            "its layout (SkParagraph _nLayout on the UI thread) did not finish within 60 s (logs), so it is not usable here. " +
            "time_to_first_display_ms = JVM start -> after the present of the first frame with text; layout_ms = composition start -> first frame."
    }
}

@Composable
fun TextScenario(run: Run) {
    val t0 = remember { System.nanoTime() }
    val loaded = remember {
        val s = System.nanoTime()
        val l = File(run.args.assets, "text1mb.txt").readText().split('\n')
        l to (System.nanoTime() - s) / 1e6
    }
    val lines = loaded.first
    val state = rememberLazyListState()
    val sel = rememberSelectionState()
    val sc = remember { TextSc(state, sel, lines, loaded.second) }
    LaunchedEffect(Unit) {
        withFrameNanos { }
        run.ops["layout_ms"] = (System.nanoTime() - t0) / 1e6
        java.awt.EventQueue.invokeLater {
            run.ops["time_to_first_display_ms"] = (System.currentTimeMillis() - java.lang.management.ManagementFactory.getRuntimeMXBean().startTime).toDouble()
        }
    }
    FrameLoop(run, sc)
    Shell {
        SelectionContainer(sel) {
            LazyColumn(Modifier.fillMaxSize(), state = state) {
                items(lines.size, key = { it }, contentType = { 0 }) { i ->
                    BasicText(lines[i], style = textStyle, modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp))
                }
            }
        }
    }
}

private val textStyle = TextStyle(color = Color(0xFFE0E0E0), fontSize = 14.sp)

// ---------------- IMAGES ----------------
class ImageCache(val dir: File) {
    val states = Array(600) { mutableStateOf<ImageBitmap?>(null) }
    val decoded = AtomicInteger(); val decodeNs = AtomicLong()
    fun decode(i: Int) {
        val t = System.nanoTime()
        val img = SkImage.makeFromEncoded(File(dir, "img%03d.png".format(i)).readBytes())
        val full = Bitmap.makeFromImage(img)           // décodage PNG complet hors thread UI
        val dst = Bitmap(); dst.allocPixels(ImageInfo(128, 128, ColorType.N32, ColorAlphaType.PREMUL))
        full.peekPixels()!!.scalePixels(dst.peekPixels()!!, SamplingMode.LINEAR)  // vignette 128x128
        // IMPORTANT : immuable, sinon Image.makeFromBitmap (appelé par SkiaBackedCanvas à CHAQUE drawImage) copie les pixels
        dst.setImmutable()
        val bm = dst.asComposeImageBitmap()
        full.close(); img.close()
        decodeNs.addAndGet(System.nanoTime() - t); decoded.incrementAndGet()
        states[i].value = bm   // écriture de snapshot state depuis un thread de fond (snapshot global, thread-safe)
    }
    private val requested = java.util.concurrent.atomic.AtomicIntegerArray(600)
    private val pool = Dispatchers.Default.limitedParallelism(2)
    /** Décodage à la demande (item composé), une seule fois par image, sur 2 threads de fond ; non annulé si l'item sort de l'écran. */
    fun request(i: Int, scope: CoroutineScope) {
        if (states[i].value == null && requested.compareAndSet(i, 0, 1)) scope.launch(pool) { decode(i) }
    }
}

class ImagesSc(val grid: LazyGridState, val cache: ImageCache, val zoom: MutableFloatState, val pan: MutableFloatState, val scope: CoroutineScope) : Sc {
    var offset = 0.0
    override suspend fun reset() { grid.scrollToItem(0); offset = 0.0 }
    override suspend fun step(t: Double, dt: Double) {
        val dir = if (((t / 3.0).toInt()) % 2 == 0) 1f else -1f   // aller-retour sur la grille
        offset += grid.scrollPx(dir * 3000f * dt.toFloat())
        val ph = (t % 6.0) / 6.0
        zoom.floatValue = (1.0 + 2.0 * (1.0 - Math.abs(2 * ph - 1.0))).toFloat()
        pan.floatValue = (Math.sin(t * 1.3) * 120.0 * (zoom.floatValue - 1f)).toFloat()
    }
    override fun finish(run: Run) {
        run.ops["images_decoded"] = cache.decoded.get()
        run.ops["decode_total_ms"] = cache.decodeNs.get() / 1e6
        run.ops["final_offset"] = Math.round(offset)
        run.notes = FRAME_NOTE + " images: LazyVerticalGrid 9 cols x 128dp (key=index, one contentType); each PNG decoded (Skia) and " +
            "downscaled to 128x128 on demand when its item is first composed (warm-up included, same script), once, on 2 background threads " +
            "(Dispatchers.Default.limitedParallelism(2)), placeholder until ready; bitmaps " +
            "set immutable (avoids a pixel copy per draw in SkiaBackedCanvas.drawImageRect -> Image.makeFromBitmap); Image() default " +
            "FilterQuality.Low (bilinear, no mipmaps); zoom 1->3->1 / 6 s + pan via one graphicsLayer (state read in the layer block: " +
            "no recomposition). images_decoded / decode_total_ms (sum of per-image decode times) cover the whole run incl. warm-up."
    }
}

@Composable
fun ImagesScenario(run: Run) {
    val cache = remember { ImageCache(File(run.args.assets, "images")) }
    val grid = rememberLazyGridState()
    val zoom = remember { mutableFloatStateOf(1f) }
    val pan = remember { mutableFloatStateOf(0f) }
    val scope = rememberCoroutineScope()
    val sc = remember { ImagesSc(grid, cache, zoom, pan, scope) }
    FrameLoop(run, sc)
    Shell {
        Box(Modifier.fillMaxSize().graphicsLayer { scaleX = zoom.floatValue; scaleY = zoom.floatValue; translationX = pan.floatValue; transformOrigin = TransformOrigin.Center }) {
            LazyVerticalGrid(GridCells.Fixed(9), Modifier.fillMaxSize().padding(horizontal = 40.dp), state = grid,
                horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                items(600, key = { it }, contentType = { 0 }) { i ->
                    LaunchedEffect(i) { cache.request(i, scope) }
                    Thumb(cache.states[i])
                }
            }
        }
    }
}

private val thumbBg = Color(0xFF33333B)

@Composable
fun Thumb(state: State<ImageBitmap?>) {
    Box(Modifier.size(128.dp).background(thumbBg)) {
        state.value?.let { Image(it, null, Modifier.fillMaxSize()) }
    }
}

// ---------------- NAV ----------------
class NavState { var screen by mutableIntStateOf(0); var forward by mutableStateOf(true); var modal by mutableStateOf(false) }

class NavSc(val nav: NavState, val scope: CoroutineScope) : Sc {
    var transitions = 0; var modals = 0
    var job: Job? = null; var burn: Job? = null
    override suspend fun reset() {
        burn = scope.launch(Dispatchers.Default) { var a = 1L; while (isActive) { a = a * 6364136223846793005L + 1442695040888963407L; if (a == 42L) println() } }
        job = scope.launch {
            var n = 0
            while (true) {
                // push, push, pop, pop ... ; une modale tous les 5 pas
                val fwd = (n / 2) % 2 == 0
                nav.forward = fwd
                nav.screen = if (fwd) (nav.screen + 1) % 3 else (nav.screen + 2) % 3
                delay(300); transitions++
                n++
                if (n % 5 == 0) { nav.modal = true; delay(200); modals++; nav.modal = false; delay(200); modals++ }
            }
        }
    }
    override fun finish(run: Run) {
        run.ops["transitions_done"] = transitions; run.ops["modal_toggles_done"] = modals
        run.notes = FRAME_NOTE + " AnimatedContent slide (tween 300ms) push/pop among 3 screens, AnimatedVisibility fade 200ms modal; a Dispatchers.Default coroutine busy-loops ~1 core. Transitions/modals run back-to-back (no static hold, so every frame changes) until duration elapses (counted in transitions_done; 20 transitions need ~7.6 s)."
    }
}

@Composable
fun NavScenario(run: Run) {
    val scope = rememberCoroutineScope()
    val nav = remember { NavState() }
    val sc = remember { NavSc(nav, scope) }
    FrameLoop(run, sc)
    Shell {
        Box(Modifier.fillMaxSize()) {
            AnimatedContent(nav.screen, transitionSpec = {
                if (nav.forward) slideInHorizontally(tween(300)) { it } togetherWith slideOutHorizontally(tween(300)) { -it / 3 }
                else slideInHorizontally(tween(300)) { -it } togetherWith slideOutHorizontally(tween(300)) { it / 3 }
            }) { s ->
                val bg = listOf(Color(0xFF2E5266), Color(0xFF6E3B5C), Color(0xFF3E6B48))[s]
                Column(Modifier.fillMaxSize().background(bg).padding(24.dp)) {
                    BasicText("Screen $s", style = TextStyle(color = Color.White, fontSize = 28.sp))
                    LazyColumn(Modifier.fillMaxSize()) { items(40) { i -> BasicText("Entry $i on screen $s", style = TextStyle(color = Color.White, fontSize = 16.sp, fontFamily = FontFamily.SansSerif), modifier = Modifier.padding(8.dp)) } }
                }
            }
            AnimatedVisibility(nav.modal, enter = fadeIn(tween(200)), exit = fadeOut(tween(200))) {
                Box(Modifier.fillMaxSize().background(Color(0x99000000)), contentAlignment = Alignment.Center) {
                    Box(Modifier.size(500.dp, 300.dp).background(Color(0xFFEEEEEE)), contentAlignment = Alignment.Center) {
                        BasicText("Modal", style = TextStyle(color = Color.Black, fontSize = 24.sp))
                    }
                }
            }
        }
    }
}
