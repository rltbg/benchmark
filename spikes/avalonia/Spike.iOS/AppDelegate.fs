namespace AvaSpike

open Foundation
open Avalonia
open Avalonia.iOS

[<Register("AppDelegate")>]
type AppDelegate() =
    inherit AvaloniaAppDelegate<Spike.App>()

    override _.CustomizeAppBuilder(builder: AppBuilder) : AppBuilder =
        Spike.Config.platform <- "ios-sim"
        Spike.Config.mobile <- true
        base.CustomizeAppBuilder(builder) |> Spike.Boot.configure
