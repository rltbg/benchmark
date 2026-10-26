# Avalonia 12.1.4 + F# + .NET 10 NativeAOT — bancs de limites v3 (notes de stack)

Dossier : `bench3/avalonia/`. Cœur : **uniquement** `core/` (Rust, C-ABI v2, `core/benchcore.h`) par P/Invoke. Aucune trace de F\* ni de cœur F# (variantes `fstar`/`fsharp` et `fstar/extracted` retirées ; `BENCH_CORE_IMPL` n'existe plus). NativeAOT uniquement (aucune variante JIT livrée ; `dev.sh` = build JIT local de mise au point, non livré, ignoré par git).

## Versions (vérifiées le 2026-10-26, index NuGet `api.nuget.org/v3-flatcontainer`, `releases.json` .NET)
| composant | version | remarque |
|---|---|---|
| .NET SDK | **10.0.403** (`/opt/dotnet10`), runtime 10.0.14, ILCompiler 10.0.14 | dernier SDK 10.0.x stable (`latest-sdk` = 10.0.403) |
| Avalonia, .X11, .Skia, .HarfBuzz, .Themes.Simple | **12.1.4** | dernière stable (12.1.2, 12.1.3, 12.1.4) |
| Avalonia.AvaloniaEdit | **12.0.0** | dernière stable (12.0.0-rc1 précède) |
| SkiaSharp / HarfBuzzSharp | 3.119.4 / 8.3.1.3 | imposés par Avalonia.Skia 12.1.4 (SkiaSharp 4.153.1 existe : non supporté par Avalonia 12.1.4, non forcé) |
| FSharp.Core | **10.1.403** | dernière stable compatible avec le compilateur F# du SDK 10.0.403 (11.0.100 existe mais cible le SDK .NET 11 : non utilisé) |
| Rust (cœur) | 1.99.1, `core/target/release/libbenchcore.a` | |
Sources de référence lues : `/opt/src/avalonia` (tag 12.1.4, commit 2d79ba55fae1b1e71c69f93a668080c9029e6a31).

## Build / lancement
- `./build.sh` : `dotnet publish -c Release -r linux-x64 -p:PublishAot=true -m:2` → `out/avalonia-bench` (+ `libSkiaSharp.so`, `libHarfBuzzSharp.so`) ; ~1 min 15 s. Logs : `logs/publish-aot.log`, `logs/warnings.txt`.
- `./run.sh ARGS…` : pose `BENCH_ROOT`, `LIBGL_ALWAYS_SOFTWARE=1`, `DOTNET_TieredPGO=0` (sans effet en AOT) puis `exec out/avalonia-bench ARGS…`.
- Ligne `harness/stacks3.tsv` (déjà présente) : `avalonia-aot	${ROOT}/bench3/avalonia/run.sh`.
- CLI : `--scenario ID [--n N] [--variant k=v]… --duration S --out F.json --assets DIR` (une valeur nue `--variant x` vaut `feature=x`).
- Outils de mise au point : `smoke.sh <dev|aot> <scénario> <durée> [args]`, `poke.sh` (envoie touches/molette/redimensionnements XTest pendant la mesure), `ab.sh cfg` (fumées A/B), `logs/ab-*.cfg|txt`.

