module Gfx
// G1 gfx-shapes, G2 gfx-paths, G3 gfx-chart, G4 gfx-images, G5 gfx-layers : canvas natif = ICustomDrawOperation + ISkiaSharpApiLeaseFeature
// (idiome documente : https://docs.avaloniaui.net/docs/graphics-animation/custom-rendering). Paints/shaders/paths/images crees une seule fois.

open System
open System.Collections.Generic
open System.Diagnostics
open System.IO
open System.Text
open System.Threading
open Avalonia
open Avalonia.Controls
open Avalonia.Media
open Avalonia.Media.Immutable
open Avalonia.Platform
open Avalonia.Rendering.SceneGraph
open Avalonia.Skia
open SkiaSharp
open Util
open Harness

// ---------------------------------------------------------------- formes (G1, F1, M1)
type ShapesFeatures = { Gradient: bool; Shadow: bool; Clip: bool; ClipCornerOpt: bool }

type Snap(n: int) =
    member val Pos : float32[] = Array.zeroCreate (2 * n)

type ShapesModel(n: int, areaW: float32, areaH: float32) =
    let x : float32[] = Array.zeroCreate n
    let y : float32[] = Array.zeroCreate n
    let vx : float32[] = Array.zeroCreate n
    let vy : float32[] = Array.zeroCreate n
    // anneau de 3 instantanes : le thread de rendu dessine la frame k pendant que l'UI prepare k+1
    let snaps = [| Snap(n); Snap(n); Snap(n) |]
    let mutable cur = 0
    member val Sizes : float32[] = Array.init n (fun i -> 12.0f + float32 (fnv (i * 5 + 3) % 28u))
    member val Kinds : int[] = Array.init n (fun i -> i % 3)
    member _.N = n
    member _.AreaW = areaW
    member _.AreaH = areaH
    member this.Reset() =
        for i in 0 .. n - 1 do
            x.[i] <- float32 (fnv (i + 3) % uint32 areaW)
            y.[i] <- float32 (fnv (i + 5) % uint32 areaH)
            vx.[i] <- float32 (int (fnv (i + 11) % 401u) - 200)
            vy.[i] <- float32 (int (fnv (i + 13) % 401u) - 200)
        cur <- 0
        this.Step 0.0f
    member this.Step(dt: float32) =
        let dt = min dt 0.05f
        let sizes = this.Sizes
        for i in 0 .. n - 1 do
            let s = sizes.[i]
            let mutable nx = x.[i] + vx.[i] * dt
            let mutable ny = y.[i] + vy.[i] * dt
            if nx < 0.0f then nx <- -nx; vx.[i] <- -vx.[i]
            elif nx > areaW - s then nx <- 2.0f * (areaW - s) - nx; vx.[i] <- -vx.[i]
            if ny < 0.0f then ny <- -ny; vy.[i] <- -vy.[i]
            elif ny > areaH - s then ny <- 2.0f * (areaH - s) - ny; vy.[i] <- -vy.[i]
            x.[i] <- nx
            y.[i] <- ny
        cur <- (cur + 1) % 3
        let p = snaps.[cur].Pos
        for i in 0 .. n - 1 do
            p.[2 * i] <- x.[i]
            p.[2 * i + 1] <- y.[i]
    member _.Current = snaps.[cur]

