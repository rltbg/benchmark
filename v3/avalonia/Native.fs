module Native
// Frontiere FFI vers le coeur C-ABI v2 (core/benchcore.h, Rust).
//  * DllImport blittable (types primitifs/pointeurs uniquement) + [<assembly: DisableRuntimeMarshalling>] : aucun stub de
//    marshalling (https://learn.microsoft.com/en-us/dotnet/standard/native-interop/disabled-marshalling).
//    LibraryImport (generateur de source C#) n'est pas utilisable en F#.
//  * NativeAOT : liaison statique de libbenchcore.a (DirectPInvoke + NativeLibrary dans le fsproj) -> appel direct, sans dlopen/dlsym.
//  * SuppressGCTransition sur les appels courts (noop/add/validate/parse/hash sur petits tampons) ; variantes avec transition GC
//    pour les appels longs (hash >= 64 Ko, fill, heavy) : la doc impose < ~1 us, sans I/O, sans callback, sans verrou.
//  * Rappels natif->manage : voir Callbacks (delegate blittable + Marshal.GetFunctionPointerForDelegate : F# ne sait pas produire
//    un pointeur [<UnmanagedCallersOnly>], cf. NOTES.md) ; file SPSC sans verrou drainee a chaque frame.

open System
open System.Reflection
open System.Runtime.CompilerServices
open System.Runtime.InteropServices
open System.Threading
open Util
open Avalonia.Threading

[<assembly: DisableRuntimeMarshalling>]
do ()

module Raw =
    [<DllImport("benchcore", EntryPoint = "core_noop"); SuppressGCTransition>]
    extern uint64 noop_s()
    [<DllImport("benchcore", EntryPoint = "core_add"); SuppressGCTransition>]
    extern uint64 add_s(uint64 a, uint64 b)
    [<DllImport("benchcore", EntryPoint = "core_validate"); SuppressGCTransition>]
    extern int validate_s(nativeint p, unativeint n)
    [<DllImport("benchcore", EntryPoint = "core_parse"); SuppressGCTransition>]
    extern int parse_s(nativeint p, unativeint n, nativeint out, unativeint cap, nativeint outLen)
    [<DllImport("benchcore", EntryPoint = "core_hash"); SuppressGCTransition>]
    extern uint32 hash_s(nativeint p, unativeint n)
    // variantes avec transition GC (comparaison, et obligatoires pour les appels longs)
    [<DllImport("benchcore", EntryPoint = "core_noop")>]
    extern uint64 noop_g()
    [<DllImport("benchcore", EntryPoint = "core_add")>]
    extern uint64 add_g(uint64 a, uint64 b)
    [<DllImport("benchcore", EntryPoint = "core_hash")>]
    extern uint32 hash_g(nativeint p, unativeint n)
    [<DllImport("benchcore", EntryPoint = "core_fill")>]
    extern void fill_g(nativeint out, unativeint n, uint32 seed)
    [<DllImport("benchcore", EntryPoint = "core_heavy")>]
    extern uint64 heavy(uint32 ms)
    [<DllImport("benchcore", EntryPoint = "core_producer_start")>]
    extern int producer_start(nativeint cb, nativeint user, uint64 rate, uint32 range)
    [<DllImport("benchcore", EntryPoint = "core_producer_stop")>]
    extern uint64 producer_stop()
    [<DllImport("benchcore", EntryPoint = "core_producer_produced")>]
    extern uint64 producer_produced()

let mutable linkNote = ""
/// Mode dev (JIT) uniquement : resolution dynamique de libbenchcore.so (BENCHCORE_LIB puis $BENCH_ROOT/core/target/release).
let installResolver (asm: Assembly) =
#if CORE_STATIC
    linkNote <- "libbenchcore.a liee statiquement (NativeAOT DirectPInvoke + NativeLibrary)"
