# STACK-NOTES : Slint (Rust), winit/X11, rendu CPU — passe v2 (2026-10-26)

## Versions (vérifiées le 2026-10-26 01:25 UTC)
| outil / crate | version | source |
|---|---|---|
| rustc / cargo | 1.99.1 (58cf8f9c3 2026-10-13) / 1.99.1 | `rustup default` (imposé par le brief) |
| slint, slint-build | =1.18.1 (dernière stable, aucune pré-version plus récente) | https://index.crates.io/sl/in/slint |
| i-slint-renderer-skia / -software / -femtovg / backend-winit | 1.18.1 (épinglés par slint) | index.crates.io |
| skia-safe / skia-bindings | 0.153.3 (dernière) ; binaires Skia précompilés téléchargés au build depuis `github.com/rust-skia/skia-binaries/releases/download/0.153.3/skia-binaries-b7f043e0b1e2a850e702-x86_64-unknown-linux-gnu-ganesh-gl-jpegd-jpege-pdf-vulkan.tar.gz` | index.crates.io + log build skia-bindings |
| winit | 0.30.13 (dernière 0.30.x ; c'est la version qu'exige `unstable-winit-030`) | index.crates.io |
| softbuffer | 0.4.8 (dernière) | index.crates.io |
| image | =0.25.10 (dernière), feature `png` seule | index.crates.io |
| serde_json | =1.0.154 (dernière) | index.crates.io |
| x11rb | =0.13.2 : 0.14.0 existe, mais j'ai gardé la version que winit 0.30.13 utilise déjà (pas de 2e copie) | index.crates.io |
| benchcore | 0.1.0, dépendance path `../../../core` (rlib) | dépôt |

Toutes les versions sont épinglées dans `app/Cargo.toml` (`=x.y.z`) et `app/Cargo.lock` (`build.sh` lance `cargo build --release --locked`). Champ JSON `version` : par ex. `slint 1.18.1 (backend winit 0.30.13 x11, renderer skia-software via skia-safe/skia-bindings 0.153.3, softbuffer 0.4.8); rustc 1.99.1 (58cf8f9c3 2026-10-13); image 0.25.10`.

## Build / lancement
- `stacks/slint/build.sh` (bash, `set -eo pipefail`, CARGO_BUILD_JOBS=2, profil release : opt 3, lto fat, codegen-units 1, panic abort, strip). Aucun chemin absolu.
- Temps de build mesurés : build à froid de toutes les dépendances, dont le téléchargement de Skia, ≈ 328 s, puis le crate de l'app ≈ 145–165 s (lto fat). Rebuild de l'app seule : 141–164 s. Logs : `logs/build-v2-*.log` (`build-v2-1.log` = erreur edition 2024 `gen`, `build-v2-4.log` = erreur `sync` x11rb, conservées telles quelles) ; passe 1 dans `logs/pass1/`.
- Lignes à ajouter à `harness/stacks.tsv` (je ne modifie pas ce fichier, qui est hors de mon dossier). La cadence par défaut est `BENCH_PACING=60` :
```
slint	${ROOT}/stacks/slint/app/target/release/slintapp	
slint-uncapped	${ROOT}/stacks/slint/app/target/release/slintapp	--env BENCH_PACING=uncapped
slint-software	${ROOT}/stacks/slint/app/target/release/slintapp	--env SLINTAPP_RENDERER=software
slint-software-uncapped	${ROOT}/stacks/slint/app/target/release/slintapp	--env SLINTAPP_RENDERER=software --env BENCH_PACING=uncapped
slint-femtovg	${ROOT}/stacks/slint/app/target/release/slintapp	--env SLINTAPP_RENDERER=femtovg
slint-femtovg-uncapped	${ROOT}/stacks/slint/app/target/release/slintapp	--env SLINTAPP_RENDERER=femtovg --env BENCH_PACING=uncapped
```
  Exemple : `python3 -I harness/run_bench.py --stack slint --cmd '${ROOT}/stacks/slint/app/target/release/slintapp' --scenario list --duration 14 --runs 10 [--env BENCH_PACING=uncapped] [--env SLINTAPP_RENDERER=software]`. `stacks/slint/run.sh` fait la même chose via `exec`. Assets : `--assets`, sinon `$BENCH_ROOT/assets`, sinon déduits de l'emplacement du binaire.

