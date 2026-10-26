using System;
using Android.App;
using Android.Content.PM;
using Android.Runtime;
using Avalonia;
using Avalonia.Android;

namespace AvaSpike;

[Application]
public class SpikeApplication : AvaloniaAndroidApplication<Spike.App>
{
    protected SpikeApplication(nint javaReference, JniHandleOwnership transfer) : base(javaReference, transfer) { }
    protected override AppBuilder CustomizeAppBuilder(AppBuilder builder)
    {
        Spike.Config.platform = "android";
        Spike.Config.mobile = true;
        Spike.Config.logSink = new Action<string>(s => Android.Util.Log.Info("SPIKE", s));
        return Spike.Boot.configure(base.CustomizeAppBuilder(builder));
    }
}

[Activity(Label = "Avalonia spike", Theme = "@style/MyTheme.NoActionBar", Icon = "@drawable/icon", MainLauncher = true,
    ConfigurationChanges = ConfigChanges.Orientation | ConfigChanges.ScreenSize | ConfigChanges.UiMode)]
public class MainActivity : AvaloniaMainActivity
{
    protected override void OnCreate(Android.OS.Bundle savedInstanceState)
    {
        var v = Intent?.Extras?.Get("autorun")?.ToString();
        Spike.Config.autorun = v == "1" || v == "true";
        base.OnCreate(savedInstanceState);
    }
}
