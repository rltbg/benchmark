module ListScenarios
// L1 list-scale, L2 list-rich, L3 list-mutate, L4 list-reset, F2 ffi-callbacks, F3 ffi-block, I1 latency-load, I2 latency-wheel
// (tous bases sur la meme liste virtualisee ; voie choisie par --variant impl=canvas|panel|vsp)

open System
open System.Collections.Generic
open System.Diagnostics
open System.Text
open System.Threading
open System.Threading.Tasks
open Avalonia
open Avalonia.Controls
open Avalonia.Input
open Avalonia.Media
open Avalonia.Threading
open Util
open Harness
open ListsCore

// ---------------------------------------------------------------- scripts de defilement
/// vitesse constante v1 pendant tConst, puis nF flings alternes (bas, haut, ...) de vitesse initiale v0, v(t)=v0*exp(-2t), duree T
let scrollScript (v1: float) (tConst: float) (v0: float) (nF: int) (T: float) (t: float) (maxOff: float) =
    let t = if t < 0.0 then t + 2.0 else t
    if t < tConst then clamp 0.0 maxOff (v1 * t)
    else
        let mutable b = clamp 0.0 maxOff (v1 * tConst)
        let mutable tt = t - tConst
        let mutable k = 0
        let mutable res = nan
        let half = v0 / 2.0
        while Double.IsNaN res do
            let dir = if k % 2 = 0 then 1.0 else -1.0
            if tt < T || k >= nF then res <- clamp 0.0 maxOff (b + dir * half * (1.0 - Math.Exp(-2.0 * min tt T)))
            else
                b <- clamp 0.0 maxOff (b + dir * half * (1.0 - Math.Exp(-2.0 * T)))
                tt <- tt - T
                k <- k + 1
        res

// ---------------------------------------------------------------- coquille
let mkShell (title: string) (center: Control) (statusText: string) =
    let head =
        Border(Height = 32.0, Background = SolidColorBrush(Color.Parse "#2B3A55"),
               Child = TextBlock(Text = title, Foreground = Brushes.White, FontSize = 14.0, VerticalAlignment = Layout.VerticalAlignment.Center, Margin = Thickness(12.0, 0.0)))
    let st = TextBlock(Text = statusText, FontSize = 12.0, VerticalAlignment = Layout.VerticalAlignment.Center, Margin = Thickness(12.0, 0.0))
    let stB = Border(Height = 24.0, Background = SolidColorBrush(Color.Parse "#DDE1EA"), Child = st)
    let dp = DockPanel(LastChildFill = true)
    DockPanel.SetDock(head, Dock.Top)
    DockPanel.SetDock(stB, Dock.Bottom)
    dp.Children.Add head
    dp.Children.Add stB
    dp.Children.Add center
    st, dp

// ---------------------------------------------------------------- producteur de fond pour L3 (valeurs + operations structurelles)
[<AllowNullLiteral>]
type MutateProducer(valueRate: int64, structRate: int64, ring: Native.SpscRing) =
    let mutable stop = false
    let mutable generated = 0L
    let mutable dropped = 0L
    let mutable thread : Thread = null
    member _.Generated = Volatile.Read(&generated)
    member _.Dropped = Volatile.Read(&dropped)
    member _.Start() =
        thread <- Thread((fun () ->
            let sw = Stopwatch.StartNew()
            let mutable x = SEED
            let mutable gv = 0L
            let mutable gs = 0L
            let mutable total = 0L
            let mutable drops = 0L
            while not (Volatile.Read(&stop)) do
                let el = sw.Elapsed.TotalSeconds
                let dueV = int64 (el * float valueRate)
                let dueS = int64 (el * float structRate)
                while gv < dueV do
                    x <- xorshift32 x
                    if not (ring.TryEnqueue(0, int (x % 100000u), 0, float x / 100.0)) then drops <- drops + 1L
                    gv <- gv + 1L
                    total <- total + 1L
                while gs < dueS do
                    x <- xorshift32 x
                    let kind = 1 + int (x % 3u)
                    x <- xorshift32 x
                    let a = int (x % 100000u)
                    x <- xorshift32 x
                    let b = int (x % 100000u)
                    if not (ring.TryEnqueue(kind, a, b, float x / 100.0)) then drops <- drops + 1L
                    gs <- gs + 1L
                    total <- total + 1L
                Volatile.Write(&generated, total)
                Volatile.Write(&dropped, drops)
                Thread.Sleep 1), IsBackground = true, Name = "mutate-producer")
        thread.Start()
    member _.Stop() = Volatile.Write(&stop, true)