type ShapesRender(model: ShapesModel, f: ShapesFeatures, clipW: float32, clipH: float32, shaderPerDraw: bool) =
    let n = model.N
    // gradient : un shader par forme (couleurs propres), cree une fois, en coordonnees locales
    let gradPaints : SKPaint[] =
        if f.Gradient && not shaderPerDraw then
            Array.init n (fun i ->
                let s = model.Sizes.[i]
                let (r1, g1, b1) = rgbOf (i * 2 + 1)
                let (r2, g2, b2) = rgbOf (i * 2 + 2)
                use sh = SKShader.CreateLinearGradient(SKPoint(0.0f, 0.0f), SKPoint(s, s), [| SKColor(r1, g1, b1); SKColor(r2, g2, b2) |], [| 0.0f; 1.0f |], SKShaderTileMode.Clamp)
                new SKPaint(IsAntialias = true, Shader = sh))
        else null
    let solid = new SKPaint(IsAntialias = true)
    // ombre portee : flou 8 px => sigma 4 (convention Avalonia BoxShadow : sigma = blur/2), offset (2,2)
    let shadowP = new SKPaint(IsAntialias = true, Color = SKColor(0uy, 0uy, 0uy, 120uy), MaskFilter = SKMaskFilter.CreateBlur(SKBlurStyle.Normal, 4.0f))
    let bgP = new SKPaint(Color = SKColor(24uy, 28uy, 38uy))
    let clipBg = new SKPaint(Color = SKColor(238uy, 240uy, 246uy))
    let clipRR = new SKRoundRect(SKRect(0.0f, 0.0f, clipW, clipH), 16.0f, 16.0f)
    member this.Shape(c: SKCanvas, k: int, x: float32, y: float32, s: float32, p: SKPaint) =
        match k with
        | 0 -> c.DrawRect(x, y, s, s * 0.7f, p)
        | 1 -> c.DrawCircle(x + s * 0.5f, y + s * 0.5f, s * 0.5f, p)
        | _ -> c.DrawRoundRect(x, y, s, s * 0.8f, s * 0.25f, s * 0.25f, p)
    member private this.OneShape(c: SKCanvas, i: int, px: float32, py: float32) =
        let s = model.Sizes.[i]
        let k = model.Kinds.[i]
        if f.Shadow then this.Shape(c, k, px + 2.0f, py + 2.0f, s, shadowP)
        if f.Gradient && shaderPerDraw then
            // v0 naive (comparaison) : un shader et un SKPaint crees puis detruits a chaque dessin de forme
            let (r1, g1, b1) = rgbOf (i * 2 + 1)
            let (r2, g2, b2) = rgbOf (i * 2 + 2)
            use sh = SKShader.CreateLinearGradient(SKPoint(px, py), SKPoint(px + s, py + s), [| SKColor(r1, g1, b1); SKColor(r2, g2, b2) |], [| 0.0f; 1.0f |], SKShaderTileMode.Clamp)
            use pt = new SKPaint(IsAntialias = true, Shader = sh)
            this.Shape(c, k, px, py, s, pt)
        elif f.Gradient then
            // shader en coordonnees locales : translation du canvas (pas de shader recree par frame)
            c.Translate(px, py)
            this.Shape(c, k, 0.0f, 0.0f, s, gradPaints.[i])
            c.Translate(-px, -py)
        else
            let (r, g, b) = rgbOf (i * 2 + 1)
            solid.Color <- SKColor(r, g, b)
            this.Shape(c, k, px, py, s, solid)
    member this.Draw(c: SKCanvas, snap: Snap) =
        let aw = model.AreaW
        let ah = model.AreaH
        c.DrawRect(0.0f, 0.0f, aw + 0.0f, ah + 0.0f, bgP)
        if f.Clip then
            c.Save() |> ignore
            if f.ClipCornerOpt then
                // variante optimisee : clip rectangle (bords entiers = meme couverture) + clip rrect AA seulement pour les formes touchant un coin
                c.ClipRect(SKRect(0.0f, 0.0f, clipW, clipH), SKClipOperation.Intersect, true)
                c.Save() |> ignore
                c.ClipRoundRect(clipRR, SKClipOperation.Intersect, true)
                c.DrawRect(0.0f, 0.0f, clipW, clipH, clipBg)
                c.Restore()
                for i in 0 .. n - 1 do
                    let px = snap.Pos.[2 * i]
                    let py = snap.Pos.[2 * i + 1]
                    let s = model.Sizes.[i]
                    let m = if f.Shadow then 14.0f else 1.0f
                    let x0 = px - m
                    let y0 = py - m
                    let x1 = px + s + m
                    let y1 = py + s + m
                    let corner = (x0 < 16.0f || x1 > clipW - 16.0f) && (y0 < 16.0f || y1 > clipH - 16.0f)
                    if corner then
                        c.Save() |> ignore
                        c.ClipRoundRect(clipRR, SKClipOperation.Intersect, true)
                    this.OneShape(c, i, px, py)
                    if corner then c.Restore()
            else
                // naturel : un seul clip rect arrondi anti-cretele pour toute la scene
                c.ClipRoundRect(clipRR, SKClipOperation.Intersect, true)
                c.DrawRect(0.0f, 0.0f, clipW, clipH, clipBg)
                for i in 0 .. n - 1 do
                    this.OneShape(c, i, snap.Pos.[2 * i], snap.Pos.[2 * i + 1])
            c.Restore()
        else
            for i in 0 .. n - 1 do
                this.OneShape(c, i, snap.Pos.[2 * i], snap.Pos.[2 * i + 1])

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

