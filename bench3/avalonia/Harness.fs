module Harness
// Pilote de frames, marqueurs, mesures (frames, GC, arret UI) et JSON de sortie.

open System
open System.Diagnostics
open System.IO
open System.Reflection
open System.Runtime.CompilerServices
open System.Runtime.InteropServices
open System.Text
open System.Threading
open System.Threading.Tasks
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Input
open Avalonia.Media
open Avalonia.Rendering.Composition
open Avalonia.Threading
open Util

let isAot = not RuntimeFeature.IsDynamicCodeSupported
let PKG_VERSIONS =
    "Avalonia 12.1.4 (X11/Skia/HarfBuzz/Themes.Simple 12.1.4), Avalonia.AvaloniaEdit 12.0.0, SkiaSharp 3.119.4, HarfBuzzSharp 8.3.1.3, FSharp.Core 10.1.403, .NET SDK 10.0.403"
let versionString () =
    let avInfo =
        try
            let a = typeof<Application>.Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()
            if isNull a then "?" else a.InformationalVersion.Split('+').[0]
        with _ -> "?"
    PKG_VERSIONS + ", core rust (core/ libbenchcore C-ABI v2), runtime " + RuntimeInformation.FrameworkDescription
    + (if isAot then ", NativeAOT" else ", JIT (dev)") + ", Avalonia assembly " + avInfo

let printMarker (s: string) =
    Console.Out.WriteLine s
    Console.Out.Flush()

// ---------------------------------------------------------------- GC (ops.gc)
type GcTracker() =
    let mutable c0 = 0
    let mutable c1 = 0
    let mutable c2 = 0
    let mutable pause0 = TimeSpan.Zero
    let mutable alloc0 = 0L
    let mutable lastIdx = 0L
    let mutable maxPause = 0.0
    member _.Start() =
        c0 <- GC.CollectionCount 0
        c1 <- GC.CollectionCount 1
        c2 <- GC.CollectionCount 2
        pause0 <- GC.GetTotalPauseDuration()
        alloc0 <- GC.GetTotalAllocatedBytes false
        lastIdx <- GC.GetGCMemoryInfo(GCKind.Any).Index
        maxPause <- 0.0
    /// relevee de la pause du dernier GC (appelee a chaque tick du minuteur d'arret UI, 4 ms) : borne inferieure de la pause max
    member _.Sample() =
        let gi = GC.GetGCMemoryInfo(GCKind.Any)
        if gi.Index <> lastIdx then
            lastIdx <- gi.Index
            let pd = gi.PauseDurations
            for k in 0 .. pd.Length - 1 do
                let ms = pd.[k].TotalMilliseconds
                if ms > maxPause then maxPause <- ms
    member this.Json() =
        this.Sample()
        let d0 = GC.CollectionCount 0 - c0
        let d1 = GC.CollectionCount 1 - c1
        let d2 = GC.CollectionCount 2 - c2
        // GC.CollectionCount(n) compte les collections de generation >= n : gen0 inclut gen1 et gen2
        "{\"collections\":" + d0.ToString(inv) + ",\"gen0\":" + d0.ToString(inv) + ",\"gen1\":" + d1.ToString(inv) + ",\"gen2\":" + d2.ToString(inv)
        + ",\"gen0_only\":" + (d0 - d1).ToString(inv) + ",\"gen1_only\":" + (d1 - d2).ToString(inv)
        + ",\"pause_ms_total\":" + (GC.GetTotalPauseDuration() - pause0).TotalMilliseconds.ToString("F3", inv)
        + ",\"pause_ms_max\":" + maxPause.ToString("F3", inv)
        + ",\"alloc_bytes\":" + (GC.GetTotalAllocatedBytes false - alloc0).ToString(inv)
        + ",\"heap_bytes\":" + GC.GetTotalMemory(false).ToString(inv)
        + ",\"server_gc\":" + (if Runtime.GCSettings.IsServerGC then "true" else "false")
        + ",\"latency_mode\":\"" + Runtime.GCSettings.LatencyMode.ToString() + "\"}"

// ---------------------------------------------------------------- statistiques du thread de rendu
module Stats =
    let mutable renderTicks = 0L
    let mutable renderOps = 0L
    let addRender (ticks: int64) =
        Interlocked.Add(&renderTicks, ticks) |> ignore
        Interlocked.Increment(&renderOps) |> ignore
    let reset () =
        Volatile.Write(&renderTicks, 0L)
        Volatile.Write(&renderOps, 0L)

