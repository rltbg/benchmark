module Ffi
// F1 ffi-calls : appels du coeur depuis 1 ou 4 threads de fond pendant que l'UI anime 1000 formes solid

open System
open System.Runtime.InteropServices
open System.Text
open System.Threading
open Avalonia.Controls
open Util
open Harness
open Gfx

type FfiCallsScn(args: Args) =
    inherit Scenario()
    let call = args.Variant("call", "noop")
    let threads = int (args.Variant("threads", "1"))
    let size = if args.NGiven then int args.N else 8
    let forceGct = args.Variant("gct", "0") = "1"
    let model = ShapesModel(1000, 1280.0f, 720.0f)
    let render = ShapesRender(model, { Gradient = false; Shadow = false; Clip = false; ClipCornerOpt = false }, 1000.0f, 600.0f, false)
    let view = ShapesView(model, render)
    // tampon cree une fois (Pinned Object Heap : adresse stable, sans GCHandle)
    let bufs = Array.init threads (fun _ -> GC.AllocateArray<byte>(max 1 size, true))
    let calls : int64[] = Array.zeroCreate (threads * 16)   // 16 = ecart anti faux-partage (128 octets)
    let accs : uint64[] = Array.zeroCreate (threads * 16)
    let mutable stop = false
    let mutable baseCalls = 0L
    let sw = Diagnostics.Stopwatch()
    let mutable sq : Square = null
    let suppress = not forceGct && (call = "noop" || call = "add" || (call = "hash" && size <= 1024))
    do model.Reset()
    member private this.Worker(k: int) =
        let buf = Marshal.UnsafeAddrOfPinnedArrayElement(bufs.[k], 0)
        let n = unativeint size
        let mutable local = 0L
        let mutable a = 0UL
        while not (Volatile.Read(&stop)) do
            // lots de 1000 appels ; compteur publie par lot
            match call with
            | "noop" ->
                if suppress then (for _ in 1 .. 1000 do a <- a + Native.Raw.noop_s ())
                else (for _ in 1 .. 1000 do a <- a + Native.Raw.noop_g ())
            | "add" ->
                if suppress then (for i in 1 .. 1000 do a <- Native.Raw.add_s (a, uint64 i))
                else (for i in 1 .. 1000 do a <- Native.Raw.add_g (a, uint64 i))
            | "hash" ->
                let reps = if size >= 65536 then 10 else 1000
                if suppress then (for _ in 1 .. reps do a <- a + uint64 (Native.Raw.hash_s (buf, n)))
                else (for _ in 1 .. reps do a <- a + uint64 (Native.Raw.hash_g (buf, n)))
            | _ ->
                let reps = if size >= 65536 then 10 else 1000
                for i in 1 .. reps do Native.Raw.fill_g (buf, n, uint32 i)
            local <- local + (if (call = "hash" || call = "fill") && size >= 65536 then 10L else 1000L)
            Volatile.Write(&calls.[k * 16], local)
            Volatile.Write(&accs.[k * 16], a)
    override _.Content = view :> Control
    override _.SetSquare(s) = sq <- s
    override this.Begin() =
        // tampon rempli cote natif une fois pour hash
        for k in 0 .. threads - 1 do
            Native.Raw.fill_g (Marshal.UnsafeAddrOfPinnedArrayElement(bufs.[k], 0), unativeint size, 1u)
            let th = Thread((fun () -> this.Worker k), IsBackground = true, Name = "ffi-worker-" + k.ToString(inv))
            th.Start()
        baseCalls <- 0L
        sw.Restart()
    override _.OnFrame(t, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Finish() = Volatile.Write(&stop, true)
    override _.Ops sb =
        let mutable total = 0L
        let mutable acc = 0UL
        for k in 0 .. threads - 1 do
            total <- total + Volatile.Read(&calls.[k * 16])
            acc <- acc + accs.[k * 16]
        let el = sw.Elapsed.TotalSeconds
        kvS sb "call" call
        kvI sb "threads" (int64 threads)
        kvI sb "buffer_bytes" (int64 size)
        kvI sb "calls" total
        kvF sb "calls_per_s" (float total / el)
        kvF sb "mb_per_s" (if call = "hash" || call = "fill" then float total * float size / el / 1048576.0 else 0.0)
        kvS sb "suppress_gc_transition" (if suppress then "true" else "false")
        kvU sb "acc" acc
        checksumValue <- fnvSeq (int64 (min size 65536)) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "F1: " + threads.ToString(inv) + " thread(s) de fond appellent core_" + call + " en boucle (DllImport blittable, DisableRuntimeMarshalling, "
        + (if suppress then "SuppressGCTransition" else "transition GC normale") + ", " + Native.linkNote + ") ; tampon de " + size.ToString(inv)
        + " octets cree une fois sur le Pinned Object Heap ; UI : 1000 formes solid animees."
