# egui / eframe 0.37.0 — notes de pile (brief v2, spec v2.1)

## Versions (vérifiées le 2026-10-26)
| composant | version | source |
|---|---|---|
| rustc / cargo | 1.99.1 (2026-10-13) / 1.99.1 | `rustup default`, `rustc --version` |
| egui, eframe, epaint, egui-wgpu, egui_glow, egui-winit, egui_system_font_provider | 0.37.0 (2026-10-15, dernière stable) | index.crates.io (`eg/ui/egui`, `ef/ra/eframe`…) |
| egui_extras | 0.37.0 (non utilisé, lu pour la revue) | index.crates.io |
| image | 0.25.10 (feature `png` seule) | index.crates.io |
| wgpu | 30.0.1 (dernière stable) | index.crates.io, Cargo.lock |
| glow | 0.17.0 (0.18.0 existe, mais eframe 0.37 exige `^0.17`) | index.crates.io |
| winit | 0.30.13 (dernière stable ; 0.31 n'existe qu'en bêta) | index.crates.io |
| glutin | 0.32.3 ; fontique 0.11.1 / harfrust 0.12.0 (imposés par epaint/egui_system_font_provider 0.37 ; 0.12 / 0.15 existent) | Cargo.lock |
| Mesa | 25.2.8 : lavapipe (Vulkan, wgpu) et llvmpipe (GL 4.5, glow), LLVM 20.1.2 | adapter info dans `notes` |

Versions épinglées `=x.y.z` dans `Cargo.toml` et `Cargo.lock` (`cargo build --locked`). Le champ JSON `version` les reprend.

