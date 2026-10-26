module ListsCore
// Listes virtualisees : modele (RowStore), rendu canvas Skia (ICustomDrawOperation), panneau virtualise de controles pooles.
// Les trois voies (canvas / panel / vsp) partagent le meme modele et la meme definition de cellule.

open System
open System.Collections.Generic
open System.Diagnostics
open System.Text
open System.Threading
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.Primitives
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Platform
open Avalonia.Rendering.SceneGraph
open Avalonia.Skia
open SkiaSharp
open Util
open Harness

// ---------------------------------------------------------------- modele
/// Lignes d'une liste. Mode statique : aucune table par ligne (hauteurs recalculees par fnv, prefixe par blocs de 64 :
/// O(N/64) octets). Mode mutable : tableaux ids/hauteurs/prix + prefixe complet recalcule une fois par lot
/// (insertion/suppression/deplacement + mises a jour de prix : L3, F2).
[<AllowNullLiteral>]
type RowStore(n: int, dense: bool, mutableMode: bool) =
    let blk = 64
    let mutable count = n
    let mutable ids : int[] = null
    let mutable hs : int[] = null
    let mutable prices : float[] = null
    let mutable prefix : int64[] = null     // mutable : prefix.[p] = haut de la ligne p ; statique : prefix.[b] = haut de la ligne b*64
    let mutable nextId = n
    let mutable dirtyFrom = Int32.MaxValue
    let mutable version = 0
    let hf (id: int) = if dense then hDense id else hStd id
    let rebuild (from: int) =
        // recalcul du prefixe a partir de la ligne `from`
        if mutableMode then
            if prefix.Length < count + 1 then Array.Resize(&prefix, max (count + 1) (prefix.Length * 2))
            let mutable acc = if from = 0 then 0L else prefix.[from]
            for p in from .. count - 1 do
                prefix.[p] <- acc
                acc <- acc + int64 hs.[p]
            prefix.[count] <- acc
    do
        if mutableMode then
            // marge de croissance : insertions L3 (N/100 par seconde pendant 14 s)
            let cap = n + max 1024 (n / 4)
            ids <- Array.zeroCreate cap
            hs <- Array.zeroCreate cap
            prices <- Array.zeroCreate cap
            prefix <- Array.zeroCreate (cap + 1)
            for i in 0 .. n - 1 do
                ids.[i] <- i
                hs.[i] <- hf i
                prices.[i] <- initialPrice i
            rebuild 0
        else
            let nb = (n + blk - 1) / blk
            prefix <- Array.zeroCreate (nb + 1)
            let mutable acc = 0L
            for b in 0 .. nb - 1 do
                prefix.[b] <- acc
                let hi = min n ((b + 1) * blk)
                for i in b * blk .. hi - 1 do acc <- acc + int64 (hf i)
            prefix.[nb] <- acc
    member _.Count = count
    member _.Dense = dense
    member _.Mutable = mutableMode
    member _.Version = version
    member _.Total : int64 = if mutableMode then prefix.[count] else prefix.[(n + blk - 1) / blk]
    member _.Id(p: int) = if mutableMode then ids.[p] else p
    member _.Height(p: int) = if mutableMode then hs.[p] else hf p
    member _.Price(p: int) = if mutableMode then prices.[p] else initialPrice p
    /// haut de la ligne p (px depuis le debut du contenu)
    member this.Top(p: int) : int64 =
        if mutableMode then prefix.[p]
        else
            let b = p / blk
            let mutable t = prefix.[b]
            for i in b * blk .. p - 1 do t <- t + int64 (hf i)
            t
    /// plus grand p tel que Top(p) <= y
    member this.RowAt(y: float) : int =
        if mutableMode then
            let mutable lo = 0
            let mutable hi = count - 1
            while lo < hi do
                let mid = (lo + hi + 1) >>> 1
                if float prefix.[mid] <= y then lo <- mid else hi <- mid - 1
            lo
        else
            let nb = (n + blk - 1) / blk
            let mutable lo = 0
            let mutable hi = nb - 1
            while lo < hi do
                let mid = (lo + hi + 1) >>> 1
                if float prefix.[mid] <= y then lo <- mid else hi <- mid - 1
            let mutable p = lo * blk
            let mutable t = prefix.[lo]
            let last = min (n - 1) (lo * blk + blk - 1)
            while p < last && float (t + int64 (hf p)) <= y do
                t <- t + int64 (hf p)
                p <- p + 1
            p
    member _.SetPrice(p: int, v: float) =
        prices.[p] <- v
        version <- version + 1
    member _.NextId() = let i = nextId in nextId <- nextId + 1; i
    member _.Insert(p: int, v: float) =
        // une ligne neuve (identite nextId) a la position p
        if count + 1 > ids.Length then
            let cap = ids.Length * 2
            Array.Resize(&ids, cap); Array.Resize(&hs, cap); Array.Resize(&prices, cap); Array.Resize(&prefix, cap + 1)
        Array.Copy(ids, p, ids, p + 1, count - p)
        Array.Copy(hs, p, hs, p + 1, count - p)
        Array.Copy(prices, p, prices, p + 1, count - p)
        let id = nextId
        nextId <- nextId + 1
        ids.[p] <- id
        hs.[p] <- hf id
        prices.[p] <- v
        count <- count + 1
        dirtyFrom <- min dirtyFrom p
        version <- version + 1
    member _.RemoveAt(p: int) =
        Array.Copy(ids, p + 1, ids, p, count - p - 1)
        Array.Copy(hs, p + 1, hs, p, count - p - 1)
        Array.Copy(prices, p + 1, prices, p, count - p - 1)
        count <- count - 1
        dirtyFrom <- min dirtyFrom p
        version <- version + 1
    member _.Move(a: int, b: int) =
        if a <> b then
            let id = ids.[a]
            let h = hs.[a]
            let pr = prices.[a]
            if a < b then
                Array.Copy(ids, a + 1, ids, a, b - a); Array.Copy(hs, a + 1, hs, a, b - a); Array.Copy(prices, a + 1, prices, a, b - a)
            else
                Array.Copy(ids, b, ids, b + 1, a - b); Array.Copy(hs, b, hs, b + 1, a - b); Array.Copy(prices, b, prices, b + 1, a - b)
            ids.[b] <- id; hs.[b] <- h; prices.[b] <- pr
            dirtyFrom <- min dirtyFrom (min a b)
            version <- version + 1
    /// fin d'un lot : recalcule le prefixe une fois
    member _.Commit() =
        if dirtyFrom <> Int32.MaxValue then
            rebuild (min dirtyFrom (count - 1) |> max 0)
            dirtyFrom <- Int32.MaxValue