/// positionne par tout changement de contenu hors animation continue (touche, molette) : la frame suivante est comptee
let mutable globalDirty = false

// ---------------------------------------------------------------- carre de latence (200x200 en (100,100), noir/blanc)
[<AllowNullLiteral>]
type Square() =
    let ctl =
        Border(Width = 200.0, Height = 200.0, Background = Brushes.Black, IsHitTestVisible = false,
               HorizontalAlignment = Layout.HorizontalAlignment.Left, VerticalAlignment = Layout.VerticalAlignment.Top,
               Margin = Thickness(100.0, 100.0, 0.0, 0.0))
    let mutable white = false
    let mutable keys = 0L
    member _.Control = ctl
    member _.Keys = keys
    member _.Toggle() =
        keys <- keys + 1L
        globalDirty <- true
        white <- not white
        ctl.Background <- (if white then Brushes.White else Brushes.Black)

// ---------------------------------------------------------------- scenario
[<AbstractClass>]
type Scenario() =
    /// contenu (sans le carre de latence)
    abstract Content : Control
    /// t < 0 : warm-up (t + 2 = temps ecoule) ; t >= 0 : temps mesure ; dt en secondes (UI thread, une fois par frame)
    abstract OnFrame : t: float * dt: float -> unit
    /// debut de la fenetre mesuree (apres le warm-up)
    abstract Begin : unit -> unit
    abstract Finish : unit -> unit
    abstract Ops : StringBuilder -> unit
    abstract Notes : string
    /// le contenu de la frame courante a-t-il change ? (compte comme frame mesuree)
    abstract Animating : bool
    /// le scenario bascule lui-meme le carre (T3) au lieu de la touche Espace
    abstract SquareOnSpace : bool
    abstract Unsupported : string
    /// frame_ms = durees des frames apres redimensionnement (U1)
    abstract FrameFromResize : bool
    abstract SetSquare : Square -> unit
    abstract OnWindow : Window -> unit
    default _.OnFrame(_, _) = ()
    default _.Begin() = ()
    default _.Finish() = ()
    default _.Notes = ""
    default _.Animating = true
    default _.SquareOnSpace = true
    default _.Unsupported = null
    default _.FrameFromResize = false
    default _.SetSquare(_) = ()
    default _.OnWindow(_) = ()

/// checksum_data (spec : empreinte du jeu de donnees) : FNV-1a 32 sur une sequence d'entiers 32 bits little-endian
let fnvSeq (n: int64) (f: int64 -> uint32) =
    let mutable h = 2166136261u
    let mutable i = 0L
    while i < n do
        let v = f i
        h <- (h ^^^ (v &&& 0xFFu)) * 16777619u
        h <- (h ^^^ ((v >>> 8) &&& 0xFFu)) * 16777619u
        h <- (h ^^^ ((v >>> 16) &&& 0xFFu)) * 16777619u
        h <- (h ^^^ ((v >>> 24) &&& 0xFFu)) * 16777619u
        i <- i + 1L
    h.ToString("x8")

/// checksum de premier niveau du JSON (fixe par le scenario)
let mutable checksumValue = "0"

// ---------------------------------------------------------------- driver de frames
let FRAME_NOTE =
    "frame_ms = intervalle entre fins de rendu consecutives du thread de rendu Avalonia : CompositionBatch.Rendered (API publique, "
    + "Compositor.RequestCompositionBatchCommitAsync, horodatee de facon synchrone sur le thread de rendu apres le rendu+present de toutes les cibles), "
    + "1 frame par batch dont le contenu a change. Cadence : X11 cree SleepLoopRenderTimer(DesiredFps = max(60, XRandR)) ([PrivateApi], aucune option publique : "
    + "ops.pacing = 60-only, BENCH_PACING=uncapped non honore)."

