module AvaloniaBench

// Bench de reference : Avalonia 12.1.4 + F# + .NET 10 (JIT et NativeAOT), X11, rendu Skia logiciel.
// UI 100 % en code F# (pas de XAML applicatif, pas de FuncUI, pas de bindings) ; dessin custom via
// ICustomDrawOperation + SkiaSharp (ISkiaSharpApiLeaseFeature). Aucun printf/sprintf (reflexion F#).

open System
open System.Collections.Generic
open System.Diagnostics
open System.Diagnostics.CodeAnalysis
open System.Globalization
open System.IO
open System.Reflection
open System.Runtime.CompilerServices
open System.Runtime.InteropServices
open System.Text
open System.Threading
open System.Threading.Channels
open System.Threading.Tasks
open Avalonia.VisualTree
open Avalonia
open Avalonia.Animation
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Media
open Avalonia.Platform
open Avalonia.Rendering
open Avalonia.Rendering.SceneGraph
open Avalonia.Skia
open Avalonia.Threading
open Avalonia.Themes.Simple
open SkiaSharp

let inv = CultureInfo.InvariantCulture

// ---------------------------------------------------------------- donnees communes
let fnv (i: int) : uint32 =
    let mutable h = 2166136261u
    let u = uint32 i
    for k in 0 .. 3 do
        h <- (h ^^^ ((u >>> (8 * k)) &&& 0xFFu)) * 16777619u
    h

let N = 100000
let heights : int[] = Array.init N (fun i -> 48 + 16 * int (fnv i % 6u))
/// prefix.[i] = Y du haut de la ligne i ; prefix.[N] = somme des hauteurs
let prefix : float[] =
    let a = Array.zeroCreate (N + 1)
    for i in 0 .. N - 1 do a.[i + 1] <- a.[i] + float heights.[i]
    a
let sumH = int64 prefix.[N]
let rowColor (i: int) =
    let h = fnv i
    SKColor(byte (h &&& 255u), byte ((h >>> 8) &&& 255u), byte ((h >>> 16) &&& 255u))
let initialPrice (i: int) = float (fnv (i + 7) % 100000u) / 100.0

/// ligne = binaire sur prefix : plus grand i tel que prefix.[i] <= y
let rowAt (y: float) =
    let mutable lo = 0
    let mutable hi = N - 1
    while lo < hi do
        let mid = (lo + hi + 1) >>> 1
        if prefix.[mid] <= y then lo <- mid else hi <- mid - 1
    lo

// ---------------------------------------------------------------- C-ABI benchcore (P/Invoke statique)
module Native =
    [<DllImport("benchcore")>]
    extern int core_validate(nativeint p, unativeint n)
    [<DllImport("benchcore")>]
    extern int core_parse(nativeint p, unativeint n, nativeint out, unativeint cap, nativeint outLen)
    [<DllImport("benchcore")>]
    extern uint32 core_hash(nativeint p, unativeint n)
    [<DllImport("benchcore")>]
    extern uint64 core_heavy(uint32 ms)
    // variantes SuppressGCTransition (appels courts uniquement)
    [<DllImport("benchcore", EntryPoint = "core_validate"); SuppressGCTransition>]
    extern int core_validate_s(nativeint p, unativeint n)
    [<DllImport("benchcore", EntryPoint = "core_parse"); SuppressGCTransition>]
    extern int core_parse_s(nativeint p, unativeint n, nativeint out, unativeint cap, nativeint outLen)
    [<DllImport("benchcore", EntryPoint = "core_hash"); SuppressGCTransition>]
    extern uint32 core_hash_s(nativeint p, unativeint n)

let installResolver (asm: Assembly) =
#if CORE_STATIC
    ()
#else
    NativeLibrary.SetDllImportResolver(asm, DllImportResolver(fun name _ _ ->
        if name = "benchcore" then
            let p = Path.Combine(AppContext.BaseDirectory, "libbenchcore.so")
            let mutable h = 0n
            if NativeLibrary.TryLoad(p, &h) then h
            elif NativeLibrary.TryLoad("/home/user/eval/core/target/release/libbenchcore.so", &h) then h
            else 0n
        else 0n))
#endif

// ---------------------------------------------------------------- arguments
type Args =
    { mutable Scenario: string; mutable Duration: float; mutable Out: string; mutable Rate: int
      mutable Shapes: int; mutable Assets: string; mutable SuppressGc: bool; mutable Uncapped: bool }

let parseArgs (argv: string[]) =
    let a = { Scenario = "idle"; Duration = 10.0; Out = "out.json"; Rate = 1000; Shapes = 5000
              Assets = "/home/user/eval/assets"; SuppressGc = false; Uncapped = true }
    let mutable durGiven = false
    let mutable i = 0
    while i < argv.Length do
        let nxt () = i <- i + 1; argv.[i]
        match argv.[i] with
        | "--scenario" -> a.Scenario <- nxt ()
        | "--duration" -> a.Duration <- Double.Parse(nxt (), inv); durGiven <- true
        | "--out" -> a.Out <- nxt ()
        | "--rate" -> a.Rate <- Int32.Parse(nxt (), inv)
        | "--shapes" -> a.Shapes <- Int32.Parse(nxt (), inv)
        | "--assets" -> a.Assets <- nxt ()
        | "--suppress-gc-transition" -> a.SuppressGc <- true
        | "--capped" -> a.Uncapped <- false
        | _ -> ()
        i <- i + 1
    if a.Scenario = "stress" && not durGiven then a.Duration <- 600.0
    a

// ---------------------------------------------------------------- stats partagees (thread de rendu)
module Stats =
    let mutable renderTicks = 0L
    let mutable renderOps = 0L
    let addRender (ticks: int64) =
        Interlocked.Add(&renderTicks, ticks) |> ignore
        Interlocked.Increment(&renderOps) |> ignore

let skFont =
    lazy (
        let tf = SKTypeface.FromFamilyName("Noto Sans")
        let tf = if isNull tf then SKTypeface.Default else tf
        new SKFont(tf, 14.0f))
let skFontSmall =
    lazy (new SKFont(skFont.Value.Typeface, 12.0f))
