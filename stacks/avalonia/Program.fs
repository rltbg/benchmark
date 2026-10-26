module AvaloniaBench

// Bench de reference v2 : Avalonia 12.1.4 + F# + .NET 10 (JIT et NativeAOT), X11, rendu Skia logiciel.
// UI 100 % en code F# (pas de XAML applicatif, pas de bindings). Dessin custom via
// DrawingContext.Custom(ICustomDrawOperation) + ISkiaSharpApiLeaseFeature (idiome documente :
// https://docs.avaloniaui.net/docs/graphics-animation/custom-rendering). Texte 1 Mo : AvaloniaEdit (virtualisation de lignes).
// Aucune API privee Avalonia (pas d'AvaloniaAccessUnstablePrivateApis, pas de reflexion). Aucun printf/sprintf F# (reflexion).

open System
open System.Diagnostics
open System.Globalization
open System.IO
open System.Reflection
open System.Runtime.CompilerServices
open System.Runtime.InteropServices
open System.Text
open System.Threading
open System.Threading.Channels
open System.Threading.Tasks
open Avalonia
open Avalonia.Animation
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Input
open Avalonia.Media
open Avalonia.Rendering.Composition
open Avalonia.Rendering.SceneGraph
open Avalonia.Skia
open Avalonia.Threading
open Avalonia.Themes.Simple
open Avalonia.VisualTree
open SkiaSharp

// Toutes nos signatures P/Invoke sont blittables : marshalling runtime desactive (pas de stub IL de marshalling).
// https://learn.microsoft.com/en-us/dotnet/standard/native-interop/disabled-marshalling
[<assembly: DisableRuntimeMarshalling>]
do ()

let inv = CultureInfo.InvariantCulture

// ---------------------------------------------------------------- versions (epinglees dans AvaloniaBench.fsproj / build.sh)
let PKG_VERSIONS = "Avalonia 12.1.4 (X11/Skia/HarfBuzz/Themes.Simple 12.1.4), Avalonia.AvaloniaEdit 12.0.0, SkiaSharp 3.119.4, HarfBuzzSharp 8.3.1.3, FSharp.Core 10.1.403, .NET SDK 10.0.403"
let isAot = not RuntimeFeature.IsDynamicCodeSupported
/// provenance du coeur selectionne (BENCH_CORE_IMPL) pour le champ JSON version
let coreImplVersion () =
    match Environment.GetEnvironmentVariable "BENCH_CORE_IMPL" with
    | "fstar" -> "fstar (F* v2026.10.25 commit 362e792b, extraction Custard fstar/extracted @27073d5, Core.fs sha256 0af40710aae1)"
    | "fsharp" -> "fsharp (temoin F# ecrit a la main)"
    | _ -> "rust (core/ libbenchcore C-ABI)"
let versionString () =
    let avInfo =
        try
            let a = typeof<Application>.Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()
            if isNull a then "?" else a.InformationalVersion.Split('+').[0]
        with _ -> "?"
    PKG_VERSIONS + ", core " + coreImplVersion () + ", runtime " + RuntimeInformation.FrameworkDescription + ", " + (if isAot then "NativeAOT" else "JIT (CoreCLR, self-contained)")
    + ", Avalonia assembly " + avInfo

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

/// ligne = recherche binaire sur prefix : plus grand i tel que prefix.[i] <= y
let rowAt (y: float) =
    let mutable lo = 0
    let mutable hi = N - 1
    while lo < hi do
        let mid = (lo + hi + 1) >>> 1
        if prefix.[mid] <= y then lo <- mid else hi <- mid - 1
    lo

// ---------------------------------------------------------------- chemins (BENCH_ROOT, sans chemin absolu code en dur)
let benchRoot =
    lazy (
        let env = Environment.GetEnvironmentVariable "BENCH_ROOT"
        if not (String.IsNullOrEmpty env) then env
        else
            // deduit de l'emplacement du binaire : remonte jusqu'au dossier contenant core/benchcore.h
            let mutable d = DirectoryInfo(AppContext.BaseDirectory)
            let mutable res = null
            while isNull res && not (isNull d) do
                if File.Exists(Path.Combine(d.FullName, "core", "benchcore.h")) then res <- d.FullName
                d <- d.Parent
            if isNull res then Directory.GetCurrentDirectory() else res)

// ---------------------------------------------------------------- C-ABI benchcore (P/Invoke DllImport blittable)
// F# n'a pas de generateurs de source : LibraryImport (generateur C#) n'est pas disponible ; DllImport blittable +
// DisableRuntimeMarshalling est l'equivalent sans stub de marshalling. En NativeAOT : DirectPInvoke + NativeLibrary (.a, lien statique).
// SuppressGCTransition sur les 3 appels courts (< 1 us, sans I/O, sans callback, sans verrou : conditions de la doc).
module Native =
    [<DllImport("benchcore", EntryPoint = "core_validate"); SuppressGCTransition>]
    extern int core_validate_s(nativeint p, unativeint n)
    [<DllImport("benchcore", EntryPoint = "core_parse"); SuppressGCTransition>]
    extern int core_parse_s(nativeint p, unativeint n, nativeint out, unativeint cap, nativeint outLen)
    [<DllImport("benchcore", EntryPoint = "core_hash"); SuppressGCTransition>]
    extern uint32 core_hash_s(nativeint p, unativeint n)
    // variantes avec transition GC (option --no-suppress-gc-transition, pour comparaison)
    [<DllImport("benchcore")>]
    extern int core_validate(nativeint p, unativeint n)
    [<DllImport("benchcore")>]
    extern int core_parse(nativeint p, unativeint n, nativeint out, unativeint cap, nativeint outLen)
    [<DllImport("benchcore")>]
    extern uint32 core_hash(nativeint p, unativeint n)
    // core_heavy dure 100 ms : jamais SuppressGCTransition
    [<DllImport("benchcore")>]
    extern uint64 core_heavy(uint32 ms)

let mutable coreLinkNote = ""
let installResolver (asm: Assembly) =
#if CORE_STATIC
    coreLinkNote <- "libbenchcore.a liee statiquement (NativeAOT DirectPInvoke + NativeLibrary)"
#else
    // dynamique : BENCHCORE_LIB puis $BENCH_ROOT/core/target/release/libbenchcore.so
    NativeLibrary.SetDllImportResolver(asm, DllImportResolver(fun name _ _ ->
        if name = "benchcore" then
            let env = Environment.GetEnvironmentVariable "BENCHCORE_LIB"
            let p =
                if not (String.IsNullOrEmpty env) then env
                else Path.Combine(benchRoot.Value, "core", "target", "release", "libbenchcore.so")
            coreLinkNote <- "libbenchcore.so dynamique (" + p + ")"
            let mutable h = 0n
            if NativeLibrary.TryLoad(p, &h) then h else 0n
        else 0n))
#endif

// ---------------------------------------------------------------- arguments
type Args =
    { mutable Scenario: string; mutable Duration: float; mutable Out: string; mutable Rate: int
      mutable Shapes: int; mutable Assets: string; mutable SuppressGc: bool; mutable TextControl: string }

let parseArgs (argv: string[]) =
    let a = { Scenario = "idle"; Duration = 10.0; Out = "out.json"; Rate = 1000; Shapes = 5000
              Assets = null; SuppressGc = true; TextControl = "avaloniaedit" }
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
        | "--no-suppress-gc-transition" -> a.SuppressGc <- false
        | "--text-control" -> a.TextControl <- nxt ()
        | _ -> ()
        i <- i + 1
    if a.Scenario = "stress" && not durGiven then a.Duration <- 600.0
    if isNull a.Assets then a.Assets <- Path.Combine(benchRoot.Value, "assets")
    a