## FFI (cœur C-ABI v2)
- `Native.fs` : `DllImport` blittable (types primitifs / `nativeint` seulement) + `[<assembly: DisableRuntimeMarshalling>]` ; `LibraryImport` est un générateur de source C# (indisponible en F#). Liaison **statique** en AOT : `<DirectPInvoke Include="benchcore"/>` + `<NativeLibrary Include="…/libbenchcore.a"/>` (aucun `dlopen`). `SuppressGCTransition` sur `core_noop`, `core_add`, `core_validate/parse/hash` (petits tampons) ; transition GC conservée pour `core_hash` ≥ 64 Ko, `core_fill`, `core_heavy`, producteur. `--variant gct=1` (F1) force les versions avec transition.
- **Rappels natif→managé (F2) : écart à la consigne `[<UnmanagedCallersOnly>]`.** F# ne sait pas produire de pointeur de fonction non managé : (1) le compilateur émet `warning FS0202: This attribute is currently unsupported by the F# compiler. Applying it will not achieve its intended effect.` sur l'attribut ; (2) `typeof<Cb>.GetMethod("OnItem").MethodHandle.GetFunctionPointer()` échoue en NativeAOT : `System.NotSupportedException: This object cannot be invoked because no code was generated for it: 'Spike+Cb.OnItem(System.IntPtr,System.UInt32,System.Double)'` (même avec `DynamicDependency`) ; (3) `[<UnmanagedCallersOnly(EntryPoint="bench_on_item")>]` + `NativeLibrary.GetExport(GetMainProgramHandle(), …)` : `System.EntryPointNotFoundException: Unable to find an entry point named 'bench_on_item' in native library.` (symbole non exporté dans un exécutable, même avec `-Wl,--export-dynamic-symbol`). **Retenu** : delegate blittable `CoreCb` (`[<UnmanagedFunctionPointer(Cdecl)>]`, `DisableRuntimeMarshalling` actif) + `Marshal.GetFunctionPointerForDelegate` (vérifié en AOT : 99 972 appels reçus sur 99 972 émis ; avertissement ILC IL3050 documenté plus bas). Coût : un thunk de transition inverse de plus qu'un `UnmanagedCallersOnly` natif. Le callback ne fait qu'une écriture dans une file SPSC sans verrou (`SpscRing`, 2 Mo d'entrées de 24 octets pour F2) drainée une fois par frame sur le thread UI. Variante de comparaison `--variant transfer=post` : un `Dispatcher.UIThread.Post` par événement (hors recommandation : voir OPTIMIZATION-LOG).
- Tampons F1 : `GC.AllocateArray<byte>(n, pinned=true)` (Pinned Object Heap), adresse stable sans `GCHandle`.

## Mesures communes (JSON)
- `frame_ms` : fin de rendu `CompositionBatch.Rendered` (API publique ; `Compositor.RequestCompositionBatchCommitAsync` dans `TopLevel.RequestAnimationFrame`), 1 frame par batch dont le contenu a changé (scénarios animés : toutes ; U1 : frames de redimensionnement ; T3/I2 : frames de frappe/molette/carré). Fusion des batches rendus dans la même passe (< 0,1 ms).
- **Cadence : `ops.pacing = "60-only"`.** X11 crée `SleepLoopRenderTimer` (`[PrivateApi]`, `src/Avalonia.X11/X11Platform.cs:82-115`) : pas d'API publique pour lever le plafond ; `BENCH_PACING=uncapped` n'est donc pas honoré (la mesure principale est à 60 Hz).
- `ops.ui_stall_ms_max` : plus long intervalle entre deux ticks d'un `DispatcherTimer` de **4 ms à priorité `Input`** sur le thread UI pendant la fenêtre mesurée (nominal ≈ 4-5 ms ; inclut GC et travail UI), + histogramme `ui_stall_hist_…` et `ui_stall_gt50ms`.
- `ops.setup_ms` : de l'entrée de `main` à la fin de rendu de la 1ʳᵉ frame (modèle construit avant). `ops.model_build_ms` (listes), `tree_build_ms` (U1), `file_read_ms`/`document_load_ms` (texte).
- `ops.gc` : `gen0/gen1/gen2` = `GC.CollectionCount(n)` en delta (comptes cumulatifs : gen0 inclut gen1/gen2 ; `gen0_only`, `gen1_only` fournis), `pause_ms_total` (`GC.GetTotalPauseDuration`), `pause_ms_max` (borne inférieure : `GetGCMemoryInfo(Any).PauseDurations` relevé à chaque tick de 4 ms), `alloc_bytes` (`GC.GetTotalAllocatedBytes`), `heap_bytes`.
- `ops.checksum_data` : FNV-1a 32 de la séquence d'entiers 32 bits LE du jeu de données : listes = `h(i)` (ou `hd(i)` pour L2) pour i < N de départ (N=100 000 → `0cf19e25`) ; texte = caractères (UTF-16 → cp) du fichier ; G1 = `fnv(5i+3)` ; autres = `fnv(i)` (voir `Ops` de chaque scénario). `ops.sum_heights` = somme exacte au départ (8 799 584 pour 100 000 lignes standard), `sum_heights_final` après mutations.
- Options publiques activées (journalisées dans OPTIMIZATION-LOG) : `X11PlatformOptions.RenderingMode=[Software]`, `UseRetainedFramebuffer=true`, `CompositionOptions.UseRegionDirtyRectClipping=true` (`--variant dirty=single` et `retained=0` pour comparer), `FontManagerOptions.FontFallbacks` (Noto Color Emoji).

