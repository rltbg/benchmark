package bench

import java.io.File

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

fun fmtPrice(v: Double): String {
    val c = Math.round(v * 100.0)
    val frac = (c % 100).toInt()
    return "$" + (c / 100) + "." + (if (frac < 10) "0" else "") + frac
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
        m["assets"] ?: "/home/user/eval/assets"
    )
}

class DList { var a = DoubleArray(1 shl 16); var n = 0
    fun add(v: Double) { if (n == a.size) a = a.copyOf(n * 2); a[n++] = v } }

class Run(val args: Args) {
    val procStartNs = System.nanoTime()
    val frames = DList()
    val ops = LinkedHashMap<String, Any?>()
    var notes = ""
    var unsupported: String? = null
    val heap = ArrayList<String>()

    fun json(): String {
        val sb = StringBuilder(frames.n * 8 + 1024)
        sb.append("{\"stack\":\"compose\",\"scenario\":\"").append(args.scenario).append("\",\"duration_s\":").append(args.duration)
        sb.append(",\"frames\":").append(frames.n).append(",\"frame_ms\":[")
        for (i in 0 until frames.n) { if (i > 0) sb.append(','); sb.append(String.format(java.util.Locale.ROOT, "%.3f", frames.a[i])) }
        sb.append("],\"ops\":{")
        var first = true
        for ((k, v) in ops) {
            if (!first) sb.append(','); first = false
            sb.append('"').append(k).append("\":")
            when (v) { is Number, is Boolean -> sb.append(v); is String -> sb.append(quote(v)); null -> sb.append("null"); else -> sb.append(v.toString()) }
        }
        var h = -2128831035
        for (c in ops.toString()) h = (h xor c.code) * 16777619
        sb.append("},\"checksum\":\"").append(Integer.toHexString(h)).append("\",")
        if (unsupported != null) sb.append("\"unsupported\":").append(quote(unsupported!!)).append(',')
        sb.append("\"notes\":").append(quote(notes)).append('}')
        return sb.toString()
    }
    private fun quote(s: String) = "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", " ") + "\""
    fun write() { File(args.out).writeText(json()) }
}

/** Défilement programmatique : retourne les px réellement consommés (clamp aux bornes). */
suspend fun androidx.compose.foundation.gestures.ScrollableState.scrollPx(d: Float): Float {
    var c = 0f
    scroll { c = scrollBy(d) }
    return c
}