// ---------------------------------------------------------------- stats partagees (thread de rendu)
module Stats =
    let mutable renderTicks = 0L
    let mutable renderOps = 0L
    let addRender (ticks: int64) =
        Interlocked.Add(&renderTicks, ticks) |> ignore
        Interlocked.Increment(&renderOps) |> ignore
    let reset () =
        Volatile.Write(&renderTicks, 0L)
        Volatile.Write(&renderOps, 0L)

// ---------------------------------------------------------------- ressources Skia partagees (creees une fois : doc "reuse Pen/Brush")
module Sk =
    let typeface =
        lazy (
            let tf = SKTypeface.FromFamilyName("Noto Sans")
            if isNull tf then SKTypeface.Default else tf)
    let font = lazy (new SKFont(typeface.Value, 14.0f))
    let fontSmall = lazy (new SKFont(typeface.Value, 12.0f))
    let fontBold =
        lazy (
            let tf = SKTypeface.FromFamilyName("Noto Sans", SKFontStyle.Bold)
            new SKFont((if isNull tf then typeface.Value else tf), 14.0f))
    let listBg = lazy (new SKPaint(Color = SKColor(250uy, 250uy, 252uy)))
    let txt = lazy (new SKPaint(IsAntialias = true, Color = SKColors.Black))
    let sub = lazy (new SKPaint(IsAntialias = true, Color = SKColor(110uy, 110uy, 120uy)))
    let sep = lazy (new SKPaint(Color = SKColor(225uy, 225uy, 230uy)))
    let priceP = lazy (new SKPaint(IsAntialias = true, Color = SKColor(20uy, 110uy, 40uy)))

// ---------------------------------------------------------------- liste virtualisee (100k, hauteurs variables)
// Panneau virtualise maison : VirtualizingStackPanel est documente comme optimise pour des hauteurs uniformes
// (extent estime -> sauts de scrollbar / re-layout avec hauteurs variables) ; ici l'extent doit etre exact (8 799 584 px)
// et l'offset scripte au pixel. https://docs.avaloniaui.net/docs/app-development/performance#variable-height-items
type ListOp(bounds: Rect, offset: float, first: int, count: int, prices: float[]) =
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
                c.DrawRect(0.0f, 0.0f, w, float32 bounds.Height, Sk.listBg.Value)
                let txt = Sk.txt.Value
                let sub = Sk.sub.Value
                let sep = Sk.sep.Value
                let priceP = Sk.priceP.Value
                let font = Sk.font.Value
                let fontS = Sk.fontSmall.Value
                let fontB = Sk.fontBold.Value
                use dot = new SKPaint(IsAntialias = true)
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
                    // sous-titre repete (clippe par la ligne et par le prix a droite)
                    c.Save() |> ignore
                    c.ClipRect(SKRect(0.0f, y, w - 110.0f, y + h))
                    c.DrawText(q + "  " + q + "  " + q + "  " + q, 64.0f, y + 42.0f, fontS, sub)
                    c.Restore()
                    c.DrawText(prices.[k].ToString("F2", inv), w - 16.0f, y + h * 0.5f + 5.0f, SKTextAlign.Right, font, priceP)
                    c.Restore()
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type Prices() =
    static member val Arr : float[] = Array.init N initialPrice with get

type ListView() as this =
    inherit Control()
    do this.ClipToBounds <- true
    member val Offset = 0.0 with get, set
    member val RowsDrawn = 0L with get, set
    member val Prices : float[] = Prices.Arr with get, set
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
        ctx.Custom(new ListOp(b, off, first, count, snap))

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
    // anneau de 3 instantanes : le thread de rendu peut dessiner la frame k pendant que l'UI prepare k+1
    // (MediaContext attend Processed du batch precedent avant d'en committer un autre : au plus 2 en vol)
    let snaps = [| Snap(n); Snap(n); Snap(n) |]
    let mutable cur = 0
    member val Sizes : float32[] = Array.init n (fun i -> 12.0f + float32 (fnv (i * 5 + 3) % 28u))
    member val Kinds : int[] = Array.init n (fun i -> i % 3)
    member _.Cfg = cfg
    static member GraphValue(f: int) =
        0.5f + 0.35f * float32 (Math.Sin(float f * 0.05)) + 0.1f * float32 (Math.Sin(float f * 0.31))
    member this.Reset() =
        // historique de la polyligne pre-rempli (frames -10000..-1) : la courbe occupe toute la largeur des le debut
        for k in 0 .. GRAPH_N - 1 do graph.[k] <- ShapesModel.GraphValue(k - GRAPH_N)
        head <- 0
        frame <- 0
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
            graph.[head] <- ShapesModel.GraphValue frame
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