## Variantes acceptées (`--variant k=v`) par scénario
(le premier est le défaut ; `N` = `--n`)
| scénario | N | variantes |
|---|---|---|
| `list-scale` (L1) | lignes (100 000 → 10 M) | `impl=panel\|canvas\|vsp` ; vsp : `model=list`(défaut)`\|oc`, `cache=0\|1…`, `recycle=1\|0` |
| `list-rich` (L2) | K ∈ {6,12,24,48,96} | `impl=panel\|canvas\|vsp` |
| `list-mutate` (L3) | mises à jour de prix/s (opérations structurelles = N/100 /s) | `impl=panel\|canvas\|vsp` ; vsp : `model=oc`(défaut)`\|list` |
| `list-reset` (L4) | lignes (10 k → 1 M) | `impl=panel\|canvas\|vsp` |
| `text-big` (T1) | Mo (1, 10, 50, 100) | — |
| `text-labels` (T2) | étiquettes | `impl=widgets\|canvas` |
| `text-edit` (T3) | caractères/s (information) | — |
| `text-rich` (T4) | spans | — (`impl=edit` non implémenté) |
| `gfx-shapes` (G1) | formes | `feature=solid\|gradient\|shadow\|clip\|all` ; `impl=skia\|avalonia` ; `shader=cached\|perdraw` ; `clipopt=natural\|corner` |
| `gfx-paths` (G2) | chemins | `feature=solid\|dashed` |
| `gfx-chart` (G3) | points (10 k, 100 k, 1 M) | — |
| `gfx-images` (G4) | images | `feature=big` (N ≥ 8192 → 8192², sinon 4096²) |
| `gfx-layers` (G5) | groupes | `feature=blur` |
| `ui-tree` (U1) | nœuds | — |
| `ui-bind` (U2) | étiquettes | `impl=compiled\|manual\|reflection` |
| `ui-anim` (U3) | éléments | `impl=composition\|ui` |
| `ffi-calls` (F1) | octets du tampon (8, 1024, 65536, 1048576) | `call=noop\|add\|hash\|fill`, `threads=1\|4`, `gct=0\|1` |
| `ffi-callbacks` (F2) | événements/s (10 k → 10 M) | `impl=…` (liste) ; `transfer=ring\|post` |
| `ffi-block` (F3) | ms de `core_heavy` (10, 100, 1000) | `where=bg\|ui` ; `impl=…` |
| `latency-load` (I1) | — | `load=idle\|fling\|heavy\|mutate` (`cpuhog` : côté harnais) ; `impl=…` |
| `latency-wheel` (I2) | — | `impl=…` ; molette = `Window.PointerWheelChanged`, 100 px par cran |
| `idle` | — | — |
Tous les scénarios sauf `idle` portent le carré de latence 200×200 en (100,100) (Espace → noir/blanc via `Window.KeyDown`, `ops.key_events` ; T3 : bascule dans `TextArea.TextEntered`). `soak`, `mem-min`, `startup-scale` : `unsupported` + raison.