let skFontBold =
    lazy (
        let tf = SKTypeface.FromFamilyName("Noto Sans", SKFontStyle.Bold)
        new SKFont((if isNull tf then skFont.Value.Typeface else tf), 14.0f))

// ---------------------------------------------------------------- liste virtualisee (100k, hauteurs variables)
type Prices() =
    static member val Arr : float[] = Array.init N initialPrice with get

type ListOp(bounds: Rect, offset: float, first: int, count: int, prices: float[], sel: int) =
    interface ICustomDrawOperation with
        member _.Bounds = bounds
        member _.HitTest(_) = false
        member _.Equals(_) = false
        member _.Dispose() = ()
        member _.Render(ctx: ImmediateDrawingContext) =
            let t0 = Stopwatch.GetTimestamp()
            let f = ctx.TryGetFeature<ISkiaSharpApiLeaseFeature>()
            if not (isNull (box f)) then
                use lease = f.Lease()
                let c = lease.SkCanvas
                let w = float32 bounds.Width
                use bg = new SKPaint(Color = SKColor(250uy, 250uy, 252uy))
                c.DrawRect(0.0f, 0.0f, w, float32 bounds.Height, bg)
                use txt = new SKPaint(IsAntialias = true, Color = SKColors.Black)
                use sub = new SKPaint(IsAntialias = true, Color = SKColor(110uy, 110uy, 120uy))
                use sep = new SKPaint(Color = SKColor(225uy, 225uy, 230uy))
                use dot = new SKPaint(IsAntialias = true)
                use priceP = new SKPaint(IsAntialias = true, Color = SKColor(20uy, 110uy, 40uy))
                let font = skFont.Value
                let fontS = skFontSmall.Value
                let fontB = skFontBold.Value
                for k in 0 .. count - 1 do
                    let i = first + k
                    let h = float32 heights.[i]
                    let y = float32 (prefix.[i] - offset)
                    c.Save() |> ignore
                    c.ClipRect(SKRect(0.0f, y, w, y + h))
                    c.DrawRect(0.0f, y + h - 1.0f, w, 1.0f, sep)
                    dot.Color <- rowColor i
                    c.DrawCircle(32.0f, y + h * 0.5f, 16.0f, dot)
                    c.DrawText("Item #" + i.ToString(inv), 64.0f, y + 22.0f, fontB, txt)
                    let q = "Quote " + i.ToString(inv) + " — " + (float (fnv (i + 7) % 100000u) / 100.0).ToString("F2", inv)
                    // sous-titre repete/tronque (clippe a la hauteur de la ligne)
                    c.DrawText(q + "  " + q + "  " + q, 64.0f, y + 42.0f, fontS, sub)
                    if h > 60.0f then c.DrawText(q + "  " + q, 64.0f, y + 60.0f, fontS, sub)
                    c.DrawText(prices.[k].ToString("F2", inv), w - 16.0f, y + h * 0.5f + 5.0f, SKTextAlign.Right, font, priceP)
                    c.Restore()
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type ListView() as this =
    inherit Control()
    do this.ClipToBounds <- true
    member val Offset = 0.0 with get, set
    member val RowsDrawn = 0L with get, set
    member val Prices : float[] = Prices.Arr with get, set
    member val ViewportH = 0.0 with get, set
    member this.MaxOffset = max 0.0 (prefix.[N] - this.Bounds.Height)
    override this.Render(ctx: DrawingContext) =
        let b = Rect(0.0, 0.0, this.Bounds.Width, this.Bounds.Height)
        let off = max 0.0 (min this.Offset this.MaxOffset)
        let first = rowAt off
        let mutable last = first
        while last < N - 1 && prefix.[last + 1] < off + b.Height do last <- last + 1
        let count = last - first + 1
        let snap = Array.sub this.Prices first count
        this.RowsDrawn <- this.RowsDrawn + int64 count
        ctx.Custom(ListOp(b, off, first, count, snap, 0))

// ---------------------------------------------------------------- canvas : formes animees
type ShapesCfg =
    { N: int; Shadows: bool; Graph: bool; AreaW: float32; AreaH: float32
      ClipW: float32; ClipH: float32; GraphX: float32; GraphY: float32; GraphW: float32; GraphH: float32 }

let GRAPH_N = 10000

type Snap(n: int) =
    member val Pos : float32[] = Array.zeroCreate (2 * n)
    member val Graph : float32[] = Array.zeroCreate GRAPH_N

type ShapesModel(cfg: ShapesCfg) =
    let n = cfg.N
    let x : float32[] = Array.zeroCreate n
    let y : float32[] = Array.zeroCreate n
    let vx : float32[] = Array.zeroCreate n
    let vy : float32[] = Array.zeroCreate n
    let graph : float32[] = Array.zeroCreate GRAPH_N
    let mutable head = 0
    let mutable frame = 0
    let snaps = [| Snap(n); Snap(n); Snap(n) |]
    let mutable cur = 0
    member val Sizes : float32[] = Array.init n (fun i -> 12.0f + float32 (fnv (i * 5 + 3) % 28u))
    member val Kinds : int[] = Array.init n (fun i -> i % 3)
    member _.Cfg = cfg
    member this.Reset() =
        for i in 0 .. n - 1 do
            x.[i] <- float32 (fnv (i + 3) % uint32 cfg.AreaW)
            y.[i] <- float32 (fnv (i + 5) % uint32 cfg.AreaH)
            vx.[i] <- float32 (int (fnv (i + 11) % 401u) - 200)
            vy.[i] <- float32 (int (fnv (i + 13) % 401u) - 200)
    member this.Step(dt: float32) =
        let dt = min dt 0.05f
        for i in 0 .. n - 1 do
            let s = this.Sizes.[i]
            let mutable nx = x.[i] + vx.[i] * dt
            let mutable ny = y.[i] + vy.[i] * dt
            if nx < 0.0f then nx <- -nx; vx.[i] <- -vx.[i]
            elif nx > cfg.AreaW - s then nx <- 2.0f * (cfg.AreaW - s) - nx; vx.[i] <- -vx.[i]
            if ny < 0.0f then ny <- -ny; vy.[i] <- -vy.[i]
            elif ny > cfg.AreaH - s then ny <- 2.0f * (cfg.AreaH - s) - ny; vy.[i] <- -vy.[i]
            x.[i] <- nx
            y.[i] <- ny
        if cfg.Graph then
            frame <- frame + 1
            let v = 0.5f + 0.35f * float32 (Math.Sin(float frame * 0.05)) + 0.1f * float32 (Math.Sin(float frame * 0.31))
            graph.[head] <- v
            head <- (head + 1) % GRAPH_N
        cur <- (cur + 1) % 3
        let s = snaps.[cur]
        for i in 0 .. n - 1 do
            s.Pos.[2 * i] <- x.[i]
            s.Pos.[2 * i + 1] <- y.[i]
        if cfg.Graph then
            Array.Copy(graph, head, s.Graph, 0, GRAPH_N - head)
            Array.Copy(graph, 0, s.Graph, GRAPH_N - head, head)
    member _.Current = snaps.[cur]