/// Ressources Skia creees une seule fois (shader de degrade par forme, en coordonnees locales ; paints d'ombre/fond)
type ShapesRender(model: ShapesModel) =
    let cfg = model.Cfg
    let paints : SKPaint[] =
        Array.init cfg.N (fun i ->
            let s = model.Sizes.[i]
            use sh = SKShader.CreateLinearGradient(SKPoint(0.0f, 0.0f), SKPoint(s, s), [| rowColor (i * 2 + 1); rowColor (i * 2 + 2) |],
                                                   [| 0.0f; 1.0f |], SKShaderTileMode.Clamp)
            new SKPaint(IsAntialias = true, Shader = sh))
    let pts : SKPoint[] = Array.zeroCreate GRAPH_N
    // ombre portee : flou 8 px => sigma 4 (convention Avalonia BoxShadow : sigma = blur/2), offset (2,2)
    let shadowP = new SKPaint(IsAntialias = true, Color = SKColor(0uy, 0uy, 0uy, 120uy), MaskFilter = SKMaskFilter.CreateBlur(SKBlurStyle.Normal, 4.0f))
    let bgP = new SKPaint(Color = SKColor(24uy, 28uy, 38uy))
    let clipBg = new SKPaint(Color = SKColor(238uy, 240uy, 246uy))
    let graphBg = new SKPaint(Color = SKColor(12uy, 14uy, 20uy))
    let line = new SKPaint(Color = SKColor(80uy, 220uy, 140uy), IsAntialias = true, Style = SKPaintStyle.Stroke, StrokeWidth = 1.0f)
    let clipRR = new SKRoundRect(SKRect(0.0f, 0.0f, cfg.ClipW, cfg.ClipH), 16.0f, 16.0f)
    member this.Shape(c: SKCanvas, k: int, x: float32, y: float32, s: float32, p: SKPaint) =
        match k with
        | 0 -> c.DrawRect(x, y, s, s * 0.7f, p)
        | 1 -> c.DrawCircle(x + s * 0.5f, y + s * 0.5f, s * 0.5f, p)
        | _ -> c.DrawRoundRect(x, y, s, s * 0.8f, s * 0.25f, s * 0.25f, p)
    /// la boite [x0,x1]x[y0,y1] touche-t-elle un des 4 coins arrondis (carres R x R) du clip ?
    member _.InCorner(x0: float32, y0: float32, x1: float32, y1: float32) =
        let r = 16.0f
        (x0 < r || x1 > cfg.ClipW - r) && (y0 < r || y1 > cfg.ClipH - r)
    member this.Draw(c: SKCanvas, snap: Snap) =
        c.DrawRect(0.0f, 0.0f, cfg.AreaW, cfg.AreaH, bgP)
        // Clip rect. arrondi 1000x600 r=16 : hors des 4 coins, le clip AA d'un rrect aux bords entiers est exactement le
        // clip rectangulaire (couverture 0 ou 1) ; le raster Skia CPU traite un clip rect aligne sur les pixels sans masque AA,
        // alors qu'un clip rrect AA force un masque de couverture pour chaque forme (mesure : ~2x plus lent).
        // => clip rect pour toutes les formes, et clip rrect AA seulement pour celles qui touchent un coin : rendu identique.
        c.Save() |> ignore
        c.ClipRect(SKRect(0.0f, 0.0f, cfg.ClipW, cfg.ClipH), SKClipOperation.Intersect, true)
        c.Save() |> ignore
        c.ClipRoundRect(clipRR, SKClipOperation.Intersect, true)
        c.DrawRect(0.0f, 0.0f, cfg.ClipW, cfg.ClipH, clipBg)
        c.Restore()
        let sizes = model.Sizes
        let kinds = model.Kinds
        for i in 0 .. cfg.N - 1 do
            let s = sizes.[i]
            let px = snap.Pos.[2 * i]
            let py = snap.Pos.[2 * i + 1]
            if px < cfg.ClipW && py < cfg.ClipH then
                let shadow = cfg.Shadows && i % 10 = 0
                // marge : ombre decalee de 2 px + flou (3 sigma = 12 px)
                let m = if shadow then 14.0f else 1.0f
                let corner = this.InCorner(px - m, py - m, px + s + m, py + s + m)
                if corner then
                    c.Save() |> ignore
                    c.ClipRoundRect(clipRR, SKClipOperation.Intersect, true)
                if shadow then
                    this.Shape(c, kinds.[i], px + 2.0f, py + 2.0f, s, shadowP)
                // le shader est en coordonnees locales (0..s) : translation du canvas (pas de shader recree par frame)
                c.Translate(px, py)
                this.Shape(c, kinds.[i], 0.0f, 0.0f, s, paints.[i])
                c.Translate(-px, -py)
                if corner then c.Restore()
        c.Restore()
        if cfg.Graph then
            c.DrawRect(cfg.GraphX, cfg.GraphY, cfg.GraphW, cfg.GraphH, graphBg)
            let dx = cfg.GraphW / float32 (GRAPH_N - 1)
            for k in 0 .. GRAPH_N - 1 do
                pts.[k] <- SKPoint(cfg.GraphX + float32 k * dx, cfg.GraphY + cfg.GraphH * (1.0f - snap.Graph.[k]))
            c.DrawPoints(SKPointMode.Polygon, pts, line)

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
        ctx.Custom(new ShapesOp(Rect(0.0, 0.0, float model.Cfg.AreaW, float model.Cfg.AreaH), r, model.Current))

// ---------------------------------------------------------------- scenario : base
[<AbstractClass>]
type Scenario() =
    /// t < 0 : warm-up (t+2 = temps de warm-up) ; t >= 0 : temps mesure
    abstract Update : t: float * dt: float -> unit
    abstract Reset : unit -> unit
    abstract Finish : unit -> unit
    abstract Ops : StringBuilder -> unit
    abstract Notes : string
    /// la frame en cours a-t-elle un contenu change ? (nav : seulement pendant une transition)
    abstract Animating : bool
    default _.Reset() = ()
    default _.Finish() = ()
    default _.Notes = ""
    default _.Animating = true

let kvI (sb: StringBuilder) (k: string) (v: int64) = sb.Append('"').Append(k).Append("\":").Append(v.ToString(inv)).Append(',') |> ignore
let kvF (sb: StringBuilder) (k: string) (v: float) = sb.Append('"').Append(k).Append("\":").Append(v.ToString("F3", inv)).Append(',') |> ignore
let kvS (sb: StringBuilder) (k: string) (v: string) = sb.Append('"').Append(k).Append("\":\"").Append(v.Replace("\"", "'")).Append("\",") |> ignore
let kvRaw (sb: StringBuilder) (k: string) (v: string) = sb.Append('"').Append(k).Append("\":").Append(v).Append(',') |> ignore

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
            // x(t) = v0/2 * (1 - exp(-2t)) = 10000 * (1 - exp(-2t))
            if tt < 1.5 then res <- clamp 0.0 maxOff (b + dir * 10000.0 * (1.0 - Math.Exp(-2.0 * tt)))
            else
                b <- clamp 0.0 maxOff (b + dir * 10000.0 * (1.0 - Math.Exp(-3.0)))
                tt <- tt - 1.5
                k <- k + 1
        res

// ---------------------------------------------------------------- flux de prix (thread de fond -> canal borne -> UI)
type Stream(rate: int) =
    // file bornee 100k ; TryWrite echoue quand elle est pleine (FullMode=Wait + TryWrite = pas de blocage) -> drop + compte
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
            while not (Volatile.Read(&stop)) do
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
    member _.Stop() = Volatile.Write(&stop, true)
    /// appele par le thread UI a chaque frame : applique le lot en attente
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

// ---------------------------------------------------------------- coeur : rust (C-ABI) / fstar (F# extrait par F*) / fsharp (temoin)
/// Provenance du coeur F* (fichiers inclus tels quels depuis $(BENCH_ROOT)/fstar/extracted, voir AvaloniaBench.fsproj)
let FSTAR_PROVENANCE =
    "coeur fstar: F* v2026.10.25 (commit 362e792b3aa882f1766b95c7450e3fe63081163d), backend Custard --codegen FSharp, "
    + "fstar/extracted/Core.fs sha256 0af40710aae1 + FStarCustard.fs sha256 772c090479f7 (commit depot 27073d5)"

let mutable coreVerifyNote = ""

/// Coeur F# ecrit a la main (temoin sans FFI) : memes regles que core/src/lib.rs
module FsCore =
    let MAGIC = 0xA5uy
    let VERSION = 1uy
    let MAX_PAYLOAD = 64
    let validate (b: ReadOnlySpan<byte>) : bool =
        b.Length >= 3 && b.[0] = MAGIC && b.[1] = VERSION && int b.[2] <= MAX_PAYLOAD && b.Length - 3 = int b.[2]
    /// copie le payload dans out ; renvoie sa longueur, ou -1 si trame invalide / capacite insuffisante
    let parse (b: ReadOnlySpan<byte>) (out: Span<byte>) : int =
        if not (validate b) then -1
        else
            let m = b.Length - 3
            if m > out.Length then -1
            else
                b.Slice(3).CopyTo(out)
                m
    /// FNV-1a 32 bits
    let hash (b: ReadOnlySpan<byte>) : uint32 =
        let mutable h = 2166136261u
        for i in 0 .. b.Length - 1 do
            h <- (h ^^^ uint32 b.[i]) * 16777619u
        h