type ShapesView(model: ShapesModel, r: ShapesRender) as this =
    inherit Control()
    do this.ClipToBounds <- true
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(new ShapesOp(Rect(0.0, 0.0, float model.AreaW, float model.AreaH), r, model.Current))

/// Voie "avalonia" : primitives du DrawingContext public (DrawRectangle + BoxShadows + brosses immuables), sans Skia direct
type AvaloniaShapesView(model: ShapesModel, f: ShapesFeatures, clipW: float, clipH: float) as this =
    inherit Control()
    let n = model.N
    let brushes : IBrush[] =
        Array.init n (fun i ->
            let (r1, g1, b1) = rgbOf (i * 2 + 1)
            if f.Gradient then
                let (r2, g2, b2) = rgbOf (i * 2 + 2)
                ImmutableLinearGradientBrush([| ImmutableGradientStop(0.0, Color.FromRgb(r1, g1, b1)); ImmutableGradientStop(1.0, Color.FromRgb(r2, g2, b2)) |],
                                             1.0, null, Nullable<RelativePoint>(), GradientSpreadMethod.Pad, Nullable(RelativePoint.TopLeft), Nullable(RelativePoint.BottomRight)) :> IBrush
            else ImmutableSolidColorBrush(Color.FromRgb(r1, g1, b1)) :> IBrush)
    let shadows = BoxShadows(BoxShadow(OffsetX = 2.0, OffsetY = 2.0, Blur = 8.0, Spread = 0.0, Color = Color.FromArgb(120uy, 0uy, 0uy, 0uy)))
    let bg = ImmutableSolidColorBrush(Color.FromRgb(24uy, 28uy, 38uy))
    let clipBg = ImmutableSolidColorBrush(Color.FromRgb(238uy, 240uy, 246uy))
    do this.ClipToBounds <- true
    override this.Render(ctx: DrawingContext) =
        let snap = model.Current
        ctx.DrawRectangle(bg, null, Rect(0.0, 0.0, float model.AreaW, float model.AreaH))
        use clip = if f.Clip then ctx.PushClip(RoundedRect(Rect(0.0, 0.0, clipW, clipH), 16.0)) else Unchecked.defaultof<_>
        if f.Clip then ctx.DrawRectangle(clipBg, null, Rect(0.0, 0.0, clipW, clipH))
        for i in 0 .. n - 1 do
            let s = float model.Sizes.[i]
            let x = float snap.Pos.[2 * i]
            let y = float snap.Pos.[2 * i + 1]
            let rr =
                match model.Kinds.[i] with
                | 0 -> RoundedRect(Rect(x, y, s, s * 0.7))
                | 1 -> RoundedRect(Rect(x, y, s, s), s * 0.5)
                | _ -> RoundedRect(Rect(x, y, s, s * 0.8), s * 0.25)
            if f.Shadow then ctx.DrawRectangle(brushes.[i], null, rr, shadows) else ctx.DrawRectangle(brushes.[i], null, rr)

