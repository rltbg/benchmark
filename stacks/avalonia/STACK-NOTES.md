# Avalonia 12.1.4 + F# + .NET 10 (NativeAOT et JIT) - notes de stack

## Versions
- .NET SDK 10.0.113 (paquet Ubuntu, runtime 10.0.13, ILCompiler 10.0.13 depuis le cache NuGet / library-packs), F# 10 (FSharp.Core 10.0.113 implicite).
- Avalonia 12.1.4 (derniere stable sur NuGet au 2026-10-25), Avalonia.X11 12.1.4, Avalonia.Skia 12.1.4, Avalonia.HarfBuzz 12.1.4, Avalonia.Themes.Simple 12.1.4.
  Dependances : SkiaSharp 3.119.4 (libSkiaSharp.so 11.2 Mo), HarfBuzzSharp 8.3.1.3, Tmds.DBus.Protocol, MicroCom.Runtime.
- Avalonia.Desktop N'est PAS reference (il tire Win32/Native/Dialogs inutiles) : `UseX11().UseSkia().UseHarfBuzz()` seulement. Theme : Simple (le plus leger).
- Approche UI : F# pur en code, sans XAML applicatif, sans Avalonia.FuncUI, sans bindings (donc aucun probleme de reflexion de bindings en AOT). Controles custom dessines via `DrawingContext.Custom(ICustomDrawOperation)` + `ISkiaSharpApiLeaseFeature` (SkiaSharp direct : SKCanvas, SKFont, shaders/paints caches).

## Build / lancement
- `./build.sh` (3 publishes, logs bruts dans `logs/publish-*.log`) -> `out/aot/`, `out/aot-static/`, `out/jit/`.
  - AOT : `dotnet publish -c Release -r linux-x64 -p:PublishAot=true -m:2` (1 min 07 s, 2 cores).
  - JIT : `dotnet publish -c Release -r linux-x64 --self-contained true -p:PublishAot=false -m:2` (7 s) - meme code.
  - AOT-static : AOT + `-p:LinkCoreStatic=true` : `<DirectPInvoke Include="benchcore"/>` + `<NativeLibrary Include=.../libbenchcore.a>` (1 min 05 s). Fonctionne (ldd ne montre plus libbenchcore.so, +0.3 Mo).
