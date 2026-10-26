# Journal d'optimisation — Avalonia 12.1.4 / F# / NativeAOT (passe v3)

Méthode : chaque ligne vient d'une fumée **non officielle** (binaire NativeAOT, 8 s mesurées + 2 s de warm-up, 1 exécution, Xvfb 1280×720, Skia logiciel, machine **partagée** avec d'autres compilations : bruit de ± 10-20 % sur les FPS limites, davantage sur les max/p99). Les fichiers bruts sont dans `logs/ab-*.txt` (configs `logs/ab-*.cfg`, script `ab.sh`). Colonnes : fps = frames/durée des frames ; `>25` = frames > 25 ms ; `gc` = gen0/gen1/gen2 (comptes cumulatifs .NET) et pause totale. Sources : chemins relatifs à `/opt/src/avalonia` (tag 12.1.4, commit 2d79ba55fae1b1e71c69f93a668080c9029e6a31).

Règle appliquée : l'implémentation par défaut de chaque scénario est la voie *retenue* (widgets/idiome du projet) ; les voies alternatives sont exposées en `--variant` pour que le harnais puisse les mesurer, mais ne sont pas utilisées par défaut quand elles réduisent le travail (ex. liste dessinée par canvas pour L2).

## 0. Options globales (tous scénarios)
| option (publique) | effet mesuré | source |
|---|---|---|
| `X11PlatformOptions.RenderingMode=[Software]` | évite un repli GLX/EGL incertain sous Xvfb ; Skia CPU | `src/Avalonia.X11/X11Platform.cs:221-286` |
| `X11PlatformOptions.UseRetainedFramebuffer=true` | **v0 (false)** : 1 allocation de 3,7 Mo (`UnmanagedBlob` + `GC.AddMemoryPressure`) par frame → **un GC gen2 par frame** : L1 N=1 M `retained=0` : 247 GC (toutes gen2), pause totale **626 ms** en 8 s ; G1 solid N=5000 : 121 gen2, 142 ms, 33,2 → 30,1 FPS. **v1 (true)** : 0 GC. | `src/Avalonia.X11/X11Platform.cs:500-507` (doc XML), `X11FramebufferSurface.cs:45-49` (libération à chaque blit si non retenu), `Avalonia.Base/Platform/Internal/UnmanagedBlob.cs:45` (`AddMemoryPressure`) ; `logs/ab-g1.txt` |
| `CompositionOptions.UseRegionDirtyRectClipping=true` | zones sales multi-rectangles (par défaut une seule boîte englobante). **Effet non mesurable ici** : L1 N=1 M `dirty=single` 60,9 FPS / `region` 61,3 ; G1 solid N=5000 35,2 (single) vs 33,2 (region) FPS (bruit) : le carré de latence et le défilement donnent de toute façon une zone ≈ plein cadre. Gardé activé (option documentée, aucun coût visible). | `src/Avalonia.Base/Rendering/Composition/CompositionOptions.cs:11`, `Server/ServerCompositionTarget.cs:65-75` |
| `FontManagerOptions.FontFallbacks` = Noto Color Emoji (U+1F000-1FAFF, U+200D, U+FE0F) | emoji couleur et familles ZWJ corrects (capture T1/T3/T4) | doc API FontManagerOptions |
| `ConcurrentGarbageCollection` (défaut) | GC non concurrent (`DOTNET_gcConcurrent=0`) : L2 K=96 51 → 44 FPS ; `GCgen0MaxBudget`/`GCgen0size` : sans effet visible | fumée 8 s |
Non retenu : `BENCH_PACING=uncapped` (`SleepLoopRenderTimer`/`IRenderLoop` sont `[PrivateApi]`, `X11Platform.cs:82-115`) → `ops.pacing="60-only"`.

