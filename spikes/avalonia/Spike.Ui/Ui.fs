namespace Spike

open System
open System.Collections.Generic
open System.Diagnostics
open System.Globalization
open System.IO
open System.Text
open System.Threading.Tasks
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Controls.Primitives
open Avalonia.Input
open Avalonia.Layout
open Avalonia.Media
open Avalonia.Media.Imaging
open Avalonia.Styling
open Avalonia.Threading
open Avalonia.Themes.Fluent

/// Configuration renseignee par la tete de plateforme avant le demarrage d'Avalonia.
module Config =
    let mutable platform = "desktop"
    let mutable mobile = false
    let mutable autorun = false
    let mutable exitWhenDone = false
    let mutable shotDir : string = null
    let mutable logSink : Action<string> = null

module Log =
    /// Une ligne par evenement ; stdout (+ journal natif de la plateforme si la tete en fournit un).
    let line (s: string) =
        Console.Out.WriteLine s
        Console.Out.Flush()
        // journal fichier optionnel (SPIKE_LOG_FILE) : evite de dependre de stdout pour les applis GUI
        (match Environment.GetEnvironmentVariable "SPIKE_LOG_FILE" with
         | null | "" -> ()
         | f -> (try File.AppendAllText(f, s + "\n") with _ -> ()))
        match Config.logSink with
        | null -> ()
        | f -> (try f.Invoke s with _ -> ())
    let spike (s: string) = line ("SPIKE:" + s)

/// Adaptateur autour du code F# extrait par F* (Core.fs / FStarCustard.fs, non modifies).
module Verified =
    let implName = "fstar-extrait(F*>F#)"
    let toList (b: byte[]) : list<uint8> = List.ofArray b
    let encode (payload: byte[]) : byte[] = Core.core_encode (toList payload) |> Array.ofList
    let validate (frame: byte[]) : bool = Core.core_validate (toList frame)
    let parse (frame: byte[]) : byte[] option = Core.core_parse (toList frame) |> Option.map Array.ofList
    let hash (b: byte[]) : uint32 = Core.core_hash (toList b)
    let maxPayload = 64
    /// Trame [0xA5][0x01][n][octets UTF-8, n <= 64, tronque]
    let frameOfText (s: string) : byte[] =
        let raw = Encoding.UTF8.GetBytes s
        encode (if raw.Length > maxPayload then Array.sub raw 0 maxPayload else raw)
    let hex (b: byte[]) = String.Join(" ", b |> Array.map (fun x -> x.ToString("X2")))

    /// None = OK ; Some detail = echec
    let selftest () : string option =
        let fails = List<string>()
        let chk name ok = if not ok then fails.Add name
        let u (s: string) = Encoding.UTF8.GetBytes s
        chk "fnv-empty" (hash [||] = 0x811C9DC5u)
        chk "fnv-a" (hash (u "a") = 0xE40C292Cu)
        chk "fnv-foobar" (hash (u "foobar") = 0xBF9CF968u)
        let ok0 = encode [||]
        chk "frame-0-valid" (validate ok0 && parse ok0 = Some [||])
        let p64 = Array.init 64 byte
        let ok64 = encode p64
        chk "frame-64-valid" (validate ok64 && parse ok64 = Some p64)
        let f65 = Array.append [| 0xA5uy; 1uy; 65uy |] (Array.create 65 7uy)
        chk "frame-65-invalid" (not (validate f65) && (parse f65).IsNone)
        chk "bad-magic" (not (validate [| 0xA4uy; 1uy; 0uy |]))
        chk "bad-version" (not (validate [| 0xA5uy; 2uy; 0uy |]))
        chk "bad-length" (not (validate [| 0xA5uy; 1uy; 3uy; 1uy; 2uy |]))
        chk "too-short" (not (validate [| 0xA5uy; 1uy |]))
        chk "roundtrip-hello" (parse (frameOfText "hello") = Some (u "hello"))
        if fails.Count = 0 then None else Some(String.Join(",", fails))

