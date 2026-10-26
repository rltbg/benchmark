module Text
// T1 text-big, T2 text-labels, T3 text-edit, T4 text-rich

open System
open System.Diagnostics
open System.IO
open System.Text
open System.Threading
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.Documents
open Avalonia.Media
open Avalonia.Platform
open Avalonia.Rendering.SceneGraph
open Avalonia.Skia
open Avalonia.Threading
open Avalonia.VisualTree
open SkiaSharp
open Util
open Harness
open ListsCore

let findScrollViewer (v: Visual) =
    v.GetVisualDescendants() |> Seq.tryPick (fun d -> match d with :? ScrollViewer as s -> Some s | _ -> None) |> Option.toObj

let textFile (mb: int) =
    match mb with
    | 1 -> "text1mb.txt"
    | 10 -> "text10mb.txt"
    | 50 -> "text50mb.txt"
    | 100 -> "text100mb.txt"
    | n -> "text" + n.ToString(inv) + "mb.txt"

let newEditor (readOnly: bool) =
    AvaloniaEdit.TextEditor(IsReadOnly = readOnly, WordWrap = true, FontSize = 14.0, ShowLineNumbers = false,
                            FontFamily = FontFamily("Noto Sans"), Background = Brushes.White)

// ---------------------------------------------------------------- T1
type TextBigScn(args: Args) =
    inherit Scenario()
    let mb = if args.NGiven then int args.N else 1
    let t0 = Stopwatch.GetTimestamp()
    let ms (t: int64) = ticksToMs (t - t0)
    let path = Path.Combine(args.Assets, textFile mb)
    let ed = newEditor true
    let text = File.ReadAllText path
    let tRead = Stopwatch.GetTimestamp()
    do ed.Document <- AvaloniaEdit.Document.TextDocument(text)
    let tDoc = Stopwatch.GetTimestamp()
    let chars = text.Length
    let st, shell = ListScenarios.mkShell ("Bench - text-big " + mb.ToString(inv) + " MB") ed "..."
    let mutable layoutMs = 0.0
    let mutable sv : ScrollViewer = null
    let mutable selected = false
    let mutable selMs = 0.0
    let mutable firstMs = -1.0
    let mutable openedAt = 0L
    let mutable sq : Square = null
    override _.Content = shell
    override _.SetSquare(s) = sq <- s
    override _.OnWindow(w: Window) =
        let tl = Stopwatch.GetTimestamp()
        w.Measure(Size(1280.0, 720.0))
        w.Arrange(Rect(0.0, 0.0, 1280.0, 720.0))
        layoutMs <- ticksToMs (Stopwatch.GetTimestamp() - tl)
    override _.Begin() = sv <- findScrollViewer ed
    override _.OnFrame(t, dt) =
        if t >= 0.0 && not (isNull sv) then
            let maxOff = max 0.0 (sv.Extent.Height - sv.Viewport.Height)
            // aller-retour 30 000 px/s sur [0, min(maxOff, 300 000)] (periode 20 s)
            let range = max 1.0 (min maxOff 300000.0)
            let ph = (30000.0 * t) % (2.0 * range)
            let y = if ph < range then ph else 2.0 * range - ph
            sv.Offset <- Vector(0.0, y)
            if t >= 5.0 && not selected then
                selected <- true
                let s0 = Stopwatch.GetTimestamp()
                ed.Focus() |> ignore
                if ed.CanSelectAll then ed.SelectAll() else ed.Select(0, ed.Document.TextLength)
                selMs <- ticksToMs (Stopwatch.GetTimestamp() - s0)
    override _.Ops sb =
        kvF sb "time_to_first_display_ms" (ticksToMs (firstRenderedTs - t0))
        kvF sb "layout_ms" layoutMs
        kvF sb "file_read_ms" (ms tRead)
        kvF sb "document_load_ms" (ms tDoc - ms tRead)
        kvI sb "chars" (int64 chars)
        kvI sb "mb" (int64 mb)
        kvF sb "extent_height" (if isNull sv then -1.0 else sv.Extent.Height)
        kvI sb "select_all_done" (if selected then 1L else 0L)
        kvF sb "select_all_call_ms" selMs
        kvS sb "text_control" "AvaloniaEdit.TextEditor"
        checksumValue <- fnvSeq (int64 (min chars 4000000)) (fun i -> uint32 text.[int i])
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "T1: AvaloniaEdit.TextEditor 12.0.0 (rope + virtualisation de lignes), IsReadOnly, WordWrap, fichier lu en UTF-8 sur le thread UI avant l'affichage ; "
        + "time_to_first_display_ms = de l'ouverture du fichier a la fin de rendu de la 1re frame ; layout_ms = Measure+Arrange explicites de la fenetre ; "
        + "defilement via ScrollViewer.Offset (TextEditor.ScrollToVerticalOffset est sans effet en 12.0.0), aller-retour 30000 px/s sur [0,min(extent,300000)] ; tout selectionner a t=5 s (TextEditor.SelectAll)."

