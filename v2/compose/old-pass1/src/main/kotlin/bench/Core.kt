package bench

import java.lang.foreign.*
import java.lang.invoke.MethodHandle

/** Pont C-ABI vers libbenchcore.so via java.lang.foreign (FFM, preview en JDK 21 -> --enable-preview). */
object Core {
    const val LIB = "/home/user/eval/core/target/release/libbenchcore.so"
    private val linker = Linker.nativeLinker()
    private val lookup = SymbolLookup.libraryLookup(LIB, Arena.global())
    private fun h(n: String, d: FunctionDescriptor): MethodHandle = linker.downcallHandle(lookup.find(n).get(), d)
    val validate: MethodHandle = h("core_validate", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG))
    val parse: MethodHandle = h("core_parse", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG, ValueLayout.ADDRESS))
    val hash: MethodHandle = h("core_hash", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG))
    val heavy: MethodHandle = h("core_heavy", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.JAVA_INT))

    /** Boucle serrée validate+parse+hash sur la trame "hello" (8 octets). Retourne le nombre de triplets. */
    fun tightLoop(stop: () -> Boolean, counter: java.util.concurrent.atomic.AtomicLong) {
        val arena = Arena.ofConfined()
        arena.use { ar ->
            val frame = ar.allocate(8)
            val bytes = byteArrayOf(0xA5.toByte(), 1, 5, 'h'.code.toByte(), 'e'.code.toByte(), 'l'.code.toByte(), 'l'.code.toByte(), 'o'.code.toByte())
            MemorySegment.copy(MemorySegment.ofArray(bytes), 0, frame, 0, 8)
            val out = ar.allocate(64)
            val outLen = ar.allocate(8)
            var local = 0L
            var sink = 0
            while (!stop()) {
                repeat(256) {
                    sink += validate.invoke(frame, 8L) as Int
                    sink += parse.invoke(frame, 8L, out, 64L, outLen) as Int
                    sink += hash.invoke(frame, 8L) as Int
                }
                local += 256
                counter.set(local)
            }
            if (sink == 42) println()
        }
    }
}
