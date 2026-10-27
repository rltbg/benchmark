module Program

open System
open System.Diagnostics
open System.IO
open System.Reflection
open Avalonia
open Avalonia.Controls
open Avalonia.Controls.ApplicationLifetimes
open Avalonia.Input
open Avalonia.Media
open Avalonia.Themes.Simple
open Avalonia.Rendering.Composition
open Util
open Harness

let mainStart = Stopwatch.GetTimestamp()
let mutable theArgs : Args = Unchecked.defaultof<Args>

let baseNotes () =
    "Avalonia 12.1.4 X11 RenderingMode=Software, Skia CPU, " + (if isAot then "NativeAOT" else "JIT (dev)") + ". " + FRAME_NOTE
    + " ui_stall_ms_max = plus long intervalle entre deux ticks consecutifs d'un DispatcherTimer de 4 ms (priorite Input) sur le thread UI pendant la fenetre mesuree ; "
    + "setup_ms = de l'entree de main a la fin de rendu de la 1re frame ; render_op_ms_mean = duree moyenne de Render() des operations custom (thread de rendu)."

let makeScenario (a: Args) : Scenario =
    match a.Scenario with
    | "list-scale" | "list-rich" | "list-mutate" | "list-reset" | "ffi-callbacks" | "ffi-block" | "latency-load" | "latency-wheel" ->
        ListScenarios.ListScn(a) :> Scenario
    | "gfx-shapes" -> Gfx.ShapesScn(a) :> Scenario
    | "gfx-chart" -> Gfx.ChartScn(a) :> Scenario
    | "gfx-paths" -> Gfx.PathsScn(a) :> Scenario
    | "gfx-images" -> Gfx.ImagesScn(a) :> Scenario
    | "gfx-layers" -> Gfx.LayersScn(a) :> Scenario
    | "text-big" -> Text.TextBigScn(a) :> Scenario
    | "text-edit" -> Text.TextEditScn(a) :> Scenario
    | "text-labels" -> Text.TextLabelsScn(a) :> Scenario
    | "text-rich" -> Text.TextRichScn(a) :> Scenario
    | "ui-tree" -> Ui.TreeScn(a) :> Scenario
    | "ui-bind" -> Ui.BindScn(a) :> Scenario
    | "ui-anim" -> Ui.AnimScn(a) :> Scenario
    | "ffi-calls" -> Ffi.FfiCallsScn(a) :> Scenario
    | _ -> failwith ("scenario inconnu: " + a.Scenario)

let writeUnsupported (a: Args) (reason: string) =
    let sb = Text.StringBuilder()
    sb.Append("{\"stack\":\"avalonia-").Append(if isAot then "aot" else "jit").Append("\",\"version\":\"").Append(versionString().Replace("\"", "'"))
      .Append("\",\"scenario\":\"").Append(a.Scenario).Append("\",\"duration_s\":").Append(a.Duration.ToString("F1", inv))
      .Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{\"n\":").Append(a.N).Append("},\"checksum\":\"0\",\"notes\":\"\",\"unsupported\":\"").Append(reason.Replace("\"", "'")).Append("\"}") |> ignore
    File.WriteAllText(a.Out, sb.ToString())

let unsupportedReason (id: string) =
    match id with
    | "soak" -> "M1 soak (30 min, composite) : non implemente (hors niveaux 1-2 de la passe v3 ; les briques L3/G1/F2/T3 existent separement)"
    | "mem-min" -> "M2 mem-min : non implemente (hors niveaux 1-2 ; le harnais peut composer list-scale N=100000 + text-big N=1 par cgroup)"
    | "startup-scale" -> "M3 startup-scale : se mesure avec ui-tree (ops.setup_ms, runtime_start) ; pas de scenario dedie"
    | _ -> "scenario inconnu: " + id

let known = set [ "list-scale"; "list-rich"; "list-mutate"; "list-reset"; "ffi-callbacks"; "ffi-block"; "latency-load"; "latency-wheel"
                  "gfx-shapes"; "gfx-chart"; "gfx-paths"; "gfx-images"; "gfx-layers"; "text-big"; "text-edit"; "text-labels"; "text-rich"
                  "ui-tree"; "ui-bind"; "ui-anim"; "ffi-calls" ]
let makeScenarioProbe (id: string) : obj = if known.Contains id then box 1 else null