type ShapesRender(model: ShapesModel) =
    let cfg = model.Cfg
    let paints : SKPaint[] = Array.zeroCreate cfg.N
    let pts : SKPoint[] = Array.zeroCreate GRAPH_N
    // ombre : meme technique qu'Avalonia pour BoxShadow (masque flou), flou 8 px => sigma 4, offset (2,2)
    let shadowP = lazy (new SKPaint(IsAntialias = true, Color = SKColor(0uy, 0uy, 0uy, 120uy), MaskFilter = SKMaskFilter.CreateBlur(SKBlurStyle.Normal, 4.0f)))
    let bgP = lazy (new SKPaint(Color = SKColor(24uy, 28uy, 38uy)))
    let clipBg = lazy (new SKPaint(Color = SKColor(238uy, 240uy, 246uy), IsAntialias = true))
    let graphBg = lazy (new SKPaint(Color = SKColor(12uy, 14uy, 20uy)))
    let line = lazy (new SKPaint(Color = SKColor(80uy, 220uy, 140uy), IsAntialias = true, Style = SKPaintStyle.Stroke, StrokeWidth = 1.0f))
    member _.Paint(i: int) =
        let p = paints.[i]
        if not (isNull p) then p
        else
            let s = model.Sizes.[i]
            let c1 = rowColor (i * 2 + 1)
            let c2 = rowColor (i * 2 + 2)
            let sh = SKShader.CreateLinearGradient(SKPoint(0.0f, 0.0f), SKPoint(s, s), [| c1; c2 |], [| 0.0f; 1.0f |], SKShaderTileMode.Clamp)
            let np = new SKPaint(IsAntialias = true, Shader = sh)
            paints.[i] <- np
            np
    member this.Shape(c: SKCanvas, i: int, s: float32, p: SKPaint) =
        match model.Kinds.[i] with
        | 0 -> c.DrawRect(0.0f, 0.0f, s, s * 0.7f, p)
        | 1 -> c.DrawCircle(s * 0.5f, s * 0.5f, s * 0.5f, p)
        | _ -> c.DrawRoundRect(0.0f, 0.0f, s, s * 0.8f, s * 0.25f, s * 0.25f, p)
    member this.Draw(c: SKCanvas, snap: Snap) =
        c.DrawRect(0.0f, 0.0f, cfg.AreaW, cfg.AreaH, bgP.Value)
        c.Save() |> ignore
        use rr = new SKRoundRect(SKRect(0.0f, 0.0f, cfg.ClipW, cfg.ClipH), 16.0f, 16.0f)
        c.ClipRoundRect(rr, SKClipOperation.Intersect, true)
        c.DrawRect(0.0f, 0.0f, cfg.ClipW, cfg.ClipH, clipBg.Value)
        for i in 0 .. cfg.N - 1 do
            let s = model.Sizes.[i]
            let px = snap.Pos.[2 * i]
            let py = snap.Pos.[2 * i + 1]
            if px < cfg.ClipW && py < cfg.ClipH then
                let p = this.Paint i
                c.Save() |> ignore
                c.Translate(px, py)
                if cfg.Shadows && i % 10 = 0 then
                    c.Translate(2.0f, 2.0f)
                    this.Shape(c, i, s, shadowP.Value)
                    c.Translate(-2.0f, -2.0f)
                this.Shape(c, i, s, p)
                c.Restore()
        c.Restore()
        if cfg.Graph then
            c.DrawRect(cfg.GraphX, cfg.GraphY, cfg.GraphW, cfg.GraphH, graphBg.Value)
            let dx = cfg.GraphW / float32 (GRAPH_N - 1)
            for k in 0 .. GRAPH_N - 1 do
                pts.[k] <- SKPoint(cfg.GraphX + float32 k * dx, cfg.GraphY + cfg.GraphH * (1.0f - snap.Graph.[k]))
            c.DrawPoints(SKPointMode.Polygon, pts, line.Value)

type ShapesOp(bounds: Rect, r: ShapesRender, snap: Snap) =
    interface ICustomDrawOperation with
        member _.Bounds = bounds
        member _.HitTest(_) = false
        member _.Equals(_) = false
        member _.Dispose() = ()
        member _.Render(ctx: ImmediateDrawingContext) =
            let t0 = Stopwatch.GetTimestamp()
            let f = ctx.TryGetFeature<ISkiaSharpApiLeaseFeature>()
            if not (isNull (box f)) then
                use lease = f.Lease()
                r.Draw(lease.SkCanvas, snap)
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type ShapesView(model: ShapesModel) as this =
    inherit Control()
    do this.ClipToBounds <- true
    let r = ShapesRender(model)
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(ShapesOp(Rect(0.0, 0.0, float model.Cfg.AreaW, float model.Cfg.AreaH), r, model.Current))

// ---------------------------------------------------------------- scenario : base
[<AbstractClass>]
type Scenario() =
    /// t < 0 : warm-up (t+2 = temps de warm-up) ; t >= 0 : temps mesure
    abstract Update : t: float * dt: float -> unit
    abstract Reset : unit -> unit
    abstract Finish : unit -> unit
    abstract Ops : StringBuilder -> unit
    abstract Notes : string
    default _.Reset() = ()
    default _.Finish() = ()
    default _.Notes = ""