/// Adaptateur de comparaison : (validate, payload parse ou null, hash de la trame) pour une implementation
let coreTriplet (impl: string) (f: byte[]) : bool * byte[] * uint32 =
    match impl with
    | "fsharp" ->
        let out = Array.zeroCreate<byte> 64
        let m = FsCore.parse (ReadOnlySpan f) (Span out)
        FsCore.validate (ReadOnlySpan f), (if m < 0 then null else Array.sub out 0 m), FsCore.hash (ReadOnlySpan f)
    | "fstar" ->
        let l = List.ofArray f
        Core.core_validate l, (match Core.core_parse l with Some p -> Array.ofList p | None -> null), Core.core_hash l
    | _ ->
        let n = f.Length
        let p = Marshal.AllocHGlobal(max 1 n)
        let o = Marshal.AllocHGlobal 64
        let ol = Marshal.AllocHGlobal 8
        try
            if n > 0 then Marshal.Copy(f, 0, p, n)
            let v = Native.core_validate (p, unativeint n) = 1
            let ok = Native.core_parse (p, unativeint n, o, 64un, ol) = 1
            let payload =
                if ok then
                    let m = int (Marshal.ReadInt64 ol)
                    let r = Array.zeroCreate<byte> m
                    Marshal.Copy(o, r, 0, m)
                    r
                else null
            v, payload, Native.core_hash (p, unativeint n)
        finally
            Marshal.FreeHGlobal p; Marshal.FreeHGlobal o; Marshal.FreeHGlobal ol

/// Verification d'equivalence au demarrage du scenario : vecteurs FNV officiels + trames valides/invalides,
/// comparees a la reference Rust (C-ABI) et aux valeurs attendues ; echoue bruyamment (exit 3) sinon.
let verifyCore (impl: string) : string =
    let ascii (s: string) = Text.Encoding.ASCII.GetBytes s
    let frame (p: byte[]) = Array.append [| 0xA5uy; 1uy; byte p.Length |] p
    let errs = ResizeArray<string>()
    // vecteurs FNV-1a 32 (hash d'octets bruts)
    for (inp, exp) in [ ascii "", 0x811C9DC5u; ascii "a", 0xE40C292Cu; ascii "foobar", 0xBF9CF968u ] do
        let _, _, h = coreTriplet impl inp
        if h <> exp then errs.Add("hash(" + Text.Encoding.ASCII.GetString inp + ")=" + h.ToString("x8") + " attendu " + exp.ToString("x8"))
    let hello = frame (ascii "hello")
    let cases =
        [ "hello", hello, true
          "n=4 (longueur inexacte)", (let g = Array.copy hello in g.[2] <- 4uy; g), false
          "n=65 (> 64)", frame (Array.zeroCreate 65), false
          "payload 64", frame (Array.init 64 byte), true
          "payload vide", frame [||], true
          "mauvais magic", (let g = Array.copy hello in g.[0] <- 0xA4uy; g), false
          "mauvaise version", (let g = Array.copy hello in g.[1] <- 2uy; g), false
          "trop court", [| 0xA5uy; 1uy |], false
          "vide", [||], false
          "octet en trop", Array.append hello [| 0uy |], false ]
    for (name, f, expValid) in cases do
        let (v, p, h) = coreTriplet impl f
        let (rv, rp, rh) = coreTriplet "rust" f
        if v <> expValid || v <> rv then errs.Add(name + ": validate=" + string v + " rust=" + string rv + " attendu=" + string expValid)
        if (isNull p) <> (isNull rp) || (not (isNull p) && p <> rp) then errs.Add(name + ": parse differe de rust")
        if not (isNull p) && p <> f.[3..] then errs.Add(name + ": payload incorrect")
        if h <> rh then errs.Add(name + ": hash=" + h.ToString("x8") + " rust=" + rh.ToString("x8"))
    if errs.Count > 0 then
        let msg = "BENCH_CORE_IMPL=" + impl + " : equivalence avec le coeur Rust ECHOUEE : " + String.Join(" ; ", errs)
        Console.Error.WriteLine msg
        Console.Error.Flush()
        exit 3
    "equivalence " + impl + "/rust verifiee au demarrage (3 vecteurs FNV + " + string (List.length cases) + " trames)"

// ---------------------------------------------------------------- coeur (threads de fond)
type CoreRunner(suppress: bool, impl: string) =
    let mutable stop = false
    let mutable calls = 0L
    let mutable heavy = 0L
    let mutable acc = 0UL
    let mutable baseCalls = 0L
    let mutable baseHeavy = 0L
    let sw = Stopwatch()
    member _.Impl = impl
    member _.Start() =
        let worker = Thread((fun () ->
            let src = [| 0xA5uy; 1uy; 5uy; byte 'h'; byte 'e'; byte 'l'; byte 'l'; byte 'o' |]
            let mutable local = 0L
            let mutable a = 0UL
            // 1 call = 1 triplet validate+parse+hash sur la trame de 8 octets ; resultat accumule et publie
            match impl with
            | "fsharp" ->
                let out = Array.zeroCreate<byte> 64
                while not (Volatile.Read(&stop)) do
                    for _ in 1 .. 1000 do
                        let f = ReadOnlySpan<byte>(src)
                        a <- a + (if FsCore.validate f then 1UL else 0UL)
                        a <- a + (if FsCore.parse f (Span<byte>(out)) >= 0 then 1UL else 0UL)
                        a <- a + uint64 (FsCore.hash f)
                    local <- local + 1000L
                    Volatile.Write(&calls, local)
                    Volatile.Write(&acc, a)
            | "fstar" ->
                // le code extrait travaille sur list<uint8> : trame construite une fois, reutilisee
                let l : uint8 list = List.ofArray src
                while not (Volatile.Read(&stop)) do
                    for _ in 1 .. 1000 do
                        a <- a + (if Core.core_validate l then 1UL else 0UL)
                        a <- a + (match Core.core_parse l with Some _ -> 1UL | None -> 0UL)
                        a <- a + uint64 (Core.core_hash l)
                    local <- local + 1000L
                    Volatile.Write(&calls, local)
                    Volatile.Write(&acc, a)
            | _ ->
                let f = Marshal.AllocHGlobal 8
                let o = Marshal.AllocHGlobal 64
                let ol = Marshal.AllocHGlobal 8
                Marshal.Copy(src, 0, f, 8)
                if suppress then
                    while not (Volatile.Read(&stop)) do
                        for _ in 1 .. 1000 do
                            a <- a + uint64 (Native.core_validate_s (f, 8un))
                            a <- a + uint64 (Native.core_parse_s (f, 8un, o, 64un, ol))
                            a <- a + uint64 (Native.core_hash_s (f, 8un))
                        local <- local + 1000L
                        Volatile.Write(&calls, local)
                        Volatile.Write(&acc, a)   // resultat accumule et publie : la boucle ne peut pas etre eliminee
                else
                    while not (Volatile.Read(&stop)) do
                        for _ in 1 .. 1000 do
                            a <- a + uint64 (Native.core_validate (f, 8un))
                            a <- a + uint64 (Native.core_parse (f, 8un, o, 64un, ol))
                            a <- a + uint64 (Native.core_hash (f, 8un))
                        local <- local + 1000L
                        Volatile.Write(&calls, local)
                        Volatile.Write(&acc, a)), IsBackground = true, Name = "core-worker")
        let hv = Thread((fun () ->
            // core_heavy(100) toutes les 1 s (echeances absolues) : toujours l'appel Rust (generateur de charge de fond,
            // hors du coeur mesure), quelle que soit BENCH_CORE_IMPL
            let t = Stopwatch.StartNew()
            let mutable k = 0L
            while not (Volatile.Read(&stop)) do
                Native.core_heavy 100u |> ignore
                Interlocked.Increment(&heavy) |> ignore
                k <- k + 1L
                while not (Volatile.Read(&stop)) && t.ElapsedMilliseconds < k * 1000L do
                    Thread.Sleep 5), IsBackground = true, Name = "core-heavy")
        worker.Start()
        hv.Start()
    member _.Stop() = Volatile.Write(&stop, true)
    member _.MarkStart() =
        baseCalls <- Volatile.Read(&calls); baseHeavy <- Volatile.Read(&heavy); sw.Restart()
    member _.CallsPerS = float (Volatile.Read(&calls) - baseCalls) / sw.Elapsed.TotalSeconds
    member _.Heavy = Volatile.Read(&heavy) - baseHeavy
    member _.Acc = Volatile.Read(&acc)

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
        nextStatus <- 0.0
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
    override _.Notes = "Liste: Control custom virtualise (recherche binaire sur le prefixe des hauteurs, extent exact) dessine par ICustomDrawOperation+SkiaSharp (SKFont.DrawText, sans shaping HarfBuzz)."

