module Ui
// U1 ui-tree, U2 ui-bind, U3 ui-anim

open System
open System.Collections.Generic
open System.ComponentModel
open System.Diagnostics
open System.Text
open System.Linq.Expressions
open Avalonia
open Avalonia.Animation
open Avalonia.Controls
open Avalonia.Controls.Shapes
open Avalonia.Data
open Avalonia.Media
open Avalonia.Rendering.Composition
open Avalonia.Rendering.Composition.Animations
open Avalonia.Styling
open Util
open Harness
open ListsCore

// ---------------------------------------------------------------- U1 : arbre statique
type TreeScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 1000
    let t0 = Stopwatch.GetTimestamp()
    // arbre 10-aire complet numerote en largeur : enfants de i = 10i+1 .. 10i+10 (< n). Feuille = Border(rect) + TextBlock ; conteneur = StackPanel.
    let nodes : Control[] = Array.zeroCreate n
    let mutable leaves = 0
    do
        for i in n - 1 .. -1 .. 0 do
            if 10 * i + 1 >= n then
                let (r, g, b) = rgbOf i
                let tb = TextBlock(Text = "n" + i.ToString(inv), FontFamily = fontFamily, FontSize = 12.0, Margin = Thickness(4.0, 2.0), Foreground = Brushes.Black)
                nodes.[i] <- Border(Background = SolidColorBrush(Color.FromRgb(r, g, b)), Height = 22.0, Child = tb)
                leaves <- leaves + 1
            else
                let sp = StackPanel(Orientation = Layout.Orientation.Vertical)
                for c in 10 * i + 1 .. min (n - 1) (10 * i + 10) do sp.Children.Add nodes.[c]
                nodes.[i] <- sp
    let sv = ScrollViewer(Content = nodes.[0], Background = Brushes.White)
    let buildMs = ticksToMs (Stopwatch.GetTimestamp() - t0)
    let mutable sq : Square = null
    override _.Content = sv :> Control
    override _.SetSquare(s) = sq <- s
    override _.Animating = false
    override _.FrameFromResize = true
    override _.Ops sb =
        kvI sb "nodes" (int64 n)
        kvI sb "leaves" (int64 leaves)
        kvF sb "tree_build_ms" buildMs
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "U1: arbre 10-aire de " + n.ToString(inv) + " noeuds (conteneur = StackPanel vertical ; feuille = Border colore 22 px + TextBlock court), dans un ScrollViewer, non virtualise ; "
        + "frame_ms = ops.resize_frame_ms = duree entre l'evenement Window.Resized et la fin de rendu du batch qui porte la nouvelle mise en page ; le redimensionnement est fait par le harnais (xdotool)."

// ---------------------------------------------------------------- U2 : liaisons
type LabelVm() =
    let ev = Event<PropertyChangedEventHandler, PropertyChangedEventArgs>()
    static let args = PropertyChangedEventArgs("Count")
    let mutable count = 0
    member _.Count
        with get () = count
        and set (v: int) =
            if v <> count then
                count <- v
                ev.Trigger(null, args)
    interface INotifyPropertyChanged with
        [<CLIEvent>]
        member _.PropertyChanged = ev.Publish

type BindScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 1000
    let impl = args.Variant("impl", "compiled")
    let cols, rows = Text.grid n
    let cw = 1280.0 / float cols
    let ch = 720.0 / float rows
    let fsz = max 3.0 (min 14.0 (min (ch * 0.7) (cw / 8.0)))
    let vms = Array.init n (fun _ -> LabelVm())
    let cv = Canvas(Background = Brushes.White)
    let blocks = Array.init n (fun i -> TextBlock(FontFamily = fontFamily, FontSize = fsz, Foreground = Brushes.Black, Width = cw, Height = ch, DataContext = vms.[i]))
    let mutable counter = 0
    let mutable sq : Square = null
    do
        // un seul objet de liaison (gabarit) partage par les N etiquettes : l'expression est analysee une fois
        let compiled =
            if impl = "compiled" then
                // F# convertit une lambda en Expression<_> par deserialisation de quotation (reflexion FSharp.Core : echoue en NativeAOT,
                // cf. NOTES) : l'arbre d'expression est donc construit a la main (equivalent de vm => vm.Count)
                let p = Linq.Expressions.Expression.Parameter(typeof<LabelVm>, "vm")
                let body = Linq.Expressions.Expression.Property(p, typeof<LabelVm>.GetProperty("Count"))
                let lam = Linq.Expressions.Expression.Lambda<Func<LabelVm, int>>(body, [| p |])
                CompiledBinding.Create<LabelVm, int>(lam) :> BindingBase
            else null
        for i in 0 .. n - 1 do
            Canvas.SetLeft(blocks.[i], float (i % cols) * cw)
            Canvas.SetTop(blocks.[i], float (i / cols) * ch)
            cv.Children.Add blocks.[i]
            match impl with
            | "compiled" -> blocks.[i].Bind(TextBlock.TextProperty, compiled) |> ignore
            | "reflection" -> blocks.[i].Bind(TextBlock.TextProperty, Binding("Count")) |> ignore   // IL2026 en AOT (reflexion)
            | _ -> ()
    override _.Content = cv :> Control
    override _.SetSquare(s) = sq <- s
    override _.OnFrame(t, dt) =
        counter <- counter + 1
        if impl = "manual" then
            for i in 0 .. n - 1 do blocks.[i].Text <- counter.ToString(inv)
        else
            for i in 0 .. n - 1 do vms.[i].Count <- counter
    override _.Ops sb =
        kvI sb "labels" (int64 n)
        kvS sb "impl" impl
        kvI sb "property_changes" (int64 counter * int64 n)
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "U2: N TextBlock lies (CompiledBinding.Create(vm => vm.Count), un gabarit partage) a N view-models INotifyPropertyChanged ; chaque frame, les N compteurs changent (PropertyChanged leve sur le thread UI) ; impl=" + impl
        + (if impl = "manual" then " : affectation directe de Text (hors spec, comparaison)" else "")