module Util =
    let noPen : IPen = null
    let typeface = Typeface.Default
    let text (s: string) (size: float) (brush: IBrush) =
        FormattedText(s, CultureInfo.InvariantCulture, FlowDirection.LeftToRight, typeface, size, brush)
    /// FNV-1a 32 bits des 4 octets petit-boutiste de i
    let fnvLe (i: int) : uint32 =
        let mutable h = 2166136261u
        for k in 0 .. 3 do
            h <- (h ^^^ uint32 ((i >>> (8 * k)) &&& 0xFF)) * 16777619u
        h
    /// String.Format : le printf F# utilise la reflexion (PrintfImpl) et echoue en NativeAOT.
    let f (fmt: string) (args: obj[]) = String.Format(CultureInfo.InvariantCulture, fmt, args)
    let nowMs () = Stopwatch.GetTimestamp() * 1000L / Stopwatch.Frequency
    let memLine () =
        try
            let p = Process.GetCurrentProcess()
            f "workingset_mb={0:F1} gc_heap_mb={1:F1}" [| box (float p.WorkingSet64 / 1048576.0); box (float (GC.GetTotalMemory false) / 1048576.0) |]
        with _ -> f "gc_heap_mb={0:F1}" [| box (float (GC.GetTotalMemory false) / 1048576.0) |]

/// Boucle d'animation liee a une frame d'affichage (TopLevel.RequestAnimationFrame).
type FrameLoop(host: Control, step: TimeSpan -> unit) =
    let mutable gen = 0
    member _.Start() =
        gen <- gen + 1
        let my = gen
        let rec go () =
            match TopLevel.GetTopLevel host with
            | null -> ()
            | top -> top.RequestAnimationFrame(fun t -> if my = gen then (step t; go ())) |> ignore
        go ()
    member _.Stop() = gen <- gen + 1