/// G1 : N formes animees, variantes feature=solid|gradient|shadow|clip|all
type ShapesScn(args: Args) =
    inherit Scenario()
    let feature = args.Variant("feature", "solid")
    let n = if args.NGiven then int args.N else 1000
    let f =
        { Gradient = (feature = "gradient" || feature = "all"); Shadow = (feature = "shadow" || feature = "all")
          Clip = (feature = "clip" || feature = "all"); ClipCornerOpt = (args.Variant("clipopt", "natural") = "corner") }
    let clipW, clipH = if f.Clip then 1000.0f, 600.0f else 1280.0f, 720.0f
    let model = ShapesModel(n, clipW, clipH)
    let impl = args.Variant("impl", "skia")
    let render = ShapesRender(model, f, clipW, clipH, (args.Variant("shader", "cached") = "perdraw"))
    let view : Control =
        if impl = "avalonia" then AvaloniaShapesView(model, f, float clipW, float clipH) :> Control
        else ShapesView(model, render) :> Control
    let mutable sq : Square = null
    do model.Reset()
    override _.Content = view
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        model.Step(float32 dt)
        view.InvalidateVisual()
    override _.Ops sb =
        kvS sb "impl" impl
        kvI sb "shapes" (int64 n)
        kvS sb "feature" feature
        kvRaw sb "not_rendered" "[]"
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i * 5 + 3))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "G1: " + feature + " : SKCanvas direct (lease Skia), formes rect/cercle/rect arrondi (1/3 chacune) rebondissant dans " + clipW.ToString(inv) + "x" + clipH.ToString(inv)
        + " ; " + (if f.Gradient then "degrade lineaire 2 couleurs par forme (SKShader cree une fois) ; " else "aplat (un SKPaint reutilise) ; ")
        + (if f.Shadow then "ombre portee = SKMaskFilter blur sigma 4 offset (2,2) sur CHAQUE forme ; " else "")
        + (if f.Clip then (if f.ClipCornerOpt then "clip rrect r=16 AA optimise (clip rect + rrect pour les formes aux coins) ; " else "clip SKRoundRect AA r=16 unique pour toute la scene ; ") else "")

// ---------------------------------------------------------------- G3 : courbe temps reel
let CHART_X = 140.0f
let CHART_Y = 380.0f
let CHART_W = 1000.0f
let CHART_H = 300.0f
let chartValue (k: int64) =
    0.5f + 0.35f * float32 (Math.Sin(float k * 0.0005)) + 0.1f * float32 (Math.Sin(float k * 0.0031)) + 0.04f * float32 (Math.Sin(float k * 0.37))

type ChartSnap(n: int) =
    member val Data : float32[] = Array.zeroCreate n

type ChartOp(bounds: Rect, snap: ChartSnap, n: int, pts: SKPoint[], bg: SKPaint, line: SKPaint) =
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
                c.DrawRect(CHART_X, CHART_Y, CHART_W, CHART_H, bg)
                let dx = CHART_W / float32 (n - 1)
                let d = snap.Data
                for k in 0 .. n - 1 do
                    pts.[k] <- SKPoint(CHART_X + float32 k * dx, CHART_Y + CHART_H * (1.0f - d.[k]))
                c.DrawPoints(SKPointMode.Polygon, pts, line)
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type ChartView(n: int) as this =
    inherit Control()
    let ring : float32[] = Array.zeroCreate n
    let mutable head = 0
    let mutable counter = 0L
    let snaps = [| ChartSnap(n); ChartSnap(n); ChartSnap(n) |]
    let mutable cur = 0
    let pts : SKPoint[] = Array.zeroCreate n
    let bg = new SKPaint(Color = SKColor(12uy, 14uy, 20uy))
    let line = new SKPaint(Color = SKColor(80uy, 220uy, 140uy), IsAntialias = true, Style = SKPaintStyle.Stroke, StrokeWidth = 1.0f)
    do
        this.ClipToBounds <- true
        // historique prerempli : la courbe occupe toute la largeur des le debut
        for k in 0 .. n - 1 do ring.[k] <- chartValue (int64 (k - n))
    member _.Added = counter
    member this.Append(count: int) =
        for _ in 1 .. count do
            counter <- counter + 1L
            ring.[head] <- chartValue counter
            head <- (head + 1) % n
        cur <- (cur + 1) % 3
        let d = snaps.[cur].Data
        Array.Copy(ring, head, d, 0, n - head)
        Array.Copy(ring, 0, d, n - head, head)
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(new ChartOp(Rect(0.0, 0.0, 1280.0, 720.0), snaps.[cur], n, pts, bg, line))

type ChartScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 10000
    let view = ChartView(n)
    let mutable sq : Square = null
    override _.Content = view :> Control
    override _.SetSquare(s) = sq <- s
    override _.Begin() = view.Append 0
    override _.OnFrame(t, dt) =
        view.Append 1000
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "points" (int64 n)
        kvI sb "points_added" view.Added
        kvRaw sb "not_rendered" "[]"
        checksumValue <- fnvSeq (int64 n) (fun i -> BitConverter.SingleToUInt32Bits(chartValue (i - int64 n)))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes = "G3: polyligne 1 px (SKCanvas.DrawPoints Polygon, AA) de N points dans 1000x300 en (140,380) ; 1000 points ajoutes par frame (anneau, instantane de N flottants par frame) ; SKPoint[] reutilise."

// ---------------------------------------------------------------- G2 : chemins de Bezier
type PathsOp(bounds: Rect, paths: SKPath[], dx: float32[], dy: float32[], dashed: bool, paint: SKPaint, bg: SKPaint) =
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
                c.DrawRect(0.0f, 0.0f, 1280.0f, 720.0f, bg)
                for i in 0 .. paths.Length - 1 do
                    c.Translate(dx.[i], dy.[i])
                    let (r, g, b) = rgbOf (i + 77)
                    paint.Color <- SKColor(r, g, b)
                    c.DrawPath(paths.[i], paint)
                    c.Translate(-dx.[i], -dy.[i])
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type PathsView(n: int, dashed: bool) as this =
    inherit Control()
    // N chemins de 8 segments cubiques ; geometrie construite une fois, animee par translation (le contenu change a chaque frame)
    let paths : SKPath[] =
        Array.init n (fun i ->
            let p = new SKPath()
            let bx = float32 (fnv (i * 3) % 1180u)
            let by = float32 (fnv (i * 3 + 1) % 620u)
            p.MoveTo(bx, by)
            let mutable cx = bx
            let mutable cy = by
            for s in 0 .. 7 do
                let h k = float32 (int (fnv (i * 64 + s * 8 + k) % 161u) - 80)
                p.CubicTo(cx + h 0, cy + h 1, cx + h 2, cy + h 3, cx + h 4 * 0.5f, cy + h 5 * 0.5f)
                cx <- cx + h 4 * 0.5f
                cy <- cy + h 5 * 0.5f
            p)
    let dx : float32[] = Array.zeroCreate n
    let dy : float32[] = Array.zeroCreate n
    let paint =
        new SKPaint(IsAntialias = true, Style = SKPaintStyle.Stroke, StrokeWidth = 2.0f,
                    PathEffect = (if dashed then SKPathEffect.CreateDash([| 8.0f; 4.0f |], 0.0f) else null))
    let bg = new SKPaint(Color = SKColor(250uy, 250uy, 252uy))
    do this.ClipToBounds <- true
    member this.Step(t: float) =
        for i in 0 .. n - 1 do
            dx.[i] <- float32 (6.0 * Math.Sin(t * 1.3 + float i))
            dy.[i] <- float32 (6.0 * Math.Cos(t * 0.9 + float i * 0.5))
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(new PathsOp(Rect(0.0, 0.0, 1280.0, 720.0), paths, dx, dy, dashed, paint, bg))

type PathsScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 100
    let dashed = (args.Variant("feature", "solid") = "dashed")
    let view = PathsView(n, dashed)
    let mutable sq : Square = null
    override _.Content = view :> Control
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        view.Step(max t 0.0)
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "paths" (int64 n)
        kvS sb "feature" (if dashed then "dashed" else "solid")
        kvRaw sb "not_rendered" "[]"
        checksumValue <- fnvSeq (int64 n * 64L) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes = "G2: N SKPath de 8 segments cubiques (largeur 2 px, AA), geometrie creee une fois, translation animee par chemin ; " + (if dashed then "SKPathEffect.CreateDash(8,4)." else "trait plein.")

