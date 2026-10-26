> **Note du dépôt (décision utilisateur)** : les variantes TSX/QuickJS et GPUI de NSH décrites dans ce fichier ont été construites en fumée par l'agent mais sont **exclues** du dépôt, des mesures et du rapport (pas de version TS/web dans le périmètre). Seule la variante Rust seul / GTK 4 (`app/`, `run.sh`) est mesurée.

# NSH (Native Shell, github.com/longbridge/nsh) — notes de pile

Date des mesures de fumée : 2026-10-25. Dépôt cloné en `src/` (`git clone --depth 1`, HEAD `6a7b56f`, 2026-10-25 « gallery: Keep the Editor demo responsive on files of 200K lines (#463) », version 0.2.0). nsh.dev et github.com (pages) n'ont pas été consultés : tout vient du dépôt (README, AGENTS.md, docs/dev/*.md, website/docs|elements|modules, crates/).

## 1. Ce qui a été construit

| Livrable | Chemin | Rôle |
|---|---|---|
| **App Rust seule (principale)** | `app/target/release/nshbench` (via `run.sh`) | Backend **GTK 4** natif de NSH, composants Rust (pas de QuickJS), 9 scénarios |
| Variante TSX (QuickJS) | `app-tsx-target/release/nshbench` + `.nsh-out/bundle.nshc` (via `run-tsx.sh`) | `list` et `stream` seulement, `List` TSX natif, bundle compilé par `nsh build` |
| Backend GPUI (second choix) | `gpui-app/target/release/nshbench-gpui` (via `run-gpui.sh`) | Mêmes composants Rust, fenêtre GPUI (wgpu → Vulkan lavapipe). **Données de frame partielles** (voir §7) |
| Build | `build.sh [tsx\|gpui]` | |
| Logs bruts | `logs/` ; fumées harnais `results-smoke/` | |

Commande de lancement pour le harnais (même CLI que la spec) :

```
python3 /home/user/eval/harness/run_bench.py --stack nsh --cmd /home/user/eval/stacks/nsh/run.sh --scenario list --duration 10 --runs N
```

`run.sh` fixe `GSK_RENDERER=cairo` (surchargeable : `--env GSK_RENDERER=ngl`), `GDK_BACKEND=x11`, `GTK_A11Y=none`. Variante TSX : `--cmd /home/user/eval/stacks/nsh/run-tsx.sh --stack nsh-tsx` (list / stream). GPUI : `--cmd .../run-gpui.sh --stack nsh-gpui`.

Le code applicatif : `app/src/main.rs` (hôte GTK, mesure, JSON : 285 lignes), `app/src/scenarios.rs` (composants : 661), `app/src/shared.rs` (config, compteurs, producteur xorshift, threads core : 197), `tsx/App.tsx` (146), `gpui-app/src/main.rs` (159, réutilise scenarios.rs et shared.rs par `#[path]`). Total ≈ 1 450 lignes.

## 2. Versions exactes

- rustc / cargo **1.99.1** (58cf8f9c3 2026-10-13). Le conteneur avait 1.97.0 ; `src/rust-toolchain.toml` (`channel = "stable"` + 7 cibles iOS/Android/macOS) fait que le **premier `cargo` lancé dans `src/`** exécute `info: syncing channel updates for 'stable-x86_64-unknown-linux-gnu'` puis télécharge rustc 1.99.1 + rust-std pour `aarch64-apple-darwin`, `aarch64-apple-ios{,-macabi,-sim}`, `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` (static.rust-lang.org, autorisé). Le toolchain `stable` du conteneur est ainsi passé à 1.99.1 pour tout le monde. Pas de nightly requis. Édition 2024.
- GTK 4.14.5, Pango 1.52.1, Cairo 1.18.0, GLib 2.80.0 (système). libadwaita : **non utilisée** par NSH. WebKitGTK 6.0 absent : sans effet (feature `webview` désactivée, `default-features = false`).
- Crates clés (Cargo.lock) : nsh-* 0.2.0, gtk4/gdk4/gsk4 0.11.5, glib 0.22.10, pango/pangocairo 0.22.9, cairo-rs 0.22.9, taffy 0.15.0, ropey 1.6.1, syntect 5.3.0, tokio 1.53.3, ureq 3.4.2, image 0.25.10, tiny-skia 0.11.4 ; TSX : quickjs-jit-sys 0.14.1 (QuickJS ; clang 18.1.3 requis) ; GPUI : gpui-fast / gpui-fast-platform 0.1.9, wgpu 29.0.4.
- libbenchcore : le `target/release/libbenchcore.rlib` livré a été compilé par rustc 1.97 et ne se lie pas avec 1.99 ; `core/` est donc utilisé **en dépendance de chemin** (`benchcore = { path = "/home/user/eval/core" }`) et recompilé dans mon `target/` (aucune modification de `core/`).

## 3. Build : commandes, durées, dépendances

```
cd /home/user/eval/stacks/nsh && ./build.sh          # Rust seul
./build.sh tsx                                       # + nsh-cli, bundle, variante QuickJS
./build.sh gpui                                      # + backend GPUI
```
`CARGO_BUILD_JOBS=2`, profil release (opt-level 3, LTO « thin », codegen-units 4).

| Étape | Durée (2 jobs, machine partagée) | Crates uniques |
|---|---|---|
| App Rust seule, deps + stub (`logs/build-deps-1.log`) | 4 min 24 s | 300 (dont 291 hors nsh/benchcore) |
| App Rust seule, recompilation du crate appli (LTO thin) après modif. | 1 min 44 s | |
| `nsh-cli` (rolldown, oxc, QuickJS…, `logs/build-nsh-cli.log`) | 11 min 40 s (un 1er essai a été tué par l'environnement — « Terminated » — après ~12 min ; le cache l'a repris) | — |
| App TSX (QuickJS, `logs/build-app-tsx*.log`) | 5 min 37 s (deps) + 2 min 18 s (recompilation) | 463 |
| GPUI (`logs/build-gpui-1.log`, `-2.log`) | 11 min 31 s (deps) + 4 min 08 s | ≈ 650 |
| Bundle TSX (`nsh build tsx --release`) | 10 ms ; bundle.js 2,8 Ko, bytecode 4,3 Ko | |

Aucune erreur de build de NSH lui-même : pas de dépendance système manquante (GTK 4.14.5 et clang présents), pas de nightly. Seules erreurs rencontrées, toutes de mon code : `error[E0618]: expected function, found '&shared::Config'` (`logs/build-app-tsx-attempt1-error.log`, variable `cfg` masquant la fonction) ; `no method named 'detach' found for struct 'Spawned<T>'` et `unresolved import 'nsh_core::ui::next_frame'` (corrigés : `next_frame` est dans `prelude`, tâches gardées par `std::mem::forget`).

Binaire : `nshbench` 30 763 952 o (22,5 Mo strippé) ; TSX 42 707 208 o ; GPUI 67 404 096 o ; `nsh` (CLI) 50 089 728 o. Dossier de déploiement Rust seul : le binaire seul (+ assets du banc lus à l'exécution) ; la variante TSX ajoute un `bundle.nshc` de 4,3 Ko. Bibliothèques dynamiques (`logs/ldd-nshbench.txt`, 67 libs, **36,8 Mo cumulés**, toutes système) : libgtk-4 (11,8 Mo), libstdc++ (2,6), libc (2,1), libgio (1,9), libglib (1,3), libcairo (1,3), libX11 (1,3), libepoxy (1,2), libharfbuzz (1,1), libm, libfreetype, libzstd, libpixman, libpango, libvulkan, libpng/tiff/jpeg/webp… Pas de runtime JS en mode Rust seul.

## 4. Architecture observée (code + /proc)

**Threads** (lus dans `/proc/PID/task` en scénario stream) : (1) thread principal `nshbench` = boucle GLib/GTK : applique les lots de montage, fait layout GTK, snapshot, rendu GSK ; (2) `nsh-engine` : **les composants Rust s'exécutent ici** (graphe réactif type Solid, layout Taffy, listes virtualisées, bind des cellules, enregistrement des dessins canvas, mise en page du texte avec Pango sur sa propre font map) ; en mode TSX c'est aussi là que tourne QuickJS (une VM par surface) ; (3) deux `nsh-tasks` = runtime tokio (futures `spawn`, réseau, `background`) ; (4) workers de décodage d'images à la demande (GDK pour PNG/JPEG, crate `image` pour GIF/WebP) ; (5) `llvmpipe-0..3` et `gmain`/`pool-*` de Mesa/GLib (les threads llvmpipe existent même avec `GSK_RENDERER=cairo`, GDK crée quand même un contexte GL). Mes threads : `producer`, `core-worker`, `core-heavy`.
Flux : le thread GTK tick (`add_tick_callback` tant que quelqu'un demande une frame) → `control.frame()` → l'engine exécute les tâches `next_frame().await` → commit → **un `MountBatch` par commit** envoyé d'un saut (`MainContext::invoke`) au thread GTK, qui l'applique (`mount.rs`) ; le thread GTK n'attend jamais le moteur. Les messages `Frame` ne provoquent un dessin que si un lot ou un scroll a invalidé des widgets : pas de « frame fantôme ».

**Vues natives** : un `NshView` (sous-classe de `gtk::Widget`) **par nœud**, le cœur NSH fait tout le layout (la vue ne se mesure jamais ; `size_allocate` place l'enfant à la frame calculée) ; `snapshot` émet des nœuds GSK (ombres/fond via `decor.rs`, texte = `gsk::TextNode` de glyph runs déjà composés par Pango sur l'engine thread, image = texture, canvas = tuiles) ; pas de widget GTK standard sauf pour champs de saisie/vidéo/web view hébergés.
**Recyclage/virtualisation** : `virtual_list(n, |at| row)` (Rust) / `<List each={…}>` (TSX) — le cœur calcule la fenêtre visible (+ marge d'un viewport de chaque côté), crée les cellules, les **recycle** (la fonction de rangée ne s'exécute qu'une fois par cellule ; `at.index()` est un signal, seules les liaisons qui le lisent se ré-évaluent), mesure la hauteur réelle des cellules (hauteurs variables **supportées**) mais **sans API de hauteur exacte par index** : la longueur du contenu = mesurées + `estimated_item_size` pour le reste (ici 88 pt = moyenne, contenu final estimé ≈ 8 799 9xx vs sumH 8 799 584). Colonnes, en-têtes collants, `scroll_to_offset/index`, viewability disponibles. « Paced binding » : une fling rend un écran de cellules d'un coup, le reste de la marge est lié progressivement.
**Canvas** : oui, API Canvas 2D (sous-ensemble : chemins, `round_rect`, dégradés linéaires/radiaux/coniques, ombres, clip, transformations, texte, `draw_image`), **enregistrée en liste d'opérations** (`Painter` → `Drawing`) puis **rejouée avec Cairo dans une `ImageSurface` CPU** au facteur d'échelle, découpée en tuiles de 256 px envoyées comme textures GSK. Il n'y a pas de scène retenue/graphe de formes : 5 000 formes = 5 000 × (save, translate, fill_style, chemin, fill, restore) ré-enregistrés (≈3 ms côté engine) et **entièrement rejoués à chaque frame** sur le thread GTK. Les redessins partiels existent seulement pour les valeurs animées natives (pas pour un dessin ré-émis). Le canvas ne sait pas utiliser le GPU sous Linux.
**Texte 1 Mo** : `Editor` de `nsh/base` (rope `ropey`, lignes virtualisées, sélection multi-curseur, soft wrap, `run(SelectAll)`, `set_scroll_offset`). `text()` avec `selectable` + `SelectionArea` existent mais pour des textes courts ; `TextView` = Markdown/HTML.
**Navigation** : `nav_stack`/`nav_page` (transitions natives : glisser 300 ms, parallaxe), `dialog()` (`ModalAnimation::Fade` = 200 ms).
**Modes** (docs/website `rust.md`) : Rust seul (aucun JS), TSX (QuickJS), mixte. Le mode TSX est l'usage « normal » du framework (apps livrées à l'exécution, rechargement à chaud) ; le Rust seul est annoncé comme performant pour la logique métier.

**C-ABI d'embarquement** (non mesurée) : `crates/nsh-platform-ios/include/nsh.h` (261 lignes, iOS uniquement, types UIKit) : `nsh_app_create/destroy/view`, `nsh_app_load_bundle`, `nsh_app_set_limits/_storage_directory/_cache/_locale/_font_scale/_theme_overrides`, `nsh_app_connect_dev_server`, `nsh_trust_*`/`nsh_package_*` (paquets `.nshpkg` signés Ed25519), `nsh_app_register_component`/`nsh_component_emit` (composants natifs), `nsh_app_set_host_params`, `nsh_app_send_host_event`, `nsh_app_set_host_handler` + `nsh_host_call_resolve/reject` (appels `nsh/host`), `nsh_app_set_open_url_handler`. Android : JNI (`NshView`, `libnsh.so`). **Les données franchissent la frontière en JSON** ; il n'y a pas de C-ABI générique pour un cœur Rust/Zig : le cœur Rust est lié directement (crate) ou exposé par `#[native_module]` / `#[module]` / `#[class]` ; ici `libbenchcore` est appelé depuis Rust par pointeurs de fonction (voir §6).

## 5. Résultats de fumée (1 passage, via `run_bench.py`, NON officiels)

Machine partagée (d'autres compilations en parallèle), Xvfb 1280×720, `GSK_RENDERER=cairo`, `LIBGL_ALWAYS_SOFTWARE=1`. `fps` = images réellement repeintes ; `paint` = temps réel de la phase de peinture GTK (before-paint → after-paint : layout + snapshot + rendu GSK/Cairo). Fichiers : `results-smoke/*.json`.

| Scénario | frames | fps | frame p50/p95/max (ms) | paint moyen (ms) | CPU moy. (% d'un cœur) | RSS médiane |
|---|---|---|---|---|---|---|
| idle (6 s) | 0 | 0 | — | — | 1,9 (démarrage inclus) | 119 Mo |
| list (10 s) | 599 | 60,0 | 16,7 / 19,6 / 27,1 | 3,9 | 16,8 | 127 Mo |
| stream 1 000/s | 599 | 60,0 | 16,7 / 19,7 / 24,0 | 3,9 | 19,2 | 128 Mo |
| stream 10 000/s | 599 | 60,0 | 16,7 / 18,6 / 28,1 | 3,7 | 19,6 | 128 Mo |
| canvas 5 000 formes | 35 | 3,6 | 262 / 371 / 422 | 278 | 100 | 147 Mo |
| canvas 20 000 formes (3 s) | 4 | ≈1 | ≈960 | ≈967 | — | 188 Mo |
| core (1 000 formes plates) | 419 | 52,5 | 17,2 / 29,0 / 47,8 | 17,7 | 187 | 126 Mo |
| text (1 Mo) | 599 | 60,0 | 16,7 / 21,6 / 31,7 | 4,8 | 32 | 147 Mo |
| images | 595 | 59,6 | 16,7 / 20,6 / 31,8 | 8,3 | 51 | 241 Mo |
| nav | 535 | 56,5 | 16,5 / 33,7 / 51,6 | 8,0 | 103 (1 cœur = brûleur) | 119 Mo |
| stress (10 s) | 76 | 7,7 | 218 / 274 / 296 | 122 | 197 | 152 Mo |
| **TSX** list | 597 | 59,8 | 16,7 / 18,4 / 56,2 | 3,7 | 17,3 | 135 Mo |
| **TSX** stream 10 000/s | 479 (fumée 8 s) | ≈60 | 16,6 / 18,2 / 32,5 | 3,8 | — | 136 Mo |

Vérifications : `ops.sum_heights == 8799584` pour idle/list/stream/stress ; stream 10 000/s : generated 100 001, applied 99 844, dropped 0 ; text : « tout sélectionner » à t=5 s → 1 000 111 octets sélectionnés (taille du fichier) ; core : ≈ 103–136 M triplets/s, `heavy_calls` = 8 en 8 s ; nav : `transitions_done` = 20 (+ 10 ouvertures/fermetures de dialogue en fondu) ; images : `images_decoded` ≈ 880–1 040 en 8–10 s (> 600 car les cellules recyclées rechargent après éviction du cache d'images).

### Points de lecture importants
1. **Le rythme est plafonné à 60 Hz par GTK** (`GdkFrameClock`, timer 16,67 ms sous X11/Xvfb) : la spec demande un redessin continu sans pacing, GTK ne permet pas de le désactiver. Dès que le travail < 16,7 ms, tous les intervalles valent ≈16,7 ms : le harnais comptera ~50 % de « dropped > 16,7 » par simple gigue (`dropped_gt16_7` ≈ 290/599 en list : **artefact**, pas des frames lentes). La capacité réelle se lit dans `ops.paint_phase_ms` (n, mean, p50, p95, max de la phase de peinture) : list ≈ 3,9 ms/frame (≈ 250 fps de marge théorique), canvas 5 000 = 278 ms.
2. **Fin de frame mesurée** = signal `after-paint` de la `GdkFrameClock` de la fenêtre, sur le thread GTK : le rendu GSK (Cairo) est terminé côté CPU ; la présentation X11 n'est pas attendue. Seules les frames réellement peintes sont comptées (l'horloge ne peint que si un lot monté ou un scroll a invalidé un widget ; un simple tick ne peint rien : idle → 0 frame). Le signal ne peut pas être émis « à vide » sans invalidation, mais je ne peux pas prouver que le contenu a changé à chaque frame (en stream, une frame sans mise à jour de prix visible mais avec scroll change tout de même).
3. Les CPU incluent les threads Mesa/llvmpipe et le moteur.

## 6. Écarts à la spec et choix d'implémentation

- **Fenêtre** : `ApplicationWindow` 1280×720, non redimensionnable, **sans décoration** (sinon la zone de contenu serait < 720). Titre/barre d'état dessinés par l'app (40 pt + 24 pt) pour idle/list/stream/text/images ; canvas/core n'ont pas de barre (1000×600 + 100 = 700 pt tiennent).
- **Hauteurs de ligne** : mesurées par les cellules, longueur de contenu estimée (§4) : le défilement en px parcourt donc l'étendue estimée ; `ops.final_offset` = dernier offset rapporté par l'événement `on_scroll` de la plateforme ; l'étendue est bornée par `contentSize - viewport` du dernier événement. `ops.rows_drawn` = nombre de (re)liaisons du titre d'une cellule (liaison `Item #i` exécutée) pendant la fenêtre mesurée, pas de « dessins de ligne ».
- **Script de défilement** : piloté par la tâche `next_frame()` de l'engine thread, `scroll_to_offset(px, animated=false)` à chaque frame, intégration pas à pas avec `dt` réel (4 s à 3 000 px/s puis flings alternés bas/haut `20000·exp(-2t)` de 1,5 s, indéfiniment ; clamp). L'horloge démarre au début de la fenêtre mesurée.
- **Stream** : producteur `std::thread` (xorshift32 seed 0x2545F491, `index = x % 100000`, `prix = x / 100`), cadencé par un sommeil de 0,5 ms + rattrapage, file `Mutex<VecDeque>` bornée à 100 000 (drop + compte, jamais bloquant) ; l'engine thread vide la file **une fois par frame** (`next_frame`) et applique le lot à un `Vec<u32>` de prix + un signal de version ; les cellules visibles relisent leur prix (re-liaison de texte). Le « thread UI » de la spec = l'engine thread de NSH (où vit l'état de l'app), pas le thread GTK.
- **Canvas** : formes (rect/cercle/rect arrondi, tailles 16–40 pt, vitesses 60–200 pt/s issues de fnv) rebondissant, dégradé linéaire 2 couleurs **créé une fois par forme (coordonnées locales, `Arc<CanvasGradient>`) et partagé**, 1 forme sur 10 avec ombre (flou 8, décalage 2,2), clip rect arrondi 1000×600 ; graphe 10 000 points défilant (un point/frame) dans un second canvas 1000×100. **Contournement documenté d'une lenteur de NSH** : `canvas.rs::shadow()` alloue et floute un masque A8 de la taille de l'extension du clip courant pour *chaque* dessin ombré ; avec le clip 1000×600 (+ marge) pour 500 ombres cela donne **≈ 6 s par frame** (mesuré : 2 frames en 6 s ; `BENCH_NAIVE_SHADOW=1` le reproduit). Je restreins donc le clip à la boîte de la forme (+16 pt) avant l'ombre : 265 ms/frame. Sans ombre ni dégradé (`BENCH_NOSHADOW=1 BENCH_FLAT=1`) : 90 ms/frame ; sans ombre seulement : 185 ms/frame. Variables `BENCH_*` = diagnostics, désactivées par défaut.
- **Core** : trois fonctions `core_validate/core_parse/core_hash` appelées sur la trame `[A5 01 05 'hello']` via des **pointeurs de fonction passés par `black_box`** (sinon LTO inline/plie la boucle : un 1er essai a affiché 4,4·10¹⁰ appels/s, valeur aberrante corrigée) ; `core_heavy(100)` toutes les secondes sur un autre thread ; canvas allégé = 1 000 formes plates, sans graphe. `max_frame_stall_ms` = plus grand intervalle entre deux frames peintes.
- **Stress** : liste+stream 5 000/s (colonne gauche 880 pt, avec barres) + canvas 5 000 formes (panneau 400×720 : formes 400×620 + graphe 400×100) + core. `managed_heap_samples` vide (aucun tas géré en Rust seul). Compteurs de threads/flux remis à zéro au début de la fenêtre mesurée (`reset_counters`).
- **Text** : `Editor` en lecture seule, soft wrap, sans numéros de ligne, défilement triangulaire 3 000 px/s entre 0 et la fin (mesurée par l'écrêtage d'un `set_scroll_offset(1e9)`), `focus()` + `SelectAll` à t ≥ 5 s. `time_to_first_display_ms` = **depuis le démarrage du processus** jusqu'à la 1re frame de l'engine où l'Editor rapporte une plage visible (observé côté engine, ≈ 270 ms, GTK init inclus). `layout_ms` = **proxy** : plus long intervalle entre deux frames engine avant l'affichage (≈ 50 ms) — NSH n'expose pas de durée de layout. IME non testée.
- **Images** : 600 vignettes 128×128 (`asset:/images/imgNNN.png`, redimensionnées au décodage par NSH), `virtual_list` à 10 colonnes ; défilement triangulaire 3 000 px/s ; zoom 1→3→1 (6 s) + pan par valeurs animées natives (`bind_scale`/`bind_translate_x` sur le conteneur : `<Scroll>` zoomable n'accepte pas les listes). `images_decoded`/`decode_total_ms` via `on_load_start`/`on_load_end` : inclut la file d'attente des workers (le décodage lui-même n'est pas instrumentable sans patch) et les rechargements ; compteurs non remis à zéro (warm-up inclus).
- **Nav** : 3 écrans (`nav_stack`), 5 cycles de push/push/dialogue/pop/pop = 20 transitions (350 ms d'attente chacune) + 10 ouvertures/fermetures de `dialog` en fondu (250 ms) ; thread brûleur ≈ 1 cœur démarré au début de la mesure. Après la 20ᵉ transition (≈ 9,5 s) plus aucune frame n'est peinte.
- **TSX** : `list` et `stream` seulement (autres scénarios → `"unsupported": "TSX variant implemented for list and stream only"`). L'hôte Rust (même `main.rs`, feature `tsx`) charge `bundle.nshc` ; il envoie `start`, répond aux `host.call("drain")` (file bornée) et reçoit les `host.post("ops")` par le canal `nsh/host` ; le défilement est piloté en JS (`requestAnimationFrame` + `scrollToOffset`), l'horloge JS est `Date.now()`. Mesures de frame identiques (GTK after-paint). JSON : `"stack": "nsh-tsx"`.
- **Rendu GSK** : voir §8.

## 7. Backend GPUI (second choix) — partiel

`nsh-platform-gpui` (workspace séparé, `gpui-fast` 0.1.9) se construit sans dépendances système supplémentaires avec `default-features = false` (sans WebKitGTK 4.1 / GStreamer / GTK 3, absents). Le binaire `nshbench-gpui` s'ouvre sous Xvfb et **utilise Vulkan lavapipe** (`libvulkan_lvp.so` chargé, threads `llvmpipe-0..3`, avertissements `libEGL warning: DRI3 error` et `ERROR gpui_linux::linux::x11::client: Found no xinput mouse pointers` sans conséquence). `run-gpui.sh` fixe `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`.
**Limite** : GPUI n'offre aucun hook de frame à l'appli ; la seule donnée est le moniteur de NSH (`AppOptions::fps`, ligne `nsh::perf` par seconde : nombre de frames peintes, intervalles entre peintures de la vue NSH — pas la présentation). Le JSON contient donc `frame_ms: []` et `ops.perf_seconds` (une entrée par seconde) + `ops.gap_mean_ms_weighted`. Fumées (6–8 s) : list ≈ 45 fps (gap moyen 22,9 ms, pics à 32 ms = frames doublées), stream 10 000/s ≈ 47 fps, canvas 5 000 formes (dégradés + ombres, sans contournement de clip) ≈ 22 fps (gap 45 ms) avec RSS 731 Mo, images ≈ 35 fps (RSS 483 Mo), core 117 frames/5 s ; CPU ≈ 2 cœurs partout (rasterisation `tiny-skia` du canvas + wgpu/lavapipe). Le canvas GPUI est donc ≈ 6× plus rapide que celui de GTK/Cairo ici, au prix de 5× la RAM. Non mesuré par le harnais, à ne pas comparer image par image avec GTK (mesure différente).

## 8. Rendu GSK sur llvmpipe

Quatre moteurs testés sous Xvfb (`logs/renderer-compare.log`, 6 s, scénarios images et core) : tous **utilisables**, mais **`cairo` est nettement le plus rapide** :

| Renderer | images : fps / paint moy. / CPU user | core (1 000 formes) : fps / paint moy. |
|---|---|---|
| **cairo** | 60 fps / 9,5 ms / 4,1 s | 52 fps (316 frames) / 18,2 ms |
| ngl (GL via llvmpipe) | 54 frames/6 s, p95 441 ms (gels de plusieurs centaines de ms, cause non investiguée) / 108 ms / 7,3 s | 230 frames / 25,9 ms |
| gl | 250 frames / 22,4 ms / 8,8 s | 194 frames / 30,6 ms |
| vulkan (lavapipe) | 82 frames / 72 ms / 6,5 s | 238 frames / 24,9 ms |

Le défaut de GTK 4.14 (sans `GSK_RENDERER`) est `ngl` : moins bon ici ; `run.sh` impose `cairo`. En list/stream (peu de nœuds) cairo et ngl donnent tous deux ≈ 60 fps.

## 9. Bugs et anomalies rencontrés

- `canvas.rs::shadow()` : masque de la taille du clip par ombre (voir §6) — vrai problème de performance de NSH, ≈ 6 s/frame pour 500 ombres sous un clip de 1000×600.
- Un `GdkFrameClock` 60 Hz impose le rythme (voir §5).
- `Gtk-WARNING: Unable to acquire session bus: Failed to execute child process "dbus-launch"` à chaque lancement (pas de dbus dans le conteneur) : sans effet, sauf portail XDG des paramètres de couleur.
- `libEGL warning: DRI3 error` / `Found no xinput mouse pointers` côté GPUI : sans effet.
- La première compilation de `nsh-cli` a été tuée par l'environnement (`Terminated`, log `build-nsh-cli.log` du 1er essai écrasé) ; relancée avec `setsid nohup`.
- Le dépôt est très jeune (premier commit 2026-10-14, 12 jours) et la doc (`AGENTS.md`) montre un développement assisté par agents ; API en mouvement (`Spawned` sans `detach`, `next_frame` seulement dans le prélude, etc.).
- Pas de redessin partiel pour un canvas ré-enregistré : coût linéaire en nombre d'opérations à chaque frame, sur le thread GTK.

## 10. Résumé par scénario

OK : idle, list, stream (1 000 et 10 000/s), canvas (5 000 et 20 000, très lent), core, text (Editor), images, nav, stress (liste+stream+canvas+core). TSX : list, stream. Aucun scénario « unsupported » en Rust seul ; pas de liste à hauteur exacte (estimation + mesure), pas de scène retenue pour le canvas, pas de vue texte « simple » de 1 Mo (on utilise l'Editor).