// ---------------------------------------------------------------- Onglet 1 : liste virtualisee
type VirtualList() as this =
    inherit Control()
    static let count = 100000
    static let heights = Array.init count (fun i -> 48 + 16 * int (Util.fnvLe i % 6u))
    static let prefix =
        let p = Array.zeroCreate<int64> (count + 1)
        for i in 0 .. count - 1 do p.[i + 1] <- p.[i] + int64 heights.[i]
        p
    let prices : float[] = Array.init count (fun i -> float (Util.fnvLe (i + 7) % 100000u) / 100.0)
    let mutable offset = 0.0
    let mutable velocity = 0.0
    let mutable dragging = false
    let mutable lastY = 0.0
    let mutable lastT = 0L
    let mutable auto = false
    let mutable autoT0 = 0L
    let mutable updates = 0L
    let mutable cts : System.Threading.CancellationTokenSource = null
    let titleBrush = SolidColorBrush(Color.FromRgb(30uy, 30uy, 40uy)) :> IBrush
    let priceBrush = SolidColorBrush(Color.FromRgb(0uy, 110uy, 60uy)) :> IBrush
    let bgA = SolidColorBrush(Color.FromRgb(250uy, 250uy, 252uy)) :> IBrush
    let bgB = SolidColorBrush(Color.FromRgb(238uy, 241uy, 247uy)) :> IBrush
    let loop = FrameLoop(this, (fun _ -> this.Tick()))
    do this.ClipToBounds <- true
    static member Total = prefix.[count]
    member _.Updates = updates
    member _.Offset = offset
    member _.AutoScroll with set (v: bool) = auto <- v; autoT0 <- Util.nowMs ()
    member private _.Clamp() =
        let mx = max 0.0 (float prefix.[count] - this.Bounds.Height)
        offset <- max 0.0 (min mx offset)
    member _.Tick() =
        let now = Util.nowMs ()
        if auto then
            // defilement simule : 9000 px/s, aller simple modulo la hauteur totale
            offset <- float ((int64 (float (now - autoT0) * 9.0)) % prefix.[count])
        elif not dragging && abs velocity > 0.02 then
            offset <- offset + velocity * 16.0
            velocity <- velocity * 0.95
        this.Clamp()
        this.InvalidateVisual()
    /// Tache de fond : 1000 mises a jour/s (xorshift32, graine 0x2545F491), index = x % 100000, prix = x / 100.
    member private _.StartPrices() =
        cts <- new System.Threading.CancellationTokenSource()
        let token = cts.Token
        Task.Run(fun () ->
            task {
                let sw = Stopwatch.StartNew()
                let mutable x = 0x2545F491u
                let mutable doneN = 0L
                while not token.IsCancellationRequested do
                    let due = sw.ElapsedMilliseconds
                    while doneN < due do
                        x <- x ^^^ (x <<< 13)
                        x <- x ^^^ (x >>> 17)
                        x <- x ^^^ (x <<< 5)
                        prices.[int (x % 100000u)] <- float x / 100.0
                        doneN <- doneN + 1L
                    updates <- doneN
                    do! Task.Delay 1
            } :> Task) |> ignore
    override _.OnAttachedToVisualTree(e) =
        base.OnAttachedToVisualTree e
        this.StartPrices()
        loop.Start()
    override _.OnDetachedFromVisualTree(e) =
        loop.Stop()
        if not (isNull cts) then cts.Cancel()
        base.OnDetachedFromVisualTree e
    override _.OnPointerWheelChanged(e) =
        offset <- offset - e.Delta.Y * 80.0
        velocity <- 0.0
        e.Handled <- true
    override _.OnPointerPressed(e) =
        dragging <- true
        velocity <- 0.0
        lastY <- e.GetPosition(this).Y
        lastT <- Util.nowMs ()
        e.Pointer.Capture this
        e.Handled <- true
    override _.OnPointerMoved(e) =
        if dragging then
            let y = e.GetPosition(this).Y
            let t = Util.nowMs ()
            let dy = lastY - y
            offset <- offset + dy
            let dt = float (max 1L (t - lastT))
            velocity <- 0.5 * velocity + 0.5 * (dy / dt)
            lastY <- y
            lastT <- t
    override _.OnPointerReleased(e) =
        dragging <- false
        e.Pointer.Capture null
    override _.Render(ctx) =
        let w = this.Bounds.Width
        let h = this.Bounds.Height
        ctx.DrawRectangle(bgA, Util.noPen, Rect(0.0, 0.0, w, h))
        // recherche binaire de la premiere ligne visible (prefix.[i] <= offset < prefix.[i+1])
        let o = int64 offset
        let mutable lo = 0
        let mutable hi = count - 1
        while lo < hi do
            let mid = (lo + hi + 1) / 2
            if prefix.[mid] <= o then lo <- mid else hi <- mid - 1
        let mutable i = lo
        let mutable y = float (prefix.[i]) - offset
        while i < count && y < h do
            let rh = float heights.[i]
            ctx.DrawRectangle((if i % 2 = 0 then bgA else bgB), Util.noPen, Rect(0.0, y, w, rh))
            let t = Util.text ("Item #" + i.ToString()) 18.0 titleBrush
            ctx.DrawText(t, Point(16.0, y + (rh - t.Height) / 2.0))
            let p = Util.text (prices.[i].ToString("F2", CultureInfo.InvariantCulture)) 16.0 priceBrush
            ctx.DrawText(p, Point(w - 16.0 - p.Width, y + (rh - p.Height) / 2.0))
            y <- y + rh
            i <- i + 1