// ---------------------------------------------------------------- U3 : animations
type AnimScn(args: Args) =
    inherit Scenario()
    let n = if args.NGiven then int args.N else 100
    let impl = args.Variant("impl", "composition")
    let cv = Canvas(Background = Brushes.White)
    let items : Border[] =
        Array.init n (fun i ->
            let (r, g, b) = rgbOf i
            let e = Border(Width = 20.0, Height = 20.0, Background = SolidColorBrush(Color.FromRgb(r, g, b)), CornerRadius = CornerRadius(4.0))
            Canvas.SetLeft(e, 0.0)
            Canvas.SetTop(e, 0.0)
            e)
    let mutable started = false
    let mutable sq : Square = null
    do for e in items do cv.Children.Add e
    member private this.StartComposition() =
        for i in 0 .. n - 1 do
            let vis = ElementComposition.GetElementVisual(items.[i])
            let comp = vis.Compositor
            let x0 = float32 (fnv (i * 3) % 1200u)
            let y0 = float32 (fnv (i * 3 + 1) % 650u)
            let x1 = float32 (fnv (i * 3 + 2) % 1200u)
            let y1 = float32 (fnv (i * 3 + 4) % 650u)
            let mv = comp.CreateVector3DKeyFrameAnimation()
            mv.Target <- "Offset"
            mv.InsertKeyFrame(0.0f, System.Numerics.Vector3(x0, y0, 0.0f))
            mv.InsertKeyFrame(1.0f, System.Numerics.Vector3(x1, y1, 0.0f))
            mv.Duration <- TimeSpan.FromMilliseconds(1500.0 + float (fnv (i + 9) % 1500u))
            mv.IterationBehavior <- AnimationIterationBehavior.Forever
            mv.Direction <- PlaybackDirection.Alternate
            vis.StartAnimation("Offset", mv)
            let op = comp.CreateScalarKeyFrameAnimation()
            op.Target <- "Opacity"
            op.InsertKeyFrame(0.0f, 0.2f)
            op.InsertKeyFrame(1.0f, 1.0f)
            op.Duration <- TimeSpan.FromMilliseconds(900.0 + float (fnv (i + 17) % 900u))
            op.IterationBehavior <- AnimationIterationBehavior.Forever
            op.Direction <- PlaybackDirection.Alternate
            vis.StartAnimation("Opacity", op)
    member private this.StartUi() =
        for i in 0 .. n - 1 do
            let x1 = float (fnv (i * 3 + 2) % 1200u)
            let y1 = float (fnv (i * 3 + 4) % 650u)
            let x0 = float (fnv (i * 3) % 1200u)
            let y0 = float (fnv (i * 3 + 1) % 650u)
            let a = Animation(Duration = TimeSpan.FromMilliseconds(1500.0 + float (fnv (i + 9) % 1500u)), IterationCount = IterationCount.Infinite,
                              PlaybackDirection = PlaybackDirection.Alternate)
            let k0 = KeyFrame(Cue = Cue(0.0))
            k0.Setters.Add(Setter(Canvas.LeftProperty, x0))
            k0.Setters.Add(Setter(Canvas.TopProperty, y0))
            k0.Setters.Add(Setter(Visual.OpacityProperty, 0.2))
            let k1 = KeyFrame(Cue = Cue(1.0))
            k1.Setters.Add(Setter(Canvas.LeftProperty, x1))
            k1.Setters.Add(Setter(Canvas.TopProperty, y1))
            k1.Setters.Add(Setter(Visual.OpacityProperty, 1.0))
            a.Children.Add k0
            a.Children.Add k1
            a.RunAsync(items.[i]) |> ignore
    override _.Content = cv :> Control
    override _.SetSquare(s) = sq <- s
    override this.OnFrame(t, dt) =
        if not started then
            started <- true
            if impl = "composition" then this.StartComposition() else this.StartUi()
    override _.Ops sb =
        kvI sb "elements" (int64 n)
        kvS sb "impl" impl
        checksumValue <- fnvSeq (int64 n) (fun i -> fnv (int i))
        kvS sb "checksum_data" checksumValue
        if not (isNull sq) then kvI sb "key_events" sq.Keys
    override _.Notes =
        "U3: N Border 20x20 animes position+opacite en boucle infinie (alternance) ; impl=" + impl
        + (if impl = "composition" then " : animations de composition (Compositor.CreateVector3DKeyFrameAnimation/CreateScalarKeyFrameAnimation, evaluees sur le thread de rendu, aucun code par frame)"
           else " : Avalonia.Animation.Animation (KeyFrames sur Canvas.Left/Top + Opacity, evaluees sur le thread UI par le moteur d'animation de propriete)")
