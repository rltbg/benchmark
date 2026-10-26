# Flutter (Linux desktop, GTK3 embedder, Dart AOT release) — notes, passe 2 (spec v2 / v2.1)

## Versions (vérifiées le 2026-10-26)
| élément | version | source |
|---|---|---|
| Flutter SDK | **3.47.8 stable**, framework `737ba8f885dc774b53b442759999a312c9461e4e` (tag `3.47.8`), engine `cf636b1dfb` (hash dfd2e6cb7d) | storage.googleapis.com/flutter_infra_release/releases/releases_linux.json (`current_release.stable`, publié 2026-10-23) + `git ls-remote --tags` |
| Dart | 3.13.7 (fourni par le SDK) | idem (`dart_sdk_version`) |
| package:ffi | **2.2.0** (dernière, 2026-02-11), épinglée `ffi: 2.2.0` | pub.dev/api/packages/ffi |
| transitifs | vector_math 2.4.0, meta 1.18.3, material_color_utilities 0.13.0, characters… : épinglés par le SDK Flutter (des versions plus récentes existent sur pub.dev mais sont refusées par les contraintes du SDK) | `flutter pub get` |
| renderer | Impeller, backend OpenGL ES (« OpenGLESSDF »), Mesa llvmpipe sous Xvfb ; variante Skia (GL) | log moteur |

