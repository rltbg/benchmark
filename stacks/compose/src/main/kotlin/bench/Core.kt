package bench

import java.io.File
import java.lang.foreign.*
import java.lang.invoke.MethodHandle
import java.util.concurrent.atomic.AtomicLong

/**
 * Pont C-ABI vers libbenchcore.so via java.lang.foreign (FFM, API finale depuis JDK 22 ; JDK 25 ici).
 * Chemin : BENCHCORE_LIB, sinon $BENCH_ROOT/core/target/release/libbenchcore.so.
 * Les 3 fonctions de la boucle serrée sont liées avec Linker.Option.critical(false) (appels courts, sans upcall ni
 * blocage : transition de thread allégée, cf. javadoc Linker.Option.critical) ; core_heavy (100 ms) en downcall normal.
 */
object Core {
    val LIB: String = System.getenv("BENCHCORE_LIB")?.takeIf { it.isNotEmpty() }
        ?: File(BENCH_ROOT, "core/target/release/libbenchcore.so").path
    private val linker = Linker.nativeLinker()
    private val lookup = SymbolLookup.libraryLookup(java.nio.file.Path.of(LIB), Arena.global())
    private val critical = Linker.Option.critical(false)
    private fun h(n: String, d: FunctionDescriptor, vararg o: Linker.Option): MethodHandle =
        linker.downcallHandle(lookup.find(n).orElseThrow(), d, *o)
    val validate: MethodHandle = h("core_validate", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG), critical)
    val parse: MethodHandle = h("core_parse", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS), critical)
    val hash: MethodHandle = h("core_hash", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG), critical)
    val heavy: MethodHandle = h("core_heavy", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT))

    /** Résultat accumulé publié (empêche toute élimination de la boucle). */
    val published = AtomicLong()

    /** Boucle serrée validate+parse+hash sur la trame A5 01 05 "hello" (8 octets). counter = nombre de triplets. */
    fun tightLoop(stop: () -> Boolean, counter: AtomicLong) {
        Arena.ofConfined().use { ar ->
            val bytes = byteArrayOf(0xA5.toByte(), 1, 5, 'h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(), 'l'.code.toByte(), 'o'.code.toByte())
            val frame = ar.allocate(8)
            MemorySegment.copy(MemorySegment.ofArray(bytes), 0, frame, 0, 8)
            val out = ar.allocate(64)
            val outLen = ar.allocate(8)
            var local = 0L
            var sink = 0L
            while (!stop()) {
                repeat(256) {
                    sink += validate.invoke(frame, 8L) as Int
                    sink += parse.invoke(frame, 8L, out, 64L, outLen) as Int
                    sink += (hash.invoke(frame, 8L) as Int).toLong() and 0xFFFFFFFFL
                }
                local += 256
                counter.set(local)
                published.set(sink)
            }
        }
    }
}
