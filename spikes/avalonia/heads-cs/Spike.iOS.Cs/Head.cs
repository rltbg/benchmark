using System;
using Foundation;
using UIKit;
using Avalonia;
using Avalonia.iOS;

namespace AvaSpike;

[Register("AppDelegate")]
public partial class AppDelegate : AvaloniaAppDelegate<Spike.App>
{
    protected override AppBuilder CustomizeAppBuilder(AppBuilder builder)
    {
        Spike.Config.platform = "ios-sim";
        Spike.Config.mobile = true;
        return Spike.Boot.configure(base.CustomizeAppBuilder(builder));
    }
}

public class Application
{
    static void Main(string[] args)
    {
        Spike.Config.autorun = Array.IndexOf(args, "--autorun") >= 0;
        UIApplication.Main(args, null, typeof(AppDelegate));
    }
}