// ---------------------------------------------------------------- chaines de cellule (communes aux 3 voies)
let titleOf (id: int) = "Item #" + id.ToString(inv)
let quoteOf (id: int) = "Quote " + id.ToString(inv) + " — " + (float (fnv (id + 7) % 100000u) / 100.0).ToString("F2", inv)
let subtitleStd (id: int) = let q = quoteOf id in q + "  " + q + "  " + q + "  " + q
let priceOf (p: float) = p.ToString("F2", inv)
let puceText (id: int) (k: int) =
    let h = fnv (id * 31 + k)
    String([| char (int 'a' + int (h % 26u)); char (int '0' + int ((h >>> 8) % 10u)) |])

// geometrie des cellules
let STD_W_RESERVE = 110.0
/// cellule dense : positions des elements de base (x, largeur)
module Dense =
    let pastX = 6.0
    let pastD = 16.0
    let titleX = 28.0
    let titleW = 90.0
    let subX = 122.0
    let subW = 150.0
    let progX = 280.0
    let progW = 80.0
    let togX = 372.0
    let togW = 28.0
    let priceX = 410.0
    let priceW = 80.0
    let puceStart = 500.0
    let puceEnd = 1270.0
    let pitch (np: int) = if np <= 0 then 30.0 else min 30.0 ((puceEnd - puceStart) / float np)

let nPuces (k: int) = max 0 ((k - 6) / 2)

