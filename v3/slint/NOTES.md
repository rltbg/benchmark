# NOTES : Slint (Rust), bancs de limites v3, renderer winit-skia-software (2026-10-26)

Dossier : `v3/slint/` (app : `app/`, build : `build.sh`, lanceur : `run.sh`, journal : `OPTIMIZATION-LOG.md`, logs de build : `logs/`).
Point de départ : `stacks/slint/` (v2), réécrit en quasi-totalité (scénarios v3, modules `src/scen/*`, UI découpée en `ui/*.slint`). Aucune trace de F\* ni de cœur F#.

## Versions (vérifiées le 2026-10-26, index.crates.io)
| composant | version | remarque |
|---|---|---|
| rustc / cargo | 1.99.1 (58cf8f9c3 2026-10-13) / 1.99.1 | dernière stable |
| slint, slint-build | =1.18.1 | dernière stable (1.18.0, 1.17.1 plus anciennes) |
| renderer | `skia-software` = Skia (CPU) dessinant dans un tampon softbuffer (`SLINTAPP_RENDERER=skia-software`, défaut) ; `software` = moteur logiciel natif de Slint (variante, `ops.not_rendered`) | `BackendSelector::backend_name("winit").renderer_name(..)` (le renderer par défaut du crate serait FemtoVG/OpenGL, jamais utilisé ici) |
| skia-safe / skia-bindings | 0.153.3 | 0.156.0 existe mais Slint 1.18.1 épingle la 0.153 |
| winit / softbuffer | 0.30.13 / 0.4.8 | |
| image | =0.25.10 (feature `png`) | |
| serde_json | =1.0.154 | |
| x11rb | =0.13.2 | 0.14.0 existe ; garde la copie déjà tirée par winit |
| benchcore | 0.1.0, crate rlib `../../../core` (C-ABI v2) | |
Sources lues : les crates 1.18.1 du registre (`~/.cargo/registry/src/*/i-slint-core-1.18.1` etc.) et le dépôt `/opt/src/slint` (HEAD 372cf0e du 2026-09-21, sparse) ; les chemins:lignes du journal sont ceux des crates 1.18.1.
Champ JSON `version` : `slint 1.18.1 (backend winit 0.30.13 x11, renderer skia-software via skia-safe/skia-bindings 0.153.3, softbuffer 0.4.8); rustc 1.99.1 (58cf8f9c3 2026-10-13); image 0.25.10`.
API instables utilisées : feature `unstable-winit-030` (`CustomApplicationHandler`, `WinitWindowAccessor::with_winit_window`) pour (a) définir la fin de frame, (b) les modes `uncapped` et `direct`. Aucune API privée.

## Lancement
`${ROOT}/v3/slint/run.sh --scenario <ID> [--n N] [--variant k=v]... --duration S --out F.json --assets DIR` (déjà dans `v3/harness/stacks3.tsv`). Variables : `SLINTAPP_RENDERER=skia-software|software`, `BENCH_PACING=60|uncapped`, `SLINTAPP_VARIANT="k=v,k2=v2"` (variantes ajoutées par l'environnement, pour des lignes `stacks3.tsv`), `SLINTAPP_DIRECT=1` (= `--variant direct`), `BENCH_ALLOC=1` (active `ops.alloc_bytes`/`alloc_count`, compteur d'allocations du tas ; sinon `null`). `run.sh` retire `SLINT_DEBUG_PERFORMANCE`. Lignes proposées pour `v3/harness/stacks3.tsv` : `slint-direct	${ROOT}/v3/slint/run.sh	--env SLINTAPP_DIRECT=1`.
Build : `v3/slint/build.sh` (release, lto fat, codegen-units 1, panic abort, `--locked`, 2 jobs). Temps mesurés : build complet (toutes dépendances, Skia déjà téléchargé) ≈ 6 min ; l'app seule ≈ 2 min 40. `strip = false` (symboles gardés pour `perf`, ne change pas le code ; binaire strippé = 22,1 Mo). `ldd` : libstdc++, libfontconfig, libfreetype, libgcc_s, libm, libc, libexpat, libz, libbz2, libpng16, libbrotli (X11/xkbcommon chargés par dlopen). Code : 3 718 lignes (Rust 2 800 + Slint 800).

