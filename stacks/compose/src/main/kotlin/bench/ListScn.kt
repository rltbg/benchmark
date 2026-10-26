package bench

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import java.util.concurrent.atomic.AtomicLong
import kotlin.math.exp

class Counter { @JvmField var drawn = 0L }

/** Prix par ligne : un MutableDoubleState créé à la demande ; seul le composable PriceText de la ligne concernée se recompose. */
class PriceStore {
    private val states = arrayOfNulls<MutableDoubleState>(ROWS)
    fun state(i: Int): MutableDoubleState {
        var s = states[i]
        if (s == null) { s = mutableDoubleStateOf(initialPrice(i)); states[i] = s }
        return s
    }
}

@Composable
fun ListView(modifier: Modifier, state: LazyListState, prices: PriceStore, counter: Counter) {
    LazyColumn(modifier, state = state) {
        // key stable (= index de la donnée) + contentType unique : réutilisation des compositions entre lignes
        items(ROWS, key = { it }, contentType = { 0 }) { i -> ItemRow(i, prices, counter) }
    }
}

private val titleStyle = TextStyle(color = Color.White, fontSize = 16.sp, fontWeight = FontWeight.Bold)
private val subStyle = TextStyle(color = Color(0xFFAAAAAA), fontSize = 13.sp)
private val priceStyle = TextStyle(color = Color(0xFF7CFC9A), fontSize = 15.sp, textAlign = TextAlign.End)
private val rowBg0 = Color(0xFF24242B)
private val rowBg1 = Color(0xFF2A2A32)

@Composable
fun ItemRow(i: Int, prices: PriceStore, counter: Counter) {
    val h = rowHeight(i)
    val f = fnv(i)
    val dot = Color(f and 255, (f shr 8) and 255, (f shr 16) and 255)
    val sub = remember(i) { "Quote $i — ${fmt2(initialPrice(i))}" }
    Row(
        Modifier.fillMaxWidth().height(h.dp)
            .drawBehind { counter.drawn++ }
            .background(if (i % 2 == 0) rowBg0 else rowBg1)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Box(Modifier.size(32.dp).background(dot, CircleShape))
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f)) {
            BasicText("Item #$i", style = titleStyle, maxLines = 1)
            BasicText(sub, style = subStyle, maxLines = 1, softWrap = false, overflow = TextOverflow.Clip)
        }
        PriceText(prices.state(i))
    }
}

/** Lecture différée de l'état du prix dans son propre scope de recomposition. */
@Composable
fun PriceText(price: DoubleState) {
    BasicText(fmtPrice(price.doubleValue), style = priceStyle, modifier = Modifier.width(110.dp), maxLines = 1)
}

class ScrollScript {
    fun velocity(t: Double): Double {
        if (t < 4.0) return 3000.0
        val u = t - 4.0
        val k = (u / 1.5).toInt()
        val tau = u - k * 1.5
        val dir = if (k % 2 == 0) 1.0 else -1.0
        return dir * 20000.0 * exp(-2.0 * tau)
    }
}

/**
 * Producteur : coroutine Dispatchers.Default, xorshift32 seed 0x2545F491, file bornée 100k (Channel capacité 100 000,
 * BufferOverflow.SUSPEND par défaut -> trySend échoue quand la file est pleine : la mise à jour est abandonnée et comptée).
 */
class UpdateStream(val rate: Int, scope: CoroutineScope) {
    val ch = Channel<Long>(100_000)
    val generated = AtomicLong(); val dropped = AtomicLong(); var applied = 0L
    @Volatile var enabled = false
    private var x = 0x2545F491
    val job = scope.launch(Dispatchers.Default) {
        var t0 = System.nanoTime(); var made = 0L
        while (isActive) {
            if (!enabled) { delay(5); t0 = System.nanoTime(); made = 0; continue }
            val due = (System.nanoTime() - t0) * rate / 1_000_000_000L - made
            for (k in 0 until due) {
                x = x xor (x shl 13); x = x xor (x ushr 17); x = x xor (x shl 5)
                val u = x.toLong() and 0xFFFFFFFFL
                generated.incrementAndGet()
                if (!ch.trySend(((u % ROWS) shl 32) or u).isSuccess) dropped.incrementAndGet()
            }
            made += due
            delay(1)
        }
    }
    fun reset() { enabled = false; while (ch.tryReceive().isSuccess) {}; generated.set(0); dropped.set(0); applied = 0; enabled = true }
    /** Application par lot sur le thread UI (à chaque frame) : écriture des snapshot states des lignes concernées. */
    fun applyAll(p: PriceStore) {
        while (true) {
            val r = ch.tryReceive().getOrNull() ?: break
            val idx = (r ushr 32).toInt(); val u = r and 0xFFFFFFFFL
            p.state(idx).doubleValue = u / 100.0
            applied++
        }
    }
}

class ListSc(val state: LazyListState, val prices: PriceStore, val counter: Counter, val stream: UpdateStream?, val input: InputState? = null) : Sc {
    val script = ScrollScript()
    var offset = 0.0
    override suspend fun reset() { state.scrollToItem(0); offset = 0.0; counter.drawn = 0; stream?.reset(); input?.keyEvents = 0 }
    override suspend fun step(t: Double, dt: Double) {
        stream?.applyAll(prices)
        val consumed = state.scrollPx((script.velocity(t) * dt).toFloat())
        offset += consumed
    }
    override fun finish(run: Run) {
        run.ops["sum_heights"] = SUM_H
        run.ops["final_offset"] = Math.round(offset)
        run.ops["rows_drawn"] = counter.drawn
        if (stream != null) {
            run.ops["rate"] = stream.rate
            run.ops["updates_generated"] = stream.generated.get(); run.ops["updates_applied"] = stream.applied; run.ops["updates_dropped"] = stream.dropped.get()
        }
        if (input != null) { run.ops["key_events"] = input.keyEvents; run.ops["focus_rerequests"] = input.refocus }
        run.notes = FRAME_NOTE + " LazyColumn (variable heights 48..128, key=index, single contentType), scripted scroll via " +
            "ScrollableState.scroll { scrollBy(v(t)*dt) } each frame; final_offset = sum of consumed px (clamped). rows_drawn = " +
            "executions of the row drawBehind block (Compose may reuse recorded row layers: not 'rows rasterized per frame')." +
            (if (input != null) " input: Window(onKeyEvent) toggles a mutableState read only in the square's drawBehind (draw-phase invalidation). Without a WM, an external XSetInputFocus on the toplevel (harness xdotool windowfocus) makes AWT/XToolkit drop keyboard focus (it keeps X focus on its FocusProxy child); the app then re-requests focus via Window.toFront() + requestFocus() on the window's mostRecentFocusOwner (the Compose component) (public AWT API), counted in focus_rerequests." else "")
    }
}

@Composable
fun ListScenario(run: Run, rate: Int, input: InputState? = null) {
    val state = rememberLazyListState()
    val prices = remember { PriceStore() }
    val counter = remember { Counter() }
    val scope = rememberCoroutineScope()
    val stream = remember { if (rate > 0) UpdateStream(rate, scope) else null }
    val sc = remember { ListSc(state, prices, counter, stream, input) }
    FrameLoop(run, sc)
    Shell { ListView(Modifier.fillMaxSize(), state, prices, counter) }
}
