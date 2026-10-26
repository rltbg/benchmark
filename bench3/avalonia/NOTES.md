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

## Tableau scénario × état
| scénario | état | remarques |
|---|---|---|
| L1 list-scale | implémenté | `panel` par défaut (extent exact, 10 M tenus à 60 Hz en fumée) ; `canvas`, `vsp` en variantes |
| L2 list-rich | implémenté | K=96 : 51-53 FPS en `panel` (rupture) ; `canvas` tient (réduit le travail : non défaut) |
| L3 list-mutate | implémenté | file SPSC bornée 1 M, lots par frame ; 100 % appliquées jusqu'à 1 M/s ; 10 M/s : 57 % |
| L4 list-reset | implémenté | modèle reconstruit en arrière-plan, échange sur l'UI |
| T1 text-big | implémenté | AvaloniaEdit ; ≈ 19 FPS à 30 000 px/s quelle que soit la taille |
| T2 text-labels | implémenté | `widgets` / `canvas` |
| T3 text-edit | implémenté | `TextEntered` → carré ; latence externe non mesurée par moi (harnais) |
| T4 text-rich | **partiel** | `TextBlock.Inlines` seulement ; pas de variante AvaloniaEdit/colorizer |
| G1 gfx-shapes | implémenté (5 variantes) | `solid`, `gradient`, `shadow`, `clip`, `all` ; `impl=avalonia` pour comparaison |
| G2 gfx-paths | implémenté | solid / dashed |
| G3 gfx-chart | implémenté | |
| G4 gfx-images | implémenté | + `feature=big` (4096²/8192²) ; échantillonnage bilinéaire (`filter=low`, défaut) |
| G5 gfx-layers | implémenté | vrai groupe (`SaveLayer`) ; pas de voie widgets `Opacity` |
| U1 ui-tree | implémenté | frames = durées de redimensionnement ; le harnais redimensionne |
| U2 ui-bind | implémenté | `CompiledBinding` (arbre d'expression manuel) |
| U3 ui-anim | implémenté | composition (défaut) / `Animation` |
| F1 ffi-calls | implémenté | noop/add/hash/fill, 1 ou 4 threads |
| F2 ffi-callbacks | implémenté (écart UCO) | delegate + `GetFunctionPointerForDelegate` ; file SPSC |
| F3 ffi-block | implémenté | `where=bg` / `where=ui` |
| I1/I2 latency-load/-wheel | implémenté (côté app) | charges `idle`, `fling`, `heavy`, `mutate` ; `cpuhog` = harnais ; molette par `PointerWheelChanged` |
| M1 soak, M2 mem-min, M3 startup-scale | **non supporté** | `unsupported` + raison (hors niveaux 1-2) ; M3 se mesure avec `ui-tree` (`setup_ms`) |
| idle | implémenté | aucun redessin, 0 frame |

## Définitions et écarts à la spec
- **U1** : N = nombre total de nœuds (conteneurs + feuilles) d'un arbre 10-aire complet numéroté en largeur (enfants de i : 10i+1..10i+10) ; conteneur = `StackPanel` vertical, feuille = `Border` coloré de 22 px + `TextBlock` court (un nœud) ; non virtualisé, dans un `ScrollViewer`. `frame_ms` = durées des frames de redimensionnement (`ops.resize_frame_ms`) : sans redimensionnement par le harnais, `frames = 0`.
- **L2** : cellule dense = pastille 16 px, titre, sous-titre (2 lignes, ellipse), barre de progression (2 `Border`), bascule (rect arrondi + cercle), prix, puis `(K-6)/2` puces (rect arrondi + texte de 2 caractères) en pas ≤ 30 px entre x=500 et x=1270 : toutes présentes même si le pas est serré (K=96 → 45 puces de 17 px).
- **L3** : indices tirés dans [0, 100 000) ; les opérations structurelles (insertion/suppression/déplacement) sont appliquées à un index ≥ première ligne visible + 1 ; suppression refusée sous 1 000 lignes.
- **T1** défilement sur [0, min(extent, 300 000 px)] à 30 000 px/s aller-retour (période ≥ 20 s).
- **G1** : les formes rebondissent dans 1000×600 pour `clip`/`all`, 1280×720 sinon ; ombre sur **chaque** forme (sigma 4, décalage (2,2)) ; dégradé 2 couleurs par forme.
- **G2** : géométrie fixe animée par translation (pas de recalcul de Bézier par frame). **G3** : 1 000 points/frame, courbe pré-remplie (historique), zone 1000×300 en (140,380).
- **G4** : 600 images cyclées (N peut dépasser 600).
- **G5** : groupe = carte 200×150 (fond + 4 rects arrondis + cercle) en `SaveLayer` alpha 0,5, dérive sinusoïdale.
- `ops.frame_ms` n'inclut pas la frame d'ouverture ; `idle` : 0 frame par construction.
- Fréquence : 60 Hz plafonnés (`60-only`) ; la mesure de FPS est donc bornée à ≈ 61,5 (période 16,3 ms observée).

## Pièges rencontrés (erreurs exactes)
1. `FS0202: This attribute is currently unsupported by the F# compiler. Applying it will not achieve its intended effect.` sur `[<UnmanagedCallersOnly>]` (voir FFI).
2. `System.NotSupportedException: This object cannot be invoked because no code was generated for it: 'Spike+Cb.OnItem(System.IntPtr,System.UInt32,System.Double)'` (`ReflectionAugments.GetFunctionPointer`, NativeAOT).
3. `System.EntryPointNotFoundException: Unable to find an entry point named 'bench_on_item' in native library.`
4. `printfn` en NativeAOT : `System.NotSupportedException: 'Microsoft.FSharp.Core.PrintfImpl+Specializations`3[…].CaptureFinal1[System.IntPtr](…)' is missing native code. MethodInfo.MakeGenericMethod() is not compatible with AOT compilation.` → aucun `printf/sprintf` dans le code (concaténation de chaînes).
5. **Quotation F# → `Expression<Func<…>>`** (nécessaire pour `CompiledBinding.Create(vm => vm.Count)`) : en AOT, `System.ArgumentNullException: Failed to bind property 'Count' (Parameter 'propName')` sur un banc minimal ; dans l'app, plantage dans `Microsoft.FSharp.Quotations.PatternsModule.deserialize` → l'arbre d'expression est construit à la main (`Expression.Property(p, typeof<LabelVm>.GetProperty("Count"))`).
6. `dotnet build` de l'app : `FS0043: The type 'X' does not have 'null' as a proper value` (types F# sans `[<AllowNullLiteral>]`), `struct` est un mot-clé (`structOps`).
7. `X11` : `System.Exception: XOpenDisplay failed` quand aucun Xvfb n'écoute sur `$DISPLAY`.
8. `F2 transfer=post` à 1 M événements/s : processus non terminé après 120 s (`rc=124` du `timeout`) — saturation de la file du dispatcher.
9. `TextEditor.ScrollToVerticalOffset` sans effet (AvaloniaEdit 12.0.0) ; un `StyleInclude` AvaloniaEdit créé en code échoue en NativeAOT (IL2026 puis `XamlLoadException: No precompiled XAML found`, constaté en v2) → `TextStyles.axaml` compilé (seul XAML du projet, repris tel quel du projet précédent).

## Warnings AOT exacts (`logs/warnings.txt`)
- `FSharp.Core.dll : warning IL2104: Assembly 'FSharp.Core' produced trim warnings.` ; `warning IL3053: Assembly 'FSharp.Core' produced AOT analysis warnings.`
- `ILC : AOT analysis warning IL3050: Native.CallbackSink.Start(Int32,UInt64,UInt32): Using member 'System.Runtime.InteropServices.Marshal.GetFunctionPointerForDelegate(Delegate)' which has 'RequiresDynamicCodeAttribute' … Use the GetFunctionPointerForDelegate<TDelegate> overload instead.` (le delegate est non générique, blittable : vérifié fonctionnel en AOT ; l'overload générique n'a pas été essayé).
- `IL3050`/`IL2026` sur `Avalonia.Data.Binding.Binding(String)` dans `BindScn` : uniquement pour la variante de comparaison `impl=reflection` (jamais le défaut).
- Aucun warning provenant d'Avalonia/AvaloniaEdit ni du reste de l'app.

## API instables / privées
- `ISkiaSharpApiLeaseFeature` (`[Unstable]`, `src/Skia/Avalonia.Skia/ISkiaSharpApiLeaseFeature.cs:8`) : utilisée pour tous les canvas. `CompositionOptions.DirtyRectMergeEagerness` (`[Unstable]`) : non utilisée. Aucune API privée, aucune réflexion sur l'interne (`typeof<LabelVm>.GetProperty` sur notre propre type).

## Déploiement / build
- `out/` : 40 Mo (`avalonia-bench` 27,9 Mo + `libSkiaSharp.so` 11,2 Mo + `libHarfBuzzSharp.so` 2,8 Mo). `ldd avalonia-bench` : libm, libgcc_s, libc (X11/fontconfig par dlopen).
- Build NativeAOT : ≈ 1 min 15 s (`-m:2`). Code : ≈ 2 940 lignes F# (`Util` 119, `Native` 139, `Harness` 338, `ListsCore` 563, `Vsp` 64, `ListScenarios` 429, `Gfx` 561, `Text` 272, `Ui` 200, `Ffi` 91, `Program` 150, `TextStyles` 10) + `TextStyles.axaml` 7.
- Fumée harnais : `harness-smoke.sh` (config `logs/harness-smoke.cfg`, sortie `logs/smoke-harness.txt`) : 27 scénarios/variantes, tous `rc=0`, `markers=True`, `checksum` stable ; `sum_heights` 8 799 584 vérifié (N=100 k).
- Vérification visuelle : captures `results/verification/avalonia-<scénario>.png` lues pour les scénarios listés dans le rapport.

## Limites connues
- T4 : pas de voie virtualisée ; U1/U2/T4 à très grand N : setup de plusieurs dizaines de secondes (sans plantage).
- `ops.pause_ms_max` : borne inférieure. Mesures de capacité : à faire par le harnais (machine partagée pendant mes fumées).
- `latency-*` : l'app ne mesure pas la latence (harnais) ; avec `load=idle`/`heavy`, 0 frame comptée tant qu'aucune touche n'arrive.