// ---------------------------------------------------------------- G4 : images
type ImagesOp(bounds: Rect, imgs: SKImage[], n: int, t: float32, paint: SKPaint, bg: SKPaint) =
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
                c.DrawRect(0.0f, 0.0f, 1280.0f, 720.0f, bg)
                let sampling = SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear)
                for i in 0 .. n - 1 do
                    let img = imgs.[i % imgs.Length]
                    let fx = float32 (fnv (i * 7) % 1180u)
                    let fy = float32 (fnv (i * 7 + 1) % 620u)
                    let ph = float32 (fnv (i * 7 + 2) % 628u) / 100.0f
                    let scale = 0.25f + 0.2f * float32 (Math.Sin(float (t * 0.9f + ph)))
                    let rot = (t * 30.0f + ph * 57.0f) % 360.0f
                    paint.Color <- SKColor(255uy, 255uy, 255uy, byte (140.0f + 100.0f * float32 (Math.Sin(float (t * 1.7f + ph)))))
                    let s = 256.0f * scale
                    c.Save() |> ignore
                    c.Translate(fx + 64.0f, fy + 64.0f)
                    c.RotateDegrees(rot)
                    c.DrawImage(img, SKRect(-s / 2.0f, -s / 2.0f, s / 2.0f, s / 2.0f), sampling, paint)
                    c.Restore()
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type ImagesBigOp(bounds: Rect, img: SKImage, t: float32, paint: SKPaint, bg: SKPaint) =
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
                c.DrawRect(0.0f, 0.0f, 1280.0f, 720.0f, bg)
                let sampling = SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear)
                // pan / zoom / rotation : l'image remplit la fenetre a l'echelle ~1 puis oscille entre 0.15x et 1.2x
                let zoom = 0.65f + 0.5f * float32 (Math.Sin(float (t * 0.5f)))
                let w = float32 img.Width
                c.Save() |> ignore
                c.Translate(640.0f + 200.0f * float32 (Math.Sin(float (t * 0.7f))), 360.0f + 120.0f * float32 (Math.Cos(float (t * 0.4f))))
                c.RotateDegrees(t * 8.0f)
                c.Scale(zoom * 1280.0f / w)
                c.DrawImage(img, -w / 2.0f, -w / 2.0f, sampling, paint)
                c.Restore()
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type ImagesView(n: int, imgs: SKImage[], big: SKImage) as this =
    inherit Control()
    let paint = new SKPaint(IsAntialias = true)
    let bg = new SKPaint(Color = SKColor(250uy, 250uy, 252uy))
    do this.ClipToBounds <- true
    member val T = 0.0f with get, set
    override this.Render(ctx: DrawingContext) =
        if isNull big then ctx.Custom(new ImagesOp(Rect(0.0, 0.0, 1280.0, 720.0), imgs, n, this.T, paint, bg))
        else ctx.Custom(new ImagesBigOp(Rect(0.0, 0.0, 1280.0, 720.0), big, this.T, paint, bg))