let kvI (sb: StringBuilder) (k: string) (v: int64) = sb.Append('"').Append(k).Append("\":").Append(v.ToString(inv)).Append(',') |> ignore
let kvF (sb: StringBuilder) (k: string) (v: float) = sb.Append('"').Append(k).Append("\":").Append(v.ToString("F3", inv)).Append(',') |> ignore
let kvS (sb: StringBuilder) (k: string) (v: string) = sb.Append('"').Append(k).Append("\":\"").Append(v.Replace("\"", "'")).Append("\",") |> ignore

let clamp lo hi v = max lo (min hi v)

/// script de defilement liste : 4 s a 3000 px/s puis flings alternes (v0=20000 px/s, v(t)=v0*exp(-2t), 1.5 s)
let scriptOffset (t: float) (maxOff: float) =
    let t = if t < 0.0 then t + 2.0 else t
    if t < 4.0 then clamp 0.0 maxOff (3000.0 * t)
    else
        let mutable b = clamp 0.0 maxOff 12000.0
        let mutable tt = t - 4.0
        let mutable k = 0
        let mutable res = nan
        while Double.IsNaN res do
            let dir = if k % 2 = 0 then 1.0 else -1.0
            if tt < 1.5 then res <- clamp 0.0 maxOff (b + dir * 10000.0 * (1.0 - Math.Exp(-2.0 * tt)))
            else
                b <- clamp 0.0 maxOff (b + dir * 10000.0 * (1.0 - Math.Exp(-3.0)))
                tt <- tt - 1.5
                k <- k + 1
        res

// ---------------------------------------------------------------- flux de prix (thread de fond -> canal borne -> UI)
type Stream(rate: int) =
    let ch = Channel.CreateBounded<uint32>(BoundedChannelOptions(100000, FullMode = BoundedChannelFullMode.Wait, SingleReader = true, SingleWriter = true))
    let mutable generated = 0L
    let mutable dropped = 0L
    let mutable applied = 0L
    let mutable bG = 0L
    let mutable bD = 0L
    let mutable bA = 0L
    let mutable stop = false
    let mutable thread : Thread = null
    member _.Start() =
        thread <- Thread((fun () ->
            let sw = Stopwatch.StartNew()
            let mutable x = 0x2545F491u
            while not stop do
                let due = int64 (sw.Elapsed.TotalSeconds * float rate)
                let mutable g = Volatile.Read(&generated)
                while g < due do
                    x <- x ^^^ (x <<< 13)
                    x <- x ^^^ (x >>> 17)
                    x <- x ^^^ (x <<< 5)
                    if not (ch.Writer.TryWrite x) then Interlocked.Increment(&dropped) |> ignore
                    g <- g + 1L
                Volatile.Write(&generated, g)
                Thread.Sleep 1), IsBackground = true, Name = "price-producer")
        thread.Start()
    member _.Stop() = stop <- true
    /// appele par le thread UI a chaque frame : applique le lot
    member _.Drain(prices: float[]) =
        let mutable x = 0u
        let mutable n = 0L
        while ch.Reader.TryRead(&x) do
            prices.[int (x % 100000u)] <- float x / 100.0
            n <- n + 1L
        applied <- applied + n
    member _.Generated = Volatile.Read(&generated) - bG
    member _.Dropped = Volatile.Read(&dropped) - bD
    member _.Applied = applied - bA
    member _.ResetCounters() =
        bG <- Volatile.Read(&generated); bD <- Volatile.Read(&dropped); bA <- applied

// ---------------------------------------------------------------- coeur C-ABI (threads de fond)
type CoreRunner(suppress: bool) =
    let mutable stop = false
    let mutable calls = 0L
    let mutable heavy = 0L
    let mutable acc = 0UL
    let mutable baseCalls = 0L
    let mutable baseHeavy = 0L
    let sw = Stopwatch()
    member _.Start() =
        let worker = Thread((fun () ->
            let f = Marshal.AllocHGlobal 8
            let o = Marshal.AllocHGlobal 64
            let ol = Marshal.AllocHGlobal 8
            let src = [| 0xA5uy; 1uy; 5uy; byte 'h'; byte 'e'; byte 'l'; byte 'l'; byte 'o' |]
            Marshal.Copy(src, 0, f, 8)
            let mutable local = 0L
            let mutable a = 0UL
            if suppress then
                while not stop do
                    for _ in 1 .. 1000 do
                        a <- a + uint64 (Native.core_validate_s (f, 8un))
                        a <- a + uint64 (Native.core_parse_s (f, 8un, o, 64un, ol))
                        a <- a + uint64 (Native.core_hash_s (f, 8un))
                    local <- local + 1000L
                    Volatile.Write(&calls, local)
            else
                while not stop do
                    for _ in 1 .. 1000 do
                        a <- a + uint64 (Native.core_validate (f, 8un))
                        a <- a + uint64 (Native.core_parse (f, 8un, o, 64un, ol))
                        a <- a + uint64 (Native.core_hash (f, 8un))
                    local <- local + 1000L
                    Volatile.Write(&calls, local)
            acc <- a), IsBackground = true, Name = "core-worker")
        let hv = Thread((fun () ->
            while not stop do
                Native.core_heavy 100u |> ignore
                Interlocked.Increment(&heavy) |> ignore
                let mutable w = 0
                while w < 900 && not stop do
                    Thread.Sleep 10
                    w <- w + 10), IsBackground = true, Name = "core-heavy")
        worker.Start()
        hv.Start()
    member _.Stop() = stop <- true
    member _.MarkStart() =
        baseCalls <- Volatile.Read(&calls); baseHeavy <- Volatile.Read(&heavy); sw.Restart()
    member _.CallsPerS = float (Volatile.Read(&calls) - baseCalls) / sw.Elapsed.TotalSeconds
    member _.Heavy = Volatile.Read(&heavy) - baseHeavy

// ---------------------------------------------------------------- UI : coquille
let mkHeader (title: string) =
    let tb = TextBlock(Text = title, Foreground = Brushes.White, FontSize = 14.0, VerticalAlignment = Layout.VerticalAlignment.Center, Margin = Thickness(12.0, 0.0))
    Border(Height = 32.0, Background = SolidColorBrush(Color.Parse "#2B3A55"), Child = tb)