// ---------------------------------------------------------------- Onglet 2 : canvas anime
type ShapeCanvas(count: int, tag: string, logFps: bool) as this =
    inherit Control()
    let x = Array.zeroCreate<float> count
    let y = Array.zeroCreate<float> count
    let vx = Array.zeroCreate<float> count
    let vy = Array.zeroCreate<float> count
    let sz = Array.zeroCreate<float> count
    let brushes : IBrush[] =
        let mutable s = 0x1234567u
        let next () =
            s <- s ^^^ (s <<< 13)
            s <- s ^^^ (s >>> 17)
            s <- s ^^^ (s <<< 5)
            s
        let b = Array.zeroCreate<IBrush> count
        for i in 0 .. count - 1 do
            let r () = byte (next () % 256u)
            let g = LinearGradientBrush()
            g.StartPoint <- RelativePoint(0.0, 0.0, RelativeUnit.Relative)
            g.EndPoint <- RelativePoint(1.0, 1.0, RelativeUnit.Relative)
            g.GradientStops.Add(GradientStop(Color.FromRgb(r (), r (), r ()), 0.0))
            g.GradientStops.Add(GradientStop(Color.FromRgb(r (), r (), r ()), 1.0))
            b.[i] <- g :> IBrush
            x.[i] <- float (next () % 1000u) / 1000.0
            y.[i] <- float (next () % 1000u) / 1000.0
            vx.[i] <- 40.0 + float (next () % 200u)
            vy.[i] <- 40.0 + float (next () % 200u)
            sz.[i] <- 14.0 + float (next () % 26u)
        b
    let stamps = Queue<int64>()
    let mutable placed = false
    let mutable lastTs = TimeSpan.Zero
    let mutable lastGap = 0.0
    let mutable maxGap = 0.0
    let mutable lastLog = 0L
    let mutable fps = 0
    let mutable frames = 0L
    let overlayBg = SolidColorBrush(Color.FromArgb(200uy, 0uy, 0uy, 0uy)) :> IBrush
    let overlayFg = Brushes.White :> IBrush
    let background = SolidColorBrush(Color.FromRgb(20uy, 24uy, 38uy)) :> IBrush
    let loop = FrameLoop(this, (fun t -> this.Step t))
    do this.ClipToBounds <- true
    member _.Fps = fps
    member _.Frames = frames
    member _.MaxGapMs with get () = maxGap and set (v: float) = maxGap <- v
    member _.Step(ts: TimeSpan) =
        let dt = if lastTs = TimeSpan.Zero then 0.016 else min 0.1 (ts - lastTs).TotalSeconds
        if lastTs <> TimeSpan.Zero then
            lastGap <- (ts - lastTs).TotalMilliseconds
            if lastGap > maxGap then maxGap <- lastGap
        lastTs <- ts
        let w = max 100.0 this.Bounds.Width
        let h = max 100.0 this.Bounds.Height
        if not placed && this.Bounds.Width > 10.0 then
            placed <- true
            for i in 0 .. count - 1 do
                x.[i] <- x.[i] * w
                y.[i] <- y.[i] * h
        for i in 0 .. count - 1 do
            x.[i] <- x.[i] + vx.[i] * dt
            y.[i] <- y.[i] + vy.[i] * dt
            let s = sz.[i]
            if x.[i] < 0.0 then (x.[i] <- 0.0; vx.[i] <- abs vx.[i])
            elif x.[i] > w - s then (x.[i] <- w - s; vx.[i] <- -(abs vx.[i]))
            if y.[i] < 0.0 then (y.[i] <- 0.0; vy.[i] <- abs vy.[i])
            elif y.[i] > h - s then (y.[i] <- h - s; vy.[i] <- -(abs vy.[i]))
        let now = Util.nowMs ()
        stamps.Enqueue now
        while stamps.Count > 0 && now - stamps.Peek() > 1000L do stamps.Dequeue() |> ignore
        fps <- stamps.Count
        frames <- frames + 1L
        if logFps && now - lastLog >= 1000L then
            lastLog <- now
            Log.spike ("fps tab=" + tag + " value=" + fps.ToString())
        this.InvalidateVisual()
    override _.OnAttachedToVisualTree(e) =
        base.OnAttachedToVisualTree e
        lastTs <- TimeSpan.Zero
        lastLog <- Util.nowMs ()
        loop.Start()
    override _.OnDetachedFromVisualTree(e) =
        loop.Stop()
        base.OnDetachedFromVisualTree e
    override _.Render(ctx) =
        ctx.DrawRectangle(background, Util.noPen, Rect(0.0, 0.0, this.Bounds.Width, this.Bounds.Height))
        for i in 0 .. count - 1 do
            let s = sz.[i]
            match i % 3 with
            | 0 -> ctx.DrawRectangle(brushes.[i], Util.noPen, Rect(x.[i], y.[i], s, s * 0.7))
            | 1 -> ctx.DrawEllipse(brushes.[i], Util.noPen, Point(x.[i] + s / 2.0, y.[i] + s / 2.0), s / 2.0, s / 2.0)
            | _ -> ctx.DrawRectangle(brushes.[i], Util.noPen, RoundedRect(Rect(x.[i], y.[i], s, s * 0.8), 8.0))
        if logFps then
            let t = Util.text ("FPS " + fps.ToString()) 20.0 overlayFg
            ctx.DrawRectangle(overlayBg, Util.noPen, Rect(8.0, 8.0, t.Width + 16.0, t.Height + 8.0))
            ctx.DrawText(t, Point(16.0, 12.0))