// ---------------------------------------------------------------- ressources Skia partagees (creees une fois)
module Sk =
    let typeface =
        lazy (
            let tf = SKTypeface.FromFamilyName("Noto Sans")
            if isNull tf then SKTypeface.Default else tf)
    let typefaceBold =
        lazy (
            let tf = SKTypeface.FromFamilyName("Noto Sans", SKFontStyle.Bold)
            if isNull tf then typeface.Value else tf)
    let font = lazy (new SKFont(typeface.Value, 14.0f))
    let fontSmall = lazy (new SKFont(typeface.Value, 12.0f))
    let fontBold = lazy (new SKFont(typefaceBold.Value, 14.0f))
    let font12b = lazy (new SKFont(typefaceBold.Value, 12.0f))
    let font10 = lazy (new SKFont(typeface.Value, 10.0f))
    let font9 = lazy (new SKFont(typeface.Value, 9.0f))
    let font8 = lazy (new SKFont(typeface.Value, 8.0f))
    let listBg = lazy (new SKPaint(Color = SKColor(250uy, 250uy, 252uy)))
    let txt = lazy (new SKPaint(IsAntialias = true, Color = SKColors.Black))
    let sub = lazy (new SKPaint(IsAntialias = true, Color = SKColor(110uy, 110uy, 120uy)))
    let sep = lazy (new SKPaint(Color = SKColor(225uy, 225uy, 230uy)))
    let priceP = lazy (new SKPaint(IsAntialias = true, Color = SKColor(20uy, 110uy, 40uy)))
    let track = lazy (new SKPaint(IsAntialias = true, Color = SKColor(222uy, 224uy, 232uy)))
    let fill = lazy (new SKPaint(IsAntialias = true, Color = SKColor(60uy, 120uy, 220uy)))
    let white = lazy (new SKPaint(IsAntialias = true, Color = SKColors.White))
    let onBg = lazy (new SKPaint(IsAntialias = true, Color = SKColor(60uy, 170uy, 90uy)))
    let offBg = lazy (new SKPaint(IsAntialias = true, Color = SKColor(170uy, 170uy, 180uy)))

// ---------------------------------------------------------------- voie 1 : canvas Skia (ICustomDrawOperation)
/// Instantane des lignes visibles, pris sur le thread UI (le thread de rendu dessine pendant que l'UI prepare la frame suivante)
type RowSnap() =
    member val Ids : int[] = Array.zeroCreate 256 with get, set
    member val Prices : float[] = Array.zeroCreate 256 with get, set
    member val Ys : float32[] = Array.zeroCreate 256 with get, set
    member val Hs : float32[] = Array.zeroCreate 256 with get, set
    member val Count = 0 with get, set
    member this.Ensure(n: int) =
        if this.Ids.Length < n then
            let m = n * 2
            this.Ids <- Array.zeroCreate m; this.Prices <- Array.zeroCreate m; this.Ys <- Array.zeroCreate m; this.Hs <- Array.zeroCreate m

let drawStdRow (c: SKCanvas) (dot: SKPaint) (w: float32) (y: float32) (h: float32) (id: int) (price: float) =
    c.Save() |> ignore
    c.ClipRect(SKRect(0.0f, y, w, y + h))
    c.DrawRect(0.0f, y + h - 1.0f, w, 1.0f, Sk.sep.Value)
    let (r, g, b) = rgbOf id
    dot.Color <- SKColor(r, g, b)
    c.DrawCircle(32.0f, y + h * 0.5f, 16.0f, dot)
    c.DrawText(titleOf id, 64.0f, y + 22.0f, Sk.fontBold.Value, Sk.txt.Value)
    c.Save() |> ignore
    c.ClipRect(SKRect(0.0f, y, w - float32 STD_W_RESERVE, y + h))
    c.DrawText(subtitleStd id, 64.0f, y + 42.0f, Sk.fontSmall.Value, Sk.sub.Value)
    c.Restore()
    c.DrawText(priceOf price, w - 16.0f, y + h * 0.5f + 5.0f, SKTextAlign.Right, Sk.font.Value, Sk.priceP.Value)
    c.Restore()