// ---------------------------------------------------------------- T3
type TextEditScn(args: Args) =
    inherit Scenario()
    let rate = if args.NGiven then args.N else 10L
    let ed = newEditor false
    let text = File.ReadAllText(Path.Combine(args.Assets, "text1mb.txt"))
    do ed.Document <- AvaloniaEdit.Document.TextDocument(text)
    let st, shell = ListScenarios.mkShell "Bench - text-edit" ed "1 MB"
    let mutable sq : Square = null
    let mutable entered = 0L
    let mutable dirty = false
    let mutable focused = false
    override _.Content = shell
    override _.SquareOnSpace = false
    override _.SetSquare(s) = sq <- s
    override _.Animating = false
    override _.OnWindow(w: Window) =
        // le carre change APRES application de la frappe a l'editeur (TextArea.TextEntered est leve apres l'insertion)
        ed.TextArea.TextEntered.Add(fun _ ->
            entered <- entered + 1L
            sq.Toggle())
    override _.OnFrame(t, dt) =
        if not focused then
            focused <- true
            // curseur au milieu du document
            ed.CaretOffset <- ed.Document.TextLength / 2
            ed.TextArea.Caret.BringCaretToView()
            ed.TextArea.Focus() |> ignore
    override _.Ops sb =
        kvI sb "chars_per_s_target" rate
        kvI sb "text_entered" entered
        kvI sb "doc_length" (int64 ed.Document.TextLength)
        checksumValue <- fnvSeq (int64 (min text.Length 1000000)) (fun i -> uint32 text.[int i])
        kvS sb "checksum_data" checksumValue
        kvI sb "key_events" (if isNull sq then 0L else sq.Keys)
    override _.Notes = "T3: TextEditor editable 1 Mo, curseur au milieu, focus clavier ; le carre bascule dans TextArea.TextEntered (apres insertion dans TextDocument) ; frames comptees quand le contenu change (frappe)."

// ---------------------------------------------------------------- T2
let grid (n: int) =
    let cols = int (Math.Ceiling(Math.Sqrt(float n * 1280.0 / 720.0)))
    let rows = (n + cols - 1) / cols
    cols, rows

let labelText (i: int) (frame: int) (buf: char[]) =
    let sp = Span<char>(buf, 0, 8)
    fnvText8 (fnv (i + frame * 31)) sp
    frame.ToString(inv) + " " + String(buf, 0, 8)

type LabelsOp(bounds: Rect, texts: string[], cols: int, cw: float32, ch: float32, font: SKFont, paint: SKPaint, bg: SKPaint) =
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
                for i in 0 .. texts.Length - 1 do
                    let x = float32 (i % cols) * cw
                    let y = float32 (i / cols) * ch
                    c.DrawText(texts.[i], x + 1.0f, y + ch - 3.0f, font, paint)
            Stats.addRender (Stopwatch.GetTimestamp() - t0)

[<AllowNullLiteral>]
type LabelsCanvas(n: int, cols: int, rows: int) as this =
    inherit Control()
    let cw = 1280.0f / float32 cols
    let ch = 720.0f / float32 rows
    let font = new SKFont(Sk.typeface.Value, max 3.0f (min 14.0f (min (ch * 0.7f) (cw / 8.0f))))
    let paint = new SKPaint(IsAntialias = true, Color = SKColors.Black)
    let bg = new SKPaint(Color = SKColors.White)
    let snaps = [| Array.create n ""; Array.create n ""; Array.create n "" |]
    let mutable cur = 0
    do this.ClipToBounds <- true
    member this.Update(frame: int) =
        cur <- (cur + 1) % 3
        let a = snaps.[cur]
        let buf = Array.zeroCreate<char> 8
        for i in 0 .. n - 1 do a.[i] <- labelText i frame buf
    override this.Render(ctx: DrawingContext) =
        ctx.Custom(new LabelsOp(Rect(0.0, 0.0, 1280.0, 720.0), snaps.[cur], cols, cw, ch, font, paint, bg))

type TextLabelsScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 500
    let impl = args.Variant("impl", "widgets")
    let cols, rows = grid n
    let cw = 1280.0 / float cols
    let ch = 720.0 / float rows
    let fsz = max 3.0 (min 14.0 (min (ch * 0.7) (cw / 8.0)))
    let blocks : TextBlock[] =
        if impl = "widgets" then
            Array.init n (fun i ->
                TextBlock(FontFamily = fontFamily, FontSize = fsz, Foreground = Brushes.Black, Width = cw, Height = ch, TextWrapping = TextWrapping.NoWrap))
        else null
    let canvasCtl = if impl = "canvas" then LabelsCanvas(n, cols, rows) else null
    let root : Control =
        if impl = "widgets" then
            let cv = Canvas(Background = Brushes.White)
            for i in 0 .. n - 1 do
                Canvas.SetLeft(blocks.[i], float (i % cols) * cw)
                Canvas.SetTop(blocks.[i], float (i / cols) * ch)
                cv.Children.Add blocks.[i]
            cv :> Control
        else canvasCtl :> Control
    let buf = Array.zeroCreate<char> 8
    let mutable frame = 0
    let mutable sq : Square = null
    override _.Content = root
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        frame <- frame + 1
        if impl = "widgets" then
            for i in 0 .. n - 1 do blocks.[i].Text <- labelText i frame buf
        else
            canvasCtl.Update frame
            canvasCtl.InvalidateVisual()
    override _.Ops sb =
        kvI sb "labels" (int64 n)
        kvS sb "impl" impl
        kvI sb "grid_cols" (int64 cols)
        kvI sb "frames_text_updates" (int64 frame)
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "T2: grille " + cols.ToString(inv) + "x" + rows.ToString(inv) + ", chaque etiquette = compteur de frame + 8 caracteres derives de fnv, change a chaque frame ; impl=" + impl
        + (if impl = "widgets" then " (TextBlock par etiquette, taille fixe, Text affecte a chaque frame)" else " (un Control, SKCanvas.DrawText sans shaping HarfBuzz)")

// ---------------------------------------------------------------- T4
let richWords =
    [| "lorem ipsum "; "dolor sit amet "; "日本語のテキスト "; "漢字かな交じり "; "한국어 텍스트 "; "中文文本 "; "emoji \U0001F600\U0001F680 "
       "family \U0001F469‍\U0001F469‍\U0001F467 "; "مرحبا بالعالم "; "שלום עולם "; "naïve façade "; "Ωmega " |]
let richFonts = [| "Noto Sans"; "Noto Serif"; "Noto Sans Mono"; "DejaVu Sans" |]

type TextRichScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 1000
    let tb = TextBlock(TextWrapping = TextWrapping.Wrap, Padding = Thickness(8.0))
    let inl = tb.Inlines
    do
        for i in 0 .. n - 1 do
            let h = fnv i
            let (r, g, b) = rgbOf i
            let run =
                Run(richWords.[int (h % uint32 richWords.Length)],
                    FontFamily = FontFamily(richFonts.[int ((h >>> 8) % 4u)]),
                    FontSize = float (12 + int ((h >>> 12) % 11u)),
                    FontWeight = (if (h >>> 20) &&& 1u = 1u then FontWeight.Bold else FontWeight.Normal),
                    Foreground = SolidColorBrush(Color.FromRgb(r / 2uy, g / 2uy, b / 2uy)))
            inl.Add run
            if i % 12 = 11 then inl.Add(LineBreak())
    let sv = ScrollViewer(Content = tb, HorizontalScrollBarVisibility = Primitives.ScrollBarVisibility.Disabled, VerticalScrollBarVisibility = Primitives.ScrollBarVisibility.Auto, Background = Brushes.White)
    let mutable sq : Square = null
    override _.Content = sv :> Control
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        let tt = if t < 0.0 then t + 2.0 else t
        let maxOff = max 0.0 (sv.Extent.Height - sv.Viewport.Height)
        if maxOff > 0.0 then
            let ph = (10000.0 * tt) % (2.0 * maxOff)
            sv.Offset <- Vector(0.0, (if ph < maxOff then ph else 2.0 * maxOff - ph))
    override _.Ops sb =
        kvI sb "spans" (int64 n)
        kvF sb "extent_height" sv.Extent.Height
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes = "T4: TextBlock.Inlines avec N Run (police/taille/couleur/gras alternes, latin, CJK, emoji couleur, arabe, hebreu), retour a la ligne, dans un ScrollViewer ; defilement 10000 px/s aller-retour."
