module Program

open System
open Avalonia
open Spike

[<EntryPoint; STAThread>]
let main argv =
    Config.platform <-
        if OperatingSystem.IsWindows() then "windows"
        elif OperatingSystem.IsMacOS() then "macos"
        else "linux"
    Config.autorun <- Environment.GetEnvironmentVariable "SPIKE_AUTORUN" = "1"
    Config.exitWhenDone <- Environment.GetEnvironmentVariable "SPIKE_EXIT" = "1"
    Config.shotDir <- Environment.GetEnvironmentVariable "SPIKE_SHOT_DIR"
    Boot.appBuilder().UsePlatformDetect().LogToTrace(areas = Array.empty).StartWithClassicDesktopLifetime(argv)