// ---------------------------------------------------------------- scenario liste generique
type ListScn(args: Args) =
    inherit Scenario()
    let sid = args.Scenario
    let impl = args.Variant("impl", "panel")
    let load = args.Variant("load", "idle")
    let K = if sid = "list-rich" then int (if args.NGiven then args.N else 6L) else 6
    let np = if sid = "list-rich" then nPuces K else 0
    let dense = (sid = "list-rich")
    let nRows =
        match sid with
        | "list-scale" -> if args.NGiven then int args.N else 100000
        | "list-reset" -> if args.NGiven then int args.N else 10000
        | _ -> 100000
    let mutableMode = (sid = "list-mutate" || sid = "ffi-callbacks" || (sid = "latency-load" && load = "mutate"))
    let t0 = Stopwatch.GetTimestamp()
    let mutable store = RowStore(nRows, dense, mutableMode)
    let storeMs = ticksToMs (Stopwatch.GetTimestamp() - t0)
    let total0 = store.Total
    let canvasView = if impl = "canvas" then CanvasListView(store, np) else Unchecked.defaultof<_>
    let panelView = if impl = "panel" then PanelListView(store, np) else Unchecked.defaultof<_>
    let viewCtl : Control = if impl = "canvas" then canvasView :> Control else panelView :> Control
    let status, shell = mkShell ("Bench - " + sid) viewCtl "..."
    // vitesse de defilement par scenario
    let script (t: float) (maxOff: float) =
        match sid with
        | "list-scale" -> scrollScript 30000.0 4.0 150000.0 6 1.5 t maxOff
        | "list-rich" -> clamp 0.0 maxOff (20000.0 * (if t < 0.0 then t + 2.0 else t))
        | "latency-wheel" -> -1.0
        | "latency-load" when load <> "fling" -> -1.0
        | "latency-load" -> scrollScript 30000.0 4.0 150000.0 6 1.5 t maxOff
        | _ -> clamp 0.0 maxOff (3000.0 * (if t < 0.0 then t + 2.0 else t))
    let mutable offset = 0.0
    let mutable nextStatus = 0.0
    let mutable animating = true
    // flux de mises a jour
    let ring = if mutableMode && (sid = "list-mutate" || load = "mutate") then Native.SpscRing(1 <<< 20) else null
    let producer =
        if isNull ring then null
        else
            let rate = if sid = "list-mutate" then args.N else 100000L
            MutateProducer(rate, max 1L (rate / 100L), ring)
    let mutable applied = 0L
    let mutable appliedBase = 0L
    let mutable structOps = 0L
    let mutable dropBase = 0L
    // rappels natifs (F2)
    let mutable cbStarted = false
    let mutable cbBaseRecv = 0L
    let mutable cbApplied = 0L
    let mutable cbCoalesced = 0L
    // F3 / I1 heavy : core_heavy sur fil de fond
    let mutable heavyStop = false
    let mutable heavyCalls = 0L
    let mutable heavyThread : Thread = null
    let mutable heavyOnUi = false
    let mutable heavyUiMs = 0.0
    let mutable heavyUiDone = false
    // L4
    let mutable nextReset = 2.0
    let mutable rebuilds = 0L
    let rebuildMs = List<float>()
    let mutable pendingStore : RowStore = null
    let mutable building = false
    let mutable wheelDelta = 0.0
    let mutable lastWheel = 0L
    let mutable square : Square = null
    let scripted = not (sid = "latency-wheel" || (sid = "latency-load" && load <> "fling"))
    let firstRow () = if impl = "canvas" then canvasView.FirstRow else panelView.FirstRow
    let maxOffset () = if impl = "canvas" then canvasView.MaxOffset else panelView.MaxOffset
    let setOffset (o: float) =
        offset <- o
        if impl = "canvas" then
            canvasView.Offset <- o
            canvasView.InvalidateVisual()
        else
            panelView.Offset <- o
            panelView.InvalidateMeasure()
    let setStore (s: RowStore) =
        store <- s
        if impl = "canvas" then canvasView.Store <- s else panelView.Store <- s
    let startHeavyThread (ms: uint32) =
        heavyThread <- Thread((fun () ->
            while not (Volatile.Read(&heavyStop)) do
                Native.Raw.heavy ms |> ignore
                Interlocked.Increment(&heavyCalls) |> ignore), IsBackground = true, Name = "core-heavy")
        heavyThread.Start()
    /// applique les operations en attente (thread UI, une fois par frame)
    let drain () =
        let avail = ring.Available
        if avail > 0 then
            let buf = ring.Buf
            let mask = ring.Mask
            let mutable tail = ring.Tail
            let fr = firstRow ()
            for _ in 1 .. avail do
                let r = &buf.[int (tail &&& mask)]
                let cnt = store.Count
                match r.Kind with
                | 0 -> if r.Idx < cnt then store.SetPrice(r.Idx, r.Price)
                | 1 ->
                    let p = fr + 1 + (r.Idx % max 1 (cnt - fr - 1))
                    store.Insert(p, r.Price)
                    structOps <- structOps + 1L
                | 2 ->
                    if cnt > 1000 then
                        let p = fr + 1 + (r.Idx % max 1 (cnt - fr - 1))
                        store.RemoveAt p
                    structOps <- structOps + 1L
                | _ ->
                    let span = max 1 (cnt - fr - 1)
                    store.Move(fr + 1 + (r.Idx % span), fr + 1 + (r.Aux % span))
                    structOps <- structOps + 1L
                tail <- tail + 1L
            ring.Advance avail
            applied <- applied + int64 avail
            store.Commit()
    let drainCb () =
        let ring = Native.CallbackSink.Ring
        let avail = ring.Available
        if avail > 0 then
            let buf = ring.Buf
            let mask = ring.Mask
            let mutable tail = ring.Tail
            let cnt = store.Count
            for _ in 1 .. avail do
                let r = &buf.[int (tail &&& mask)]
                if r.Idx < cnt then store.SetPrice(r.Idx, r.Price)
                tail <- tail + 1L
            ring.Advance avail
            cbApplied <- cbApplied + int64 avail
    member _.Store = store
    override _.Content = shell
    override _.SetSquare(s) = square <- s
    override _.OnWindow(w: Window) =
        if sid = "latency-wheel" || sid = "latency-load" then
            viewCtl.PointerWheelChanged.Add(fun e ->
                // 100 px par cran (delta.Y = +-1)
                wheelDelta <- wheelDelta - e.Delta.Y * 100.0
                globalDirty <- true
                lastWheel <- Stopwatch.GetTimestamp())
    override this.Begin() =
        offset <- 0.0
        nextStatus <- 0.0
        appliedBase <- applied
        if not (isNull producer) then producer.Start()
        match sid with
        | "ffi-callbacks" ->
            let rate = uint64 (if args.NGiven then args.N else 10000L)
            cbStarted <- Native.CallbackSink.Start(1 <<< 21, rate, 100000u)
        | "ffi-block" ->
            let ms = uint32 (if args.NGiven then args.N else 100L)
            if args.Variant("where", "bg") = "ui" then heavyOnUi <- true else startHeavyThread ms
        | "latency-load" when load = "heavy" -> startHeavyThread 100u
        | _ -> ()
        if not (isNull ring) then
            // le producteur est (re)demarre au debut de la fenetre mesuree : compteurs propres
            ()
    override this.OnFrame(t, dt) =
        if t < 0.0 then
            // warm-up : defilement identique, pas de flux
            if scripted then setOffset (script t (maxOffset ()))
        else
            if t >= 0.0 && not (isNull producer) && producer.Generated = 0L && t < 0.5 then
                // demarrage du producteur a la premiere frame mesuree
                ()
            if scripted then setOffset (script t (maxOffset ()))
            elif wheelDelta <> 0.0 then
                let o = clamp 0.0 (maxOffset ()) (offset + wheelDelta)
                wheelDelta <- 0.0
                setOffset o
            if not (isNull ring) then
                drain ()
                if impl = "canvas" then canvasView.InvalidateVisual() else panelView.InvalidateMeasure()
            if sid = "ffi-callbacks" && cbStarted then
                drainCb ()
                if impl = "canvas" then canvasView.InvalidateVisual() else panelView.InvalidateMeasure()
            if heavyOnUi && not heavyUiDone && t >= 3.0 then
                // F3 variante ui : core_heavy sur le thread UI (gel mesure)
                heavyUiDone <- true
                let h0 = Stopwatch.GetTimestamp()
                Native.Raw.heavy (uint32 (if args.NGiven then args.N else 100L)) |> ignore
                heavyUiMs <- ticksToMs (Stopwatch.GetTimestamp() - h0)
            if sid = "list-reset" && t >= nextReset && not building then
                nextReset <- nextReset + 2.0
                building <- true
                let n = nRows
                let rb0 = Stopwatch.GetTimestamp()
                Task.Run(fun () ->
                    let s = RowStore(n, dense, false)
                    Dispatcher.UIThread.Post(fun () ->
                        setStore s
                        setOffset offset
                        rebuilds <- rebuilds + 1L
                        rebuildMs.Add(ticksToMs (Stopwatch.GetTimestamp() - rb0))
                        building <- false)) |> ignore
            if t >= nextStatus then
                nextStatus <- t + 0.5
                status.Text <- "offset " + (int64 offset).ToString(inv) + " / " + (int64 (maxOffset ())).ToString(inv)
        animating <- scripted || not (isNull ring) || sid = "ffi-callbacks"
    override _.Animating = animating
    override _.Finish() =
        if not (isNull producer) then producer.Stop()
        if cbStarted then Native.CallbackSink.Stop() |> ignore
        Volatile.Write(&heavyStop, true)
    override this.Ops sb =
        kvI sb "sum_heights" total0
        kvI sb "sum_heights_final" store.Total
        kvI sb "rows" (int64 store.Count)
        kvF sb "final_offset" offset
        kvF sb "model_build_ms" storeMs
        kvI sb "rows_drawn" (if impl = "canvas" then canvasView.RowsDrawn else panelView.RowsDrawn)
        if impl = "panel" then kvI sb "rebinds" panelView.Rebinds
        kvS sb "impl" impl
        if sid = "list-rich" then kvI sb "K" (int64 K)
        let sumH =
            // somme exacte des hauteurs du modele initial (hors mutations) : verification de la spec
            let mutable s = 0L
            let s0 = if sid = "list-mutate" || sid = "ffi-callbacks" then 0 else nRows
            for i in 0 .. s0 - 1 do s <- s + int64 (if dense then hDense i else hStd i)
            s
        if not mutableMode then kvI sb "sum_heights_check" sumH
        checksumValue <- fnvSeq (int64 nRows) (fun i -> uint32 (if dense then hDense (int i) else hStd (int i)))
        kvS sb "checksum_data" checksumValue
        if not (isNull producer) then
            kvI sb "updates_generated" producer.Generated
            kvI sb "updates_applied" (applied - appliedBase)
            kvI sb "updates_dropped" producer.Dropped
            kvI sb "structural_applied" structOps
        if sid = "ffi-callbacks" then
            let produced = int64 (Native.Raw.producer_produced ())
            kvI sb "updates_generated" produced
            kvI sb "updates_received" Native.CallbackSink.Received
            kvI sb "updates_applied" cbApplied
            kvI sb "updates_dropped" Native.CallbackSink.Dropped
            kvS sb "callback" "delegate+GetFunctionPointerForDelegate"
        if sid = "list-reset" then
            kvI sb "rebuilds" rebuilds
            kvFArr sb "rebuild_ms" rebuildMs
        if sid = "ffi-block" then
            kvI sb "heavy_calls" heavyCalls
            if heavyOnUi then kvF sb "heavy_ui_ms" heavyUiMs
        if sid = "latency-load" then
            kvS sb "load" load
            kvI sb "heavy_calls" heavyCalls
        if not (isNull square) then kvI sb "key_events" square.Keys
    override _.Notes = "liste: impl=" + impl + " (" + (if impl = "canvas" then "Control custom + ICustomDrawOperation/Skia lease" else "panneau virtualise maison de controles pooles") + ")."