## Décisions demandées (points 1 à 5)
1. **Renderer** (doc https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/ et …/backend_winit/). La doc liste toujours pour le moteur logiciel : « No support for drop-shadow-* properties », « No support for border-radius in combination with clip: true », « No support for rotations or scaling », « Text rendering currently limited to western scripts ». Le code 1.18.1 le confirme : `draw_box_shadow` = `// TODO`, `combine_clip` ignore le rayon (`// TODO: handle radius`), et un fond dégradé est envoyé à `process_linear_gradient` sur le rectangle entier, ce qui fait que les cercles et rectangles arrondis à dégradé sortent carrés. Capture `logs/verification-v2/sw-canvas60.png` (60 formes) : formes carrées, aucune ombre. Le backend winit documente aussi **`winit-skia-software`** (« Software-only rendering with Skia »). Capture `skia-canvas60.png` : cercles, rectangles arrondis à dégradé, ombres et clip arrondi sont rendus. **Variante principale = `skia-software`** (`SLINTAPP_RENDERER` par défaut, équivalent de `SLINT_BACKEND=winit-skia-software` via `BackendSelector::renderer_name`). Secondaire : `software` (JSON `ops.not_rendered` = ombre, dégradé arrondi, clip arrondi). Tertiaire : `femtovg` (OpenGL sur llvmpipe, CPU également). Coût de Skia : canvas 5000 à ≈ 10 fps au lieu de 18 fps (mais rendu fidèle), stress à 7 fps au lieu de 40, et 6 Mo de plus sur le binaire.
2. **ListView** (https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/listview/) : la doc dit seulement que seuls les éléments visibles sont instanciés. Le code `i-slint-core/model/repeater.rs` (`update_visible_instances`) **estime** la hauteur des éléments non instanciés par la moyenne des instances (`cached_item_height`). Avec des hauteurs variables, l'offset en px ne correspond donc pas exactement aux lignes, alors que la spec fixe un offset déterministe et Σh = 8 799 584. Aucune API officielle ne prend des hauteurs exactes. **Je garde la virtualisation maison**, relue : sommes préfixes `Vec<u32>` de 100 001 entrées. `lo` = dernière ligne avec prefix ≤ offset, `hi` = dernière ligne qui commence avant le bas de la vue, une ligne de marge de chaque côté, clamp à N−1. Anneau de 32 slots indexé par i % 32, alors que 18 lignes au plus sont visibles (720/48 + 3). L'origine est recalée par blocs de 16 lignes pour garder une bonne précision f32. Bug corrigé : `rows_drawn` était compté à chaque tick, même sans frame ; il est maintenant compté à chaque fin de frame.
3. **Plafond 60 Hz / cadence (spec v2.1, `BENCH_PACING`)** : l'app lit `BENCH_PACING`. Avec `60` (défaut, mesure principale), c'est la **cadence native de Slint** : `slint::Window::request_redraw` passe par le `TimerBasedFrameThrottle` de winit/X11, un Timer répété de `1000000 / refresh_millihertz` ms, arrondi à la milliseconde inférieure. Cela donne **16 ms à 60 Hz, soit 62,5 Hz et non 16,667 ms** (frame_throttle.rs) : la spec demande la cadence native quand elle existe, je la garde et je documente l'écart. Avec `uncapped`, c'est le contournement par API publique décrit ci-dessous. `SLINTAPP_THROTTLE` reste un alias historique, utilisé seulement si `BENCH_PACING` est absent (1 → 60, 0 → uncapped). `ops.pacing` vaut `"60"` ou `"uncapped"`, et le mécanisme est écrit dans `notes`. Détail du plafond : sur X11, Slint passe tout `Window::request_redraw` par un `TimerBasedFrameThrottle` interne, à la fréquence du moniteur (`frame_throttle.rs`) ; aucune API publique ne le désactive. Le contournement n'utilise que des API publiques documentées sur docs.rs : `slint::winit_030::CustomApplicationHandler` (feature `unstable-winit-030`, « may be removed or changed in future minor releases », https://docs.rs/slint/1.18.1/slint/winit_030/index.html), puis `WinitWindowAccessor::with_winit_window` et `winit::Window::request_redraw`. Pas d'API privée, donc conforme au brief ; **gardé et documenté** (`notes` du JSON). La variante plafonnée `SLINTAPP_THROTTLE=1` a été mesurée en fumée (voir tableau).
4. **Cœur** : la crate rlib remplace `libbenchcore.a` (dépendance path relative). Elle supprime le hack `-Wl,--allow-multiple-definition` : le staticlib embarquait sa propre libstd, d'où `rust_eh_personality` en double ; avec la rlib il n'y a qu'une seule std et rien à résoudre à l'édition de liens. En contrepartie, la LTO fat pourrait inliner le cœur dans la boucle. Pour garder un vrai appel ABI C, comme depuis un autre langage, la boucle appelle `core_validate`, `core_parse`, `core_hash` et `core_heavy` par des pointeurs `unsafe extern "C" fn` opacifiés par `std::hint::black_box`. Le résultat est accumulé et publié dans un `AtomicU64`. Note : la rlib est recompilée avec le profil de l'app ; le `.rlib` présent dans `core/target` n'est pas réutilisé.
5. **Spec v2** : marqueurs `BENCH:measure_start`/`measure_end` imprimés avec flush (y compris pour idle, via deux Timers). Champs `version`, `unsupported: null` et `ops.not_rendered` ajoutés. Plus aucun `/home/user/eval` (main.rs, build.rs et build.sh sont réécrits). Nouveau scénario `input`, décrit plus bas.