Pas de bêta/RC (3.50.0-0.2.pre existe en beta, non utilisée). Analytics désactivés (`flutter config --no-analytics`, `dart --disable-analytics`, `FLUTTER_SUPPRESS_ANALYTICS=true`). Le JSON contient `"version": "Flutter 3.47.8 (framework 737ba8f885, engine cf636b1dfb), Dart 3.13.7, package:ffi 2.2.0, Impeller-GLES|Skia-GL"` (dart-defines `FLUTTER_VERSION`… injectés par l'outil flutter).

## Build / exécution
- `build.sh` : clone `-b 3.47.8` (vérifie le commit), `PUB_CACHE=stacks/flutter/pub-cache`, `flutter pub get`, `flutter build linux --release` avec `CMAKE_BUILD_PARALLEL_LEVEL=2 NINJAFLAGS=-j2` ; `CLEAN=1 ./build.sh` = rebuild complet. Échec détecté par le code retour (log complet : `logs/build-release.log`).
- Temps : build complet (`CLEAN=1`, artefacts SDK déjà en cache) **16,2 s** mur, RSS max 393 Mo (logs/build-clean.log) ; ~25 s sous charge. Clone SDK : logs/00-sdk-clone.log.
- Bug de build trouvé : supprimer seulement `app/build/` fait échouer le build suivant (`CMake Error at cmake_install.cmake:112`, libapp.so absente, tampons périmés dans `.dart_tool/flutter_build`) et l'ancien build.sh masquait l'échec (pipe `tee | grep || true`) — log exact `logs/build-failed-stale-dart_tool.log`. Corrigé (code retour vérifié, `CLEAN=1` → `flutter clean`).
- **Commandes `harness/stacks.tsv`** :
  - `flutter	${ROOT}/stacks/flutter/app/build/linux/x64/release/bundle/benchapp	`
  - `flutter-skia	${ROOT}/stacks/flutter/app/build/linux/x64/release/bundle/benchapp	--env BENCH_FLUTTER_IMPELLER=0`
  - pas de variante `uncapped` (voir Cadence : `ops.pacing = "60-only"`).
- Chemins : `--assets` (défaut `$BENCH_ROOT/assets`), racine = `BENCH_ROOT` sinon premier ancêtre de l'exécutable contenant `core/benchcore.h` ; cœur = `BENCHCORE_LIB` puis `$BENCH_ROOT/core/target/release/libbenchcore.so` (dart:ffi, lien dynamique). Plus aucun chemin `/home/user/eval` (bench.dart, smoke.sh ; `.idea/` et `benchapp.iml` supprimés).
- `smoke.sh SCENARIO DURATION TAG [args]` : fumée locale sur Xvfb privé (:200–:599).

## Déploiement (bundle `build/linux/x64/release/bundle`, 21 790 177 octets)
| fichier | octets |
|---|---|
| lib/libflutter_linux_gtk.so (moteur) | 17 256 536 |
| lib/libapp.so (snapshot AOT) | 3 507 080 |
| data/icudtl.dat | 864 880 |
| data/flutter_assets (NOTICES.Z 98 130, shaders, manifestes) | ~140 000 |
| benchapp (lanceur GTK) | 23 944 |

`ldd` : 71 bibliothèques système (GTK3, GDK, Pango, Cairo, HarfBuzz, libepoxy, fontconfig, X11…, 32,3 Mo au total) — liste complète : logs/bundle-and-ldd.txt. Il faut GTK3 + libepoxy + un GL (Mesa). Code : 1 571 lignes Dart (lib/, après `dart format -l 120`) + runner C++ du template (154 lignes, +5 lignes de sélection du renderer).

## Revue contre la documentation officielle (écarts trouvés → correction)
1. **Renderer Linux** — la passe 1 disait « Skia non sélectionnable ». Faux : docs.flutter.dev/perf/impeller#linux documente l'opt-out en déploiement `fl_dart_project_set_enable_impeller(project, FALSE)` dans `linux/runner/my_application.cc` (API publique, https://api.flutter.dev/linux-embedder/fl__dart__project_8h.html). Ajouté au runner, piloté par `BENCH_FLUTTER_IMPELLER=0` → variante **flutter-skia** (Skia GL, texte compris). Les `FLUTTER_ENGINE_SWITCHES` ne sont lus que hors release (`#ifndef FLUTTER_RELEASE`, engine/…/common/engine_switches.cc) : d'où l'« ignoré » de la passe 1. `FLUTTER_LINUX_RENDERER=software` **n'est pas documenté** (absent de docs.flutter.dev, seulement dans fl_engine.cc) → non utilisé ; il plante toujours dès qu'il y a du texte : `[FATAL:flutter/display_list/skia/dl_sk_dispatcher.cc(328)] Check failed: blob. Impeller DlText cannot be drawn to a Skia canvas.` (logs/smoke-undocumented-software-renderer.stdout). https://docs.flutter.dev/platform-integration/linux/building ne documente aucune autre option de rendu.
2. **Cadence 60 Hz** — non documentée sur docs.flutter.dev. Source : l'embedder Linux ne fournit pas de `vsync_callback`, le moteur utilise `VsyncWaiterFallback` à tick fixe 1/60 s (engine/src/flutter/shell/common/vsync_waiter_fallback.cc). Aucune option publique → gardée, documentée (`notes`, `ops.pacing`).
3. **Canvas** — passe 1 : un draw par forme avec shader dégradé, ≈0,5 ms/draw sous Impeller-GLES/llvmpipe (5 000 formes : 1 frame en 8 s). Idiomes vérifiés : `repaint:` Listenable (déjà OK, https://api.flutter.dev/flutter/rendering/CustomPainter-class.html), `shouldRepaint` corrigé (`true` → `!identical(old.s, s)`), `Paint`/`Shader` créés une fois par forme (déjà le cas). Nouveau rendu par défaut : chaque forme (dégradé propre + ombre floue sigma 4 décalée (2,2) pour 1/10) est **pré-rendue une fois** dans des pages d'atlas 512 px (`PictureRecorder` + `Picture.toImageSync`, https://api.flutter.dev/flutter/dart-ui/Picture/toImageSync.html), puis chaque frame = un `Canvas.drawRawAtlas` par page (transforms de translation, échantillonnage bilinéaire) dans le `clipRRect` 1000×600 — idiome « sprites » de https://api.flutter.dev/flutter/dart-ui/Canvas/drawAtlas.html et « precalculate… cache it » de https://docs.flutter.dev/perf/best-practices. Les formes sont assignées aux pages dans l'ordre d'indice → ordre z identique. Ombres de l'atlas : une couche `saveLayer` + `ImageFilter.blur(sigma 4)` par page (sprites disjoints ⇒ même résultat que `MaskFilter.blur` par forme ; `BENCH_ATLAS_SHADOW=mask` pour l'ancien). Mesures 5 000 formes (fumée, charge machine variable) : direct 0–1 frame/8 s ; atlas une page 4096 : 2,8–4,6 fps, RSS 1,5 Go ; pages 1024 : 881 Mo ; pages 512 : ~6–11 fps, ~830 Mo. Variante `drawVertices` (dégradé affine exact en couleurs par sommet, formes tessellées) essayée : **pire** (Impeller : 1 frame/8 s, 3,5 s de raster ; Skia : 2,8 fps) → abandonnée (logs/canvas-modes/). Graphe : `drawRawPoints(PointMode.polygon)` 10 000 points, trait 1 px (hairline essayé : pas de gain mesurable) ; historique pré-rempli pour que les 10 000 points existent dès la 1re frame (avant : seul un trait vertical était visible, capture vérifiée). `BENCH_CANVAS_MODE=direct` garde l'ancien chemin (diagnostic).
4. **Liste** — `ListView.builder` + **`itemExtentBuilder`** (existe en 3.47 : « more efficient than letting the children determine their own extent », https://api.flutter.dev/flutter/widgets/ListView/itemExtentBuilder.html). Coût : SliverVariedExtentList recalcule l'offset en O(index) par layout (sliver_fixed_extent_list.dart) ; build p50 mesuré 0,8 ms, acceptable. `cacheExtent` est déprécié depuis 3.41 (non utilisé). **Flux** : Isolate + `SendPort` (lots `Uint32List`), file bornée 100k comptée côté producteur via compteurs en mémoire native, drop + compte ; ajout `updates_backlog_at_start`. **Texte** : pas de widget officiel pour un texte géant virtualisé ; lignes dans `ListView.builder` sous `SelectableRegion` (base de `SelectionArea`, qui exige MaterialLocalizations), `selectAll()` public (https://api.flutter.dev/flutter/widgets/SelectableRegionState/selectAll.html). **Bug** : sous `WidgetsApp` il n'y a pas de `DefaultSelectionStyle` → la sélection n'était pas peinte ; ajouté (https://api.flutter.dev/flutter/widgets/DefaultSelectionStyle-class.html), vérifié sur capture. **Images** : chargeur maison remplacé par l'idiome documenté `Image(image: ResizeImage(FileImage(f), width: 128, height: 128))` (https://api.flutter.dev/flutter/painting/ResizeImage-class.html, `cacheWidth` d'`Image.file`) : décodage 128×128 par le moteur hors isolate UI, ImageCache.
5. **Mesure** — `frame_ms` = écarts entre `FrameTiming.rasterFinish` (SchedulerBinding.addTimingsCallback). **Bug** : en release les timings sont livrés « within about 1 second » (https://api.flutter.dev/flutter/dart-ui/PlatformDispatcher/onReportTimings.html) mais l'app n'attendait que 800 ms → frames de fin perdues ; maintenant attente d'un timing postérieur à la fenêtre (≤1,5 s). Marqueurs `BENCH:measure_start/end` via `stdout` (bloquant, https://api.dart.dev/dart-io/Stdout-class.html) + `flush`. `ops.gc = null` : Dart n'expose aucun compteur GC en AOT release (pas de VM service). `input` : `Focus(autofocus)` + `onKeyEvent` (KeyDownEvent Espace, répétitions ignorées), carré 200×200 en (100,100) noir/blanc, `ops.key_events`.
6. **FFI** — `size_t` était typé `IntPtr` ; corrigé en `Size` (type ABI de dart:ffi, https://api.dart.dev/dart-ffi/Size-class.html). Le résultat accumulé est publié en mémoire partagée (`ops.core_acc`) ; avant, seul un `if (acc == -1) print` le protégeait. Appels `isLeaf: true` pour validate/parse/hash, non-leaf pour `core_heavy` (bloquant 100 ms).
7. Divers : `analysis_options`, `flutter analyze` : 0 problème.

## Cadence (spec v2.1)
`BENCH_PACING=60` (défaut) : cadence native = frames Ticker/SchedulerBinding pilotées par le vsync du moteur (repli 60 Hz fixe, cf. revue 2) → `ops.pacing = "60"`. `BENCH_PACING=uncapped` : aucune API publique ne débride → `ops.pacing = "60-only"` (vérifié en fumée, logs/smoke-harness-impeller/flutter-list-uncapped.run0.json).

## Vérification visuelle (verify_gui, PNG relus)
list/stream/idle/input : lignes, pastilles, titres, sous-titres clippés, prix ; carré noir en (100,100) au-dessus de la liste. canvas/core : dégradés, ombres floues, clip arrondi, graphe (après correctif). text : CJK, hangul, hébreu/arabe RTL, accents OK, surlignage de sélection OK après t=5 s pour les lignes construites ; **emoji monochromes** (glyphes DejaVu Sans choisis par le repli fontconfig, alors que Noto Color Emoji est installé ; forcer `fontFamilyFallback: ['Noto Color Emoji']` donne la couleur mais Noto Color Emoji fournit alors espaces/chiffres → espacement faux) et séquence ZWJ famille non composée → `ops.not_rendered` du scénario text. images : vignettes, zoom. nav : écrans/cartes. Même rendu sous Skia. Note : :390 (verify_gui) était occupé par d'autres agents ; une copie du script dans le scratchpad avec un display privé a été utilisée pour les dernières captures.

## Fumée harnais (8 s, 1 run, Impeller par défaut, 4 vCPU partagés ; logs/smoke-harness-impeller, -skia)
| scénario | frames / fps moy. / p50 ms | RSS médian | ops clés |
|---|---|---|---|
| idle | 0 | 249 Mo | CPU 0 % |
| list | 379 / 47,5 / 20,3 | 268 Mo | sum_heights 8799584 |
| input | 360 / 45,1 / 22,1 | 265 Mo | latence p50 34,8 ms p95 64,1, 0 perdu, key_events 27 |
| stream 1k / 10k | 427 / 53,4 ; 414 / 51,8 | 267 Mo | 10k : généré 80186, appliqué 80130, drop 0 |
| canvas 5000 | 49 / 10,9 / 85,8 | 831 Mo | not_rendered [] |
| canvas 20000 | **0** (1re frame ≈17 s : rastérisation de l'atlas, 20 000 dégradés) ; 25 s : 2,1–3,4 fps | 627 Mo | |
| text | 230 / 28,8 / 33,2 | 333 Mo | ttfd 193 ms, layout 14,6 ms |
| images | 401 / 50,2 / 19,1 | 382 Mo | 600 décodées |
| nav | 186 / 23,4 / 40,9 | 268 Mo | 18 transitions + 4 modales en 8 s |
| core | 265 / 33,5 / 27,9 | 351 Mo | 4,37e7 calls/s, heavy 8 |
| stress (15 s) | 61 / 5,3 / 186 | 872 Mo | backlog stream 9037 au départ, drop 0 |
Skia : list 42 fps (241 Mo), input p50 28,6 ms, canvas 5000 10,5 fps (331 Mo), canvas 20000 6,2 fps (539 Mo), text 37,5 fps, stress 9,8 fps (351 Mo).

## Écarts / limites restants
- Frame = `rasterFinish` (pas de timestamp de présentation X11 public) ; plafond 60 Hz non désactivable.
- Impeller-GLES sur llvmpipe : très lent pour les dégradés/flous et gourmand en mémoire dès qu'il y a un canvas (10 formes : ~510 Mo RSS ; Skia 5000 formes : 331 Mo) ; canvas 20000 sans frame dans une fenêtre courte.
- Atlas : rendu identique en contenu mais formes rastérisées une fois puis échantillonnées (bilinéaire) aux positions fractionnaires.
- Texte : sélection « tout » limitée aux lignes construites (liste paresseuse) ; emoji monochromes.
- `decode_total_ms` = somme des latences demande→1re frame par image (lecture + décodage + attente), pas un temps CPU de décodage ; aucune API publique ne donne ce dernier.
- Workers Dart = Isolates (pas de threads OS bruts) ; flux envoyé par lots.
- Non exécuté : mesures officielles, protocole IME manuel.