let mkStatus (text: string) =
    let tb = TextBlock(Text = text, FontSize = 12.0, VerticalAlignment = Layout.VerticalAlignment.Center, Margin = Thickness(12.0, 0.0))
    tb, Border(Height = 24.0, Background = SolidColorBrush(Color.Parse "#DDE1EA"), Child = tb)

let mkShell (title: string) (center: Control) (statusText: string) =
    let head = mkHeader title
    let st, stBorder = mkStatus statusText
    let dp = DockPanel(LastChildFill = true)
    DockPanel.SetDock(head, Dock.Top)
    DockPanel.SetDock(stBorder, Dock.Bottom)
    dp.Children.Add head
    dp.Children.Add stBorder
    dp.Children.Add center
    st, dp

// ---------------------------------------------------------------- scenarios
type ListScenario(view: ListView, status: TextBlock, stream: Stream option) =
    inherit Scenario()
    let mutable nextStatus = 0.0
    override _.Update(t, dt) =
        view.Offset <- scriptOffset t view.MaxOffset
        match stream with
        | Some s -> s.Drain view.Prices
        | None -> ()
        if t >= nextStatus then
            nextStatus <- t + 0.5
            status.Text <- "offset " + (int view.Offset).ToString(inv) + " / " + (int view.MaxOffset).ToString(inv)
        view.InvalidateVisual()
    override _.Reset() =
        view.RowsDrawn <- 0L
        match stream with Some s -> s.ResetCounters() | None -> ()
    override _.Finish() = match stream with Some s -> s.Stop() | None -> ()
    override _.Ops sb =
        kvI sb "sum_heights" sumH
        kvF sb "final_offset" view.Offset
        kvI sb "rows_drawn" view.RowsDrawn
        match stream with
        | Some s ->
            kvI sb "updates_generated" s.Generated
            kvI sb "updates_applied" s.Applied
            kvI sb "updates_dropped" s.Dropped
        | None -> ()
    override _.Notes = "UI-thread ListView custom (ICustomDrawOperation+SkiaSharp), virtualise par recherche binaire sur prefix des hauteurs."