#else
    NativeLibrary.SetDllImportResolver(asm, DllImportResolver(fun name _ _ ->
        if name = "benchcore" then
            let env = Environment.GetEnvironmentVariable "BENCHCORE_LIB"
            let p = if not (String.IsNullOrEmpty env) then env else IO.Path.Combine(benchRoot.Value, "core", "target", "release", "libbenchcore.so")
            linkNote <- "libbenchcore.so dynamique (" + p + ")"
            let mutable h = 0n
            if NativeLibrary.TryLoad(p, &h) then h else 0n
        else 0n))
#endif

// ---------------------------------------------------------------- file SPSC sans verrou (un producteur, un consommateur : le thread UI)
[<Struct; StructLayout(LayoutKind.Sequential)>]
type Upd =
    val mutable Kind: int      // 0 = prix ; 1 = insertion ; 2 = suppression ; 3 = deplacement
    val mutable Idx: int
    val mutable Aux: int
    val mutable Price: float

/// File circulaire bornee (capacite = puissance de 2). TryEnqueue ne bloque jamais : false si pleine (l'appelant compte le drop).
[<AllowNullLiteral>]
type SpscRing(capacity: int) =
    let cap = capacity
    let mask = int64 (capacity - 1)
    let buf : Upd[] = GC.AllocateUninitializedArray<Upd> capacity
    let mutable head = 0L   // ecrit par le producteur
    let mutable tail = 0L   // ecrit par le consommateur
    member _.Buf = buf
    member _.Capacity = cap
    member _.TryEnqueue(kind: int, idx: int, aux: int, price: float) : bool =
        let h = head
        if h - Volatile.Read(&tail) >= int64 cap then false
        else
            let r = &buf.[int (h &&& mask)]
            r.Kind <- kind
            r.Idx <- idx
            r.Aux <- aux
            r.Price <- price
            Volatile.Write(&head, h + 1L)
            true
    /// Consommateur : nombre d'elements disponibles et position de lecture
    member _.Available = int (Volatile.Read(&head) - tail)
    member _.Tail = tail
    member _.Mask = mask
    member _.Advance(n: int) = Volatile.Write(&tail, tail + int64 n)

// ---------------------------------------------------------------- rappels natif -> manage (F2)
[<UnmanagedFunctionPointer(CallingConvention.Cdecl)>]
type CoreCb = delegate of nativeint * uint32 * float -> unit

type CallbackSink() =
    static let mutable ring : SpscRing = null
    static let mutable received = 0L
    static let mutable dropped = 0L
    static let mutable pinned : CoreCb = null
    static let mutable postMode = false
    static let mutable postHandler : Action<int, float> = null
    static member SetPost(h: Action<int, float>) =
        postMode <- true
        postHandler <- h
    static member Ring = ring
    static member Received = Volatile.Read(&received)
    static member Dropped = Volatile.Read(&dropped)
    /// appele depuis le thread producteur natif : une ecriture dans la file SPSC, jamais de verrou ni d'allocation
    static member OnItem(_user: nativeint, index: uint32, price: float) : unit =
        if postMode then
            // variante de comparaison (hors recommandation) : un Dispatcher.Post par evenement
            let h = postHandler
            Dispatcher.UIThread.Post((fun () -> h.Invoke(int index, price)), DispatcherPriority.Default)
            received <- received + 1L
        elif ring.TryEnqueue(0, int index, 0, price) then received <- received + 1L   // seul le producteur ecrit received
        else Interlocked.Increment(&dropped) |> ignore
    static member Start(capacity: int, rate: uint64, range: uint32) : bool =
        ring <- SpscRing(capacity)
        received <- 0L
        dropped <- 0L
        pinned <- CoreCb(fun u i p -> CallbackSink.OnItem(u, i, p))
        let fp = Marshal.GetFunctionPointerForDelegate pinned
        Raw.producer_start (fp, 0n, rate, range) = 1
    static member Stop() : uint64 =
        let n = Raw.producer_stop ()
        GC.KeepAlive pinned
        n
