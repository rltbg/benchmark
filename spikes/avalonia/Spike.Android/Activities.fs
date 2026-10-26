namespace AvaSpike

open Android.App
open Android.Content.PM
open Avalonia.Android

[<Activity(
    Label = "Avalonia spike",
    Theme = "@style/MyTheme.NoActionBar",
    Icon = "@drawable/icon",
    MainLauncher = true,
    ConfigurationChanges = (ConfigChanges.Orientation ||| ConfigChanges.ScreenSize ||| ConfigChanges.UiMode))>]
type MainActivity() =
    inherit AvaloniaMainActivity()

    override this.OnCreate(savedInstanceState) =
        // extra d'intent : adb shell am start -n <pkg>/<activite> --es autorun 1
        let extras = if isNull this.Intent then null else this.Intent.Extras
        let v = if isNull extras then null else extras.Get "autorun"
        Spike.Config.autorun <- (not (isNull v)) && (let s = v.ToString() in s = "1" || s = "true")
        base.OnCreate savedInstanceState