type CanvasScenario(model: ShapesModel, view: ShapesView) =
    inherit Scenario()
    override _.Reset() = model.Reset()
    override _.Update(_, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "shapes" (int64 model.Cfg.N)
        kvI sb "graph_points" (int64 GRAPH_N)

type CoreScenario(model: ShapesModel, view: ShapesView, core: CoreRunner, suppress: bool) =
    inherit Scenario()
    let mutable maxStall = 0.0
    override _.Reset() = model.Reset(); core.MarkStart()
    override _.Update(_, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Finish() = core.Stop()
    override _.Ops sb =
        kvF sb "calls_per_s" core.CallsPerS
        kvI sb "heavy_calls" core.Heavy
        kvS sb "suppress_gc_transition" (if suppress then "true" else "false")
        // max_frame_stall_ms est complete par le driver (max de frame_ms)

type StressScenario(list: ListScenario, model: ShapesModel, view: ShapesView, core: CoreRunner, suppress: bool) =
    inherit Scenario()
    let heap = ResizeArray<string>()
    let mutable nextSample = 0.0
    override _.Reset() = list.Reset(); model.Reset(); core.MarkStart()
    override _.Update(t, dt) =
        list.Update(t, dt)
        model.Step(float32 dt)
        view.InvalidateVisual()
        if t >= nextSample then
            heap.Add("{\"t\":" + (max t 0.0).ToString("F1", inv) + ",\"managed_heap_kb\":" + (GC.GetTotalMemory(false) / 1024L).ToString(inv) + "}")
            nextSample <- (max t 0.0) + 10.0
    override _.Finish() = list.Finish(); core.Stop()
    override _.Ops sb =
        list.Ops sb
        kvF sb "calls_per_s" core.CallsPerS
        kvI sb "heavy_calls" core.Heavy
        sb.Append("\"heap_samples\":[").Append(String.Join(",", heap)).Append("],") |> ignore


// ---------------------------------------------------------------- text : 1 Mo dans un TextBox lecture seule
type TextScenario(tb: TextBox, status: TextBlock, tFirstStart: int64, layoutMs: float) =
    inherit Scenario()
    let mutable sv : ScrollViewer = null
    let mutable selected = false
    let mutable firstDisplayMs = -1.0
    let mutable maxOff = 0.0
    override _.Reset() =
        sv <- tb.GetVisualDescendants() |> Seq.tryPick (fun v -> match v with :? ScrollViewer as s -> Some s | _ -> None) |> Option.toObj
        if not (isNull sv) then maxOff <- max 0.0 (sv.Extent.Height - sv.Viewport.Height)
    override _.Update(t, dt) =
        if firstDisplayMs < 0.0 then
            firstDisplayMs <- float (Stopwatch.GetTimestamp() - tFirstStart) * 1000.0 / float Stopwatch.Frequency
        if t >= 0.0 && not (isNull sv) then
            // aller-retour 3000 px/s sur [0, min(maxOff, 30000)] (periode 20 s)
            let range = min maxOff 30000.0
            let ph = (3000.0 * t) % (2.0 * range)
            let y = if ph < range then ph else 2.0 * range - ph
            sv.Offset <- Vector(0.0, y)
            if t >= 5.0 && not selected then
                selected <- true
                tb.Focus() |> ignore
                tb.SelectAll()
    override _.Ops sb =
        kvF sb "time_to_first_display_ms" firstDisplayMs
        kvF sb "layout_ms" layoutMs
        kvF sb "extent_height" (if isNull sv then -1.0 else sv.Extent.Height)
        kvI sb "select_all_done" (if selected then 1L else 0L)
    override _.Notes = "TextBox IsReadOnly+AcceptsReturn+Wrap sur 1 Mo ; layout_ms = Measure() explicite du TextBox apres Text=1Mo ; time_to_first_display = de avant Text= jusqu'au premier callback d'animation apres ouverture."

// ---------------------------------------------------------------- images : grille 600 vignettes, decodage asynchrone
type ImageGrid() as this =
    inherit Control()
    do this.ClipToBounds <- true
    static let cell = 136.0
    static let cols = 9
    member val Bitmaps : Imaging.Bitmap[] = Array.zeroCreate 600
    member val Offset = 0.0 with get, set
    member val Scale = 1.0 with get, set
    static member ContentH = float ((600 + cols - 1) / cols) * cell
    override this.Render(ctx: DrawingContext) =
        let w = this.Bounds.Width
        let h = this.Bounds.Height
        ctx.FillRectangle(Brushes.White, Rect(0.0, 0.0, w, h))
        let s = this.Scale
        let cx = w / 2.0
        let cy = h / 2.0
        use _t = ctx.PushTransform(Matrix.CreateTranslation(-cx, -cy) * Matrix.CreateScale(s, s) * Matrix.CreateTranslation(cx, cy))
        let off = this.Offset
        let y0 = off + cy - cy / s - cell
        let y1 = off + cy + cy / s
        let r0 = max 0 (int (y0 / cell))
        let r1 = min ((600 + cols - 1) / cols - 1) (int (y1 / cell))
        for r in r0 .. r1 do
            for c in 0 .. cols - 1 do
                let idx = r * cols + c
                if idx < 600 then
                    let x = 8.0 + float c * cell
                    let y = float r * cell - off
                    let bmp = Volatile.Read(&this.Bitmaps.[idx])
                    if isNull bmp then ctx.FillRectangle(Brushes.LightGray, Rect(x, y, 128.0, 128.0))
                    else ctx.DrawImage(bmp, Rect(0.0, 0.0, 128.0, 128.0), Rect(x, y, 128.0, 128.0))

type ImagesScenario(grid: ImageGrid, assets: string) =
    inherit Scenario()
    let mutable decoded = 0L
    let mutable decodeTicks = 0L
    let mutable next = -1
    let mutable stop = false
    member _.StartDecoders() =
        for k in 0 .. 1 do
            let th = Thread((fun () ->
                let mutable go = true
                while go && not stop do
                    let i = Interlocked.Increment(&next)
                    if i >= 600 then go <- false
                    else
                        let t0 = Stopwatch.GetTimestamp()
                        use fs = File.OpenRead(Path.Combine(assets, "images", "img" + i.ToString("D3", inv) + ".png"))
                        let bmp = Imaging.Bitmap.DecodeToWidth(fs, 128, Imaging.BitmapInterpolationMode.MediumQuality)
                        Interlocked.Add(&decodeTicks, Stopwatch.GetTimestamp() - t0) |> ignore
                        Volatile.Write(&grid.Bitmaps.[i], bmp)
                        Interlocked.Increment(&decoded) |> ignore), IsBackground = true, Name = "img-decoder-" + k.ToString(inv))
            th.Start()
    override _.Update(t, dt) =
        let tt = if t < 0.0 then t + 2.0 else t
        let range = max 1.0 (ImageGrid.ContentH - grid.Bounds.Height)
        let ph = (3000.0 * tt) % (2.0 * range)
        grid.Offset <- (if ph < range then ph else 2.0 * range - ph)
        // zoom 1 -> 3 -> 1 sur 6 s (triangle), repete
        let z = (tt % 6.0) / 3.0
        grid.Scale <- 1.0 + 2.0 * (if z < 1.0 then z else 2.0 - z)
        grid.InvalidateVisual()
    override _.Finish() = stop <- true
    override _.Ops sb =
        kvI sb "images_decoded" (Volatile.Read(&decoded))
        kvF sb "decode_total_ms" (float (Volatile.Read(&decodeTicks)) * 1000.0 / float Stopwatch.Frequency)
    override _.Notes = "Bitmap.DecodeToWidth(128) sur 2 threads de fond (decodage demarre des le warm-up, compteurs sur toute la vie du process) ; dessin DrawingContext.DrawImage ; placeholder gris tant que non decode."

// ---------------------------------------------------------------- nav : 3 ecrans, PageSlide 300 ms, modale CrossFade 200 ms, thread de calcul
type NavScenario(screens: Control[], modal: Control) =
    inherit Scenario()
    let mutable stop = false
    let mutable transitions = 0L
    let mutable modals = 0L
    let cts = new CancellationTokenSource()
    member this.Run() : Task =
        task {
            let slide = PageSlide(TimeSpan.FromMilliseconds 300.0, PageSlide.SlideAxis.Horizontal)
            let fade = CrossFade(TimeSpan.FromMilliseconds 200.0)
            let mutable cur = 0
            let mutable k = 0
            while not stop do
                let dirFwd = (k % 4) < 2
                let nxt = if dirFwd then cur + 1 else cur - 1
                do! slide.Start(screens.[cur], screens.[nxt], dirFwd, cts.Token)
                cur <- nxt
                transitions <- transitions + 1L
                if k % 2 = 1 && not stop then
                    do! fade.Start(null, modal, cts.Token)
                    do! fade.Start(modal, null, cts.Token)
                    modals <- modals + 1L
                k <- k + 1
        } :> Task
    override this.Reset() = transitions <- 0L; modals <- 0L; this.Run() |> ignore
    override _.Update(_, _) = ()
    override _.Finish() = stop <- true; cts.Cancel()
    override _.Ops sb =
        kvI sb "transitions_done" transitions
        kvI sb "modal_cycles" modals
    override _.Notes = "PageSlide(300 ms, horizontal) entre 3 ecrans (push,push,pop,pop) ; CrossFade(200 ms) ouverture+fermeture de modale toutes les 2 transitions ; thread de fond de calcul ~1 coeur ; la boucle de transitions demarre a la fin du warm-up et se repete jusqu'a duration."

// ---------------------------------------------------------------- driver de frames
type Driver(top: TopLevel, sc: Scenario, args: Args, notes: string) =
    let frames = ResizeArray<float32>(1 <<< 16)
    let mutable measuring = false
    let mutable startTs = 0L
    let mutable lastTs = 0L
    let mutable warmStart = 0L
    let mutable finished = false
    let mutable cb : Action<TimeSpan> = null
    let freq = float Stopwatch.Frequency
    member this.WriteJson(extra: string) =
        let sb = StringBuilder(1 <<< 20)
        let aot = if RuntimeFeature.IsDynamicCodeSupported then "jit" else "aot"
        sb.Append("{\"stack\":\"avalonia-").Append(aot).Append("\",\"scenario\":\"").Append(args.Scenario)
          .Append("\",\"duration_s\":").Append(args.Duration.ToString("F1", inv))
          .Append(",\"frames\":").Append(frames.Count).Append(",\"frame_ms\":[") |> ignore
        for i in 0 .. frames.Count - 1 do
            if i > 0 then sb.Append(',') |> ignore
            sb.Append(frames.[i].ToString("F3", inv)) |> ignore
        sb.Append("],\"ops\":{") |> ignore
        sc.Ops sb
        let mutable mx = 0.0f
        for f in frames do if f > mx then mx <- f
        kvF sb "max_frame_stall_ms" (float mx)
        let ro = Volatile.Read(&Stats.renderOps)
        kvI sb "render_ops" ro
        kvF sb "render_op_ms_mean" (if ro > 0L then float (Volatile.Read(&Stats.renderTicks)) * 1000.0 / freq / float ro else 0.0)
        sb.Length <- sb.Length - 1
        sb.Append("},\"checksum\":\"").Append(((fnv (int sumH)) ^^^ uint32 frames.Count).ToString("x8")).Append("\",\"notes\":\"")
          .Append(notes.Replace("\"", "'")).Append(" ").Append(sc.Notes.Replace("\"", "'")).Append(extra).Append("\"}") |> ignore
        File.WriteAllText(args.Out, sb.ToString())
    member this.Finish() =
        if not finished then
            finished <- true
            sc.Finish()
            this.WriteJson ""
            match Application.Current.ApplicationLifetime with
            | :? IClassicDesktopStyleApplicationLifetime as d -> d.Shutdown 0
            | _ -> exit 0
    member this.Start() =
        warmStart <- Stopwatch.GetTimestamp()
        lastTs <- warmStart
        cb <- Action<TimeSpan>(fun _ ->
            let now = Stopwatch.GetTimestamp()
            if not measuring then
                let tw = float (now - warmStart) / freq
                if tw >= 2.0 then
                    measuring <- true
                    startTs <- now
                    lastTs <- now
                    sc.Reset()
                    sc.Update(0.0, 0.016)
                else
                    sc.Update(tw - 2.0, float (now - lastTs) / freq)
                    lastTs <- now
                top.RequestAnimationFrame cb
            else
                let t = float (now - startTs) / freq
                frames.Add(float32 (float (now - lastTs) * 1000.0 / freq))
                let dt = float (now - lastTs) / freq
                lastTs <- now
                if t >= args.Duration then this.Finish()
                else
                    sc.Update(t, dt)
                    top.RequestAnimationFrame cb)
        top.RequestAnimationFrame cb

// ---------------------------------------------------------------- deplafonnement du timer de rendu (60 Hz)
// X11 cree en interne SleepLoopRenderTimer(60) puis un Compositor : il n'y a pas de hook public.
// Contournement : API privees (AvaloniaAccessUnstablePrivateApis) + reflexion sur DefaultRenderLoop.Timer (internal)
// pour fixer SleepLoopRenderTimer.DesiredFps tres haut. LoopProc n'attend alors plus : tick immediat apres le precedent.
[<DynamicDependency("Timer", "Avalonia.Rendering.DefaultRenderLoop", "Avalonia.Base")>]
[<UnconditionalSuppressMessage("Trimming", "IL2075", Justification = "DefaultRenderLoop.Timer preserved by DynamicDependency")>]
let uncapRenderTimer () : string =
    try
        let loop = AvaloniaLocator.Current.GetService<IRenderLoop>()
        let p = loop.GetType().GetProperty("Timer", BindingFlags.NonPublic ||| BindingFlags.Public ||| BindingFlags.Instance)
        match p.GetValue(loop) with
        | :? SleepLoopRenderTimer as t -> t.DesiredFps <- 100000; "uncapped SleepLoopRenderTimer"
        | o -> "timer type " + o.GetType().Name + " (non deplafonne)"
    with e -> "uncap failed: " + e.Message

// ---------------------------------------------------------------- application
let mutable theArgs : Args = Unchecked.defaultof<Args>
let mutable uncapNote = "timer 60Hz par defaut"

let buildWindow () : Window =
    let a = theArgs
    let win = Window(Width = 1280.0, Height = 720.0, CanResize = false, WindowDecorations = WindowDecorations.None,
                     Background = SolidColorBrush(Color.Parse "#FAFAFC"), Title = "avalonia-bench",
                     WindowStartupLocation = WindowStartupLocation.Manual, Position = PixelPoint(0, 0))
    let aot = if RuntimeFeature.IsDynamicCodeSupported then "JIT" else "NativeAOT"
    let baseNotes =
        "Avalonia 12.1.4 X11 RenderingMode=Software, Skia CPU, " + aot + ", " + uncapNote + ". frame_ms = intervalle entre callbacks TopLevel.RequestAnimationFrame consecutifs (UI thread, debut de frame; le commit suivant attend la fin du batch precedent cote render thread). render_op_ms_mean = duree moyenne de Render() des operations custom sur le thread de rendu (hors flush/present X11)."
    let mutable sc : Scenario = Unchecked.defaultof<Scenario>
    let mutable idle = false
    let core () = CoreRunner(a.SuppressGc)
    match a.Scenario with
    | "idle" ->
        let lv = ListView()
        let _, shell = mkShell "Bench - idle" lv "idle"
        win.Content <- shell
        idle <- true
    | "list" | "stream" ->
        let lv = ListView()
        let st, shell = mkShell ("Bench - " + a.Scenario) lv "..."
        win.Content <- shell
        let s = if a.Scenario = "stream" then (let s = Stream(a.Rate) in s.Start(); Some s) else None
        sc <- ListScenario(lv, st, s)
    | "canvas" ->
        let cfg = { N = a.Shapes; Shadows = true; Graph = true; AreaW = 1280.0f; AreaH = 720.0f; ClipW = 1000.0f; ClipH = 600.0f
                    GraphX = 0.0f; GraphY = 610.0f; GraphW = 1000.0f; GraphH = 100.0f }
        let m = ShapesModel(cfg)
        let v = ShapesView(m)
        win.Content <- v
        sc <- CanvasScenario(m, v)
    | "core" ->
        let cfg = { N = 1000; Shadows = false; Graph = false; AreaW = 1280.0f; AreaH = 720.0f; ClipW = 1000.0f; ClipH = 600.0f
                    GraphX = 0.0f; GraphY = 610.0f; GraphW = 1000.0f; GraphH = 100.0f }
        let m = ShapesModel(cfg)
        let v = ShapesView(m)
        win.Content <- v
        let c = core ()
        c.Start()
        sc <- CoreScenario(m, v, c, a.SuppressGc)
    | "stress" ->
        let lv = ListView()
        let st, shell = mkShell "Bench - stress" lv "..."
        let cfg = { N = a.Shapes; Shadows = true; Graph = true; AreaW = 400.0f; AreaH = 720.0f; ClipW = 380.0f; ClipH = 600.0f
                    GraphX = 0.0f; GraphY = 610.0f; GraphW = 380.0f; GraphH = 100.0f }
        let m = ShapesModel(cfg)
        let v = ShapesView(m)
        v.Width <- 400.0
        let g = Grid()
        g.ColumnDefinitions.Add(ColumnDefinition(GridLength(880.0)))
        g.ColumnDefinitions.Add(ColumnDefinition(GridLength(400.0)))
        Grid.SetColumn(v, 1)
        g.Children.Add shell
        g.Children.Add v
        win.Content <- g
        let s = Stream(5000)
        s.Start()
        let ls = ListScenario(lv, st, Some s)
        let c = core ()
        c.Start()
        sc <- StressScenario(ls, m, v, c, a.SuppressGc)
    | "text" ->
        let text = File.ReadAllText(Path.Combine(a.Assets, "text1mb.txt"))
        let t0 = Stopwatch.GetTimestamp()
        let tb = TextBox(IsReadOnly = true, AcceptsReturn = true, TextWrapping = Media.TextWrapping.Wrap, FontSize = 14.0, BorderThickness = Thickness(0.0))
        tb.Text <- text
        let st, shell = mkShell "Bench - text" tb "1 MB"
        win.Content <- shell
        win.Measure(Size(1280.0, 720.0))
        let lm = float (Stopwatch.GetTimestamp() - t0) * 1000.0 / float Stopwatch.Frequency
        win.Arrange(Rect(0.0, 0.0, 1280.0, 720.0))
        sc <- TextScenario(tb, st, t0, lm)
    | "images" ->
        let g = ImageGrid()
        let _, shell = mkShell "Bench - images" g "600 images"
        win.Content <- shell
        let isc = ImagesScenario(g, a.Assets)
        isc.StartDecoders()
        sc <- isc
    | "nav" ->
        let mkScreen (name: string) (off: float) (col: string) =
            let lv = ListView(Offset = off)
            let g = Grid(Background = SolidColorBrush(Color.Parse col))
            g.Children.Add lv
            g.Children.Add(TextBlock(Text = name, FontSize = 28.0, Margin = Thickness(40.0, 20.0), Foreground = Brushes.Black))
            g :> Control
        let screens = [| mkScreen "Screen A" 0.0 "#FFFFFF"; mkScreen "Screen B" 5000.0 "#FFF4E0"; mkScreen "Screen C" 20000.0 "#E6F4FF" |]
        screens.[1].IsVisible <- false
        screens.[2].IsVisible <- false
        let modal = Border(Background = SolidColorBrush(Color.Parse "#99000000"), IsVisible = false,
                           Child = Border(Width = 500.0, Height = 300.0, Background = Brushes.White, CornerRadius = CornerRadius(12.0),
                                          HorizontalAlignment = Layout.HorizontalAlignment.Center, VerticalAlignment = Layout.VerticalAlignment.Center,
                                          Child = TextBlock(Text = "Modal", FontSize = 24.0, HorizontalAlignment = Layout.HorizontalAlignment.Center, VerticalAlignment = Layout.VerticalAlignment.Center)))
        let root = Grid()
        for sct in screens do root.Children.Add sct
        root.Children.Add modal
        let _, shell = mkShell "Bench - nav" root "nav"
        win.Content <- shell
        let burn = Thread((fun () ->
                        let mutable acc = 1469598103934665603UL
                        while true do acc <- (acc ^^^ (acc >>> 7)) * 1099511628211UL + 12345UL), IsBackground = true, Name = "burner")
        burn.Start()
        sc <- NavScenario(screens, modal :> Control)
    | other ->
        win.Content <- TextBlock(Text = "unsupported scenario " + other)
        idle <- true
    if idle then
        win.Opened.Add(fun _ ->
            let timer = DispatcherTimer(Interval = TimeSpan.FromSeconds(a.Duration + 2.0))
            timer.Tick.Add(fun _ ->
                timer.Stop()
                let sb = StringBuilder()
                sb.Append("{\"stack\":\"avalonia-").Append(if RuntimeFeature.IsDynamicCodeSupported then "jit" else "aot")
                  .Append("\",\"scenario\":\"").Append(a.Scenario).Append("\",\"duration_s\":").Append(a.Duration.ToString("F1", inv))
                  .Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{\"sum_heights\":").Append(sumH).Append("},\"checksum\":\"0\",\"notes\":\"")
                  .Append(baseNotes).Append(" Pas d'animation, thread UI endormi.\"}") |> ignore
                File.WriteAllText(a.Out, sb.ToString())
                (Application.Current.ApplicationLifetime :?> IClassicDesktopStyleApplicationLifetime).Shutdown 0)
            timer.Start())
    else
        win.Opened.Add(fun _ -> Driver(win, sc, a, baseNotes).Start())
    win

type App() =
    inherit Application()
    override this.Initialize() = this.Styles.Add(SimpleTheme())
    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as d -> d.MainWindow <- buildWindow ()
        | _ -> ()
        base.OnFrameworkInitializationCompleted()

[<EntryPoint>]
let main argv =
    installResolver (Assembly.GetExecutingAssembly())
    theArgs <- parseArgs argv
    let b =
        AppBuilder.Configure<App>()
            .UseX11()
            .UseSkia()
            .UseHarfBuzz()
            .With(X11PlatformOptions(RenderingMode = [| X11RenderingMode.Software |]))
    let b =
        if theArgs.Uncapped && theArgs.Scenario <> "idle" then
            b.AfterPlatformServicesSetup(fun _ -> uncapNote <- uncapRenderTimer ())
        else b
    b.StartWithClassicDesktopLifetime(argv)