## L1 list-scale (100 k → 10 M lignes, hauteurs variables, `sum_heights` exact)
| version | description | résultat |
|---|---|---|
| v0 | `ItemsControl`+`VirtualizingStackPanel`, modèle **liste virtuelle** O(1) (`IList`, pas de matérialisation), `FuncDataTemplate` **sans recyclage** (`recycle=0`) : `RowView` recréé à chaque réalisation | N=1 M : 60,6 FPS, 3 GC (gen2 : 1), pause 100 ms, 345 Mo alloués |
| v0' | idem mais modèle **matérialisé** `ObservableCollection<RowItem>` (1 M objets) | N=1 M : 59,0 FPS, setup 613 ms, 3 GC dont 1 gen2, pause 260 ms, max d'arrêt UI 184 ms (vs 60 ms en liste virtuelle) → `ItemsSourceView` copie tout `IEnumerable` non `IList` (`src/Avalonia.Controls/ItemsSourceView.cs:278-282`) ; matérialiser 10 M d'éléments n'est pas tenté |
| v1 | `FuncDataTemplate(..., supportsRecycling=true)` + `RowView.OnDataContextChanged` (le conteneur recyclé est re-lié par `Bind`, sans nouvelle allocation de contrôles) | N=10 M : 60,7 FPS, 2 GC (gen1), pause 89 ms, 222 Mo |
| v1b | `CacheLength=1` (public, défaut 0) | N=1 M : 60,0 FPS, **pas de gain** (max 97 ms, pause 137 ms) : plus d'éléments réalisés à chaque remesure (`VirtualizingStackPanel.cs:58-60`) |
| **v2 retenu `panel`** | panneau virtualisé maison (`PanelListView`) : pool fixe de `RowView` (enfants à positions fixes, sans Grid), **extent exact** par préfixe des hauteurs par blocs de 64 (O(N/64) octets, `RowAt` = recherche binaire), rebind seulement si identité/prix/hauteur changent | **N=10 M : 61,1-61,3 FPS, 0 GC, 51 Mo alloués, setup 91-164 ms**, p99 21-24 ms, `sum_heights`=8 799 584 (N=100 k) |
| `canvas` | un `Control` + `ICustomDrawOperation` (lease Skia) | N=10 M : 59,9 FPS (p99 33,7, 12 frames > 25 ms) |
Justification : le panneau de la pile (`VirtualizingStackPanel`) **estime** l'extent des éléments non mesurés (`EstimateElementSizeU`, moyenne des éléments réalisés : `VirtualizingStackPanel.cs:73,806-832`) ; la spec exige un extent exact (hauteurs variables) et un décalage scripté au pixel ; la doc Avalonia déconseille d'ailleurs ce panneau pour des tailles variables (voir https://docs.avaloniaui.net/docs/app-development/performance). Les trois voies tiennent 60 Hz jusqu'à 10 M ; le panneau maison est retenu parce qu'il a l'extent exact, le plus petit p99 et zéro GC. `ItemsRepeater` : absent du paquet Avalonia 12.1.4 (`Avalonia.Controls.ItemsRepeater` est un paquet séparé, non évalué).

## L2 list-rich (100 k lignes denses, K éléments par ligne)
| K | panel (retenu, widgets) | canvas | vsp (recyclage) |
|---|---|---|---|
| 6 | 59,6 FPS | — | — |
| 24 | 59,3-59,8 FPS, p99 28 ms, pause 190-216 ms | 61,3 | 48,2 FPS, p99 100, GC 9/9/4, pause 624 ms |
| 48 | 58,7-59,7 FPS, p99 31-48 ms | — | — |
| 96 | **51-53 FPS** (53 frames > 25 ms), 1,26 Go alloués, 10 GC (pause 330-360 ms) | **60,6-61,0 FPS**, p99 24 ms, 115 Mo | **13,2-18,9 FPS**, 23 GC (gen2 : 7), pause 1,6 s |
- v0 panel : liaison des lignes dans `MeasureOverride` (le rebind invalide les mesures des enfants pendant la mesure → 2ᵉ passe de mise en page par frame) + `String` neuve par puce. v1 : (a) table de 260 chaînes pré-calculées pour les puces, (b) rebind déplacé **avant** la passe de mise en page (`PanelListView.Sync`) : K=96 28-53 FPS (v0, deux séries) → 51-53 FPS (v1, 5 séries) ; K=24/48 inchangés (dans le bruit). Gain non démontré hors bruit ; conservé car il supprime la 2ᵉ passe de layout par construction.
- Cause de la rupture à K=96 : chaque frame re-lie ≈ 20 lignes × 96 contrôles (textes → `TextLayout`, pinceaux) ≈ 3 Mo alloués par frame, pas de zone morte exploitable à 20 000 px/s.
- Choix : `panel` reste le défaut (contrôles réels, comme les listes à composants des autres piles). `canvas` **réduit le travail** (pas de contrôles, pas de mise en page) : exposé en variante, pas en défaut.
- `vsp` : `ListBox`/`ItemsControl` standard ; avec K élevé le coût de re-templating domine.
- GC : `DOTNET_gcConcurrent=0` 51 → 44 FPS ; `GCgen0MaxBudget`, `GCgen0size` sans effet.

## L3 list-mutate (100 k lignes, N mises à jour/s + N/100 opérations structurelles/s)
| version | N=100 k/s | N=1 M/s |
|---|---|---|
| **panel (retenu)** : producteur sur un thread de fond → file SPSC sans verrou (1 M entrées) → application par lot à chaque frame (prix : tableau ; insertion/suppression/déplacement : `Array.Copy` + recalcul du préfixe **une fois par lot**) | 61,1 FPS ; 810 048/810 048 appliquées ; 0 GC | 61,2-61,4 FPS ; 8 106 989/8 106 989 ; 0 GC |
| canvas | — | 61,2 FPS, 8 100 953/8 100 953 |
| vsp, `ObservableCollection<RowItem>` (1 notification par opération structurelle, `INotifyPropertyChanged` par ligne pour le prix) | 61,4 FPS, 0 frame > 25 ms, 31 Mo | 59,8 FPS, 15 frames > 25 ms, 48 Mo |
| vsp, liste personnalisée + `Reset` **groupé** par frame (`INotifyCollectionChanged`) | 60,5 FPS, **2 GC, pause 115 ms, arrêt UI max 85 ms**, 337 Mo (le Reset recycle tous les conteneurs et réalloue des `RowItem`) | — |
Conclusion : à 1 000 opérations structurelles/s, une notification par opération (ObservableCollection) est moins chère qu'un Reset par frame. À 10 M/s (`ops.updates_generated` 81 M) le thread UI applique 46 M (57 %) et tombe à 10 FPS : au-delà de la capacité (le producteur managé n'est pas le facteur limitant : il génère 81 M en 8 s).

