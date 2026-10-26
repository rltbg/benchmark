module AvaSpike.Main

open UIKit

[<EntryPoint>]
let main args =
    // simctl launch ... --autorun
    Spike.Config.autorun <- (args |> Array.contains "--autorun")
    UIApplication.Main(args, null, typeof<AppDelegate>)
    0
