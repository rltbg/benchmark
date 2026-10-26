# Avalonia 12.1.4 + F# + .NET 10 (NativeAOT et JIT) — notes de stack (passe v2, 2026-10-26)

## Versions (vérifiées le 2026-10-26)
| composant | version | source |
|---|---|---|
| .NET SDK | 10.0.403 (`/opt/dotnet10`, `dotnet --version`) ; runtime 10.0.14 ; ILCompiler 10.0.14 (dernière 10.0.x stable sur NuGet) | `dotnet --list-sdks/--list-runtimes`, api.nuget.org flatcontainer `microsoft.dotnet.ilcompiler` |
| F# / FSharp.Core | FSharp.Core **10.1.403** (publié 2026-10-25, dernière stable listée ; 11.0.100 est *non listé* sur NuGet, 11.0.101 est une RC) — même version que celle livrée par le SDK (`sdk/10.0.403/FSharp/library-packs`), épinglée explicitement | api.nuget.org registration `fsharp.core` |
| Avalonia, Avalonia.X11, Avalonia.Skia, Avalonia.HarfBuzz, Avalonia.Themes.Simple | **12.1.4** (dernière stable) | api.nuget.org flatcontainer |
| Avalonia.AvaloniaEdit | **12.0.0** (dernière stable, propriétaire `avaloniaui`, MIT, dépend d'Avalonia ≥ 12.0.0) | api.nuget.org ; tag git `12.0.0` de github.com/AvaloniaUI/AvaloniaEdit |
| SkiaSharp / HarfBuzzSharp | 3.119.4 / 8.3.1.3 (versions exactes imposées par Avalonia.Skia 12.1.4 ; SkiaSharp 4.153.1 existe mais est une version majeure non supportée par Avalonia 12.1.4 → non forcée) | nuspec Avalonia.Skia 12.1.4 |

Champ JSON `version` (exemple) : `Avalonia 12.1.4 (X11/Skia/HarfBuzz/Themes.Simple 12.1.4), Avalonia.AvaloniaEdit 12.0.0, SkiaSharp 3.119.4, HarfBuzzSharp 8.3.1.3, FSharp.Core 10.1.403, .NET SDK 10.0.403, runtime .NET 10.0.14, NativeAOT, Avalonia assembly 12.1.4` (runtime et version d'assembly Avalonia lus à l'exécution).

## Build / lancement
- `./build.sh` (vérifie le SDK 10.0.403, `-m:2`) → `out/aot/` (NativeAOT, `libbenchcore.a` liée statiquement) et `out/jit/` (self-contained JIT, `libbenchcore.so` résolue par `BENCHCORE_LIB` puis `$BENCH_ROOT/core/target/release/libbenchcore.so`, `BENCH_ROOT` sinon déduit en remontant depuis le binaire jusqu'à `core/benchcore.h`). Logs : `logs/publish-aot.log`, `logs/publish-jit.log`, `logs/warnings.txt`.
- Temps de build (machine partagée, load ~6) : AOT 58,7 s–1 min 22 s, JIT 8,2 s.
- Lignes `harness/stacks.tsv` (à reporter, le fichier n'est pas dans mon dossier) :
  - `avalonia-aot	${ROOT}/stacks/avalonia/out/aot/avalonia-bench`
  - `avalonia-jit	${ROOT}/stacks/avalonia/out/jit/avalonia-bench`
- Options hors spec : `--no-suppress-gc-transition` (core/stress), `--text-control textbox` (ancienne voie TextBox, pour comparaison). `run1.sh <aot|jit> <scenario> <durée>` : fumée manuelle.
- Code : `Program.fs` 1194 lignes, `TextStyles.fs` 10, `TextStyles.axaml` 7, fsproj 50, build.sh 22.

## Définition de frame (notes JSON)
`frame_ms[i]` = intervalle entre deux **fins de rendu** consécutives du thread de rendu : `CompositionBatch.Rendered` (API publique, obtenue par `Compositor.RequestCompositionBatchCommitAsync()` dans le callback `TopLevel.RequestAnimationFrame` qui modifie le contenu ; continuation synchrone horodatée sur le thread de rendu). Dans Avalonia 12.1.4, `Rendered` est signalé après `Render()` de toutes les cibles, c.-à-d. après le raster Skia et le present X11 (`XPutImage` + `XSync`) (`ServerCompositor.RenderReentrancySafe`, `X11FramebufferSurface.Blit`). Une frame = un batch qui porte une modification (nav : seulement pendant les transitions). À la fin, l'app attend le `Rendered` des batches déjà commités avant `BENCH:measure_end`.

**Plafond 60 Hz conservé** : X11 crée `SleepLoopRenderTimer(DesiredFps = max(60, taux XRandR max))` dans `AvaloniaX11Platform.Initialize` (X11Platform.cs l. 82-115, tag 12.1.4) ; ni `X11PlatformOptions` ni `CompositionOptions` n'exposent de FPS (`ShouldRenderOnUIThread` → `UiThreadRenderTimer(60)`). Le contournement v1 (réflexion sur `DefaultRenderLoop.Timer`, `AvaloniaAccessUnstablePrivateApis`) est **supprimé**. Mesuré : p50 ≈ 16,4 ms.

## Revue (écarts trouvés → correction, avec justification)
1. **API privée pour lever le plafond 60 Hz** (`AvaloniaAccessUnstablePrivateApis`, `AvaloniaLocator`, réflexion `DefaultRenderLoop.Timer`, `--capped`) → supprimé, plafond documenté. Brief v2 ; https://docs.avaloniaui.net/api/avalonia/x11platformoptions, https://docs.avaloniaui.net/api/avalonia/rendering/composition/compositionoptions (aucune option FPS).
2. **Mesure de frame au début de frame** (intervalle entre callbacks RAF) → fin de rendu `CompositionBatch.Rendered` (voir plus haut). Source Avalonia 12.1.4 `Rendering/Composition/Transport/Batch.cs` (doc XML : « Indicates that batch got rendered on the render thread »).
3. **GC gen2 à chaque frame** (bug de perf trouvé) : avec `UseRetainedFramebuffer` par défaut (false), X11 alloue un framebuffer de 3,7 Mo (`UnmanagedBlob` + `GC.AddMemoryPressure`) à chaque frame : 117 GC gen2 en 4 s (list, JIT). → `X11PlatformOptions.UseRetainedFramebuffer = true` (option publique documentée : « improving performance by saving a blit ») : 0 GC dans list/stream/canvas/core/stress. https://docs.avaloniaui.net/api/avalonia/x11platformoptions
4. **Canvas** : shaders de dégradé déjà créés une fois par forme (OK) ; la liste recréait 6 `SKPaint` par frame → `SKPaint`/`SKFont` partagés créés une fois (doc : « Reuse Pen, Brush… avoid allocations », https://docs.avaloniaui.net/docs/graphics-animation/custom-rendering). L'approche `ICustomDrawOperation` + `ISkiaSharpApiLeaseFeature` est celle de la doc (même page, section « ICustomDrawOperation for SkiaSharp »). Profil (JIT, 5000 formes, ms/frame de Render) : complet 150-190 ; sans clip rrect AA 72-84 ; sans dégradé 26-53 ; sans ombre −25 ; sans graphe −10. Le clip **rrect AA** force un masque de couverture pour chaque forme dans le raster Skia CPU → clip rectangle (bords entiers, couverture 0/1 identique) pour toutes les formes et clip rrect AA seulement pour les formes (ombre comprise) qui touchent un des 4 coins de rayon 16 : rendu identique, **render ≈ 88-96 ms au lieu de ≈ 150-190** (≈10 FPS au lieu de ≈6). Le coût restant est le dégradé par forme (raster CPU) ; pas de cache de sprites (ce serait changer la méthode de rendu). Le graphe est pré-rempli (historique de 10 000 points) pour être visible sur toute la largeur.
5. **Texte 1 Mo** : `TextBox` ne virtualise pas (AOT, mesuré : layout_ms 14 160, time_to_first_display 15 310 ms, `SelectAll` 42,7 s, 5,9 FPS). La doc Avalonia ne propose comme contrôle de document virtualisé que `RichTextEditor` (Avalonia **Pro**, licence : https://docs.avaloniaui.net/controls/documents/richtexteditor/) ; `TextBox`/`SelectableTextBlock` n'ont aucune guidance pour gros textes (https://docs.avaloniaui.net/controls/input/text-input/textbox, .../selectabletextblock). Voie retenue : **AvaloniaEdit 12.0.0** (paquet AvaloniaUI, README : « Line virtualization », document en rope) en `IsReadOnly`, `WordWrap` : AOT layout_ms 42-54, time_to_first_display 127-151 ms, ~58 FPS. Bug AvaloniaEdit 12.0.0 : `TextEditor.ScrollToVerticalOffset` est un no-op (corps commenté) → `ScrollViewer.Offset` du template. Styles AvaloniaEdit chargés par XAML compilé (`TextStyles.axaml`, x:Class) : un `StyleInclude` créé en code donne IL2026 puis `XamlLoadException` en NativeAOT (doc : « Avoid dynamic XAML loading at runtime », https://docs.avaloniaui.net/docs/deployment/native-aot).
6. **Emoji monochromes / ZWJ non fusionnés** (capture) → `FontManagerOptions.FontFallbacks` = Noto Color Emoji pour U+1F000-1FAFF, U+200D, U+FE0F (« A fallback is fulfilled before anything else », https://docs.avaloniaui.net/api/avalonia/media/fontmanageroptions). Vérifié sur capture : CJK, arabe/hébreu (RTL), emoji couleur, famille ZWJ.
7. **Liste** : `VirtualizingStackPanel` est documenté comme optimisé pour hauteurs uniformes (extent estimé, sauts de scrollbar avec hauteurs variables : https://docs.avaloniaui.net/docs/app-development/performance#variable-height-items) ; le source 12.1.4 calcule l'extent = réalisé + restant × `_lastEstimatedElementSizeU`. La spec exige un extent exact (`sum_heights` = 8 799 584) et un offset scripté au pixel → panneau virtualisé maison conservé (recherche binaire sur préfixe des hauteurs). `ItemsRepeater` (Avalonia.Controls.ItemsRepeater 12.0.0) n'est pas dans la doc 12.x.
8. **P/Invoke** : `LibraryImport` est un générateur de source **C#** (« recognizes the LibraryImportAttribute in C# code », https://learn.microsoft.com/en-us/dotnet/standard/native-interop/pinvoke-source-generation) → indisponible en F# ; `DllImport` blittable + `[<assembly: DisableRuntimeMarshalling>]` (aucun stub de marshalling, https://learn.microsoft.com/en-us/dotnet/standard/native-interop/disabled-marshalling). **NativeAOT : liaison statique** `DirectPInvoke` + `NativeLibrary libbenchcore.a` (https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/interop), désormais la variante AOT par défaut (vérifié : aucun `libbenchcore` dans `/proc/PID/maps`). `SuppressGCTransition` **par défaut** sur validate/parse/hash (conditions de la doc remplies : < 1 µs, pas d'I/O, pas de callback, pas d'exception, pas de verrou : https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.suppressgctransitionattribute) ; jamais sur `core_heavy` (100 ms). Mesuré AOT statique : 111 M triplets/s avec, 38,6 M sans (`--no-suppress-gc-transition`) ; JIT dynamique : 36-59 M/s.
9. **Chemins absolus** `/home/user/eval/...` (Program.fs, fsproj, build.sh, run1.sh) → `BENCH_ROOT`/`--assets`/`BENCHCORE_LIB`, fsproj `$(BENCH_ROOT)` ou chemin relatif au projet.
10. **Spec v2** : marqueurs `BENCH:measure_start/end` (stdout + flush) ; scénario `input` (Border 200×200 en (100,100) au-dessus de la liste, `Window.KeyDown`, Key.Space → noir/blanc, `ops.key_events`) ; `ops.gc` (`GC.CollectionCount(0..2)`, `GC.GetTotalPauseDuration()` en delta sur la fenêtre mesurée, `pause_ms_max` = max des `GC.GetGCMemoryInfo(kind).PauseDurations` relevées à chaque frame quand le compteur change : borne inférieure si plusieurs GC du même type entre deux frames) ; `ops.not_rendered: []` (canvas/stress) ; `unsupported: null` (scénario inconnu → `unsupported`) ; `version`.
11. Divers : `idle` dormait `duration+2 s` sans marqueurs → warm-up 2 s puis `duration` entre marqueurs ; `nav` bouclait sans fin → 20 transitions + 10 modales ; `core_heavy` toutes les 1 s à échéances absolues (au lieu de 100 ms + 900 ms de sommeil) ; accumulateur core publié (`ops.acc`) ; checksum déterministe (FNV-1a des hauteurs) au lieu de `fnv xor frames` ; images : pan ajouté au zoom 1→3→1 ; GC par défaut (concurrent) au lieu de `ConcurrentGarbageCollection=false` non justifié ; `IsAotCompatible=true` (doc Avalonia NativeAOT) ; FSharp.Core épinglé.

## Scénarios (état)
| scénario | état | remarques |
|---|---|---|
| idle | OK | coquille, aucun redessin (verify : 0 % de pixels changés) |
| list / stream | OK | 60 FPS plafonnés ; stream 10 000/s : 0 drop, `System.Threading.Channels` borné 100k (TryWrite → drop+compte), drain par lot à chaque frame |
| canvas | OK, lent | 5 000 formes ≈ 10 FPS, 20 000 ≈ 2,9 FPS (AOT) ; `not_rendered: []` |
| text | OK | AvaloniaEdit ; sélection totale à t=5 s visible sur capture |
| images | OK | `Bitmap.DecodeToWidth(128)` sur 2 threads (démarrés au lancement ; compteurs sur toute la vie du process) |
| nav | OK | PageSlide 300 ms ×20, CrossFade 200 ms modale toutes les 2 transitions, thread ~1 cœur |
| core | OK | UI 1000 formes |
| input | OK | latence externe p50 ≈ 29 ms (≈ 2 frames à 60 Hz), 0 perdu |
| stress | OK (8 s testées seulement) | ≈ 4-5 FPS ; `heap_samples` toutes les 10 s |

## Fumée harnais (8 s, 1 run, machine partagée load ~6 : pas des mesures) — `logs/smoke-summary.txt`
AOT : idle RSS 46,6 Mo CPU 0 % ; list 61,1 FPS p95 18,9 ms CPU 12 % RSS 51,4 Mo ; stream-10k 60,5 FPS 0 drop ; canvas 10,4 FPS ; text 58,2 FPS ttfd 127 ms layout 42 ms RSS 120 Mo (3 GC, pause max 92 ms) ; images 60,9 FPS 600 décodées ; nav 58,4 FPS 16 transitions/8 s ; core 51,7 FPS 111 M/s 8 heavy ; input 61 FPS latence p50/p95 29,3/35,2 ms 0 perdu ; stress 4,8 FPS. Tous : rc 0, `markers: True`, `sum_heights` 8 799 584, gc 0 (sauf text).
JIT : idle RSS 83,5 Mo ; list 58,7 FPS ; canvas 10,0 ; text 47,7 FPS ttfd 711 ms ; core 35,7 FPS 58,6 M/s ; input p50 29,1 ms ; stress 3,6 FPS.

## Déploiement
- `out/aot` 37,8 Mo : `avalonia-bench` 25,6 Mo + `libSkiaSharp.so` 11,2 Mo + `libHarfBuzzSharp.so` 2,8 Mo. `ldd avalonia-bench` : libm, libgcc_s, libc (X11/fontconfig chargés par dlopen). `ldd libSkiaSharp.so` : libpthread, libfontconfig, libdl, libm, libc, libfreetype, libexpat, libz, libbz2, libpng16, libbrotlidec/common.
- `out/jit` 102,2 Mo (217 fichiers, runtime self-contained non trimé) ; `ldd` apphost : libdl, libpthread, libstdc++, libm, libgcc_s, libc.

## Warnings AOT exacts (`logs/warnings.txt`, détail `logs/publish-aot-singlewarn-false.log`)
- `FSharp.Core.dll : warning IL2104: Assembly 'FSharp.Core' produced trim warnings.` et `warning IL3053: Assembly 'FSharp.Core' produced AOT analysis warnings.` — détail (TrimmerSingleWarn=false) : IL3050/IL2060/IL2070/IL2072/IL2075/IL2080/IL2067 dans `PrintfImpl`, `StructuredPrintfImpl`, `FSharp.Reflection.Impl` (atteints via `ToString()` généré des records/options F#, jamais appelé). Aucun warning venant d'Avalonia/AvaloniaEdit ni du code de l'app. Erreurs de mise au point : `logs/errors-encountered-v2.txt` ; passe v1 : `logs/v1-first-pass/`.

## Écarts / limites restants
- Plafond 60 Hz non désactivable publiquement sur X11 (gardé) ; FPS des scénarios légers = 60.
- Texte de liste via `SKFont.DrawText` (pas de shaping HarfBuzz ; contenu ASCII + « — ») ; texte du scénario `text` : shaping complet Avalonia/HarfBuzz.
- Canvas limité par le dégradé linéaire par forme en raster CPU.
- `pause_ms_max` : borne inférieure (voir revue 10). `text` : défilement aller-retour sur [0, min(extent, 30 000 px)].
- Pas d'IME réelle, pas de mesures officielles, stress 600 s non exécuté.