## L4 list-reset (remplacement complet du modèle toutes les 2 s)
Modèle reconstruit sur un thread de fond (`Task.Run`) puis échangé sur le thread UI (`Dispatcher.UIThread.Post`) : N=1 M : 61,2 FPS, reconstruction 3-7 ms, arrêt UI max 8-52 ms (panel), 26 ms (vsp), 52 ms (canvas). Pas de v0 mesurée (le modèle statique est O(N/64)) : la reconstruction d'un modèle matérialisé coûterait O(N) objets (voir L1 v0').

## F2 ffi-callbacks (producteur natif → UI)
| version | N=100 k/s | N=1 M/s | N=10 M/s | N=20 M/s |
|---|---|---|---|---|
| v0 : **un `Dispatcher.UIThread.Post` par événement** (`transfer=post`) | 61,3 FPS, 4 GC, 427 Mo alloués, 800 524/800 524 | **ne termine pas** (rc=124 après 120 s : file de dispatcher saturée) | — | — |
| **v1 retenu** : callback = 1 écriture dans une file SPSC sans verrou (2 M entrées), drainage une fois par frame | 61,3 FPS, 801 690/801 690, 11 Mo | 61,5 FPS, 8 011 163/8 011 163 | 60,5 FPS, 80 125 487/80 125 487, arrêt UI max 58 ms | 61,3 FPS, 160 256 305/160 256 305 |
Source : un `Post` à priorité ≥ `Input` n'est jamais interrompu par l'entrée (`src/Avalonia.Base/Threading/Dispatcher.Queue.cs:164-185`) ; coalescence non nécessaire avec la file (chaque événement est appliqué, aucun n'est fusionné). Callback : delegate blittable + `Marshal.GetFunctionPointerForDelegate` faute de `UnmanagedCallersOnly` utilisable en F# (voir NOTES). Le coût du thunk n'a pas pu être comparé à un vrai `UnmanagedCallersOnly` (pas de shim C#).

## F3 ffi-block
`core_heavy` sur un fil de fond : arrêt UI max 11,4 ms (100 ms) / 9,9 ms (1000 ms), 61 FPS ; variante `where=ui` : arrêt 103,7 ms (100 ms) / **1003 ms** (1000 ms), 1 frame de 1004 ms.

## F1 ffi-calls (1 000 formes solid animées en UI, 60 FPS tenus)
| appel | 1 thread | note |
|---|---|---|
| `core_noop` | **577,6 M appels/s** (`SuppressGCTransition`) vs **265,4 M/s** avec transition GC (`gct=1`) : ×2,2 | 4 threads : 1,36 G/s (FPS 55) |
| `core_add` | 637,6 M/s | |
| `core_hash` 8 o | 256,2 M/s (1 954 Mo/s) | |
| `core_hash` 1 Ko | 749 k/s (731 Mo/s) | |
| `core_hash` 64 Ko | 11,2 k/s (697 Mo/s), transition GC conservée | |
| `core_hash` 1 Mo, 4 threads | 2 285 Mo/s (FPS 56,7) | |
| `core_fill` 64 Ko | 464 Mo/s | |
Règles : `SuppressGCTransition` seulement pour les appels < ~1 µs sans rappel (doc https://learn.microsoft.com/en-us/dotnet/api/system.runtime.interopservices.suppressgctransitionattribute) ; tampon sur le Pinned Object Heap ; compteurs par thread séparés de 128 octets (pas de faux partage).

## G1 gfx-shapes (N=1 000, canvas Skia ; fumée 8 s)
| version | solid | gradient | shadow | clip | all |
|---|---|---|---|---|---|
| **retenu `impl=skia`** (lease Skia, paints/shaders/filtres créés une fois) | 60,9 | 38,2 | 36,8 | 61,4 | 18,1 |
| gradient : shader recréé à chaque dessin (`shader=perdraw`, v0) | | 37,9 (**alloc 82 Mo** vs 4 Mo) | | | |
| clip : optimisation « rect + rrect aux coins » (`clipopt=corner`) | | | | 61,3 (identique) | |
| `impl=avalonia` (primitives `DrawingContext` + `BoxShadows` + brosses immuables) | 61,2 | 32,9 (1 GC, pause 55 ms, 124 Mo) | **14,2** | 61,1 | **9,3** |
- Le lease Skia est ≈ 2,6× plus rapide que `DrawRectangle(BoxShadows)` pour l'ombre (14,2 → 36,8 FPS) : `DrawingContextImpl.cs:334` crée un `SKImageFilter.CreateBlur` **par dessin** (`src/Skia/Avalonia.Skia/DrawingContextImpl.cs:334`) et les dégradés un `SKShader` par dessin (`:927,939`) ; la voie lease garde un seul `SKMaskFilter` d'ombre et un shader par forme.
- Re-créer le shader de dégradé à chaque dessin ne change pas les FPS (le coût est le raster CPU du dégradé) mais multiplie les allocations par 20.
- `clip` : le clip rrect AA est appliqué **une fois** à toute la scène (`ClipRoundRect`, naturel) : coût nul mesuré à N=1 000 (61,4 FPS). L'optimisation de v2 (clip rect + rrect aux coins, rendu identique) n'apporte rien à cette taille ; elle reste disponible (`clipopt=corner`) mais n'est **pas** le défaut (l'objet de la variante est justement de mesurer le clip).
- Capacité (N ↑) : solid N=5 000 : 33 FPS ; N=50 000 : 3,7 FPS ; all N=20 000 : 0,9 FPS (dégradation continue, pas d'effondrement brutal).
- `PushOpacity` n'est pas utilisé ici (G5 : voir plus bas).

## G2 gfx-paths : N=100 : 44,7 FPS ; N=1 000 : 5,4 ; dashed N=100 : 36,9 ; N=10 000 : 0,5. Chemins `SKPath` créés une fois, animés par translation (le contenu change à chaque frame).
## G3 gfx-chart (1 000 points ajoutés/frame) : N=10 k : 60,6 FPS ; 100 k : 15,1 ; 1 M : 1,9 (tout le travail est `DrawPoints(Polygon)` AA de N segments + conversion `SKPoint[]` ; tableau réutilisé). Source : `ISkiaSharpApiLeaseFeature` est `[Unstable]` (`src/Skia/Avalonia.Skia/ISkiaSharpApiLeaseFeature.cs:8`).
## G4 gfx-images
| échantillonnage | n=10 | n=100 | n=1000 |
|---|---|---|---|
| v0 `high` (bilinéaire + mipmaps, équivalent de `BitmapInterpolationMode.HighQuality`) | 50,6 FPS | 6,1-6,3 | 0,6 |
| **v1 retenu `low`** (bilinéaire seul, équivalent `LowQuality`, aligné sur les autres piles) | — | **27,5** | 2,9 |
`big` : 4096² 6,8 FPS, 8192² 4,0 FPS (pan/zoom/rotation). Les 600 PNG sont décodés **une fois** (`SKBitmap.Decode`→`SKImage`, 0,95-1,3 s de setup).
## G5 gfx-layers (vrai groupe = `SaveLayer`, alpha 0,5) : N=4 : 61,2 FPS ; N=16 : 57,2 ; blur 4 px : N=4 26,4 FPS, N=16 7,0, N=100 1,1. Distinction : `DrawingContext.PushOpacity` **n'ouvre pas de couche** par défaut (l'alpha est multiplié dans chaque primitive, `DrawingContextImpl.cs:713-741`, `SkiaOptions.UseOpacitySaveLayer=false`, `RenderOptions.RequiresFullOpacityHandling`) : ce qui est mesuré ici est donc un **vrai groupe** (`SKCanvas.SaveLayer`), sémantiquement équivalent à un groupe d'opacité des autres piles ; la voie widgets (`Opacity=0.5`) n'est pas implémentée. Le flou est `SKImageFilter.CreateBlur(4,4)` créé une fois.

## U1 ui-tree (fumée `poke.sh resize`, 10 aller-retours 1280×720 ↔ 1100×650, AOT)
N=1 000 : frames de redimensionnement 24-77 ms ; N=10 000 : 165-494 ms ; N=100 000 : 3,0-3,3 s (2 frames) ; construction : N=100 k setup 12,4 s, **N=500 k setup 79,5 s** (RSS élevé, aucun plantage, 0 redimensionnement envoyé dans cette fumée). Arbre `StackPanel` imbriqués + `Border`/`TextBlock` : mise en page non virtualisée O(N) à chaque redimensionnement (largeur stretch). Pas d'optimisation retenue : un arbre statique non virtualisé est précisément ce que mesure la spec (voir NOTES pour la définition de N).

## U2 ui-bind (N étiquettes, N propriétés changent à chaque frame)
| version | N=1 000 | N=5 000 |
|---|---|---|
| **retenu `compiled`** : `CompiledBinding.Create` (arbre d'expression construit à la main : l'`Expression<_>` F# par quotation échoue en AOT, voir NOTES), 1 gabarit partagé, `INotifyPropertyChanged` (événement F#) levé sur le thread UI | 42,3 FPS, 12 GC, **1,66 Go alloués**, pause 305 ms | 8,6 FPS |
| `manual` (affectation directe de `Text`, hors spec) | 52,2 FPS, 14 GC, 2,0 Go | 8,7 FPS |
| `reflection` (`Binding("Count")`, `ReflectionBinding` : `RequiresUnreferencedCode`/`RequiresDynamicCode`, `src/Avalonia.Base/Data/ReflectionBinding.cs:15-19`) | 44,9 FPS, 14/12/1 GC | — |
Le coût dominant est la mise en forme du texte (`TextLayout`) à chaque changement (≈ 4,8 Ko/maj), pas la liaison : la liaison compilée coûte ≈ 20 % de FPS à N=1 000 par rapport à l'affectation directe. N=100 000 : setup 10,1 s, 0,2 FPS (sans plantage).

## U3 ui-anim
| version | N=1 000 | N=3 000 / 10 000 |
|---|---|---|
| **retenu `composition`** (`Compositor.CreateVector3DKeyFrameAnimation` Offset + `CreateScalarKeyFrameAnimation` Opacity, boucle infinie alternée, évaluées sur le thread de rendu, aucun code par frame) | 61,3 FPS, 0 GC, 4 Mo | N=10 000 : 10,7 FPS (setup 409 ms) ; N=100 000 : 0,9 FPS (setup 5,6 s) |
| `ui` (`Avalonia.Animation.Animation` + KeyFrames sur `Canvas.Left/Top` et `Opacity`, évalué sur le thread UI) | 59,3 FPS, 3 GC, 485 Mo, arrêt UI 80 ms | N=3 000 : 34,3 FPS, 7 GC, 842 Mo |
Équité : seules les animations de composition sont l'équivalent d'un moteur d'animation hors code applicatif (comme QML/Slint) ; les deux voies sont mesurables, la voie composition est le défaut.

## T1 text-big (1 / 10 / 50 / 100 Mo, défilement 30 000 px/s aller-retour, tout sélectionner à 5 s)
| Mo | FPS | 1ᵉʳ affichage | layout_ms | setup_ms | GC |
|---|---|---|---|---|---|
| 1 | 19,4 (18,9) | 138 ms | 52 | 157 | 2/2/1, pause 297 ms |
| 10 | 19,1 | 253 ms | — | 275 | 2/1/0, pause 327 ms |
| 50 | 18,3 | 659 ms | — | 677 | 3/3/1, pause 485 ms |
| 100 | 18,4 | 1 542 ms | 175 | 1 563 | 2/2/0, pause 423 ms |
`AvaloniaEdit.TextEditor` 12.0.0 (rope + virtualisation de lignes) : le temps de première image croît ≈ linéairement avec la taille (lecture + construction du rope) mais le FPS ne dépend pas de la taille : le goulot est la mise en forme des ≈ 35 lignes visibles par frame (CJK/emoji/arabe, bidi) à 500 px/frame (v2 à 3 000 px/s : ≈ 58 FPS). Un `TextBox` (v2) mettait 14 s à s'afficher pour 1 Mo (non retesté). Pas de contrôle de document virtualisé dans le paquet Avalonia gratuit (le `RichTextEditor` est Pro) ; chargement synchrone (avant la 1ʳᵉ frame), pas de chargement en arrière-plan (le `TextDocument` a une affinité de thread : non tenté). `TextEditor.ScrollToVerticalOffset` est sans effet en 12.0.0 : défilement par `ScrollViewer.Offset` du modèle.
## T2 text-labels (le texte change à chaque frame)
| N | widgets (`TextBlock` + `Text`) | canvas (`SKCanvas.DrawText`, sans shaping HarfBuzz) |
|---|---|---|
| 500 | 58,4 FPS, 8 GC, 1,15 Go | 61,5 FPS, 71 Mo |
| 3 000 | 12,6 FPS | 60,6 FPS |
| 20 000 | 1,6 FPS (pause GC 2,5 s) | 15,5 FPS |
Le défaut est `widgets` (retenu : composants, mise en forme complète) ; `canvas` ne fait pas de shaping : c'est une variante qui réduit le travail.
## T3 text-edit : `TextEditor` éditable 1 Mo ; le carré bascule dans `TextArea.TextEntered` (après insertion dans le `TextDocument`) : fumée poke : 30 frappes → 30 `text_entered`, 30 `key_events`. Latence externe : mesurée par le harnais.
## T4 text-rich : `TextBlock.Inlines` (N `Run`) : N=1 000 : 20,6 FPS ; 10 000 : 7,5 FPS (setup 3,2 s) ; 100 000 : 1,0 FPS, **setup 99 s** (sans plantage). Variante `AvaloniaEdit` + colorizer (virtualisé) : non implémentée.