## Build / lancement
- `./build.sh` : `cargo +1.99.1 build --release --locked`, CARGO_BUILD_JOBS=2, log dans `logs/build-last.log`. Build quasi froid : **8 min 43 s** (523 s, RSS max 1,0 Go, `logs/build-cold-time.log`) ; incrémental de l'app ≈ 1 min.
- Un seul binaire `target/release/egui_bench`, deux renderers : **wgpu (principal, défaut d'eframe)** ; `EGUI_BENCH_RENDERER=glow` (secondaire).
- Cadence : `BENCH_PACING=60` (défaut) ou `BENCH_PACING=uncapped`.
- Diagnostic uniquement : `EGUI_BENCH_PRESENT=vsync` (garde AutoVsync / SwapInterval::Wait) ; `EGUI_BENCH_NO_COLOR_EMOJI=1`.
- Chemins : `--assets`, sinon `$BENCH_ROOT/assets`, sinon déduit de l'emplacement du binaire. Aucun chemin absolu dans le code ; la police emoji est localisée par `fc-match`.
- `smoke.sh <scenario> [durée] [options run_bench]` : appelle le harnais (l'ancien `shot.sh` a été supprimé, `verify_gui.py` le remplace).

Lignes pour `harness/stacks.tsv` :
```
egui	${ROOT}/stacks/egui/target/release/egui_bench	
egui-uncapped	${ROOT}/stacks/egui/target/release/egui_bench	--env BENCH_PACING=uncapped
egui-glow	${ROOT}/stacks/egui/target/release/egui_bench	--env EGUI_BENCH_RENDERER=glow
egui-glow-uncapped	${ROOT}/stacks/egui/target/release/egui_bench	--env EGUI_BENCH_RENDERER=glow --env BENCH_PACING=uncapped
```

## Revue (contre la documentation officielle 0.37.0)
1. **Renderer par défaut = wgpu, pas glow.** Le README d'eframe dit « eframe defaults to using wgpu for rendering (with an option to change to glow) » (https://raw.githubusercontent.com/emilk/egui/main/crates/eframe/README.md). `default` des features d'eframe 0.37 contient `wgpu`, et `Renderer::default()` renvoie `Wgpu` même si `glow` est aussi activée (https://docs.rs/eframe/0.37.0/eframe/enum.Renderer.html, src/epi.rs). L'ancienne implémentation avait glow en principal et `default-features = false` : **corrigé**, wgpu passe en principal et glow en secondaire, choisi à l'exécution.
2. **Features par défaut** : la v1 désactivait `system_fonts`, accesskit, clipboard et wayland. Je repasse aux features par défaut (+`glow`). `system_fonts` est le repli de polices système officiel de 0.37 (« Colored emojis! », `Context::add_font_provider`, `egui_system_font_provider` sur fontique ; CHANGELOG egui 0.37.0 https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md). Il remplace le chargement manuel de Noto CJK/DejaVu par chemins `/usr/share/fonts` (supprimé).
3. **Vsync** : `glow_options.vsync` (eframe 0.35, « Group glow config in a struct ») et `wgpu_options.surface.present_mode` (défaut `SurfaceConfig::LOW_LATENCY` = AutoVsync, frame latency 1). Effet réel sous Xvfb : **aucun**. Avec vsync demandé et pacing uncapped, list fait 133 fps en glow et 85 fps en wgpu (`/tmp/egui-smoke4`) ; sans vsync, 60 à 180 fps selon la charge. Donc aucun plafond 60 Hz natif.
4. **Frame** : `App::logic`/`App::ui` tournent dans `Context::run_ui`, qui peut faire plusieurs passes par frame (https://docs.rs/egui/0.37.0/egui/struct.Context.html#method.run_ui). La v1 horodatait et avançait l'état dans `logic`, donc pouvait compter deux fois par frame et faire avancer la polyligne deux fois. **Corrigé** : l'horodatage et l'avance d'état se font dans `App::raw_input_hook`, appelé une seule fois par frame avant les passes (https://docs.rs/eframe/0.37.0/eframe/trait.App.html#method.raw_input_hook). C'est le premier code applicatif après le retour de `present`/`swap_buffers` ; eframe n'expose aucun hook post-présentation. Le reste du travail par frame est gardé par `ctx.current_pass_index() == 0`.
5. **Bug v1 : drapeau `changed` remis à zéro dans `logic`** après l'application du flux, ce qui perdait l'information de changement. **Corrigé** : un seul calcul de « contenu changé » par frame.
6. **Canvas**
   - Dégradés : epaint n'a pas de remplissage dégradé pour les formes ; le seul outil documenté est `Mesh` à couleurs de sommets (`Shape::gradient_rect` est lui-même un Mesh, https://docs.rs/epaint/0.37.0/epaint/enum.Shape.html#method.gradient_rect). Conservé.
   - Anticrénelage : le feathering du tessellateur (https://docs.rs/epaint/0.37.0/epaint/tessellator/struct.TessellationOptions.html#structfield.feathering) ne s'applique pas aux `Shape::Mesh`. Les bords de la v1 étaient crénelés. **Corrigé** : un anneau de feathering de 1 px (sommets ±0,5 px, alpha 0 à l'extérieur) est construit dans le Mesh, comme le fait le tessellateur.
   - Ombres : `epaint::Shadow::as_shape` donne une `RectShape` avec `blur_width`, flou par forme (https://docs.rs/epaint/0.37.0/epaint/struct.Shadow.html). En v1, toutes les ombres étaient sous toutes les formes. **Corrigé** : le mesh est coupé à chaque forme ombrée (~1000 shapes par frame pour 5000 formes), ce qui respecte l'ordre de peinture.
   - Clip arrondi : `Painter::with_clip_rect` ne prend qu'un `Rect` (scissor) ; `RoundedRect` (nouveau en 0.37) est une géométrie, pas un clip. → **`ops.not_rendered: ["rounded_clip"]`** ; le clip reste rectangulaire et seul le fond est arrondi.
   - Polyligne : `Shape::line`, 10 000 points, avance d'un point par frame (dans le hook).
7. **Liste virtualisée** : `ScrollArea::show_rows` ne gère que des hauteurs uniformes et `show_viewport` donne seulement le rectangle visible (https://docs.rs/egui/0.37.0/egui/containers/scroll_area/struct.ScrollArea.html#method.show_rows). Aucune API pour des hauteurs variables, donc je garde `show_viewport` + sommes préfixes u32 + `partition_point`, avec les positions écran calculées en f64. Sous-titre : la v1 répétait 4 fois la chaîne et la faisait passer à la ligne ; **corrigé** : chaîne exacte de la spec sur une ligne, clippée.
8. **Texte 1 Mo** : la doc de `TextEdit` donne l'idiome « texte sélectionnable mais non éditable : passer `&mut &str` » (https://docs.rs/egui/0.37.0/egui/widgets/text_edit/struct.TextEdit.html). La v1 utilisait un `Label` par ligne (`show_rows`), sans « tout sélectionner » possible. **Remplacé** par `TextEdit::multiline(&mut &str)` dans une `ScrollArea` : un seul Galley pour tout le Mo, mis en cache, lignes hors clip éliminées par `coarse_tessellation_culling`. « Tout sélectionner » à t=5 s : `request_focus` + `TextEditState::cursor.set_char_range(CCursorRange::select_all)` + `store` (https://docs.rs/egui/0.37.0/egui/widgets/text_edit/struct.TextEditState.html). La sélection n'est peinte que si le widget a le focus. Coût observé : RSS de 323 à 464 Mo, et des frames jusqu'à 60–120 ms une fois tout le Mo sélectionné.
9. **Polices CJK / emoji / RTL** : le repli `system_fonts` donne bien CJK, hébreu et arabe (shaping harfrust), mais fontique choisit DejaVu pour les emoji, qui sortent monochromes et en ZWJ décomposé. J'ajoute donc « Noto Color Emoji » (CBDT, rendu couleur par epaint `color_fonts`), trouvée par `fc-match`, en fin de famille. Résultat vérifié sur capture : emoji en couleur et **famille ZWJ composée en un seul glyphe**.
10. **Images** : `egui_extras::install_image_loaders` décode bien hors thread (src/loaders/background_decode.rs), mais ignore le `SizeHint`, garde des textures 256×256 et n'expose pas de durée de décodage. Je garde donc 2 threads `image` qui décodent puis réduisent en **128×128** (spec ; la v1 gardait 256×256), avec au plus 16 `load_texture` par frame. `decode_total_ms` = somme des décodages + réductions.
11. **Cœur** : les appels passent par des pointeurs `extern "C"` opacifiés par `black_box`, l'accumulateur est publié (atomique, `ops.core_acc`, entre dans le checksum). `core_heavy` est appelé toutes les 1 s de début à début (la v1 avait 1,1 s : 100 ms d'appel + 1 s de sommeil).
12. Edition 2024 (`unsafe extern "C"`) ; `persist_window = false` ; JSON avec échappement correct (la v1 remplaçait les `"`), plus `version`, `unsupported: null` et `ops.not_rendered`. Aucun `Id::new` (déprécié en 0.37).

## Cadence (spec v2.1)
- Le vsync n'a pas d'effet sous Xvfb (point 3). `BENCH_PACING=60` utilise donc l'API publique `Context::request_repaint_after` vers la prochaine échéance d'une grille fixe de 16,667 ms depuis le démarrage (pas de dérive) ; une frame en retard déclenche un repaint immédiat. `uncapped` appelle `request_repaint()` à chaque frame. Le mode effectif est écrit dans `ops.pacing`.
- Les événements clavier provoquent un repaint immédiat d'egui, d'où 64 fps en `input`.

## Frame et marqueurs
- `frame_ms[k]` = intervalle entre deux `raw_input_hook` consécutifs, compté seulement si la frame précédente avait un contenu changé. `unchanged_frames_excluded` donne le nombre de frames exclues.
- `BENCH:measure_start` / `BENCH:measure_end` sont écrits sur stdout avec flush (dans un thread pour `idle`).
- Entrée (`input`) : `egui::Event::Key{Space, pressed, !repeat}` lu dans `RawInput` (chemin winit → egui-winit), carré 200×200 en (100,100) noir/blanc. Le focus clavier vient du harnais (XSetInputFocus) ; aucune bidouille X11 dans l'app, et aucune n'a été nécessaire.

## Vérification (verify_gui, captures dans results/verification/egui-*.png)
Captures relues pour tous les scénarios : liste (pastilles, sous-titre, prix, barres titre/état), canvas (dégradés, bords lissés, ombres douces, coins **non clippés** en arrondi), texte (CJK/hébreu/arabe, emoji couleur, ZWJ composé, tout sélectionné à t>5 s), images (vignettes 128 zoomées), nav (glissé), core, input (carré noir), stress. Déchirure possible sur la capture `list` (lecture du framebuffer en cours de copie). Quelques runs de verify_gui ont collisionné sur :390 avec un autre agent (fenêtres « benchapp », « bench-MainKt ») : relancés.

## Fumée brute (harnais, 8 s, 1 run, machine chargée : load ≈ 7 sur 4 vCPU ; pas des mesures)
wgpu, pacing 60 (`/tmp/egui-smoke`) :
| scénario | fps | p95 ms | CPU % | PSS Mo | ops |
|---|---|---|---|---|---|
| idle | – | – | 0,0 | 100 | sum_heights 8799584 |
| list | 58,3 | 23,3 | 84,6 | 101 | sum 8799584, final_offset 20638.8 |
| stream 10k | 59,9 | 18,5 | 88,8 | 125 | 80149 générés / 79986 appliqués / 0 perdu |
| canvas 5000 | 6,6 | 201 | 229 | 287 | not_rendered [rounded_clip] |
| text | 43,0 | 60,4 | 110 | 287 | layout 728 ms, ttfd 928 ms, select_all true |
| images | 57,3 | 24,8 | 121 | 155 | 600 décodées, 1799 ms cumulés |
| nav | 55,9 | 26,2 | 212 (burner ≈ 100) | 107 | 20 transitions, 9 modales |
| core | 14,4 | 106 | 164 | 122 | 4,7e7 calls/s, 8 heavy, stall max 146 ms |
| input | 64,5 | 21,1 | 97 | 101 | 29 touches, 0 perdue, latence p50 16,1 / p95 31,0 ms |
| stress (8 s) | 4,9 | 323 | 259 | 271 | flux 0 perte, 8 heavy |

60 contre uncapped (`/tmp/egui-smoke3`, list) : wgpu 59,5 fps / 98 % CPU contre 111,6 fps / 167 % ; glow 59,6 fps / 73 % contre 182,8 fps / 240 %. Canvas 5000 en glow : 3,7 à 5,6 fps (série bruitée).

## Écarts / limites
- `rounded_clip` non rendu (limite du moteur).
- Frame = proxy post-present (pas de timestamp de présentation exposé par eframe).
- Canvas et stress très en dessous de 60 Hz en rendu logiciel. Le coût CPU du Mesh (~20 ms/frame, `frame_cpu_usage_ms_mean`) s'ajoute au raster lavapipe/llvmpipe.
- Texte : un « tout sélectionné » sur 1 Mo coûte cher (repeinture de la sélection du Galley entier).
- `heap_samples` (stress) = tas natif glibc (`mallinfo2`) + RSS, pas un tas géré (Rust n'a pas de GC ; pas de `ops.gc`).
- Prix du flux = x/100 avec x sur 32 bits (spec), donc jusqu'à 42 949 672,95.

## Déploiement
- Binaire 20 641 864 o (17 284 784 o strippé), wgpu + glow + fontique inclus. `ldd` : libgcc_s.so.1, libm.so.6, libc.so.6 seulement. Le reste est ouvert par dlopen : libX11/xcb/xkbcommon (winit), libvulkan + lavapipe ou libGL + llvmpipe (Mesa, libgallium ~43 Mo, libLLVM ~144 Mo), fontconfig (fontique). Polices système Noto CJK / Noto Color Emoji / DejaVu.
- Code applicatif : 1 115 lignes (main 482, scen 286, canvas 206, data 141).
