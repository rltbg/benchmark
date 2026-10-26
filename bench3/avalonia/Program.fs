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
    | _ -> failwith ("scenario inconnu: " + a.Scenario)

let writeUnsupported (a: Args) (reason: string) =
    let sb = Text.StringBuilder()
    sb.Append("{\"stack\":\"avalonia-").Append(if isAot then "aot" else "jit").Append("\",\"version\":\"").Append(versionString().Replace("\"", "'"))
      .Append("\",\"scenario\":\"").Append(a.Scenario).Append("\",\"duration_s\":").Append(a.Duration.ToString("F1", inv))
      .Append(",\"frames\":0,\"frame_ms\":[],\"ops\":{\"n\":").Append(a.N).Append("},\"checksum\":\"0\",\"notes\":\"\",\"unsupported\":\"").Append(reason.Replace("\"", "'")).Append("\"}") |> ignore
    File.WriteAllText(a.Out, sb.ToString())

let buildWindow () : Window =
    let a = theArgs
    let win = Window(Width = 1280.0, Height = 720.0, CanResize = true, WindowDecorations = WindowDecorations.None,
                     Background = SolidColorBrush(Color.Parse "#FAFAFC"), Title = "avalonia-bench",
                     WindowStartupLocation = WindowStartupLocation.Manual, Position = PixelPoint(0, 0))
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
    override this.OnFrameworkInitializationCompleted() =
        match this.ApplicationLifetime with
        | :? IClassicDesktopStyleApplicationLifetime as d -> d.MainWindow <- buildWindow ()
        | _ -> ()
        base.OnFrameworkInitializationCompleted()

[<EntryPoint>]
let main argv =
    Native.installResolver (Assembly.GetExecutingAssembly())
    theArgs <- parseArgs argv
    AppBuilder.Configure<App>()
        .UseX11()
        .UseSkia()
        .UseHarfBuzz()
        .With(X11PlatformOptions(RenderingMode = [| X11RenderingMode.Software |],
                                  // garde le framebuffer logiciel entre frames : evite l'allocation de 3,7 Mo (UnmanagedBlob
                                  // + GC.AddMemoryPressure => GC gen2 a chaque frame) et le blit complet
                                  UseRetainedFramebuffer = Nullable true))
        .With(FontManagerOptions(FontFallbacks = [| FontFallback(FontFamily = FontFamily("Noto Color Emoji"),
                                                                 UnicodeRange = UnicodeRange.Parse("U+1F000-1FAFF, U+200D, U+FE0F")) |]))
        .StartWithClassicDesktopLifetime(argv)