// ---------------------------------------------------------------- Onglet 3 : coeur verifie
type CoreView() as this =
    inherit UserControl()
    let input = TextBox(PlaceholderText = "Saisir du texte (clavier / IME)", Margin = Thickness(0.0, 0.0, 0.0, 8.0))
    let frameTb = TextBlock(TextWrapping = TextWrapping.Wrap, FontFamily = FontFamily("monospace"))
    let validTb = TextBlock()
    let payloadTb = TextBlock(TextWrapping = TextWrapping.Wrap)
    let hashTb = TextBlock(FontFamily = FontFamily("monospace"))
    let heavyTb = TextBlock(Text = "charge : inactive")
    let heavyBtn = Button(Content = "charge 100 ms", Margin = Thickness(0.0, 8.0, 0.0, 4.0))
    let mini = ShapeCanvas(300, "core-mini", false, Height = 140.0, Margin = Thickness(0.0, 8.0, 0.0, 0.0))
    let update () =
        let f = Verified.frameOfText input.Text
        let v = Verified.validate f
        frameTb.Text <- "trame : " + Verified.hex f
        validTb.Text <- "validate : " + (if v then "true" else "false")
        match Verified.parse f with
        | Some p ->
            payloadTb.Text <- "payload : " + Encoding.UTF8.GetString p
            hashTb.Text <- "hash FNV-1a : 0x" + (Verified.hash p).ToString("X8")
        | None ->
            payloadTb.Text <- "payload : (parse a refuse la trame)"
            hashTb.Text <- "hash FNV-1a : -"
    do
        input.Text <- ""
        let panel = StackPanel(Margin = Thickness(16.0))
        for c in [ input :> Control; frameTb; validTb; payloadTb; hashTb; heavyBtn; heavyTb; mini ] do
            panel.Children.Add c
        this.Content <- ScrollViewer(Content = panel)
        input.TextChanged.Add(fun _ -> update ())
        heavyBtn.Click.Add(fun _ -> this.RunHeavy() |> ignore)
        update ()
    member _.Input = input
    /// Equivalent de core_heavy : boucle de calcul de 100 ms sur un fil de fond (FNV du cœur verifie).
    member _.RunHeavy() : Task =
        task {
            heavyBtn.IsEnabled <- false
            heavyTb.Text <- "charge : en cours..."
            mini.MaxGapMs <- 0.0
            let lst = List.init 64 byte
            let! (ms, n) =
                Task.Run(fun () ->
                    let sw = Stopwatch.StartNew()
                    let mutable n = 0L
                    let mutable acc = 0u
                    while sw.ElapsedMilliseconds < 100L do
                        acc <- acc + Core.core_hash lst
                        n <- n + 1L
                    (sw.ElapsedMilliseconds, n + int64 (acc &&& 0u)))
            do! Task.Delay 200
            let gap = mini.MaxGapMs
            heavyTb.Text <- Util.f "charge : {0} ms, {1} hash ; plus grand ecart de frame du mini-canvas : {2:F0} ms" [| box ms; box n; box gap |]
            Log.spike (Util.f "heavy ms={0} iters={1} max_frame_gap_ms={2:F0}" [| box ms; box n; box gap |])
            heavyBtn.IsEnabled <- true
        } :> Task