type ImagesScn(args: Args) =
    inherit Scenario()
    let isBig = (args.Variant("feature", "") = "big")
    let n = if args.NGiven then int args.N else 10
    let size = if isBig then (if n >= 8192 then 8192 else 4096) else 256
    let t0 = Stopwatch.GetTimestamp()
    let imgs : SKImage[] =
        if isBig then null
        else
            Array.init 600 (fun i ->
                use bmp = SKBitmap.Decode(Path.Combine(args.Assets, "images", "img" + i.ToString("D3", inv) + ".png"))
                SKImage.FromBitmap bmp)
    let big : SKImage =
        if isBig then
            use bmp = SKBitmap.Decode(Path.Combine(args.Assets, (if size = 8192 then "big8192.png" else "big4096.png")))
            SKImage.FromBitmap bmp
        else null
    let decodeMs = ticksToMs (Stopwatch.GetTimestamp() - t0)
    let view = ImagesView(n, imgs, big)
    let mutable sq : Square = null
    override _.Content = view :> Control
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        view.T <- float32 (max t 0.0)
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "images" (int64 n)
        kvF sb "decode_total_ms" decodeMs
        kvS sb "feature" (if isBig then "big" else "")
        kvI sb "big_size" (if isBig then int64 size else 0L)
        kvRaw sb "not_rendered" "[]"
        checksumValue <- fnvSeq 600L (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        if isBig then "G4 big: image " + size.ToString(inv) + "^2 (PNG decode une fois -> SKImage), pan/zoom/rotation, echantillonnage lineaire + mipmaps (Skia). n>=8192 selectionne 8192^2, sinon 4096^2."
        else "G4: N images 256^2 (600 PNG decodes une fois en SKImage, cycle i%600), echelle/rotation/opacite deterministes variant a chaque frame, SKCanvas.DrawImage (echantillonnage lineaire + mipmaps)."

// ---------------------------------------------------------------- G5 : groupes semi-transparents (+ flou)
/// groupe = carte 200x150 : fond colore + 4 rects arrondis + 1 cercle ; opacite 0,5 sur le groupe ; blur = flou 4 px sur le groupe
type LayersOp(bounds: Rect, n: int, t: float32, blur: bool, bg: SKPaint, layerPaint: SKPaint, p: SKPaint) =
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
                c.DrawRect(0.0f, 0.0f, 1280.0f, 720.0f, bg)
                for i in 0 .. n - 1 do
                    let gx = 40.0f + float32 (fnv (i * 5) % 1000u) + 30.0f * float32 (Math.Sin(float (t + float32 i)))
                    let gy = 40.0f + float32 (fnv (i * 5 + 1) % 500u) + 30.0f * float32 (Math.Cos(float (t * 0.8f + float32 i)))
                    c.Save() |> ignore
                    c.Translate(gx, gy)
                    c.SaveLayer(layerPaint) |> ignore
                    let (r, g, b) = rgbOf (i * 9)
                    p.Color <- SKColor(r, g, b)
                    c.DrawRoundRect(0.0f, 0.0f, 200.0f, 150.0f, 12.0f, 12.0f, p)
                    for k in 1 .. 4 do
                        let (r, g, b) = rgbOf (i * 9 + k)
                        p.Color <- SKColor(r, g, b)
                        c.DrawRoundRect(float32 (10 + (k - 1) % 2 * 90), float32 (10 + (k - 1) / 2 * 65), 80.0f, 55.0f, 8.0f, 8.0f, p)
                    p.Color <- SKColors.White
                    c.DrawCircle(100.0f, 75.0f, 20.0f, p)
                    c.Restore()
                    c.Restore()
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

type LayersView(n: int, blur: bool) as this =
    inherit Control()
    let bg = new SKPaint(Color = SKColor(250uy, 250uy, 252uy))
    // groupe = SaveLayer avec paint alpha 0,5 (+ SKImageFilter blur 4 px)
    let layerPaint =
        new SKPaint(Color = SKColor(255uy, 255uy, 255uy, 128uy),
                    ImageFilter = (if blur then SKImageFilter.CreateBlur(4.0f, 4.0f) else null))
    let p = new SKPaint(IsAntialias = true)
    do this.ClipToBounds <- true
    member val T = 0.0f with get, set
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(new LayersOp(Rect(0.0, 0.0, 1280.0, 720.0), n, this.T, blur, bg, layerPaint, p))

type LayersScn(args: Args) =
    inherit Scenario()
    let blur = (args.Variant("feature", "") = "blur")
    let n = if args.NGiven then int args.N else 4
    let view = LayersView(n, blur)
    let mutable sq : Square = null
    override _.Content = view :> Control
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        view.T <- float32 (max t 0.0)
        view.InvalidateVisual()
    override _.Ops sb =
        kvI sb "groups" (int64 n)
        kvS sb "feature" (if blur then "blur" else "")
        kvRaw sb "not_rendered" "[]"
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i * 5))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes = "G5: N groupes 200x150 (fond + 4 rects arrondis + cercle) en SaveLayer(alpha 0,5" + (if blur then " + SKImageFilter.CreateBlur(4)" else "") + "), derive animee."