/// idle : coquille (titre, 1re page de liste, statut), aucun redessin ; 2 s de warm-up puis `duration` entre les marqueurs
let runIdle (a: Args) =
    let gc = GcTracker()
    let t1 = Avalonia.Threading.DispatcherTimer(Interval = TimeSpan.FromSeconds 2.0)
    t1.Tick.Add(fun _ ->
        t1.Stop()
        gc.Start()
        printMarker "BENCH:measure_start"
        let t2 = Avalonia.Threading.DispatcherTimer(Interval = TimeSpan.FromSeconds a.Duration)
        t2.Tick.Add(fun _ ->
            t2.Stop()
            printMarker "BENCH:measure_end"
            let sb = Text.StringBuilder()
            sb.Append("{\"stack\":\"avalonia-").Append(if isAot then "aot" else "jit").Append("\",\"version\":\"").Append(versionString().Replace("\"", "'"))
              .Append("\",\"scenario\":\"idle\",\"duration_s\":").Append(a.Duration.ToString("F1", inv))
              .Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{\"n\":0,\"pacing\":\"60-only\",\"gc\":").Append(gc.Json())
              .Append("},\"checksum\":\"0\",\"notes\":\"idle: coquille (titre, 1re page de liste, statut), aucun redessin, DispatcherTimer.\",\"unsupported\":null}") |> ignore
            File.WriteAllText(a.Out, sb.ToString())
            (Application.Current.ApplicationLifetime :?> IClassicDesktopStyleApplicationLifetime).Shutdown 0)
        t2.Start())
    t1.Start()

let buildWindow () : Window =
    let a = theArgs
    let win = Window(Width = 1280.0, Height = 720.0, CanResize = true, WindowDecorations = WindowDecorations.None,
                     Background = SolidColorBrush(Color.Parse "#FAFAFC"), Title = "avalonia-bench",
                     WindowStartupLocation = WindowStartupLocation.Manual, Position = PixelPoint(0, 0))
    if a.Scenario = "idle" then
        let lv = ListsCore.CanvasListView(ListsCore.RowStore(100000, false, false), 0)
        let _, shell = ListScenarios.mkShell "Bench - idle" lv "idle"
        win.Content <- shell
        win.Opened.Add(fun _ -> runIdle a)
        win
    else
    let sc = makeScenario a
    let sq = Square()
    sc.SetSquare sq
    let root = Panel()
    root.Children.Add sc.Content
    root.Children.Add sq.Control
    win.Content <- root
    sc.OnWindow win
    if sc.SquareOnSpace then
        win.KeyDown.Add(fun e ->
            if e.Key = Key.Space then
                sq.Toggle()
                e.Handled <- true)
    win.Opened.Add(fun _ ->
        let d = Driver(win, sc, a, baseNotes (), mainStart)
        d.Start())
    win

type App() =
    inherit Application()
    override this.Initialize() =
        this.Styles.Add(SimpleTheme())
        if theArgs.Scenario = "text-big" || theArgs.Scenario = "text-edit" then
            // styles AvaloniaEdit via XAML compile (un StyleInclude cree en code echoue en NativeAOT, cf. NOTES)
            this.Styles.Add(AvaloniaBenchUi.TextStyles())
    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as d -> d.MainWindow <- buildWindow ()
        | _ -> ()
        base.OnFrameworkInitializationCompleted()

[<EntryPoint>]
let main argv =
    Native.installResolver (Assembly.GetExecutingAssembly())
    theArgs <- parseArgs argv
    if theArgs.Scenario <> "idle" && theArgs.Scenario <> "latency-idle" && isNull (makeScenarioProbe theArgs.Scenario) then
        writeUnsupported theArgs (unsupportedReason theArgs.Scenario)
        printMarker "BENCH:measure_start"
        printMarker "BENCH:measure_end"
        exit 0
    AppBuilder.Configure<App>()
        .UseX11()
        .UseSkia()
        .UseHarfBuzz()
        .With(X11PlatformOptions(RenderingMode = [| X11RenderingMode.Software |],
                                  // garde le framebuffer logiciel entre frames : evite l'allocation de 3,7 Mo (UnmanagedBlob
                                  // + GC.AddMemoryPressure => GC gen2 a chaque frame) et le blit complet
                                  UseRetainedFramebuffer = Nullable (theArgs.Variant("retained", "1") = "1")))
        // zones sales multi-rectangles (defaut : une seule boite englobante) : CompositionOptions public, nul par defaut
        // (Rendering/Composition/CompositionOptions.cs:11 ; Server/ServerCompositionTarget.cs:65). --variant dirty=single pour le desactiver.
        .With(CompositionOptions(UseRegionDirtyRectClipping = Nullable (theArgs.Variant("dirty", "region") <> "single")))
        .With(FontManagerOptions(FontFallbacks = [| FontFallback(FontFamily = FontFamily("Noto Color Emoji"),
                                                                 UnicodeRange = UnicodeRange.Parse("U+1F000-1FAFF, U+200D, U+FE0F")) |]))
        .StartWithClassicDesktopLifetime(argv)
