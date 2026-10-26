import { dotnet } from './_framework/dotnet.js'

const is_browser = typeof window != "undefined";
if (!is_browser) throw new Error(`Expected to be running in a browser`);

// ?autorun=1 -> variable d'environnement SPIKE_AUTORUN=1 cote .NET
const q = new URLSearchParams(globalThis.location.search);
let builder = dotnet.withDiagnosticTracing(false).withApplicationArgumentsFromQuery();
if (q.get('autorun') === '1') builder = builder.withEnvironmentVariable('SPIKE_AUTORUN', '1');
const dotnetRuntime = await builder.create();
const config = dotnetRuntime.getConfig();
await dotnetRuntime.runMain(config.mainAssemblyName, [globalThis.location.href]);