let drawDenseRow (c: SKCanvas) (dot: SKPaint) (w: float32) (y: float32) (h: float32) (id: int) (price: float) (np: int) =
    c.Save() |> ignore
    c.ClipRect(SKRect(0.0f, y, w, y + h))
    c.DrawRect(0.0f, y + h - 1.0f, w, 1.0f, Sk.sep.Value)
    let (r, g, b) = rgbOf id
    dot.Color <- SKColor(r, g, b)
    c.DrawCircle(float32 (Dense.pastX + Dense.pastD / 2.0), y + h * 0.5f, float32 (Dense.pastD / 2.0), dot)
    c.DrawText(titleOf id, float32 Dense.titleX, y + 14.0f, Sk.font12b.Value, Sk.txt.Value)
    // sous-titre : 2 lignes, ellipse (texte coupe a la largeur : comparable a MaxLines=2 + CharacterEllipsis)
    let q = quoteOf id
    c.Save() |> ignore
    c.ClipRect(SKRect(float32 Dense.subX, y, float32 (Dense.subX + Dense.subW), y + h))
    c.DrawText(q, float32 Dense.subX, y + 10.0f, Sk.font9.Value, Sk.sub.Value)
    c.DrawText(q + " …", float32 Dense.subX, y + 21.0f, Sk.font9.Value, Sk.sub.Value)
    c.Restore()
    // barre de progression
    let frac = float32 (fnv (id + 3) % 100u) / 100.0f
    let py = y + h * 0.5f - 3.0f
    c.DrawRoundRect(float32 Dense.progX, py, float32 Dense.progW, 6.0f, 3.0f, 3.0f, Sk.track.Value)
    c.DrawRoundRect(float32 Dense.progX, py, float32 Dense.progW * frac, 6.0f, 3.0f, 3.0f, Sk.fill.Value)
    // bascule : rect arrondi + cercle
    let on = (fnv (id + 5) &&& 1u) = 1u
    let ty = y + h * 0.5f - 7.0f
    c.DrawRoundRect(float32 Dense.togX, ty, float32 Dense.togW, 14.0f, 7.0f, 7.0f, (if on then Sk.onBg.Value else Sk.offBg.Value))
    c.DrawCircle(float32 Dense.togX + (if on then float32 Dense.togW - 7.0f else 7.0f), ty + 7.0f, 5.0f, Sk.white.Value)
    c.DrawText(priceOf price, float32 (Dense.priceX + Dense.priceW), y + h * 0.5f + 4.0f, SKTextAlign.Right, Sk.font12b.Value, Sk.priceP.Value)
    // puces : rect arrondi colore + texte court
    if np > 0 then
        let pitch = float32 (Dense.pitch np)
        let mutable px = float32 Dense.puceStart
        for k in 0 .. np - 1 do
            let (r, g, b) = rgbOf (id * 131 + k)
            dot.Color <- SKColor(r, g, b)
            c.DrawRoundRect(px, y + h * 0.5f - 7.0f, pitch - 4.0f, 14.0f, 4.0f, 4.0f, dot)
            c.DrawText(puceText id k, px + 2.0f, y + h * 0.5f + 3.0f, Sk.font8.Value, Sk.txt.Value)
            px <- px + pitch
    c.Restore()

type RowsOp(bounds: Rect, snap: RowSnap, dense: bool, np: int) =
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
                use dot = new SKPaint(IsAntialias = true)
                for k in 0 .. snap.Count - 1 do
                    if dense then drawDenseRow c dot w snap.Ys.[k] snap.Hs.[k] snap.Ids.[k] snap.Prices.[k] np
                    else drawStdRow c dot w snap.Ys.[k] snap.Hs.[k] snap.Ids.[k] snap.Prices.[k]
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

/// Voie canvas : un seul Control, rendu par ICustomDrawOperation (idiome documente de rendu personnalise)
type CanvasListView(store0: RowStore, np: int) as this =
    inherit Control()
    let mutable store = store0
    let snaps = [| RowSnap(); RowSnap(); RowSnap() |]
    let mutable cur = 0
    do this.ClipToBounds <- true
    member val Offset = 0.0 with get, set
    member val FirstRow = 0 with get, set
    member val RowsDrawn = 0L with get, set
    member _.Store with get () = store and set (v: RowStore) = store <- v
    member this.MaxOffset = max 0.0 (float store.Total - this.Bounds.Height)
    override this.Render(ctx: DrawingContext) =
        let b = Rect(0.0, 0.0, this.Bounds.Width, this.Bounds.Height)
        let off = max 0.0 (min this.Offset this.MaxOffset)
        let first = store.RowAt off
        this.FirstRow <- first
        cur <- (cur + 1) % 3
        let s = snaps.[cur]
        let mutable p = first
        let mutable y = float (store.Top first) - off
        let mutable k = 0
        let cnt = store.Count
        while p < cnt && y < b.Height do
            s.Ensure(k + 1)
            let h = store.Height p
            s.Ids.[k] <- store.Id p
            s.Prices.[k] <- store.Price p
            s.Ys.[k] <- float32 y
            s.Hs.[k] <- float32 h
            y <- y + float h
            p <- p + 1
            k <- k + 1
        s.Count <- k
        this.RowsDrawn <- this.RowsDrawn + int64 k
        ctx.Custom(new RowsOp(b, s, store.Dense, np))