## Revue contre la doc (écarts trouvés → correction)
- Images décodées par `slint::Image::load_from_path` dans des threads de travail. Or la doc dit « Image is not Send, because it uses internal cache that are local to the Slint thread ; … construct a SharedPixelBuffer » (https://docs.rs/slint/1.18.1/slint/struct.Image.html#sending-image-to-a-thread), et le code met en cache jusqu'à 5 Mo par thread. → Décodage avec `image::open` (crate image 0.25.10, l'exemple de cette même page), réduction 2×2, envoi du `SharedPixelBuffer` par `Weak::upgrade_in_event_loop`, puis `Image::from_rgba8` dans le thread UI.
- Feature `compat-1-2` (obsolète) → `compat-1-18`, exigée par `slint` 1.18 (lib.rs : « The feature `compat-1-18` must be enabled »).
- Sous-titre des lignes mal placé (centré verticalement) : un élément hors layout sans `y` est centré dans son parent (https://docs.slint.dev/latest/docs/slint/guide/language/coding/positioning-and-layouts/). → `y: 0px`, vérifié sur capture.
- Frame de fin : `Window::set_rendering_notifier` renvoie `Unsupported` en logiciel, et `RenderingState::AfterRendering` arrive « before the back buffer was sent for display presentation » (https://docs.rs/slint/1.18.1/slint/enum.RenderingState.html). Je garde donc comme fin de frame le premier `about_to_wait` après `RedrawRequested`. Les hooks sont appelés « before Slint sees them » (doc `CustomApplicationHandler`), et Slint rend puis présente (softbuffer `present_with_damage`, ou swap GL) dans le traitement de `RedrawRequested`.
- TextEdit : `content-y`, `content-height`, `visible-height` et `select-all()` existent tels quels (les anciens noms `viewport-*` sont des alias dépréciés et ne sont pas utilisés) : https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/textedit/. `read-only` laisse la sélection active : conforme.
- Path : `viewbox-width/height` + `width/height` mettent la polyligne 1000×100 à l'échelle (https://docs.slint.dev/latest/docs/slint/reference/elements/path/) : conforme.
- `max_frame_stall_ms` (core, stress) : c'était l'intervalle entre ticks, c'est maintenant le max de `frame_ms`, ce qui correspond mieux à la définition.
- Edition 2021 → 2024 : `gen` est un mot réservé (erreur exacte dans `build-v2-1.log`), renommé `generated`.
- Spec relue : FNV-1a LE, h(i), titres et sous-titres, pastille rgb, xorshift32 (seed 0x2545F491, idx = x % 100000, prix = x/100.0 : les prix vont donc jusqu'à 42 949 672,95, comme le dit la spec), file `sync_channel(100_000)` + `try_send` (drop compté), lots appliqués à chaque tick, script de défilement analytique avec clamp, 1/3 de chaque forme, 1/10 avec ombre (flou 8, offset 2,2). Tout est conforme.

## Scénario input
List + `Rectangle` 200×200 en (100,100) au-dessus. Un `FocusScope` `.slint` (`key-pressed`, `e.text == " "`) bascule `square-white` et appelle `space-pressed()`, que Rust compte dans `ops.key_events`. C'est le chemin normal X11 → winit → Slint. **Problème trouvé** : sans gestionnaire de fenêtres (Xvfb), le focus X est PointerRoot. winit ignore alors les `XI_KeyPress` tant qu'il n'a pas reçu de `FocusIn` (`active_window == None`, event_processor.rs), et `Window::focus_window` passe par `_NET_ACTIVE_WINDOW`, qui demande un WM. Résultat de la 1re fumée : 0 key_events. → Après la première frame, l'app fait un `SetInputFocus` X11 sur sa fenêtre (x11rb, protocole X public). Ensuite : 5/5 touches reçues en test manuel, 31/31 en fumée.

## Limites / écarts restants
- Canvas : un `Rectangle` par forme (Slint n'a pas d'API canvas immédiate), avec `set_row_data` sur toutes les formes à chaque frame. Le `Path` de 10 000 points est re-parsé à chaque frame.
- Texte (skia-software) : CJK, hébreu et arabe sont rendus ; les emoji sont monochromes. Avec `software`, un emoji ZWJ sort vide. IME réelle hors cloud.
- Images : le zoom est obtenu en redimensionnant les cellules (portable ; le moteur `software` n'a pas de transformation d'échelle).
- nav : frames comptées seulement pendant les animations, plus un Timer 2 ms de réveil.
- Checksums indicatifs (ils dépendent du timing).
- Harnais : en v2.1, la fumée `input` donne `window: markers` et 0 perdu (harnais mis à jour entre-temps). Constat v2 : en `input`, `input_latency()` bloque la boucle d'échantillonnage. On obtient `window: whole_run` sans CPU ni RSS, et les 40 frappes durent environ 11 s : avec `--duration 8`, 9 ou 10 frappes arrivent après la fin de l'app (`input_lost`).

## Fumée v2.1 (cadence ; harnais, 8 s, 1 run ; JSON dans logs/smoke-v2.1/ ; build logs/build-v2.1.log, 173 s)
| variante | scénario | fps | p50 / p99 ms | >16,7 / >25 ms | CPU % | RSS Mo | input |
|---|---|---|---|---|---|---|---|
| skia-sw, BENCH_PACING=60 (défaut) | list | 61,6 | 16,0 / 22,9 | 78 / 4 | 11,3 | 29,8 | |
| skia-sw, 60 | input | 60,7 | 16,1 / 28,4 | 105 / 7 | 11,1 | 29,7 | p50 14,3 / p95 24,3 ms, 30 évts, 0 perdu, key_events 30 |
| skia-sw, uncapped | list | 442,9 | 1,9 / 7,9 | 4 / 1 | 60,6 | 29,6 | |
| skia-sw, uncapped | input | 512,3 | 1,85 / 3,9 | 0 / 0 | 67,8 | 29,6 | p50 4,4 / p95 10,7 ms, 31 évts, 0 perdu |
| software, 60 | list | 59,3 | 16,05 / 39,4 | 121 / - | 11,3 | - | |
À 60 Hz, 16 à 25 % des frames dépassent 16,7 ms : c'est la gigue du Timer de Slint (période 16 ms + temps de rendu, relancé depuis la boucle d'événements). Le harnais les compte comme frames manquées. Le tableau ci-dessous (v2) a été mesuré quand le défaut était uncapped.

## Fumée v2 (harnais, 8 s, 1 run, défaut de l'époque = uncapped ; 10 scénarios ; JSON dans logs/smoke-v2/, captures dans logs/verification-v2/)
| variante / scénario | frames | fps | p50 / p99 ms | CPU % | RSS Mo | autres |
|---|---|---|---|---|---|---|
| skia-sw idle | 0 | - | - | 0,0 | 28,7 | markers ok |
| skia-sw list | 3991 | 499 | 1,8 / 6,1 | 66 | 28,6 | sum_heights 8799584, final_offset 20638 |
| skia-sw stream 10000 | 4101 | 513 | 1,8 / 4,6 | 70 | 31,6 | gen = applied 80015, dropped 0 |
| skia-sw canvas 5000 | 78 | 9,8 | 95 / 166 | 95 | 39,4 | not_rendered [] |
| skia-sw text | 1916 | 239 | 3,8 / 8,9 | 81 | 187,8 | first_display 1934 ms, select_all |
| skia-sw images | 693 | 87 | 10,9 / 21,5 | 88 | 83,8 | 600 décodées, 1323 ms |
| skia-sw nav | 4183 | 522 | 2,1 / 9,1 | 118 | 29,5 | 26 transitions |
| skia-sw core | 313 | 39 | 22,9 / 52 | 129 | 27,1 | 4,77e7 calls/s, heavy 8, stall 229 ms |
| skia-sw input | 4159 | 520 | - / 4,0 | n/a | n/a | latence p50 5,06 / p95 7,61 / max 9,61 ms, 31 évts, 9 perdus |
| skia-sw stress | 56 | 6,8 | 105 / 568 | 135 | 46,5 | 5,17e7 calls/s |
| 60 Hz (SLINTAPP_THROTTLE=1) list | 491 | 61,5 | 16,1 / 20,5 | 10,9 | 29,1 | pacing 60 Hz natif |
| 60 Hz (SLINTAPP_THROTTLE=1) input | 487 | 60,9 | 16,1 / 20,5 | n/a | n/a | latence p50 13,2 / p95 23,8 ms |
| software list | 3947 | 494 | 1,9 / 5,8 | 69 | 25,8 | |
| software canvas | 146 | 18,4 | 52 / 82 | 92 | 34,2 | ombres et arrondis non rendus |
| software stress | 321 | 40 | 23,5 / 39 | 203 | 37,7 | |
| software input | 3883 | 486 | 2,0 / 3,8 | n/a | n/a | latence p50 4,1 / p95 10,3 ms |
| femtovg list | 917 | 115 | 8,0 / 17,4 | 188 | 136,8 | llvmpipe |
| femtovg canvas | 10 | 1,3 | 758 / 964 | 123 | 222,9 | |
Mesures de fumée seulement (≤ 15 s), pas des résultats officiels.

## Déploiement
- Binaire release strippé : 22 373 864 o (≈ 21,3 Mo, Skia statique + logiciel + FemtoVG + std-widgets ; 15,9 Mo en passe 1, sans Skia).
- `ldd` : libstdc++ 2,59 Mo (nouveau, à cause de Skia), libc 2,13, libm 0,95, libfreetype 0,83, libfontconfig 0,33, libpng16 0,22, libexpat 0,19, libgcc_s 0,18, libbrotlicommon 0,14, libz 0,11, libbz2 0,08, libbrotlidec 0,05 : total 7,81 Mo de bibliothèques système. libX11/libxcb/libxkbcommon/libEGL/libGL sont chargées par dlopen. Déploiement = binaire seul + polices système.
- Code : src/main.rs 283 + src/sim.rs 800 + ui/app.slint 313 + build.rs 9 = 1405 lignes (+ Cargo.toml 33).
- Au lancement : « Error watching for xdg desktop settings: Failed to connect to address unix:path=/run/user/0/bus » (pas de D-Bus, inoffensif).
