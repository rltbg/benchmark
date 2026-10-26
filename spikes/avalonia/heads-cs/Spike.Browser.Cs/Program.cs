using System;
using System.Runtime.Versioning;
using System.Threading.Tasks;
using Avalonia;
using Avalonia.Browser;

[assembly: SupportedOSPlatform("browser")]

internal sealed class Program
{
    private static Task Main(string[] args)
    {
        Spike.Config.platform = "web";
        Spike.Config.autorun = Environment.GetEnvironmentVariable("SPIKE_AUTORUN") == "1";
        return Spike.Boot.appBuilder().StartBrowserAppAsync("out");
    }
}