// ---------------------------------------------------------------- voie 2 : panneau virtualise de controles pooles
let fontFamily = FontFamily("Noto Sans")
let inline brush (r: byte, g: byte, b: byte) = SolidColorBrush(Color.FromRgb(r, g, b))

/// Element de liste (voie vsp) : view-model leger INotifyPropertyChanged (le prix change par flux)
[<AllowNullLiteral>]
type RowItem(id: int, h: int, price0: float) =
    static let pcArgs = System.ComponentModel.PropertyChangedEventArgs("Price")
    let ev = Event<System.ComponentModel.PropertyChangedEventHandler, System.ComponentModel.PropertyChangedEventArgs>()
    let mutable price = price0
    member _.Id = id
    member _.Height = h
    member this.Price
        with get () = price
        and set (v: float) =
            if v <> price then
                price <- v
                ev.Trigger(this, pcArgs)
    interface System.ComponentModel.INotifyPropertyChanged with
        [<CLIEvent>]
        member _.PropertyChanged = ev.Publish

/// Une ligne retenue : controles enfants a positions fixes (pas de Grid : layout O(enfants) sans resolution de tailles)
type RowView(dense: bool, np: int) as this =
    inherit Panel()
    let pastBrush = SolidColorBrush(Colors.Black)
    let past = Border(Background = pastBrush)
    let title = TextBlock(FontFamily = fontFamily, FontWeight = FontWeight.Bold, FontSize = (if dense then 12.0 else 14.0), Foreground = Brushes.Black)
    let sub = TextBlock(FontFamily = fontFamily, FontSize = (if dense then 9.0 else 12.0), Foreground = SolidColorBrush(Color.FromRgb(110uy, 110uy, 120uy)),
                        TextWrapping = (if dense then TextWrapping.Wrap else TextWrapping.NoWrap), TextTrimming = TextTrimming.CharacterEllipsis,
                        MaxLines = (if dense then 2 else 1))
    let priceT = TextBlock(FontFamily = fontFamily, FontSize = (if dense then 12.0 else 14.0), FontWeight = (if dense then FontWeight.Bold else FontWeight.Normal),
                           Foreground = SolidColorBrush(Color.FromRgb(20uy, 110uy, 40uy)), TextAlignment = TextAlignment.Right)
    let sepB = Border(Background = SolidColorBrush(Color.FromRgb(225uy, 225uy, 230uy)), Height = 1.0)
    // dense uniquement
    let progTrack = Border(Background = SolidColorBrush(Color.FromRgb(222uy, 224uy, 232uy)), CornerRadius = CornerRadius(3.0))
    let progFill = Border(Background = SolidColorBrush(Color.FromRgb(60uy, 120uy, 220uy)), CornerRadius = CornerRadius(3.0), HorizontalAlignment = HorizontalAlignment.Left)
    let togBg = Border(Background = SolidColorBrush(Color.FromRgb(170uy, 170uy, 180uy)), CornerRadius = CornerRadius(7.0))
    let togKnob = Border(Background = Brushes.White, CornerRadius = CornerRadius(5.0), Width = 10.0, Height = 10.0)
    let puceBrushes : SolidColorBrush[] = Array.init np (fun _ -> SolidColorBrush(Colors.Black))
    let puceRects : Border[] = Array.init np (fun k -> Border(Background = puceBrushes.[k], CornerRadius = CornerRadius(4.0)))
    let puceTexts : TextBlock[] = Array.init np (fun _ -> TextBlock(FontFamily = fontFamily, FontSize = 8.0, Foreground = Brushes.Black))
    let rects : Rect[] = Array.zeroCreate (if dense then 9 + 2 * np else 5)
    let onBrush = SolidColorBrush(Color.FromRgb(60uy, 170uy, 90uy))
    let offBrush = SolidColorBrush(Color.FromRgb(170uy, 170uy, 180uy))
    let mutable boundId = -1
    let mutable boundPrice = nan
    let mutable boundH = -1.0
    let mutable laidH = -1.0
    let mutable laidW = -1.0
    let mutable on = false
    let mutable item : RowItem = null
    let onItemChanged = System.ComponentModel.PropertyChangedEventHandler(fun _ _ ->
        if not (isNull item) && item.Price <> boundPrice then
            priceT.Text <- priceOf item.Price
            boundPrice <- item.Price)
    do
        this.ClipToBounds <- true
        this.Children.Add past
        this.Children.Add title
        this.Children.Add sub
        this.Children.Add priceT
        this.Children.Add sepB
        if dense then
            this.Children.Add progTrack
            this.Children.Add progFill
            this.Children.Add togBg
            this.Children.Add togKnob
            for k in 0 .. np - 1 do
                this.Children.Add puceRects.[k]
                this.Children.Add puceTexts.[k]
    member _.BoundId = boundId
    member _.BoundPrice = boundPrice
    /// (re)lie la ligne : identite (textes, couleurs), prix, hauteur. Les rects sont calcules au Measure (largeur disponible).
    member this.Bind(id: int, price: float, h: float) =
        if id <> boundId then
            let (r, g, b) = rgbOf id
            pastBrush.Color <- Color.FromRgb(r, g, b)
            title.Text <- titleOf id
            if dense then
                sub.Text <- (let q = quoteOf id in q + "  " + q + "  " + q)
                progFill.Width <- Dense.progW * (float (fnv (id + 3) % 100u) / 100.0)
                on <- (fnv (id + 5) &&& 1u) = 1u
                togBg.Background <- (if on then onBrush else offBrush)
                for k in 0 .. np - 1 do
                    let (r, g, b) = rgbOf (id * 131 + k)
                    puceBrushes.[k].Color <- Color.FromRgb(r, g, b)
                    puceTexts.[k].Text <- puceText id k
                laidH <- -1.0   // la position du bouton depend de on
            else
                sub.Text <- subtitleStd id
            boundId <- id
        if price <> boundPrice then
            priceT.Text <- priceOf price
            boundPrice <- price
        if h <> boundH then
            boundH <- h
            this.InvalidateMeasure()
    override this.OnDataContextChanged(e: EventArgs) =
        base.OnDataContextChanged e
        // voie vsp : le conteneur recycle recoit un nouvel element (DataContext) -> rebind + abonnement au prix
        if not (isNull item) then (item :> System.ComponentModel.INotifyPropertyChanged).PropertyChanged.RemoveHandler onItemChanged
        item <- (match this.DataContext with :? RowItem as r -> r | _ -> null)
        if not (isNull item) then
            (item :> System.ComponentModel.INotifyPropertyChanged).PropertyChanged.AddHandler onItemChanged
            this.Bind(item.Id, item.Price, float item.Height)
    override this.MeasureOverride(avail: Size) =
        let w = if Double.IsInfinity avail.Width then 1280.0 else avail.Width
        let h = boundH
        if w <> laidW || h <> laidH then
            laidW <- w
            laidH <- h
            if dense then
                let cy = h * 0.5
                rects.[0] <- Rect(Dense.pastX, cy - Dense.pastD / 2.0, Dense.pastD, Dense.pastD)
                past.CornerRadius <- CornerRadius(Dense.pastD / 2.0)
                rects.[1] <- Rect(Dense.titleX, 1.0, Dense.titleW, 14.0)
                rects.[2] <- Rect(Dense.subX, 0.0, Dense.subW, 24.0)
                rects.[3] <- Rect(Dense.priceX, cy - 8.0, Dense.priceW, 16.0)
                rects.[4] <- Rect(0.0, h - 1.0, w, 1.0)
                rects.[5] <- Rect(Dense.progX, cy - 3.0, Dense.progW, 6.0)
                rects.[6] <- Rect(Dense.progX, cy - 3.0, progFill.Width, 6.0)
                rects.[7] <- Rect(Dense.togX, cy - 7.0, Dense.togW, 14.0)
                rects.[8] <- Rect(Dense.togX + (if on then Dense.togW - 12.0 else 2.0), cy - 5.0, 10.0, 10.0)
                let pitch = Dense.pitch np
                for k in 0 .. np - 1 do
                    let px = Dense.puceStart + float k * pitch
                    rects.[9 + 2 * k] <- Rect(px, cy - 7.0, pitch - 4.0, 14.0)
                    rects.[10 + 2 * k] <- Rect(px + 2.0, cy - 6.0, pitch - 4.0, 12.0)
            else
                rects.[0] <- Rect(16.0, h * 0.5 - 16.0, 32.0, 32.0)
                past.CornerRadius <- CornerRadius(16.0)
                rects.[1] <- Rect(64.0, 6.0, 400.0, 20.0)
                rects.[2] <- Rect(64.0, 28.0, max 10.0 (w - 64.0 - STD_W_RESERVE), 18.0)
                rects.[3] <- Rect(w - 16.0 - 100.0, h * 0.5 - 10.0, 100.0, 20.0)
                rects.[4] <- Rect(0.0, h - 1.0, w, 1.0)
        let ch = this.Children
        for i in 0 .. ch.Count - 1 do
            ch.[i].Measure(rects.[i].Size)
        Size(w, h)
    override this.ArrangeOverride(fin: Size) =
        let ch = this.Children
        for i in 0 .. ch.Count - 1 do
            ch.[i].Arrange rects.[i]
        fin