## Noms de `variant` acceptés (`--variant k=v`, ou clé nue `--variant k` pour les booléens ; défaut en gras)
| scénario | variantes |
|---|---|
| tous | `sizes=explicit|implicit` (implicit = v0 : Text sans hauteur explicite), `direct` (redessin immédiat sur événement) |
| `list-scale` | `impl=native|pool` (défaut **native**) |
| `list-rich` (N = K) | `impl=pool|native` (défaut **pool**) |
| `list-mutate` (N = mises à jour/s) | – |
| `list-reset` | `impl=native|pool`, `build=bg|ui` (défaut **bg** ; `ui` = reconstruction sur le fil UI) |
| `text-big` | – (N = Mo : 1, 10, 50, 100) |
| `text-labels` | – |
| `text-edit` | `wrap=word|none`, `docsize=<Ko>` (exploration, hors spec) |
| `text-rich` | – (partiel) |
| `gfx-shapes` | `feature=solid|gradient|shadow|clip|all` (aussi en clé nue : `--variant gradient`), `drive=bind|model` (défaut **bind**) |
| `gfx-paths` | `dashed` → `unsupported` |
| `gfx-chart` | `render=pixbuf|path` (défaut **pixbuf**) |
| `gfx-images` | `big` (4096² puis 8192²), `premul=off` |
| `gfx-layers` | `blur` → `unsupported` |
| `ui-tree` | `layout=layout|manual` (défaut **layout**), `selfresize` (l'app se redimensionne 100 fois, pour valider sans harnais) |
| `ui-bind` | `bind=model|derived` (défaut **model**) |
| `ui-anim` | – |
| `ffi-calls` | `threads=1|4` ; N = taille du tampon en octets |
| `ffi-callbacks` | `src=queue|coalesce|invoke` (défaut **queue**) |
| `ffi-block` | `ui` (appel sur le fil UI, 1 fois par s) ; N = ms de `core_heavy` |
| `latency-load` (I1) / `latency-wheel` (I2) | `load=idle|fling|cpuhog|heavy|mutate` (`cpuhog` = processus externes du harnais, l'app est inactive ; `mutate` = L3 à 100 k/s, ou `--variant rate=R`), `impl=native|pool` |
| `idle` | – |
`M1 soak`, `M2 mem-min` : **non implémentés** (composites hors priorités du brief). `M3 startup-scale` : ce n'est pas un scénario de l'app ; utiliser `ui-tree --n 1000|100000` et `ops.setup_ms`.

## État par scénario
Légende : OK = implémenté et vérifié (capture PNG lue, `markers: True`) ; P = partiel ; U = unsupported.
| ID | état | notes |
|---|---|---|
| idle | OK | coquille (barre de titre, 1re page de liste, barre d'état), pas de minuteur de 4 ms (donc pas de `ui_stall_ms_max`) |
| L1 list-scale | OK | `ListView` sur modèle paresseux (0 octet par ligne) ; 10 M lignes : 62,1 fps. `sum_heights` exact (8 799 584 à 100 k). Hauteurs variables : l'offset en px est celui de l'estimateur du répéteur, pas exact (voir journal) ; variante `pool` exacte |
| L2 list-rich | OK | 100 k lignes denses, K éléments (6 + (K-6)/2 puces = rect arrondi coloré + texte) ; défaut `pool` |
| L3 list-mutate | OK | `ListView` + `Model` personnalisé (`Vec` + `ModelNotify`), file SPSC sans verrou 2^20 |
| L4 list-reset | OK | remplacement toutes les 2 s du modèle ; données reconstruites dans un fil de fond |
| T1 text-big | OK | `TextEdit` lecture seule, `select-all()` à t = 5 s. 100 Mo non exécuté (voir limites) |
| T2 text-labels | OK | |
| T3 text-edit | OK | `TextEdit` 1 Mo, curseur au milieu ; carré basculé dans `edited` |
| T4 text-rich | **P** | `StyledText` (gras, italique, code, barré, souligné, couleur, CJK/emoji/arabe/hébreu rendus) ; polices et tailles par span non exprimables → `ops.not_rendered` |
| G1 gfx-shapes | OK | un `Rectangle` par forme (pas d'API canvas) ; 5 variantes |
| G2 gfx-paths | OK / U `dashed` | un `Path` par courbe, 8 segments cubiques, trait 2 px |
| G3 gfx-chart | OK | `Image` rendue par Rust (`SharedPixelBuffer`) ; variante `path` |
| G4 gfx-images | OK | `Image` partageant ≤ 600 textures, rotation/échelle/opacité ; `big` OK |
| G5 gfx-layers | OK / U `blur` | `opacity: 0.5` sur des groupes de 3 formes ; flou absent de Slint |
| U1 ui-tree | OK | N = nombre de feuilles |
| U2 ui-bind | OK | |
| U3 ui-anim | OK | animations `animate` + `iteration-count: -1` |
| F1 ffi-calls | OK | |
| F2 ffi-callbacks | OK | 3 modes étiquetés |
| F3 ffi-block | OK | |
| I1 / I2 | OK | carré de latence dans tous les scénarios ; chemin clavier `FocusScope` ; molette via `Flickable` natif (`native`) ou rappel `scroll-event` (`pool`) |

## Choix d'implémentation à connaître
- **Fin de frame / `frame_ms`** : premier `about_to_wait` winit après un `RedrawRequested` (Slint rend et présente de façon synchrone dans le traitement de `RedrawRequested`). `set_rendering_notifier` n'est pas utilisable (renvoie `Unsupported` en logiciel ; `AfterRendering` précède la présentation). Cadence `60` = celle de Slint : `TimerBasedFrameThrottle` (`i-slint-backend-winit-1.18.1/frame_throttle.rs:40-72`), `Timer` répété de 16 ms (1 000 000 / 60 000 mHz, ms entières) = **62,5 Hz** (d'où fps ≈ 62,1 mesurés). `uncapped` : `with_winit_window(|w| w.request_redraw())`.
- **Pilote** : l'état d'un scénario n'avance qu'une fois par frame (quand la précédente est finie) ; les scénarios animés par le moteur (U3) ou par événements (U1, T3, I1) ne demandent aucun redessin.
- **`ops.ui_stall_ms_max`** : `slint::Timer` répété de 4 ms sur le fil UI, plus long intervalle entre deux déclenchements consécutifs depuis `measure_start` (inclut donc le temps de rendu d'une frame : ≈ 10 ms au repos à 62,5 Hz).
- **`ops.setup_ms`** : début de `main` → fin de la première frame (fenêtre créée, données construites). `ops.model_build_ms` : temps de création du modèle seul (listes).
- **`ops.checksum_data`** (listes) : FNV-1a 32 bits du flux little-endian des hauteurs (4 octets par ligne) ; 100 k lignes standard : `0cf19e25`, denses (L2) : `4ae3d07d`... (la valeur dépend de N, et de `hd` pour L2). `sum_heights` : 8 799 584 (standard, 100 k).
- **Carré de latence** : `Rectangle` 200×200 en (100,100) au-dessus de tout, bascule noir/blanc dans un `FocusScope` (`key-pressed` sur `" "`) ; sous Xvfb le focus X est donné par `x11rb::set_input_focus` (sans gestionnaire de fenêtres winit ignore sinon les touches : v2). En T3 il bascule dans `TextEdit.edited` (après application au texte) ; `ops.key_events` compte ces appels.
- **Un élément par forme** (G1, G2, G4, G5, T2, U1–U3) : Slint n'a pas d'API de canvas immédiat ; le seul contournement idiomatique est l'image rendue côté Rust (utilisé uniquement en G3, sans pré-rendre des effets que le moteur ne sait pas faire).
- **Moteur logiciel natif (`SLINTAPP_RENDERER=software`)** : ne dessine ni les ombres, ni les dégradés sur formes arrondies, ni `border-radius` avec `clip`, ni les rotations ; ces cas sont listés dans `ops.not_rendered` (G1 : gradient/shadow/clip/all ; G4 : rotation en `gfx-images`, voir ci-dessous). Pas de contournement.
- **Images** : décodées une seule fois hors fil UI (3 fils, crate `image`), prémultipliées, `Image::from_rgba8_premultiplied` sur le fil UI (`Image` n'est pas `Send`), conservées par l'application (le cache par chemin de Slint est plafonné à 5 Mio, `i-slint-core/graphics/image/cache.rs`).
- **Cœur C-ABI** : `src/ffi.rs`, pointeurs `extern "C"` opacifiés par `black_box` (la LTO fat n'inline pas le cœur) ; rappel natif→UI : `core_producer_start` + file SPSC 2^21×16 o drainée une fois par frame (défaut), ou coalescence « dernière valeur gagne » (`coalesce`, les événements écrasés sont comptés comme appliqués), ou `invoke_from_event_loop` par événement (`invoke`, v0 naïf : `write(eventfd)` par événement, `i-slint-backend-winit-1.18.1/event_loop.rs`, `calloop ping`).
- **Dernier lot** : en fin de mesure le producteur est arrêté puis la file est drainée une dernière fois avant de compter `updates_applied` (L3, F2) ; sinon le retard d'une frame (16 ms sur 12 s) donne 99,86 % < 99,9 %.
- **`ops.rate_achieved_per_s` / `rate_fraction`** (F2) : débit réellement émis ; en mode `invoke` à 5 M/s le producteur natif est ralenti à 34 % du débit demandé (rappel bloquant) et `updates_generated` reflète ce débit réduit : lire `rate_fraction`.

## Fumées (6–8 s, pas des mesures officielles ; `skia-software`, `BENCH_PACING=60`, machine partagée avec d'autres compilations)
fps / p99 ms / frames > 25 ms / CPU % cœur / RSS Mo / `ui_stall_ms_max` ms.
| scénario, N, variante | fps | p99 | >25 | CPU | RSS | stall |
|---|---|---|---|---|---|---|
| L1 100 k | 62,2 | 19,8 | 0 | 18,5 | 28 | 10 |
| L1 10 M | 62,1 | 22,8 | 0 | 20,7 | 28 | 12 |
| L2 K=6 | 62,2 | 18,9 | 1 | 23 | 29 | 13 |
| L2 K=96 (pool) | 45,9 | 35 | 64 | 96 | 40 | 38 |
| L3 1 k / 100 k / 1 M /s | 62,1 / 62,2 / 61,0 | 19 / 18 / 26 | 0 / 0 / 4 | 20 / 22 / 50 | 55 | 10 / 8 / 30 |
| L3 10 M /s | 7,4 | 191 | 45 | 103 | 55 | 195 (61 % des ops perdues : file pleine) |
| L4 10 k / 1 M | 62,3 / 62,2 | 19 / 18 | 0 | 18 / 17 | 29 / 33 | 11 / 10 (reconstruction ≈ 2 ms) |
| T1 1 Mo / 10 Mo | 62,0 / 62,1 | 20 / 20 | 0 / 1 | 27 / 30 | 189 / 1 006 | 12 / 19 (1er affichage 1,8 s / 19 s) |
| T1 50 Mo | 59,5 | 31 | 10 | 35 | 4 599 | 37 (1er affichage 117 s) |
| T2 500 / 5 k / 20 k | 57,4 / 49,6 / 29,6 | 23 / 29 / 46 | 1 / 39 / 177 | 88 / 95 / 96 | 29 / 33 / 45 | 22 / 46 / 47 |
| T3 1 Mo | latence touche→pixel ≈ 935 ms (p50), 1 touche ≈ 1 s de CPU (mise en forme du document entier) | | | | | 1 060 |
| T4 1 k / 100 k spans | 62,1 / 62,3 | 20 / 21 | 0 / 0 | 48 / 62 | 137 / 332 | 14 / 986 (1er affichage 3,4 s à 100 k) |
| G1 250 solid / gradient / shadow / clip / all | 62,2 / 62,2 / 62,0 / 62,2 / 61,9 | 19–23 | 0–1 | 17 / 25 / 63 / 20 / 72 | 25–28 | 10–25 |
| G1 5 000 solid / gradient / shadow / clip | 29,8 / 15,9 / 5,6 / 20,6 | 50 / 99 / 219 / 74 | 178 / 95 / 33 / 123 | 97 | 34–74 | 52–219 |
| G2 100 / 1 000 / 5 000 paths | 41,0 / 3,6 / 0,8 | 35 / 367 / 1 320 | 70 / 21 / 4 | 96–99 | 25–38 | |
| G3 10 k / 100 k / 1 M (pixbuf) | 62,2 / 62,2 / 17,4 | 18 / 18 / 85 | 0 / 0 / 104 | 15 / 42 / 98 | 31–34 | 11 / 15 / 104 |
| G3 100 k / 1 M (path) | 59,7 / 6,2 | 24 / 196 | 2 / 37 | 94 / 99 | 31 / 82 | 26 / 196 |
| G4 10 / 200 / 1 000 images | 62,2 / 3,9 / 0,8 | 20 / 462 / 1 254 | 0 / 23 / 5 | 62 / 92 / 98 | 35 / 133 / 441 | |
| G5 4 / 50 / 200 groupes | 62,0 / 34,6 / 8,5 | 18 / 43 / 168 | 0 / 177 / 50 | 10 / 96 / 98 | 29 | 8 / 55 / 168 |
| U1 1 k / 100 k / 500 k feuilles (statique, 0 frame) | – | – | – | 1 / 1 / 9 | 33 / 712 / 3 492 | setup 94 ms / 5,0 s / 43,9 s |
| U1 10 k, 95 redimensionnements (selfresize) | frames de redim. ≈ 62 ms (limitées par les pas de 60 ms) | | | | 94 | 46 |
| U1 100 k, 33 redimensionnements : layout / manual | frame 312 ms / 178 ms | | | | 712 / 273 | 332 / 238 |
| U2 1 k / 10 k / 100 k / 1 M (model) | 59,6 / 36,1 / 8,6 / 1,0 | 22 / 42 / 154 / 1 095 | 2 / 128 / 51 / 6 | 87 / 97 / 97 / 97 | 31 / 40 / 142 / 821 | |
| U3 100 / 1 k / 10 k / 100 k | 62,0 / 61,6 / 11,6 / 1,2 | 19 / 21 / 124 / 928 | 0 / 0 / 69 / 6 | 10 / 45 / 99 / 97 | 25 / 27 / 52 / 296 | |
| F1 threads=1 N=1 Ko | 62,2 | 21 | 0 | 151 | 26 | 17 (1,28 G appels/s, toutes fonctions confondues) |
| F1 threads=4 N=1 Mo | 58,9 | 30 | 13 | 384 | 32 | 29 |
| F2 10 k / 10 M / 20 M /s (queue) | 62,1 / 62,2 / 62,3 | 19 / 19 / 19 | 0 | 16 / 35 / 45 | 63 | 11 / 10 / 15 ; 100 % appliquées, 0 perdue |
| F2 invoke 100 k / 1 M / 5 M /s | 62,3 / 62,4 / 62,0 | 19 / 19 / 21 | 0 / 1 / 1 | 27 / 111 / 172 | 63–68 | à 5 M/s : débit émis 34 % du demandé |
| F2 coalesce 10 M /s | 62,2 | 19 | 0 | 46 | 63 | 11 |
| F3 10 / 1 000 ms (fil de fond) | 62,1 / 62,2 | 19 / 18 | 0 | 113 | 62 | 11 / 11 |
| F3 `ui` 100 ms / 1 000 ms | 57,7 / 1,0 | 103 / 1 005 | 5 / 6 | 22 / 99 | 62 | 104 / 1 005 |
| I1 touche→pixel (p50 ms) idle / fling / heavy / mutate | 17,3 / 11,6 / 17,4 / 12,6 | | | | | `direct` : 1,4 / 3,6 / 1,4 / 3,0 |
| I2 molette→pixel (p50 ms) | 20,3 (native), 20,5 (pool) | | | | | `direct` (pool) : 4,2 |
Moteur logiciel natif (`software`) : L1 62,2 fps/18,5 % ; G1 all 1 000 formes 62,2 fps/29 % (mais ombres, dégradés arrondis et clip arrondi non dessinés) ; G4 100 images 43 fps (rotation non rendue) ; T4 62 fps.

## Pièges et erreurs rencontrés (messages exacts)
- `error: expected identifier, found reserved keyword `gen`` (édition 2024) : renommé `generation`.
- `Unknown unqualified identifier 'd'. Did you mean 'self.d'?` (Slint) : dans un composant qui hérite d'un autre, les propriétés `in` du parent s'écrivent `self.d.y`.
- `cannot update the lock file ... because --locked was passed` après le retrait de la feature `renderer-femtovg` : `Cargo.lock` régénéré une fois (`cargo check --offline`).
- Changer `strip` dans le profil release recompile les 378 crates (≈ 6 min) ; mon nettoyage trop large de `target/release/deps` (filtre par date) a imposé une 2e recompilation complète : disque, 2,7 Go pour `target/`.
- T3 : `ui_stall_ms_max` 5 s observé une fois sur la 1re touche quand la machine était chargée par d'autres compilations ; le coût stable est ≈ 1 s/touche (voir journal).
- « Error watching for xdg desktop settings: Failed to connect to address `unix:path=/run/user/0/bus` » au lancement : sans D-Bus, inoffensif.

## Limites / non faits
- T1 100 Mo non exécuté : extrapolation 10 Mo → 1,0 Go RSS / 19 s ; 50 Mo → 4,6 Go / 117 s, soit ≈ 9 Go et ≈ 4 min pour 100 Mo (risque d'OOM de la machine partagée) ; le harnais appliquera son critère (1er affichage ≤ 2 s : seul N = 1 passe).
- `ui-tree` 500 k : 3,5 Go de RSS et 44 s de setup (pas de crash). Au-delà d'un `for` de plus de 1 048 576 instances hors `ListView` Slint refuse (`MAX_EAGER_INSTANCE_COUNT`, `i-slint-core/model/repeater.rs:172`) : non atteint ici (500 k feuilles = 50 k conteneurs × 10).
- `ui-bind` 1 M étiquettes : 821 Mo, 1 fps, sans crash. `ui-anim` 100 k : 296 Mo, 1,2 fps.
- Pas de mesure `cpuhog` côté app (charge externe) ni de M1/M2.
- `ops.gc` : sans objet (pas de runtime géré) ; `ops.alloc_bytes` seulement avec `BENCH_ALLOC=1`.
- Les checksums de flux de mise à jour (`updates_*`) dépendent du timing.