type Driver(top: Window, sc: Scenario, args: Args, notes: string, mainStart: int64) =
    let frames = ResizeArray<float32>(1 <<< 16)
    let resizeMs = ResizeArray<float>()
    let gate = obj ()
    let gc = GcTracker()
    let mutable measuring = false
    let mutable finished = false
    let mutable finishing = false
    let mutable startTs = 0L
    let mutable lastRaf = 0L
    let mutable lastRendered = 0L
    let mutable warmStart = 0L
    let mutable firstRendered = false
    let mutable setupMs = -1.0
    let mutable cb : Action<TimeSpan> = null
    let freq = float Stopwatch.Frequency
    let mutable compositor : Compositor = null
    // arret UI : minuteur de 4 ms sur le thread UI (DispatcherPriority.Input)
    let mutable stallTimer : DispatcherTimer = null
    let mutable lastTick = 0L
    let mutable stallMax = 0.0
    let mutable stallN = 0L
    let mutable stall50 = 0L
    let stallHist = Array.zeroCreate<int64> 8   // <8, <16, <33, <50, <100, <250, <1000, >=1000 ms
    // redimensionnements en attente (U1)
    let pendingResize = Collections.Generic.List<int64>()
    let mutable resizes = 0L
    let mutable measureStartMs = 0.0
    /// fin de rendu d'un batch : appele sur le thread de rendu (continuation synchrone)
    let onRendered (count: bool) =
        let now = Stopwatch.GetTimestamp()
        lock gate (fun () ->
            if not firstRendered then
                firstRendered <- true
                setupMs <- float (now - mainStart) * 1000.0 / freq
            // plusieurs batches rendus dans la meme passe = une seule frame
            if now - lastRendered > int64 (freq * 0.0001) then
                if count && measuring && not finished && lastRendered > 0L then
                    frames.Add(float32 (float (now - lastRendered) * 1000.0 / freq))
                lastRendered <- now)
    let renderedAction0 = Action<Task>(fun _ -> onRendered false)
    let renderedAction1 = Action<Task>(fun _ -> onRendered true)
    member val OnFirstRendered : Action<float> = null with get, set
    member _.SetupMs = setupMs
    member this.WriteJson() =
        let sb = StringBuilder(1 <<< 20)
        sb.Append("{\"stack\":\"avalonia-").Append(if isAot then "aot" else "jit").Append("\",\"version\":\"").Append(versionString().Replace("\"", "'"))
          .Append("\",\"scenario\":\"").Append(args.Scenario).Append("\",\"duration_s\":").Append(args.Duration.ToString("F1", inv)) |> ignore
        let fr = lock gate (fun () -> frames.ToArray())
        let fr = if sc.FrameFromResize then (resizeMs |> Seq.map float32 |> Array.ofSeq) else fr
        sb.Append(",\"frames\":").Append(fr.Length).Append(",\"frame_ms\":[") |> ignore
        for i in 0 .. fr.Length - 1 do
            if i > 0 then sb.Append(',') |> ignore
            sb.Append(fr.[i].ToString("F3", inv)) |> ignore
        sb.Append("],\"ops\":{") |> ignore
        kvI sb "n" args.N
        let vs = StringBuilder()
        for kv in args.Variants do
            if vs.Length > 0 then vs.Append(',') |> ignore
            vs.Append(kv.Key).Append('=').Append(kv.Value) |> ignore
        kvS sb "variant" (vs.ToString())
        kvS sb "pacing" "60-only"
        sc.Ops sb
        if resizeMs.Count > 0 then kvFArr sb "resize_frame_ms" resizeMs
        let mutable mx = 0.0f
        for f in fr do if f > mx then mx <- f
        kvF sb "max_frame_stall_ms" (float mx)
        kvF sb "ui_stall_ms_max" stallMax
        kvI sb "ui_stall_ticks" stallN
        kvI sb "ui_stall_gt50ms" stall50
        kvRaw sb "ui_stall_hist_lt8_16_33_50_100_250_1000_ge1000" ("[" + String.Join(",", stallHist) + "]")
        kvF sb "setup_ms" setupMs
        kvRaw sb "gc" (gc.Json())
        let ro = Volatile.Read(&Stats.renderOps)
        kvI sb "render_ops" ro
        kvF sb "render_op_ms_mean" (if ro > 0L then float (Volatile.Read(&Stats.renderTicks)) * 1000.0 / freq / float ro else 0.0)
        sb.Length <- sb.Length - 1
        sb.Append("},\"checksum\":\"").Append(checksumValue).Append("\",\"notes\":\"")
          .Append((notes + " " + sc.Notes).Replace("\\", "/").Replace("\"", "'").Replace("\n", " "))
          .Append("\",\"unsupported\":").Append(if isNull sc.Unsupported then "null" else "\"" + sc.Unsupported.Replace("\"", "'") + "\"").Append("}") |> ignore
        File.WriteAllText(args.Out, sb.ToString())
    member this.Finish() =
        if not finished then
            lock gate (fun () -> finished <- true)
            printMarker "BENCH:measure_end"
            if not (isNull stallTimer) then stallTimer.Stop()
            sc.Finish()
            this.WriteJson()
            match Application.Current.ApplicationLifetime with
            | :? IClassicDesktopStyleApplicationLifetime as d -> d.Shutdown 0
            | _ -> exit 0
    member private this.Track() =
        // le batch qui portera les changements faits dans ce callback
        let b = compositor.RequestCompositionBatchCommitAsync()
        let changed = sc.Animating || globalDirty
        globalDirty <- false
        b.Rendered.ContinueWith((if changed then renderedAction1 else renderedAction0), CancellationToken.None,
                                TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default) |> ignore
    member private this.StartStall() =
        lastTick <- Stopwatch.GetTimestamp()
        stallTimer <- DispatcherTimer(TimeSpan.FromMilliseconds 4.0, DispatcherPriority.Input, EventHandler(fun _ _ ->
            let now = Stopwatch.GetTimestamp()
            let ms = float (now - lastTick) * 1000.0 / freq
            lastTick <- now
            if measuring then
                stallN <- stallN + 1L
                if ms > stallMax then stallMax <- ms
                if ms > 50.0 then stall50 <- stall50 + 1L
                let b = if ms < 8.0 then 0 elif ms < 16.0 then 1 elif ms < 33.0 then 2 elif ms < 50.0 then 3 elif ms < 100.0 then 4 elif ms < 250.0 then 5 elif ms < 1000.0 then 6 else 7
                stallHist.[b] <- stallHist.[b] + 1L
                gc.Sample()))
        stallTimer.Start()
    /// redimensionnement : duree entre l'evenement Resized et la fin du rendu du batch qui porte la nouvelle mise en page
    member private this.HookResize() =
        top.Resized.Add(fun _ -> if measuring then pendingResize.Add(Stopwatch.GetTimestamp()))
        top.LayoutUpdated.Add(fun _ ->
            if pendingResize.Count > 0 then
                let stamps = pendingResize.ToArray()
                pendingResize.Clear()
                let b = compositor.RequestCompositionBatchCommitAsync()
                b.Rendered.ContinueWith((fun (_: Task) ->
                    let now = Stopwatch.GetTimestamp()
                    lock gate (fun () ->
                        if measuring && not finished then
                            for s in stamps do
                                resizeMs.Add(float (now - s) * 1000.0 / freq)
                                resizes <- resizes + 1L)), CancellationToken.None, TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default) |> ignore)
    member _.ResizeMs = resizeMs
    member _.Resizes = resizes
    member this.Start() =
        compositor <- ElementComposition.GetElementVisual(top).Compositor
        warmStart <- Stopwatch.GetTimestamp()
        lastRaf <- warmStart
        this.HookResize()
        this.StartStall()
        cb <- Action<TimeSpan>(fun _ ->
            let now = Stopwatch.GetTimestamp()
            if not measuring then
                let tw = float (now - warmStart) / freq
                if tw >= 2.0 then
                    sc.Begin()
                    Stats.reset ()
                    gc.Start()
                    startTs <- now
                    lastRaf <- now
                    stallMax <- 0.0
                    lastTick <- now
                    lock gate (fun () -> measuring <- true)
                    printMarker "BENCH:measure_start"
                    sc.OnFrame(0.0, 0.016)
                else
                    sc.OnFrame(tw - 2.0, float (now - lastRaf) / freq)
                    lastRaf <- now
                this.Track()
                top.RequestAnimationFrame cb
            else
                let t = float (now - startTs) / freq
                let dt = float (now - lastRaf) / freq
                lastRaf <- now
                if t >= args.Duration then
                    // attendre la fin de rendu des batches deja commites puis terminer
                    if not finishing then
                        finishing <- true
                        let b = compositor.RequestCompositionBatchCommitAsync()
                        b.Rendered.ContinueWith((fun (_: Task) -> Dispatcher.UIThread.Post(fun () -> this.Finish())), CancellationToken.None,
                                                TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default) |> ignore
                else
                    sc.OnFrame(t, dt)
                    this.Track()
                    top.RequestAnimationFrame cb)
        top.RequestAnimationFrame cb