/// Panneau virtualise : pool fixe de RowView, positions exactes (prefixe des hauteurs), rebind seulement quand l'identite/prix change
type PanelListView(store0: RowStore, np: int) as this =
    inherit Panel()
    let mutable store = store0
    let dense = store0.Dense
    let minH = if dense then 24.0 else 48.0
    let mutable pool : RowView[] = null
    let mutable slotPos : int[] = null
    let mutable offset = 0.0
    let mutable first = 0
    let mutable lastCount = 0
    do this.ClipToBounds <- true
    member _.Offset
        with get () = offset
        and set (v: float) = offset <- v
    member _.FirstRow = first
    member _.Store with get () = store and set (v: RowStore) = store <- v
    member val RowsDrawn = 0L with get, set
    member val Rebinds = 0L with get, set
    member this.MaxOffset = max 0.0 (float store.Total - this.Bounds.Height)
    override this.MeasureOverride(avail: Size) =
        let h = if Double.IsInfinity avail.Height then 720.0 else avail.Height
        let w = if Double.IsInfinity avail.Width then 1280.0 else avail.Width
        let need = int (Math.Ceiling(h / minH)) + 2
        if isNull pool then
            pool <- Array.init need (fun _ -> RowView(dense, np))
            slotPos <- Array.create need -1
            for r in pool do this.Children.Add r
        let off = max 0.0 (min offset (max 0.0 (float store.Total - h)))
        first <- store.RowAt off
        let mutable p = first
        let mutable y = float (store.Top first) - off
        let mutable k = 0
        let cnt = store.Count
        while p < cnt && y < h && k < pool.Length do
            let slot = p % pool.Length
            let rv = pool.[slot]
            let hh = float (store.Height p)
            let id = store.Id p
            let pr = store.Price p
            if slotPos.[slot] <> p || rv.BoundId <> id || rv.BoundPrice <> pr then
                this.Rebinds <- this.Rebinds + 1L
            rv.Bind(id, pr, hh)
            slotPos.[slot] <- p
            rv.IsVisible <- true
            rv.Measure(Size(w, hh))
            y <- y + hh
            p <- p + 1
            k <- k + 1
        // lignes non utilisees : masquees
        for s in 0 .. pool.Length - 1 do
            let pp = slotPos.[s]
            if pp < first || pp >= p then
                if pool.[s].IsVisible then pool.[s].IsVisible <- false
                slotPos.[s] <- -1
        lastCount <- k
        this.RowsDrawn <- this.RowsDrawn + int64 k
        Size(w, h)
    override this.ArrangeOverride(fin: Size) =
        if not (isNull pool) then
            let off = max 0.0 (min offset (max 0.0 (float store.Total - fin.Height)))
            for s in 0 .. pool.Length - 1 do
                let pp = slotPos.[s]
                if pp >= 0 then
                    let rv = pool.[s]
                    let y = float (store.Top pp) - off
                    rv.Arrange(Rect(0.0, y, fin.Width, float (store.Height pp)))
        fin