- Commande harnais : `/home/user/eval/stacks/avalonia/out/aot/avalonia-bench` (ou `out/jit/...`, `out/aot-static/...`). Stack names JSON : `avalonia-aot` / `avalonia-jit` (detecte via RuntimeFeature.IsDynamicCodeSupported).
- Option hors spec : `--suppress-gc-transition` (core/stress : bascule sur les P/Invoke SuppressGCTransition), `--capped` (garde le timer 60 Hz d'Avalonia).
- `libbenchcore.so` est copie a cote du binaire (resolu par NativeLibrary.SetDllImportResolver sur AppContext.BaseDirectory, fallback sur /home/user/eval/core/target/release).
- Code applicatif : `Program.fs` 927 lignes (+ fsproj 41, build.sh 14). Helper de fumee : `run1.sh`.

## Rendu logiciel / plateforme
`X11PlatformOptions(RenderingMode = [| X11RenderingMode.Software |])` (nom exact verifie dans 12.1.4 ; valeurs Software/Glx/Egl/Vulkan) + `.UseSkia()`. Pas de GPU. Fenetre 1280x720, `WindowDecorations.None`, position (0,0) (sans WM, sinon placee a (10,10)).

## Mesure des frames (important)
- `frame_ms` = intervalle entre callbacks `TopLevel.RequestAnimationFrame` consecutifs (thread UI, debut de frame, chaque callback redemande le suivant). Le commit compositeur suivant attend le batch precedent cote thread de rendu, donc l'intervalle inclut le raster logiciel + present X11 (verifie : render_ops ~ frames). Ce n'est PAS un timestamp de fin de swap observable.
- `ops.render_op_ms_mean` : duree moyenne de `ICustomDrawOperation.Render` sur le thread de rendu (dessin de nos controles seul, hors flush/XPutImage).
- PACING 60 Hz : Avalonia/X11 cree `SleepLoopRenderTimer(60)` DANS le constructeur d'AvaloniaX11Platform puis cree le Compositor immediatement ; IRenderTimer/IRenderLoop sont `[PrivateApi]` (absents des assemblies de reference). Sans contournement : ~60 FPS plafonnes (mesure : p50 16.4 ms). Contournement : `AvaloniaAccessUnstablePrivateApis=true` + reflexion sur `DefaultRenderLoop.Timer` (internal) pour mettre `SleepLoopRenderTimer.DesiredFps = 100000` => ticks enchaines sans attente (~150-190 FPS en liste). `[DynamicDependency]` + `UnconditionalSuppressMessage IL2075` : fonctionne en NativeAOT (verifie). `--capped` restaure le comportement Avalonia par defaut. Ceci est un ecart "hors chemin public" a mentionner dans les comparatifs.
- Warm-up 2 s non mesure (meme script, frames non enregistrees), puis `duration` s.

## Scenarios
| scenario | etat | notes |
|---|---|---|
| idle | OK | coquille (titre, ListView 1re page, statut), DispatcherTimer, aucun redessin ; sleep duration+2 s (warm-up inclus). frame_ms vide. |
| list | OK | ListView custom (Control + ICustomDrawOperation/Skia), virtualisee par recherche binaire sur le prefix des hauteurs (VirtualizingStackPanel non utilise : hauteurs variables = mesure par item, pas teste). 100k lignes, sum_heights=8799584 verifie. Script de scroll exact de la spec (3000 px/s 4 s puis 6+ flings, clamp). rows_drawn = lignes dessinees cumulees. |
| stream | OK | thread producteur (xorshift32 seed 0x2545F491, pacing Stopwatch + Sleep(1)) -> `System.Threading.Channels` borne 100k (TryWrite ; drop+compte si plein) -> UI draine par lot a chaque frame (callback RAF). updates_dropped=0 a 10 000/s. NB : le drain est dans le callback de frame, pas via Dispatcher.UIThread.Post. |
| canvas | OK (lent) | N formes (SKCanvas direct, shader degrade lineaire par forme en cache, ombre = SKMaskFilter blur sigma 4 offset (2,2) comme BoxShadow d'Avalonia, 1/10 formes), clip rrect AA 1000x600, graphe polyline 10 000 points (DrawPoints Polygon) 1000x100 a y=610. Formes rebondissent dans la fenetre 1280x720 (clip 1000x600 en haut a gauche). Positions copiees par frame dans un anneau de 3 snapshots (thread de rendu asynchrone). |
| core | OK | P/Invoke statique `DllImport` (blittable, nativeint) ; thread worker validate+parse+hash en boucle serree, thread `core_heavy(100)` chaque ~1 s, UI anime 1000 formes (gradient + clip, sans ombre ni graphe). max_frame_stall_ms = max(frame_ms). |
| stress | OK | liste 880 px + panneau lateral 400x720 (formes 5000 par defaut, clip 380x600, graphe 380x100) + stream 5000/s + core ; echantillons `heap_samples` (GC.GetTotalMemory) toutes les 10 s ; duree 600 s par defaut. Teste 12 s seulement. |
| text | OK mais injouable | `TextBox` readonly/AcceptsReturn/Wrap avec 1 Mo : layout_ms ~12-13 s (JIT et AOT), time_to_first_display ~12.4-13.3 s, 1 stall de 35-42 s (SelectAll a t=5 s), ~1 FPS. Defilement via le ScrollViewer interne (Offset). Resultat tel quel (pas de vue virtualisee custom). |
| images | OK | `ImageGrid` custom (DrawingContext.DrawImage), 600 vignettes 128 px, `Bitmap.DecodeToWidth(128)` sur 2 threads de fond (demarre au warm-up, compteurs sur toute la vie du process), scroll 3000 px/s aller-retour + zoom 1->3->1 sur 6 s (PushTransform). 600 decodees : decode_total_ms 3.3 s (AOT) / 8.2 s (JIT) cumules. |
| nav | OK | 3 ecrans (Grid + ListView statiques), `PageSlide(300 ms)` push,push,pop,pop, `CrossFade(200 ms)` modale ouverture+fermeture toutes les 2 transitions, thread de calcul ~1 coeur. transitions_done=12 en 6 s (AOT). Boucle repetee jusqu'a duration (pas limitee a 20). |

## Resultats de fumee bruts (machine chargee par d'autres agents : load avg ~5 sur 4 vCPU, a ne pas prendre comme mesures officielles)
AOT : list ~177 FPS (p50 5.5 ms, p95 7.9) ; stream 10 000/s ~180 FPS ; canvas 5000 formes ~8.6 FPS (p50 113 ms, render_op 119 ms) ; canvas 20000 (JIT, bruit) ~1.5 FPS ; core ~40 FPS (UI 1000 formes) avec 35 M triplets/s ; stress ~5.5 FPS ; images ~104 FPS ; nav ~120 FPS.
JIT : list ~150 FPS ; stream 10 000/s ~177 FPS (harnais, 8 s : CPU moyen 88 %, RSS 94 Mo).
Canvas : le cout dominant est le degrade par forme (Skia CPU : ~18 us/forme ; sans degrade 31 ms au lieu de ~120 ms pour 5000) puis le clip AA (-25 ms sans), le graphe 10k points et l'ombre (~12 ms) sont minoritaires. Pas de cache de sprites (non fidele a la spec).
P/Invoke core (6 s, 2 runs, bruit eleve) : dynamique 20-28 M/s ; dynamique+SuppressGCTransition 29-44 M/s ; statique (DirectPInvoke .a) 35-40 M/s ; statique+SuppressGCTransition 86-91 M/s.

## Taille / demarrage / RSS (fumee)
- out/aot : 34.2 Mo = avalonia-bench 19.8 Mo + libSkiaSharp.so 11.2 Mo + libHarfBuzzSharp.so 2.8 Mo + libbenchcore.so 0.4 Mo (out/aot-static : 34.1 Mo, sans libbenchcore.so).
- out/jit : 106.9 Mo (runtime self-contained complet ; pas de trimming) ; apphost 78 Ko + ~170 dlls.
- `ldd avalonia-bench` (AOT) : libm, libc seulement (X11/fontconfig/etc. charges par dlopen). `ldd libSkiaSharp.so` : libfontconfig, libfreetype, libexpat, libz, libbz2, libpng16, libbrotlidec/common, libpthread, libdl, libm, libc. libHarfBuzzSharp.so : libc/libm/libpthread. X11 charge dynamiquement libX11 etc.
- Demarrage jusqu'a la 1re frame (wall - warm-up 2 s, `list --duration 0.3`) : AOT ~0.07 s, JIT ~1.1 s.
- RSS (max, /usr/bin/time) : idle AOT 48.7-50 Mo, JIT 88 Mo ; list AOT 52.7 Mo, JIT 98.8 Mo ; canvas AOT 55 Mo, JIT 95 Mo.

## Warnings / erreurs AOT exacts (publish-aot.log)
- `FSharp.Core.dll : warning IL2104: Assembly 'FSharp.Core' produced trim warnings.` et `warning IL3053: Assembly 'FSharp.Core' produced AOT analysis warnings.` (FSharp.Core 10.0.113, netstandard2.1).
- `warning AVA3001: AvaloniaAccessUnstablePrivateApis is Enabled ...` (voulu).
- `ILC : Trim analysis warning IL2075: AvaloniaBench.uncapRenderTimer(): 'this' argument does not satisfy 'DynamicallyAccessedMemberTypes.PublicProperties', 'DynamicallyAccessedMemberTypes.NonPublicProperties' in call to 'System.Type.GetProperty(String,BindingFlags)'...` -> resolu par DynamicDependency + UnconditionalSuppressMessage (plus present dans le build final).
- `warning FS0760` (new IDisposable) : cosmetique. Aucune erreur AOT ; aucun warning IL venant d'Avalonia (assemblies annotes trim/AOT-compatibles) ; pas de reflexion de bindings (aucun binding, aucun XAML applicatif ; le theme Simple est du XAML precompile). Aucun printf/sprintf F# (reflexion).
- Voir aussi `logs/errors-encountered.txt` : erreurs de compilation/execution exactes de la mise au point (PrivateApi, UseHarfBuzz, WindowDecorations...). `logs/restore.log`, `logs/build.sh.out`.
- Bruit reseau : la telemetrie de build d'Avalonia.BuildServices tente `av-build-tel-api-v1.avaloniaui.net:443` (bloque par le proxy, sans consequence ; `AVALONIA_TELEMETRY_OPTOUT=1` dans build.sh).

## Ecarts a la spec / limites
- Liste : pas de VirtualizingStackPanel/ItemsControl (non verifie pour 100k hauteurs variables) ; panneau custom.
- Texte de liste dessine via SkiaSharp (SKFont.DrawText, sans shaping HarfBuzz), pas via TextLayout/GlyphRun d'Avalonia : plus rapide, moins "idiomatique Avalonia", pas de bidi/complexe.
- Stream : drain dans le callback de frame (pas de Dispatcher.UIThread.Post).
- Pacing : voir plus haut (reflexion sur API privee). Pas de mesure de la vraie fin de present X11.
- Les snapshots de prix de la liste sont copies a la creation de l'op ; les positions des formes sont partagees via un anneau de 3 buffers (course benigne possible si le rendu prend >2 frames de retard).
- `--duration` de `idle` = duration + 2 s de sommeil. `text` : le stall de SelectAll depasse `duration` (le programme sort apres la fin de la frame bloquee). Checksum = fnv(sumH) xor frames (non deterministe).
- A (non) fait : ni IME, ni mesures officielles, ni 600 s de stress complets (12 s testes).