type InputScenario(list: ListScenario, square: Border) =
    inherit Scenario()
    let mutable keys = 0L
    let mutable white = false
    member _.OnKey(e: KeyEventArgs) =
        if e.Key = Key.Space then
            keys <- keys + 1L
            white <- not white
            square.Background <- (if white then Brushes.White else Brushes.Black)
            e.Handled <- true
    override _.Update(t, dt) = list.Update(t, dt)
    override _.Reset() = list.Reset(); keys <- 0L
    override _.Finish() = list.Finish()
    override _.Ops sb =
        list.Ops sb
        kvI sb "key_events" keys
    override _.Notes = list.Notes + " input: Border 200x200 en (100,100) par-dessus la liste ; Window.KeyDown (chemin d'evenements normal, routage bulle) bascule Background noir/blanc sur Key.Space."

type CanvasScenario(model: ShapesModel, view: ShapesView) =
    inherit Scenario()
    override _.Reset() = model.Reset()
    override _.Update(_, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "shapes" (int64 model.Cfg.N)
        kvI sb "graph_points" (int64 GRAPH_N)
        kvRaw sb "not_rendered" "[]"
    override _.Notes = "Canvas: SKCanvas direct (lease Skia) ; degrade lineaire 2 couleurs = SKShader par forme cree une fois ; ombre = SKMaskFilter blur sigma 4 offset (2,2) sur 1/10 ; clip SKRoundRect AA 1000x600 r=16 ; polyligne DrawPoints(Polygon) 10 000 points 1000x100 en y=610."

type CoreScenario(model: ShapesModel, view: ShapesView, core: CoreRunner, suppress: bool) =
    inherit Scenario()
    override _.Reset() = model.Reset(); core.MarkStart()
    override _.Update(_, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Finish() = core.Stop()
    override _.Ops sb =
        kvF sb "calls_per_s" core.CallsPerS
        kvI sb "heavy_calls" core.Heavy
        kvS sb "core_impl" core.Impl
        kvS sb "suppress_gc_transition" (if suppress && core.Impl = "rust" then "true" else "false")
        kvS sb "acc" (core.Acc.ToString("x", inv))
    override _.Notes =
        (match core.Impl with
         | "fstar" -> "core: impl fstar = Core.core_validate/core_parse/core_hash extraits (list<uint8>, trame construite une fois ; parse renvoie la queue de liste sans copie) ; " + FSTAR_PROVENANCE
         | "fsharp" -> "core: impl fsharp = coeur F# ecrit a la main (ReadOnlySpan<byte>, sans FFI ; parse copie le payload)"
         | _ -> "core: impl rust = DllImport blittable" + (if suppress then " + SuppressGCTransition (validate/parse/hash)" else " (avec transition GC)") + ", " + coreLinkNote)
        + " ; " + coreVerifyNote + " ; core_heavy(100)/s = appel Rust (charge de fond hors coeur mesure) ; UI 1000 formes."

type StressScenario(list: ListScenario, model: ShapesModel, view: ShapesView, core: CoreRunner) =
    inherit Scenario()
    let heap = ResizeArray<string>()
    let mutable nextSample = 0.0
    override _.Reset() = list.Reset(); model.Reset(); core.MarkStart(); nextSample <- 0.0; heap.Clear()
    override _.Update(t, dt) =
        list.Update(t, dt)
        model.Step(float32 dt)
        view.InvalidateVisual()
        if t >= nextSample then
            let gi = GC.GetGCMemoryInfo()
            heap.Add("{\"t\":" + (max t 0.0).ToString("F1", inv) + ",\"managed_heap_kb\":" + (GC.GetTotalMemory(false) / 1024L).ToString(inv)
                     + ",\"heap_size_kb\":" + (gi.HeapSizeBytes / 1024L).ToString(inv) + ",\"committed_kb\":" + (gi.TotalCommittedBytes / 1024L).ToString(inv) + "}")
            nextSample <- (max t 0.0) + 10.0
    override _.Finish() = list.Finish(); core.Stop()
    override _.Ops sb =
        list.Ops sb
        kvS sb "core_impl" core.Impl
        kvF sb "calls_per_s" core.CallsPerS
        kvI sb "heavy_calls" core.Heavy
        kvRaw sb "not_rendered" "[]"
        sb.Append("\"heap_samples\":[").Append(String.Join(",", heap)).Append("],") |> ignore
    override _.Notes = list.Notes + " stress: liste 880 px + panneau formes 400x720 (clip 380x600, graphe 380x100) + flux 5000/s + core."

// ---------------------------------------------------------------- text : 1 Mo, lecture seule, selection
// Voie retenue : AvaloniaEdit.TextEditor (paquet officiel AvaloniaUI, MIT) : document en rope + virtualisation de lignes
// (seules les VisualLines visibles sont mises en forme). TextBox ne virtualise pas (layout du texte entier).
// --text-control textbox garde l'ancienne voie pour comparaison.
type TextScenario(ctrl: Control, getSv: unit -> ScrollViewer, selectAll: unit -> unit, kind: string, docMs: float, layoutMs: float) =
    inherit Scenario()
    let mutable sv : ScrollViewer = null
    let mutable selected = false
    let mutable maxOff = 0.0
    let mutable selMs = 0.0
    member val FirstDisplayMs = -1.0 with get, set
    override _.Reset() =
        sv <- getSv ()
        if not (isNull sv) then maxOff <- max 0.0 (sv.Extent.Height - sv.Viewport.Height)
    override _.Update(t, dt) =
        if t >= 0.0 && not (isNull sv) then
            // aller-retour 3000 px/s sur [0, min(maxOff, 30000)] (periode 20 s)
            maxOff <- max 0.0 (sv.Extent.Height - sv.Viewport.Height)
            let range = max 1.0 (min maxOff 30000.0)
            let ph = (3000.0 * t) % (2.0 * range)
            let y = if ph < range then ph else 2.0 * range - ph
            sv.Offset <- Vector(0.0, y)
            if t >= 5.0 && not selected then
                selected <- true
                let t0 = Stopwatch.GetTimestamp()
                selectAll ()
                selMs <- float (Stopwatch.GetTimestamp() - t0) * 1000.0 / float Stopwatch.Frequency
    override this.Ops sb =
        kvF sb "time_to_first_display_ms" this.FirstDisplayMs
        kvF sb "layout_ms" layoutMs
        kvF sb "document_load_ms" docMs
        kvF sb "extent_height" (if isNull sv then -1.0 else sv.Extent.Height)
        kvI sb "select_all_done" (if selected then 1L else 0L)
        kvF sb "select_all_call_ms" selMs
        kvS sb "text_control" kind
    override _.Notes =
        "text: " + kind + " IsReadOnly, retour a la ligne ; layout_ms = Measure+Arrange explicites de la fenetre apres affectation du document ; document_load_ms = construction du document (TextDocument/Text=) ; time_to_first_display_ms = de la lecture du fichier a la fin de presentation de la 1re frame (CompositionBatch.Rendered) ; defilement via ScrollViewer.Offset ; tout selectionner a t=5 s."

// ---------------------------------------------------------------- images : grille 600 vignettes, decodage asynchrone
type ImageGrid() as this =
    inherit Control()
    do this.ClipToBounds <- true
    static let cell = 136.0
    static let cols = 9
    member val Bitmaps : Imaging.Bitmap[] = Array.zeroCreate 600
    member val Offset = 0.0 with get, set
    member val Scale = 1.0 with get, set
    member val PanX = 0.0 with get, set
    static member ContentH = float ((600 + cols - 1) / cols) * cell
    override this.Render(ctx: DrawingContext) =
        let w = this.Bounds.Width
        let h = this.Bounds.Height
        ctx.FillRectangle(Brushes.White, Rect(0.0, 0.0, w, h))
        let s = this.Scale
        let cx = w / 2.0
        let cy = h / 2.0
        use _t = ctx.PushTransform(Matrix.CreateTranslation(-cx + this.PanX, -cy) * Matrix.CreateScale(s, s) * Matrix.CreateTranslation(cx, cy))
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
                while go && not (Volatile.Read(&stop)) do
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
        // zoom 1 -> 3 -> 1 sur 6 s (triangle) + pan horizontal sinusoidal, repete
        let z = (tt % 6.0) / 3.0
        let tri = if z < 1.0 then z else 2.0 - z
        grid.Scale <- 1.0 + 2.0 * tri
        grid.PanX <- 200.0 * tri * Math.Sin(tt * Math.PI / 3.0)
        grid.InvalidateVisual()
    override _.Finish() = stop <- true
    override _.Ops sb =
        kvI sb "images_decoded" (Volatile.Read(&decoded))
        kvF sb "decode_total_ms" (float (Volatile.Read(&decodeTicks)) * 1000.0 / float Stopwatch.Frequency)
    override _.Notes = "images: Bitmap.DecodeToWidth(128) sur 2 threads de fond (demarre au lancement, compteurs sur toute la vie du process) ; DrawingContext.DrawImage ; placeholder gris tant que non decode ; zoom 1->3->1 / 6 s + pan."

// ---------------------------------------------------------------- nav : 3 ecrans, PageSlide 300 ms, modale CrossFade 200 ms, thread de calcul
type NavScenario(screens: Control[], modal: Control) =
    inherit Scenario()
    let mutable stop = false
    let mutable transitions = 0L
    let mutable modals = 0L
    let mutable animating = false
    let cts = new CancellationTokenSource()
    member this.Run() : Task =
        task {
            let slide = PageSlide(TimeSpan.FromMilliseconds 300.0, PageSlide.SlideAxis.Horizontal)
            let fade = CrossFade(TimeSpan.FromMilliseconds 200.0)
            let mutable cur = 0
            let mutable k = 0
            // 20 transitions (push,push,pop,pop,...) + une modale (ouverture+fermeture en fondu) toutes les 2 transitions
            while not stop && k < 20 do
                let dirFwd = (k % 4) < 2
                let nxt = if dirFwd then cur + 1 else cur - 1
                animating <- true
                do! slide.Start(screens.[cur], screens.[nxt], dirFwd, cts.Token)
                cur <- nxt
                transitions <- transitions + 1L
                if k % 2 = 1 && not stop then
                    do! fade.Start(null, modal, cts.Token)
                    do! fade.Start(modal, null, cts.Token)
                    modals <- modals + 1L
                k <- k + 1
            animating <- false
        } :> Task
    override this.Reset() = transitions <- 0L; modals <- 0L; this.Run() |> ignore
    override _.Update(_, _) = ()
    override _.Animating = animating
    override _.Finish() = stop <- true; cts.Cancel()
    override _.Ops sb =
        kvI sb "transitions_done" transitions
        kvI sb "modal_cycles" modals
    override _.Notes = "nav: PageSlide(300 ms, horizontal) entre 3 ecrans (push,push,pop,pop) x20 ; CrossFade(200 ms) ouverture+fermeture de modale toutes les 2 transitions ; thread de fond de calcul ~1 coeur ; frames comptees seulement pendant les animations."

// ---------------------------------------------------------------- GC (ops.gc)
type GcTracker() =
    let mutable c0 = 0
    let mutable c1 = 0
    let mutable c2 = 0
    let mutable pause0 = TimeSpan.Zero
    let mutable lastCount = 0
    let mutable maxPause = 0.0
    let observe () =
        // pause max : pauses du dernier GC de chaque type (GetGCMemoryInfo) relevees quand le compteur change ;
        // borne inferieure si plusieurs GC du meme type surviennent entre deux relevés
        for kind in [| GCKind.Ephemeral; GCKind.FullBlocking; GCKind.Background |] do
            let gi = GC.GetGCMemoryInfo(kind)
            if gi.Index > 0L && gi.Generation >= 0 then
                let pd = gi.PauseDurations
                for k in 0 .. pd.Length - 1 do
                    let ms = pd.[k].TotalMilliseconds
                    if ms > maxPause then maxPause <- ms
    member _.Start() =
        c0 <- GC.CollectionCount 0; c1 <- GC.CollectionCount 1; c2 <- GC.CollectionCount 2
        pause0 <- GC.GetTotalPauseDuration()
        lastCount <- c0
        maxPause <- 0.0
    member _.Sample() =
        let n = GC.CollectionCount 0
        if n <> lastCount then
            lastCount <- n
            observe ()
    member this.Json() =
        this.Sample()
        let n = GC.CollectionCount 0 - c0
        "{\"collections\":" + n.ToString(inv) + ",\"gen0\":" + n.ToString(inv) + ",\"gen1\":" + (GC.CollectionCount 1 - c1).ToString(inv)
        + ",\"gen2\":" + (GC.CollectionCount 2 - c2).ToString(inv)
        + ",\"pause_ms_total\":" + (GC.GetTotalPauseDuration() - pause0).TotalMilliseconds.ToString("F3", inv)
        + ",\"pause_ms_max\":" + (if n > 0 then maxPause else 0.0).ToString("F3", inv)
        + ",\"server_gc\":" + (if Runtime.GCSettings.IsServerGC then "true" else "false")
        + ",\"latency_mode\":\"" + Runtime.GCSettings.LatencyMode.ToString() + "\"}"

let printMarker (s: string) =
    Console.Out.WriteLine s
    Console.Out.Flush()

let FRAME_NOTE =
    "frame_ms = intervalle entre fins de rendu consecutives du thread de rendu Avalonia : CompositionBatch.Rendered (API publique, "
    + "Compositor.RequestCompositionBatchCommitAsync, horodatee de facon synchrone sur le thread de rendu apres le rendu+present de toutes les cibles), "
    + "1 frame par batch contenant une modification. Plafond : X11 cree SleepLoopRenderTimer(DesiredFps = max(60, taux XRandR max)) sans option publique "
    + "pour le changer (X11PlatformOptions n'expose pas de FPS ; ShouldRenderOnUIThread -> UiThreadRenderTimer(60)) : plafond garde (pas d'API privee)."

let jsonHead (sb: StringBuilder) (scenario: string) (duration: float) =
    sb.Append("{\"stack\":\"avalonia-").Append(if isAot then "aot" else "jit").Append("\",\"version\":\"").Append(versionString().Replace("\"", "'"))
      .Append("\",\"scenario\":\"").Append(scenario).Append("\",\"duration_s\":").Append(duration.ToString("F1", inv)) |> ignore

/// checksum deterministe du modele de donnees : FNV-1a 32 des hauteurs (le checksum ne depend pas du timing)
let dataChecksum =
    lazy (
        let mutable h = 2166136261u
        for i in 0 .. N - 1 do
            let v = uint32 heights.[i]
            for k in 0 .. 3 do h <- (h ^^^ ((v >>> (8 * k)) &&& 0xFFu)) * 16777619u
        h.ToString("x8"))

// ---------------------------------------------------------------- driver de frames
type Driver(top: TopLevel, sc: Scenario, args: Args, notes: string, onFirstRendered: float -> unit) =
    let frames = ResizeArray<float32>(1 <<< 16)
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
    let mutable cb : Action<TimeSpan> = null
    let freq = float Stopwatch.Frequency
    let mutable compositor : Compositor = null
    /// fin de rendu d'un batch : appele sur le thread de rendu (continuation synchrone)
    let onRendered (count: bool) =
        let now = Stopwatch.GetTimestamp()
        lock gate (fun () ->
            if not firstRendered then
                firstRendered <- true
                let ms = float (now - warmStart) * 1000.0 / freq
                Dispatcher.UIThread.Post(fun () -> onFirstRendered ms)
            // plusieurs batches rendus dans la meme passe = une seule frame
            if now - lastRendered > int64 (freq * 0.0001) then
                if count && measuring && not finished && lastRendered > 0L then
                    frames.Add(float32 (float (now - lastRendered) * 1000.0 / freq))
                lastRendered <- now)
    let renderedAction0 = Action<Task>(fun _ -> onRendered false)
    let renderedAction1 = Action<Task>(fun _ -> onRendered true)
    member this.WriteJson() =
        let sb = StringBuilder(1 <<< 20)
        jsonHead sb args.Scenario args.Duration
        let fr = lock gate (fun () -> frames.ToArray())
        sb.Append(",\"frames\":").Append(fr.Length).Append(",\"frame_ms\":[") |> ignore
        for i in 0 .. fr.Length - 1 do
            if i > 0 then sb.Append(',') |> ignore
            sb.Append(fr.[i].ToString("F3", inv)) |> ignore
        sb.Append("],\"ops\":{") |> ignore
        sc.Ops sb
        let mutable mx = 0.0f
        for f in fr do if f > mx then mx <- f
        kvF sb "max_frame_stall_ms" (float mx)
        kvRaw sb "gc" (gc.Json())
        let ro = Volatile.Read(&Stats.renderOps)
        kvI sb "render_ops" ro
        kvF sb "render_op_ms_mean" (if ro > 0L then float (Volatile.Read(&Stats.renderTicks)) * 1000.0 / freq / float ro else 0.0)
        sb.Length <- sb.Length - 1
        sb.Append("},\"checksum\":\"").Append(dataChecksum.Value).Append("\",\"notes\":\"")
          .Append((notes + " " + sc.Notes).Replace("\"", "'")).Append("\",\"unsupported\":null}") |> ignore
        File.WriteAllText(args.Out, sb.ToString())
    member this.Finish() =
        if not finished then
            lock gate (fun () -> finished <- true)
            printMarker "BENCH:measure_end"
            sc.Finish()
            this.WriteJson()
            match Application.Current.ApplicationLifetime with
            | :? IClassicDesktopStyleApplicationLifetime as d -> d.Shutdown 0
            | _ -> exit 0
    member private this.Track() =
        // le batch qui portera les changements faits dans ce callback
        let b = compositor.RequestCompositionBatchCommitAsync()
        b.Rendered.ContinueWith((if sc.Animating then renderedAction1 else renderedAction0), CancellationToken.None,
                                TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default) |> ignore
    member this.Start() =
        compositor <- ElementComposition.GetElementVisual(top).Compositor
        warmStart <- Stopwatch.GetTimestamp()
        lastRaf <- warmStart
        cb <- Action<TimeSpan>(fun _ ->
            let now = Stopwatch.GetTimestamp()
            if not measuring then
                let tw = float (now - warmStart) / freq
                if tw >= 2.0 then
                    sc.Reset()
                    Stats.reset ()
                    gc.Start()
                    startTs <- now
                    lastRaf <- now
                    lock gate (fun () -> measuring <- true)
                    printMarker "BENCH:measure_start"
                    sc.Update(0.0, 0.016)
                else
                    sc.Update(tw - 2.0, float (now - lastRaf) / freq)
                    lastRaf <- now
                this.Track()
                top.RequestAnimationFrame cb
            else
                let t = float (now - startTs) / freq
                let dt = float (now - lastRaf) / freq
                lastRaf <- now
                gc.Sample()
                if t >= args.Duration then
                    // attendre la fin de rendu des batches deja commites (la derniere frame mesuree, p. ex. apres un long
                    // blocage du thread UI) : commit vide (non compte) puis Finish a son Rendered
                    if not finishing then
                        finishing <- true
                        let b = compositor.RequestCompositionBatchCommitAsync()
                        b.Rendered.ContinueWith((fun (_: Task) -> Dispatcher.UIThread.Post(fun () -> this.Finish())), CancellationToken.None,
                                                TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default) |> ignore
                else
                    sc.Update(t, dt)
                    this.Track()
                    top.RequestAnimationFrame cb)
        top.RequestAnimationFrame cb

/// idle : aucune animation ni redessin continu ; 2 s de warm-up puis sommeil `duration` entre les marqueurs
let runIdle (a: Args) (notes: string) =
    let gc = GcTracker()
    let t1 = DispatcherTimer(Interval = TimeSpan.FromSeconds 2.0)
    t1.Tick.Add(fun _ ->
        t1.Stop()
        gc.Start()
        printMarker "BENCH:measure_start"
        let t2 = DispatcherTimer(Interval = TimeSpan.FromSeconds a.Duration)
        t2.Tick.Add(fun _ ->
            t2.Stop()
            printMarker "BENCH:measure_end"
            let sb = StringBuilder()
            jsonHead sb a.Scenario a.Duration
            sb.Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{\"sum_heights\":").Append(sumH).Append(",\"gc\":").Append(gc.Json())
              .Append("},\"checksum\":\"").Append(dataChecksum.Value).Append("\",\"notes\":\"")
              .Append(notes.Replace("\"", "'")).Append(" idle: coquille (titre, 1re page de liste, statut), aucun redessin ; DispatcherTimer.\",\"unsupported\":null}") |> ignore
            File.WriteAllText(a.Out, sb.ToString())
            (Application.Current.ApplicationLifetime :?> IClassicDesktopStyleApplicationLifetime).Shutdown 0)
        t2.Start())
    t1.Start()

// ---------------------------------------------------------------- application
let mutable theArgs : Args = Unchecked.defaultof<Args>

let findScrollViewer (v: Visual) =
    v.GetVisualDescendants() |> Seq.tryPick (fun d -> match d with :? ScrollViewer as s -> Some s | _ -> None) |> Option.toObj

let buildWindow () : Window =
    let a = theArgs
    let win = Window(Width = 1280.0, Height = 720.0, CanResize = false, WindowDecorations = WindowDecorations.None,
                     Background = SolidColorBrush(Color.Parse "#FAFAFC"), Title = "avalonia-bench",
                     WindowStartupLocation = WindowStartupLocation.Manual, Position = PixelPoint(0, 0))
    let baseNotes = "Avalonia 12.1.4 X11 RenderingMode=Software, Skia CPU, " + (if isAot then "NativeAOT" else "JIT") + ". " + FRAME_NOTE
                    + " render_op_ms_mean = duree moyenne de Render() des operations custom (thread de rendu, hors flush/present)."
    let mutable sc : Scenario = Unchecked.defaultof<Scenario>
    let mutable idle = false
    let mutable onFirst : float -> unit = ignore
    let core () =
        let impl = match Environment.GetEnvironmentVariable "BENCH_CORE_IMPL" with
                   | null | "" -> "rust"
                   | "rust" | "fstar" | "fsharp" as v -> v
                   | v -> Console.Error.WriteLine("BENCH_CORE_IMPL inconnu: " + v + " (rust|fstar|fsharp)"); exit 3
        coreVerifyNote <- verifyCore impl
        CoreRunner(a.SuppressGc, impl)
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
    | "input" ->
        let lv = ListView()
        let st, shell = mkShell "Bench - input" lv "..."
        let square = Border(Width = 200.0, Height = 200.0, Background = Brushes.Black, IsHitTestVisible = false,
                            HorizontalAlignment = Layout.HorizontalAlignment.Left, VerticalAlignment = Layout.VerticalAlignment.Top,
                            Margin = Thickness(100.0, 100.0, 0.0, 0.0))
        let root = Panel()
        root.Children.Add shell
        root.Children.Add square
        win.Content <- root
        let isc = InputScenario(ListScenario(lv, st, None), square)
        win.KeyDown.Add(fun e -> isc.OnKey e)
        sc <- isc
    | "canvas" ->
        let cfg = { N = a.Shapes; Shadows = true; Graph = true; AreaW = 1280.0f; AreaH = 720.0f; ClipW = 1000.0f; ClipH = 600.0f
                    GraphX = 0.0f; GraphY = 610.0f; GraphW = 1000.0f; GraphH = 100.0f }
        let m = ShapesModel(cfg)
        m.Reset()
        let v = ShapesView(m)
        win.Content <- v
        sc <- CanvasScenario(m, v)
    | "core" ->
        let cfg = { N = 1000; Shadows = false; Graph = false; AreaW = 1280.0f; AreaH = 720.0f; ClipW = 1000.0f; ClipH = 600.0f
                    GraphX = 0.0f; GraphY = 610.0f; GraphW = 1000.0f; GraphH = 100.0f }
        let m = ShapesModel(cfg)
        m.Reset()
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
        m.Reset()
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
        sc <- StressScenario(ls, m, v, c)
    | "text" ->
        let t0 = Stopwatch.GetTimestamp()
        let ms (t: int64) = float (t - t0) * 1000.0 / float Stopwatch.Frequency
        let text = File.ReadAllText(Path.Combine(a.Assets, "text1mb.txt"))
        let ctrl, getSv, selAll, kind, tDoc =
            if a.TextControl = "textbox" then
                let tb = TextBox(IsReadOnly = true, AcceptsReturn = true, TextWrapping = Media.TextWrapping.Wrap, FontSize = 14.0, BorderThickness = Thickness(0.0))
                tb.Text <- text
                let tDoc = Stopwatch.GetTimestamp()
                (tb :> Control), (fun () -> findScrollViewer tb), (fun () -> tb.Focus() |> ignore; tb.SelectAll()), "TextBox", tDoc
            else
                let ed = AvaloniaEdit.TextEditor(IsReadOnly = true, WordWrap = true, FontSize = 14.0, ShowLineNumbers = false,
                                                 FontFamily = FontFamily("Noto Sans"), Background = Brushes.White)
                ed.Document <- AvaloniaEdit.Document.TextDocument(text)
                let tDoc = Stopwatch.GetTimestamp()
                // NB : TextEditor.ScrollToVerticalOffset est un no-op dans AvaloniaEdit 12.0.0 (corps commente) -> ScrollViewer du template
                let sel () =
                    ed.Focus() |> ignore
                    if ed.CanSelectAll then ed.SelectAll() else ed.Select(0, ed.Document.TextLength)
                (ed :> Control), (fun () -> findScrollViewer ed), sel, "AvaloniaEdit.TextEditor", tDoc
        let st, shell = mkShell "Bench - text" ctrl "1 MB"
        win.Content <- shell
        let tl0 = Stopwatch.GetTimestamp()
        win.Measure(Size(1280.0, 720.0))
        win.Arrange(Rect(0.0, 0.0, 1280.0, 720.0))
        let lm = float (Stopwatch.GetTimestamp() - tl0) * 1000.0 / float Stopwatch.Frequency
        let docMs = ms tDoc
        let tsc = TextScenario(ctrl, getSv, selAll, kind, docMs, lm)
        // temps jusqu'a la 1re fin de rendu, depuis t0 (lecture du fichier)
        let warmOffset = ref 0.0
        win.Opened.Add(fun _ -> warmOffset.Value <- ms (Stopwatch.GetTimestamp()))
        onFirst <- (fun sinceWarm -> tsc.FirstDisplayMs <- warmOffset.Value + sinceWarm)
        sc <- tsc
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
        // scenario inconnu : resultat "unsupported" (pas de simulation)
        let sb = StringBuilder()
        jsonHead sb other a.Duration
        sb.Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{},\"checksum\":\"0\",\"notes\":\"\",\"unsupported\":\"scenario inconnu: ").Append(other.Replace("\"", "'")).Append("\"}") |> ignore
        File.WriteAllText(a.Out, sb.ToString())
        exit 2
    if idle then win.Opened.Add(fun _ -> runIdle a baseNotes)
    else win.Opened.Add(fun _ -> Driver(win, sc, a, baseNotes, onFirst).Start())
    win

type App() =
    inherit Application()
    override this.Initialize() =
        this.Styles.Add(SimpleTheme())
        if theArgs.Scenario = "text" && theArgs.TextControl <> "textbox" then
            // styles AvaloniaEdit requis (README AvaloniaEdit), variante Simple
            this.Styles.Add(AvaloniaBenchUi.TextStyles())
    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as d -> d.MainWindow <- buildWindow ()
        | _ -> ()
        base.OnFrameworkInitializationCompleted()

[<EntryPoint>]
let main argv =
    installResolver (Assembly.GetExecutingAssembly())
    theArgs <- parseArgs argv
    AppBuilder.Configure<App>()
        .UseX11()
        .UseSkia()
        .UseHarfBuzz()
        .With(X11PlatformOptions(RenderingMode = [| X11RenderingMode.Software |],
                                  // garde le framebuffer logiciel entre frames : evite l'allocation de 3,7 Mo (UnmanagedBlob
                                  // + GC.AddMemoryPressure => GC gen2 a chaque frame) et le blit complet ; option publique documentee
                                  UseRetainedFramebuffer = Nullable true))
        // emoji en couleur : sans repli explicite, le repli systeme (fontconfig) choisit une police monochrome ;
        // FontManagerOptions.FontFallbacks est consulte avant tout autre repli (doc API FontManagerOptions)
        .With(FontManagerOptions(FontFallbacks = [| FontFallback(FontFamily = FontFamily("Noto Color Emoji"),
                                                                 UnicodeRange = UnicodeRange.Parse("U+1F000-1FAFF, U+200D, U+FE0F")) |]))
        .StartWithClassicDesktopLifetime(argv)