// ---------------------------------------------------------------- Fenetre principale
type MainView() as this =
    inherit UserControl()
    let selftest = Verified.selftest ()
    let banner =
        TextBlock(
            Text = "avalonia · " + Config.platform + " · cœur=" + Verified.implName + " · autotest=" + (if selftest.IsNone then "OK" else "ÉCHEC"),
            Margin = Thickness(12.0, 8.0), FontSize = 14.0, TextWrapping = TextWrapping.Wrap)
    let host = ContentControl()
    let list = VirtualList()
    let canvas = ShapeCanvas(2000, "canvas", true)
    let core = CoreView()
    let names = [| "Liste"; "Canvas"; "Cœur" |]
    let buttons = names |> Array.map (fun n -> ToggleButton(Content = n, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Center, Margin = Thickness(2.0)))
    let mutable current = -1
    let mutable started = false
    let select (i: int) =
        current <- i
        host.Content <-
            (match i with
             | 0 -> list :> Control
             | 1 -> canvas :> Control
             | _ -> core :> Control)
        buttons |> Array.iteri (fun k b -> b.IsChecked <- Nullable<bool>((k = i)))
    let delay (ms: int) = Task.Delay ms
    let shot (name: string) : Task =
        task {
            Log.spike ("shot " + name)
            MainView.Shot(this, name)
            do! delay 700
        } :> Task
    let autorun () : Task =
        task {
            select 0
            list.AutoScroll <- true
            do! delay 3000
            list.AutoScroll <- false
            do! shot "tab1-liste"
            select 1
            do! delay 3000
            do! shot "tab2-canvas"
            select 2
            for c in "hello" do
                core.Input.Text <- core.Input.Text + string c
                do! delay 100
            do! core.RunHeavy()
            do! delay 1000
            do! shot "tab3-coeur"
            Log.spike ("mem " + Util.memLine () + " updates=" + list.Updates.ToString() + " canvas_frames=" + canvas.Frames.ToString())
            Log.spike "done"
            if Config.exitWhenDone then
                match Application.Current.ApplicationLifetime with
                | :? IClassicDesktopStyleApplicationLifetime as d -> d.Shutdown()
                | _ -> ()
        } :> Task
    do
        let bar = UniformGrid(Rows = 1)
        buttons |> Array.iteri (fun i b ->
            b.Click.Add(fun _ -> select i)
            bar.Children.Add b)
        let dock = DockPanel()
        DockPanel.SetDock(banner, Dock.Top)
        DockPanel.SetDock(bar, (if Config.mobile then Dock.Bottom else Dock.Top))
        dock.Children.Add banner
        dock.Children.Add bar
        dock.Children.Add host
        this.Content <- dock
        select 0
    static member Shot(v: Control, name: string) =
        if not (isNull Config.shotDir) then
            try
                let top = TopLevel.GetTopLevel v
                let target : Visual = if isNull top then (v :> Visual) else (top :> Visual)
                let size = PixelSize(max 1 (int v.Bounds.Width), max 1 (int v.Bounds.Height))
                Directory.CreateDirectory Config.shotDir |> ignore
                use rtb = new RenderTargetBitmap(size, Vector(96.0, 96.0))
                rtb.Render v
                rtb.Save(Path.Combine(Config.shotDir, name + ".png"))
            with e -> Log.spike ("screenshot-error " + e.GetType().Name + ": " + e.Message)
    override _.OnAttachedToVisualTree(e) =
        base.OnAttachedToVisualTree e
        if not started then
            started <- true
            match TopLevel.GetTopLevel this with
            | null -> ()
            | top ->
                top.RequestAnimationFrame(fun _ ->
                    Log.spike ("ready platform=" + Config.platform + " stack=avalonia core=" + Verified.implName)
                    (match selftest with
                     | None -> Log.spike "selftest ok"
                     | Some d -> Log.spike ("selftest fail " + d))
                    (try
                        let st = Process.GetCurrentProcess().StartTime
                        Log.spike ("startup_ms=" + ((DateTime.Now - st).TotalMilliseconds).ToString("F0", CultureInfo.InvariantCulture))
                     with _ -> ())
                    Log.spike ("mem " + Util.memLine ())
                    if Config.autorun then
                        DispatcherTimer.RunOnce((fun () -> autorun () |> ignore), TimeSpan.FromMilliseconds 500.0) |> ignore) |> ignore

type App() =
    inherit Application()
    override this.Initialize() =
        this.Styles.Add(FluentTheme())
        this.RequestedThemeVariant <- ThemeVariant.Light
    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as d ->
            d.MainWindow <- Window(Title = "Avalonia spike", Width = 900.0, Height = 700.0, Content = MainView())
        | :? IActivityApplicationLifetime as a -> a.MainViewFactory <- Func<Control>(fun () -> MainView() :> Control)
        | :? ISingleViewApplicationLifetime as s -> s.MainView <- MainView()
        | _ -> ()
        base.OnFrameworkInitializationCompleted()

module Boot =
    /// Police Inter embarquee (indispensable sur web/mobile ou il n'y a pas de police systeme garantie).
    let configure (b: AppBuilder) : AppBuilder = b.WithInterFont()
    let appBuilder () : AppBuilder = AppBuilder.Configure<App>() |> configure
