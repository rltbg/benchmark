namespace AvaSpike

open System
open Android.App
open Android.Runtime
open Avalonia
open Avalonia.Android

[<Application>]
type SpikeApplication(javaReference: nativeint, transfer: JniHandleOwnership) =
    inherit AvaloniaAndroidApplication<Spike.App>(javaReference, transfer)

    override _.CustomizeAppBuilder(builder: AppBuilder) : AppBuilder =
        Spike.Config.platform <- "android"
        Spike.Config.mobile <- true
        Spike.Config.logSink <- Action<string>(fun s -> Android.Util.Log.Info("SPIKE", s) |> ignore)
        base.CustomizeAppBuilder(builder) |> Spike.Boot.configure
