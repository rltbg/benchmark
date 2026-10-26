package bench

import java.io.File
import java.lang.management.GarbageCollectorMXBean
import java.lang.management.ManagementFactory
import javax.management.NotificationEmitter
import javax.management.openmbean.CompositeData

fun fnv(i: Int): Int {
    var h = -2128831035 // 0x811C9DC5
    var v = i
    repeat(4) { h = (h xor (v and 255)) * 16777619; v = v ushr 8 }
    return h
}
fun fnvU(i: Int): Long = fnv(i).toLong() and 0xFFFFFFFFL
fun rowHeight(i: Int): Int = 48 + 16 * (fnvU(i) % 6).toInt()
fun initialPrice(i: Int): Double = (fnvU(i + 7) % 100000) / 100.0
const val ROWS = 100_000
val SUM_H: Long = (0 until ROWS).sumOf { rowHeight(it).toLong() }

/** Prix en centimes -> "123.45" (2 décimales, sans Locale). */
fun fmt2(v: Double): String {
    val c = Math.round(v * 100.0)
    val frac = (c % 100).toInt()
    return "" + (c / 100) + "." + (if (frac < 10) "0" else "") + frac
}
fun fmtPrice(v: Double): String = "$" + fmt2(v)

/** Versions exactes (vérifiées 2026-10-26 ; voir STACK-NOTES.md). Le JDK est lu à l'exécution. */
val VERSION: String
    get() = "Compose Multiplatform 1.12.2 (Jetpack Compose 1.12.1 base); Kotlin 2.4.21; Skiko 0.150.2; kotlinx-coroutines 1.11.0; " +
        "JVM ${System.getProperty("java.vm.name")} ${System.getProperty("java.runtime.version")}; GC ${gcNames()}"

private fun gcNames() = ManagementFactory.getGarbageCollectorMXBeans().joinToString("+") { it.name }

/** Racine du dépôt : BENCH_ROOT, sinon remontée depuis l'emplacement du code jusqu'à core/benchcore.h, sinon répertoire courant. */
val BENCH_ROOT: File by lazy {
    System.getenv("BENCH_ROOT")?.takeIf { it.isNotEmpty() }?.let { return@lazy File(it) }
    val src = runCatching { File(Args::class.java.protectionDomain.codeSource.location.toURI()) }.getOrNull()
    var d: File? = src?.absoluteFile
    while (d != null) { if (File(d, "core/benchcore.h").isFile) return@lazy d; d = d.parentFile }
    File(".").absoluteFile
}

class Args(val scenario: String, val duration: Double, val out: String, val rate: Int, val shapes: Int, val assets: String)

fun parseArgs(a: Array<String>): Args {
    val m = HashMap<String, String>()
    var i = 0
    while (i < a.size) { if (a[i].startsWith("--") && i + 1 < a.size) { m[a[i].substring(2)] = a[i + 1]; i += 2 } else i++ }
    val sc = m["scenario"] ?: "idle"
    return Args(
        sc, (m["duration"] ?: (if (sc == "stress") "600" else "10")).toDouble(), m["out"] ?: "out.json",
        (m["rate"] ?: (if (sc == "stress") "5000" else "1000")).toInt(), (m["shapes"] ?: "5000").toInt(),
        m["assets"] ?: File(BENCH_ROOT, "assets").path
    )
}

class DList { var a = DoubleArray(1 shl 16); var n = 0
    fun add(v: Double) { if (n == a.size) a = a.copyOf(n * 2); a[n++] = v } }

/** Fragment JSON brut (objets/tableaux imbriqués dans ops). */
class RawJson(val s: String) { override fun toString() = s }

fun jsonOf(v: Any?): String = when (v) {
    null -> "null"
    is RawJson -> v.s
    is Double -> if (v.isFinite()) String.format(java.util.Locale.ROOT, "%.3f", v) else "null"
    is Number, is Boolean -> v.toString()
    is String -> "\"" + v.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", " ") + "\""
    is Map<*, *> -> v.entries.joinToString(",", "{", "}") { jsonOf(it.key.toString()) + ":" + jsonOf(it.value) }
    is Iterable<*> -> v.joinToString(",", "[", "]") { jsonOf(it) }
    else -> jsonOf(v.toString())
}

