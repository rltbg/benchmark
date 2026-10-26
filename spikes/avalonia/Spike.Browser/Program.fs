module Program

open System
open System.Runtime.Versioning
open Avalonia
open Avalonia.Browser
open Spike

[<assembly: SupportedOSPlatform("browser")>]
do ()

[<EntryPoint>]
let main _argv =
    Config.platform <- "web"
    Config.autorun <- Environment.GetEnvironmentVariable "SPIKE_AUTORUN" = "1"
    Boot.appBuilder().StartBrowserAppAsync("out") |> ignore
    0
