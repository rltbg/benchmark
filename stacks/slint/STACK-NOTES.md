# STACK-NOTES : Slint (Rust), winit/X11, rendu logiciel

## Versions
- rustc/cargo 1.97.0 (2026-07), crates : slint 1.18.1 (dernière stable de index.crates.io, non yanked), slint-build 1.18.1,
  i-slint-backend-winit 1.18.1, winit 0.30.13, softbuffer 0.4.8, i-slint-renderer-software 1.18.1, femtovg 0.27.0 (variante GL),
  serde_json 1.0.154. Features slint : `std backend-winit-x11 renderer-software renderer-femtovg software-renderer-path compat-1-2 image-default-formats unstable-winit-030`
  (default-features = false). `software-renderer-path` est nécessaire sinon le logiciel ne dessine pas `Path` (« Path rendering is disabled without the path feature »).
- core : `libbenchcore.a` (/home/user/eval/core/target/release) lié statiquement via `extern "C"` dans build.rs (pas de dépendance crate).
  Le staticlib Rust embarque sa propre libstd -> `duplicate symbol: rust_eh_personality` (rust-lld) ; contourné par `-Wl,--allow-multiple-definition` (build.rs).

## Build / lancement
- Build : `/home/user/eval/stacks/slint/build.sh` (CARGO_BUILD_JOBS=2 ; profil release : opt 3, lto fat, codegen-units 1, panic abort, strip).
- Binaire : `/home/user/eval/stacks/slint/app/target/release/slintapp`
- Commande harnais : `--cmd /home/user/eval/stacks/slint/app/target/release/slintapp` (rendu logiciel par défaut).
- Variantes (variables d'env, lues par slintapp) :
  - `SLINTAPP_RENDERER=femtovg` (+ `LIBGL_ALWAYS_SOFTWARE=1`) : même binaire, backend winit + FemtoVG (OpenGL ES) sur llvmpipe. Défaut : `software`.
  - `SLINTAPP_THROTTLE=1` : désactive mon contournement du frame-throttle (voir plus bas) -> pacing natif Slint (60 Hz).
- Temps de compilation : premier build complet (toutes dépendances, -j2) 6 min 27 s ; rebuild de l'appli seule (lto fat) 1 min 45 s à 2 min 15 s. Logs bruts : logs/build1..8.log.

## Ce qui est mesuré (frame_ms)
Slint n'offre PAS de `set_rendering_notifier` avec le moteur logiciel (SoftwareRenderer : « not supported »). On passe donc par
`BackendSelector::with_winit_custom_application_handler` (feature `unstable-winit-030`) :
- `window_event(RedrawRequested)` marque qu'un draw Slint va avoir lieu ; `about_to_wait` (appelé après le draw : rendu Slint + `present_with_damage` softbuffer, ou swap GL) horodate la **fin de frame** ;
  `frame_ms` = intervalle entre deux fins de frame consécutives (mesure : début de `about_to_wait` suivant le redraw).
  Le même hook sert pour FemtoVG. Il inclut donc la mise à jour applicative faite dans `about_to_wait` (scripts, modèles) + le draw.
- **Redessin continu** : à la fin de `about_to_wait` l'app appelle `winit_window.request_redraw()`. Sur X11 Slint temporise normalement les redraws par un Timer à la fréquence du moniteur (60 Hz, `TimerBasedFrameThrottle`) ; la spec interdit tout pacing, donc ce throttle est contourné (par défaut). Mesure témoin du pacing natif : list, `SLINTAPP_THROTTLE=1` -> ~16.2 ms/frame (~62 fps) vs ~2 ms sans throttle.
- Seul `nav` est gaté : redraw demandé seulement pendant les animations (sinon on compterait des frames vides à 0,04 ms sans changement de contenu) ; un Timer 2 ms (callback vide) réveille la boucle pour lancer les transitions et détecter la fin.
- Warm-up 2 s non enregistré ; le script (offset, formes, etc.) redémarre à t=0 au début de la mesure. `ops.app_step_ms_mean` = temps passé dans mon code de mise à jour par tick ; `ops.draw_ms_mean` = RedrawRequested -> fin de frame (inclut l'évaluation des bindings Slint, le rendu et le present).
- Idle : aucun redraw, boucle d'événements endormie, un Timer unique à 2+duration s écrit le JSON (frames = 0). CPU ≈ 0,5 %, RSS ≈ 22,9 Mo.

## Implémentation par scénario
- **list / stream** : OK. Virtualisation maison : préfixe de hauteurs cumulées (`Vec<u32>` 100 001 entrées, sum = 8 799 584 vérifié dans `ops.sum_heights`), recherche binaire de la fenêtre visible (+1 ligne de marge), 32 « slots » `Row` instanciés (anneau `i % 32`) alimentés par un `VecModel<RowData>`, un seul `scroll-y` (relatif à un origine recalée par blocs de 16 lignes) déplace le conteneur. `ListView` Slint (hauteur fixe uniquement) n'a pas été utilisé. `ops.rows_drawn` = somme sur les frames du nombre de lignes de la fenêtre (visibles + marge).
  Défilement : 4 s à 3000 px/s puis flings alternés bas/haut v0=20000 px/s, e^{-2t}, 1,5 s, répétés au-delà de 6 ; position intégrée analytiquement entre deux ticks et clampée à chaque tick.
  Flux : thread producteur xorshift32 (seed 0x2545F491, idx = x % 100000, prix = x/100), canal `std::sync::mpsc::sync_channel(100_000)` + `try_send` (drop + compteur si plein), pacing par rattrapage temporel ; le thread UI vide le canal à chaque tick (`updates_applied`), met à jour le prix des lignes visibles (`set_row_data`). `invoke_from_event_loop` n'est pas utilisé pour les prix (un event par update serait plus coûteux) ; il l'est pour l'arrivée des images.
- **canvas** : OK mais très limité en logiciel. Pas d'API canvas immédiate dans Slint : un élément `Rectangle` par forme dans un `for` (5000/20000 instances, `VecModel<ShapeData>`, tous les `set_row_data` à chaque frame), dégradé = `@linear-gradient`, clip = `Rectangle clip: true`. Graphe : élément `Path` dont la chaîne `commands` (10 000 points « M/L ») est reconstruite à chaque frame (1 point ajouté/frame). Pas de variante SharedPixelBuffer/raster maison implémentée (un widget par forme est la voie idiomatique, mesurée telle quelle).
  **Limites du moteur logiciel 1.18.1 (lues dans le code de i-slint-renderer-software)** : `draw_box_shadow` = `// TODO` (ombres portées NON dessinées : `ops.shadows_rendered=false`) ; les rectangles à dégradé sont dessinés SANS rayon (cercles et rect arrondis apparaissent carrés, vérifié visuellement) ; `combine_clip` ignore le rayon (clip rectangulaire, mais le fond du panneau garde ses coins arrondis) ; `scale`/`rotate` = TODO. Avec FemtoVG ces effets sont rendus mais 5000 formes -> ~780 ms/frame sur llvmpipe.
  Coût mesuré (fumée 6 s) : 5000 formes + graphe ≈ 52 ms/frame (19 fps) ; 20 000 formes ≈ 141 ms ; 1 forme + graphe seul ≈ 28 ms -> le Path de 10 000 points (parse + tessellation lyon chaque frame) coûte à lui seul ~26 ms.
- **core** : OK. Thread worker (boucle serrée validate+parse+hash, trame 8 octets A5 01 05 'hello', compteur par lots de 1024), thread heavy (`core_heavy(100)` chaque seconde), 1000 formes animées (sans graphe) côté UI. `ops.calls_per_s` ~7e7, `max_frame_stall_ms`.
- **stress** : OK (sauf `managed_heap_kb` : pas de tas managé en Rust, donc pas d'échantillons ; RSS est mesuré par le harnais). list (880 px) + stream (5000/s par défaut) + canvas 5000 formes (panneau 400×720 à droite, sans graphe) + core.
- **text** : OK, partiel quant à la sélection. `TextEdit` std-widgets (style fluent) en lecture seule, word-wrap, `content-y` piloté (aller-retour 3000 px/s) ; « tout sélectionner » via `TextEdit.select-all()` à t=5 s (appelé ; non vérifié visuellement après coup). `time_to_first_display_ms` ≈ 0,9–1,2 s (set_text 1,2 ms ; le layout paresseux du 1 Mo se fait avant le draw) ; `layout_ms` = first_display − premier draw. Contenu : 1 000 111 octets, ≈162 000 px de hauteur. RSS ≈ 116 Mo. IME / bidi / emoji non vérifiés (protocole manuel) ; la capture montre bidi et emoji couleur rendus en monochrome/approximatif.
- **images** : OK. 600 `Rectangle`+`Image` (grille 9 colonnes) tous instanciés (le logiciel ne dessine que ce qui est visible) ; décodage sur 2 threads (`Image::load_from_path` + réduction 2×2 en 128×128), remise au thread UI par `Weak::upgrade_in_event_loop`. Le logiciel n'a pas de transformation d'échelle : le zoom 1→3→1 sur 6 s est obtenu en redimensionnant les cellules (coût de redimensionnement d'image à chaque draw) ; défilement 3000 px/s ET zoom/pan simultanés pendant toute la mesure. `ops.images_decoded`=600, `decode_total_ms` ≈ 1,7 s cumulé.
- **nav** : OK. 3 écrans (`Rectangle` avec `animate x 300ms ease-in-out`), modale (`animate opacity 200ms`), cycle push, push, modale ouverte, modale fermée, pop, pop à raison d'un pas par 300 ms ; thread brûleur ≈ 1 cœur. `transitions_done` compte les pas dont l'animation est écoulée (≈ duration/0,3 − 1). Frames comptées uniquement pendant les animations.

## Écarts à la spec / honnêteté
- Contournement du frame-throttle Slint (voir ci-dessus) : fait partie de la mesure par défaut ; le pacing natif est disponible via SLINTAPP_THROTTLE=1.
- Ombres portées et formes arrondies à dégradé : non rendues par le moteur logiciel (limites de la stack, pas simulées). Le JSON porte `shadows_rendered:false`.
- Checksums : XOR de fnv(final_offset) / fnv(position d'une forme) / fnv(accumulateur core), seulement indicatifs (dépendent du timing).
- `rows_drawn`, `app_step_ms_mean`, `draw_ms_mean`, `set_text_ms`, `select_all_called`, `pool_rows`, `shadows_rendered`, `decode_threads` sont des ops supplémentaires.
- Les mesures de fumée (6–8 s) ne sont pas des résultats officiels.

## Fumée (harnais, 6 s, 1 run, Xvfb 1280×720, llvmpipe)
| scénario | frames | ≈ fps | p99 frame_ms | CPU % | RSS Mo |
|---|---|---|---|---|---|
| idle | 0 | - | - | 0,5 | 22,9 |
| list | 3092 | 515 | 4,5 | 76 | 23,2 |
| stream 10000/s | 2876 | 480 | 4,2 | 75 | 25,9 |
| canvas 5000 | 113 | 19 | 79 | 95 | 31,2 |
| core | 1203 | 200 | 9,5 | 184 | 22,2 |
| stress | 219 | 37 | 40 | 205 | 35,9 |
| text | 1410 | 235 | 6,5 | 91 | 115,8 |
| images | 1688 | 281 | 6,1 | 98 | 77,7 |
| nav (gaté anim.) | 4185 | ~700 pendant anim. | 4,2 | 99 | 22 |
Hors harnais : canvas 20 000 formes ≈ 7 fps ; FemtoVG/llvmpipe : list ≈ 154 fps (6,5 ms), canvas 5000 ≈ 1,3 fps, stress ≈ 1,4 fps. JSON bruts dans logs/smoke/.

## Taille / exécution
- Binaire release strippé : 16 Mo (15 881 320 o, FemtoVG + logiciel + std-widgets).
- Libs dynamiques (ldd) : libc 2,1 Mo, libm 0,95, libfreetype 0,83, libfontconfig 0,33, libpng16 0,22, libexpat 0,19, libgcc_s 0,18, libbrotlicommon 0,14, libz 0,11, libbz2 0,08, libbrotlidec 0,05 (≈ 5,2 Mo système). libX11/libxcb/libxkbcommon/libEGL/libGL chargées par dlopen à l'exécution (winit/glutin). Dossier de déploiement = le binaire seul (+ polices système via fontconfig, assets en `--assets`).
- Code applicatif : src/main.rs 189 lignes + src/sim.rs 765 + ui/app.slint 283 + build.rs 10 = 1247 lignes.

## Bugs / messages rencontrés
- `error: Cannot access id 'parent'` (app.slint:8, `width: parent.width` sur la racine d'un composant) -> largeur passée par l'instanciation.
- `rust-lld: error: duplicate symbol: rust_eh_personality ... libbenchcore.a` -> `-Wl,--allow-multiple-definition`.
- `slint::Window::set_rendering_notifier` indisponible en logiciel (voir plus haut).
- 1re version de nav : le harnais a tué le binaire après 126 s (plus aucun événement -> plus de tick) ; corrigé par le Timer 2 ms.
- Avertissement de build : champs `pos`/`last_t` de `ImagesSim` non lus (sans effet).
- Message au lancement : « Error watching for xdg desktop settings: Failed to connect to address unix:path=/run/user/0/bus » (pas de D-Bus ; inoffensif).