/**
 * ops.gc via java.lang.management.GarbageCollectorMXBean (getCollectionCount / getCollectionTime, différence entre les
 * marqueurs) + durée de chaque collecte via les notifications com.sun.management.GarbageCollectionNotificationInfo
 * (GcInfo.duration) pour le max. Avec G1 (défaut), « G1 Young/Old Generation » et « G1 Concurrent GC » (pauses Remark/Cleanup)
 * sont des pauses STW ; les phases concurrentes ne sont pas comptées par ces beans.
 */
class GcProbe {
    private val beans: List<GarbageCollectorMXBean> = ManagementFactory.getGarbageCollectorMXBeans()
    private var c0 = LongArray(beans.size); private var t0 = LongArray(beans.size)
    @Volatile private var maxMs = 0L
    @Volatile private var on = false
    init {
        for (b in beans) (b as? NotificationEmitter)?.addNotificationListener({ n, _ ->
            if (on && n.type == "com.sun.management.gc.notification") {
                val ud = n.userData as? CompositeData ?: return@addNotificationListener
                val info = ud.get("gcInfo") as? CompositeData ?: return@addNotificationListener
                val d = (info.get("duration") as? Long) ?: 0L
                if (d > maxMs) maxMs = d
            }
        }, null, null)
    }
    fun start() { for ((k, b) in beans.withIndex()) { c0[k] = b.collectionCount; t0[k] = b.collectionTime }; maxMs = 0; on = true }
    fun result(): Map<String, Any?> {
        on = false
        var n = 0L; var t = 0L
        val per = LinkedHashMap<String, Any?>()
        for ((k, b) in beans.withIndex()) {
            val dn = b.collectionCount - c0[k]; val dt = b.collectionTime - t0[k]
            n += dn; t += dt; per[b.name] = mapOf("collections" to dn, "time_ms" to dt)
        }
        return linkedMapOf("collections" to n, "pause_ms_total" to t, "pause_ms_max" to maxMs, "per_collector" to per,
            "source" to "GarbageCollectorMXBean count/time deltas between markers; max = max GcInfo.duration from GC notifications (ms resolution)")
    }
}

class Run(val args: Args) {
    val frames = DList()
    val ops = LinkedHashMap<String, Any?>()
    var notes = ""
    var unsupported: String? = null
    val gc = GcProbe()
    var measureStartNs = 0L

    fun markStart() {
        gc.start()
        measureStartNs = System.nanoTime()
        println("BENCH:measure_start"); System.out.flush()
    }
    fun markEnd() {
        println("BENCH:measure_end"); System.out.flush()
        ops["gc"] = gc.result()
    }

    fun json(): String {
        val sb = StringBuilder(frames.n * 8 + 2048)
        sb.append("{\"stack\":\"compose\",\"version\":").append(jsonOf(VERSION))
        sb.append(",\"scenario\":\"").append(args.scenario).append("\",\"duration_s\":").append(args.duration)
        sb.append(",\"frames\":").append(frames.n).append(",\"frame_ms\":[")
        for (i in 0 until frames.n) { if (i > 0) sb.append(','); sb.append(String.format(java.util.Locale.ROOT, "%.3f", frames.a[i])) }
        sb.append("],\"ops\":").append(jsonOf(ops))
        // checksum : FNV-1a des ops déterministes (sum_heights, compteurs de contenu), pas des durées
        var h = -2128831035
        for (c in "${ops["sum_heights"]}|${ops["images_decoded"]}|${ops["lines"]}|${ops["shapes"]}") h = (h xor c.code) * 16777619
        sb.append(",\"checksum\":\"").append(Integer.toHexString(h)).append('"')
        sb.append(",\"notes\":").append(jsonOf(notes))
        sb.append(",\"unsupported\":").append(jsonOf(unsupported)).append('}')
        return sb.toString()
    }
    fun write() { File(args.out).writeText(json()) }
}

/** Défilement programmatique : retourne les px réellement consommés (clamp aux bornes). */
suspend fun androidx.compose.foundation.gestures.ScrollableState.scrollPx(d: Float): Float {
    var c = 0f
    scroll { c = scrollBy(d) }
    return c
}
