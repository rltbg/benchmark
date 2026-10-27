# iced 0.14.0 : notes de pile (rendu logiciel iced_tiny_skia, variante wgpu/lavapipe)

## Versions
- rustc/cargo 1.99.1 ; `iced` 0.14.0 (derniere version de index.crates.io, verifiee : 0.13.1 -> 0.14.0, rien de plus recent, aucun yank), iced_core 0.14.0, iced_widget 0.14.2, iced_graphics 0.14.0, **iced_tiny_skia 0.14.1** (tiny-skia 0.11.4, softbuffer 0.4.8), iced_winit 0.14.1 (winit 0.30.13, X11), cosmic-text 0.15.0 (fontdb 0.23, swash 0.2.10, skrifa 0.44), lyon_path 1.0.19, image 0.25.10 (feature png seule, utilise par mes threads de decodage), iced_wgpu 0.14.0 / wgpu 27.0.1 (variante secondaire seulement).
- Features iced (default-features = false) : `tiny-skia canvas image-without-codecs advanced x11 thread-pool crisp web-colors` ; variante : + `wgpu` (feature cargo `wgpu` de l'app). Pas de tokio (executeur = thread-pool futures). Pas de wayland, pas de detection de theme (dbus).
- core : dependance `path = ../../core` (rlib, appels Rust -> `extern "C"` lies statiquement, pas de .so).
- Mesa 25.2.8 / lavapipe (variante wgpu) : confirme par /proc/PID/maps (libvulkan_lvp.so, libgallium, libLLVM chargees).

## Build / lancement
- `./build.sh` -> `target/release/iced_bench` (tiny-skia seul, wgpu non compile) ; `./build.sh wgpu` -> `target-wgpu/release/iced_bench` (wgpu + tiny-skia en repli, wgpu choisi par defaut via `ICED_BACKEND=wgpu` pose par l'app si absent, `ICED_PRESENT_MODE=no_vsync` idem). CARGO_BUILD_JOBS=2.
- Profil : opt-level 3, lto thin, codegen-units 1, panic abort (pas de strip).
- Temps : build froid tiny-skia 2 min 21 s (user 4 min 07 ; logs/build-cold-tinyskia.log) ; build froid wgpu 3 min 24 s (user 6 min 07 ; logs/build-wgpu.log) ; rebuild incremental de l'app seule ~17 s (tiny-skia) a 55 s (wgpu). Aucune erreur de compilation (compile du premier coup apres la lecture des sources) ; 1 avertissement (`SUM_H` inutilise).
- Hotes reseau : seulement index.crates.io / static.crates.io (rien de bloque).

Commande harnais (principal) : `--cmd "/home/user/eval/stacks/iced/target/release/iced_bench"`.
Mode `--full-damage` (voir plus bas, diagnostic hors spec mais **a mesurer aussi**) : `--cmd ".../iced_bench --full-damage"` ou `--env ICED_FULL_DAMAGE=1`.
Variante : `--cmd "/home/user/eval/stacks/iced/target-wgpu/release/iced_bench"` (ne necessite pas LIBGL_ALWAYS_SOFTWARE ; Vulkan = lavapipe auto ; un message `XDG_RUNTIME_DIR is invalid or not set` est affiche, sans effet).
Options conformes (`--scenario --duration --out --rate --shapes --assets`) + diagnostics `--no-shadows --no-graph --full-damage`. Chaque scenario : 2 s de warm-up non mesure (meme scene) puis `duration` s ; JSON ecrit puis `process::exit(0)`.
Scripts : `smoke.sh <scenario> <duree> [args]` (Xvfb prive :200-599, ne tue que son PID ; `BIN=` pour la variante), `smoke-all.sh`, `smoke-wgpu.sh`, `shot.sh` (capture). Captures : logs/shots/*.png.

## Ce qui est mesure (frame_ms) — a lire
- Boucle iced : `RedrawRequested` -> (`interface.update` des widgets) -> `draw` -> `present` (softbuffer XPutImage / swap wgpu) ; la subscription `window::frames()` diffuse un message a chaque `RedrawRequested` ; ce message est traite dans `AboutToWait` (donc **apres** le present de cette frame) : `update()` avance le script, la boucle reconstruit `view()` et redemande immediatement un redraw (`window.raw.request_redraw()`). Redessin continu sans timer ni vsync : verifie dans le code (iced_winit run_instance) et en pratique (dizaines a 230 fps, aucun plafond a 60 Hz ; iced ne plafonne rien sur X11).
- `frame_ms[k]` = intervalle entre deux traitements consecutifs de `Message::Frame` dans `update()` : marque « post-present » ; inclut update, view, layout, draw, raster tiny-skia, present, aller-retour de la boucle d'evenements. iced n'expose pas de hook post-present ni d'horodatage de presentation reelle (sous Xvfb elle n'est pas observable).
- `ops.redraw_start_interval_p50/p95_ms` = intervalles entre les `Instant` de `RedrawRequested` (debut de frame) : en regime permanent ~ identique a frame_ms.
- Seules les frames dont le contenu a change comptent (`ops.unchanged_frames_excluded`) : list/stream 2, text jusqu'a ~15 % (le defilement par `Action::Scroll{lines}` a une granularite d'une ligne de 18 px : a >166 fps plusieurs frames consecutives sont identiques et exclues). iced ne produit pas de frames fantomes en redessin continu (une frame = un `RedrawRequested` = un draw reel). `ops.custom_draws` / `frames_without_custom_draw` ne comptent que les draws de MES widgets personnalises (VList, ImageGrid, Slide, canvas) : sans objet pour text (text_editor) et partiel pour nav.

## Resultat majeur : fragmentation des regions de dommage de iced_tiny_skia
- `iced_tiny_skia::window::compositor::present` calcule les regions de dommage par difference de couches (`Layer::damage`, quads/primitives/textes compares position par position), les fusionne avec `iced_graphics::damage::group` (tri par distance du centre a l'origine, fusion seulement si l'aire ajoutee <= 20 000 px2 ; une grande region ne « mange » pas les fragments deja emis), puis **redessine toutes les couches pour chaque region** (`Renderer::draw` boucle sur `damage`), chaque quad etant re-rempli entierement (fill_path AA + masque de clip) a chaque region qu'il touche. Des qu'une grande partie de la scene change (liste qui defile, 5 000 formes qui bougent), on obtient des centaines de regions -> surcout multiplicatif.
- Verifie par experience : forcer UNE seule region (couche dont les bornes changent de 0,25 px a chaque frame -> `Layer::damage` renvoie les bornes entieres) fait passer list de 13-25 fps a 190-240 fps et canvas 5 000 de 0,2 fps (4,7 a 9 s par frame) a ~4 fps. Echantillonnage gdb (binaire avec lignes de debug) : ~70 % du temps dans `draw_quad` -> `fill_path` AA.
- `--full-damage` (ou `ICED_FULL_DAMAGE=1`) = ce contournement : les couches de mes widgets (VList, ImageGrid, Slide, ShadowLayer), une enveloppe racine `FullDamage` et la hauteur des widgets canvas (-0,25 px une frame sur deux) alternent. Visuellement sans effet. **Hors spec / hors idiome courant : le mode par defaut est l'implementation « naturelle » (sans contournement)** ; les deux jeux de chiffres sont rapportes, a mesurer tous les deux. Meme contenu, memes scripts ; seule la strategie de dommage change. Avec wgpu le mode est sans effet notable (pas de dommage).
- Autres proprietes du renderer relevees dans le code : tous les quads (meme rectangles sans rayon) passent par `fill_path` anti-aliase (pas de `fill_rect` rapide) ; chaque glyphe est un `draw_pixmap` ; le texte n'est clippe que par les bornes de **couche** (le `clip_bounds` de `fill_text` n'est qu'un test d'intersection ; s'il est inclus dans la couche, aucun masque n'est applique) -> les lignes de liste coupees en haut/bas sont dessinees dans leur propre couche (`with_layer`), sinon le texte debordait sur la barre d'etat (bug constate sur capture, corrige).

## Scenarios
| scenario | etat | remarques |
|---|---|---|
| idle | OK | coquille (barre titre, liste 1re page, barre d'etat) ; aucune subscription, aucun redraw ; thread qui dort puis ecrit le JSON. Harnais : CPU 0,5 %, RSS 16,7 Mo. |
| list | OK | sum_heights = 8799584 verifie. Widget `VList` (advanced::Widget) : sommes prefixes `u32` + recherche binaire, positions ecran en f64, ~8 lignes/frame dessinees directement (`fill_quad` pour fond/pastille ronde, `fill_text` x3). Pas de `Scrollable` (ni barre de defilement, sans interaction souris) : le decalage est impose par le script. |
| stream | OK | 1000 et 10000/s : generes ~ appliques (+ < 1 lot en attente), 0 drop ; producteur xorshift32 a cadence temps reel, `sync_channel(100000)` + `try_send` ; un lot par frame dans `update`. |
| canvas | OK mais ECARTS | `iced::widget::canvas` (Frame, Path, gradient::Linear). 5 000 formes : ~4 fps (--full-damage) / ~0,2-0,3 fps (naturel) ; 20 000 formes : ~1,1 fps (--full-damage, 14 frames/12 s) ; le naturel est injouable (plusieurs secondes/frame, non lance). |
| text | OK (limites) | voir plus bas. |
| images | OK | 600 PNG decodes par 2 threads `image`, `Handle::from_rgba` (textures 256x256 gardees, pas de reduction) ; grille dessinee par un widget `ImageGrid` (`draw_image`, filtre lineaire). 8 s : 600 decodees, ~660 ms cumulees. |
| nav | OK | 20 transitions 300 ms (push/pop, glissement par widget `Slide` qui translate l'enfant, smoothstep) + modales fondu 200 ms (quads alpha), 1 thread de calcul ; sequence en boucle, `transitions_done` compte tout. Ecrans = widgets standard (24 `container`+`text` par ecran), vue reconstruite a chaque frame. |
| core | OK | `calls_per_s` ~1,6-1,8e8 (1 call = triplet), `heavy_calls` 1/s, 1 000 formes sans ombre/graphe ; frames ~27-30 fps (--full-damage) ou ~11-16 fps (naturel, avec stalls max 1,9-2,3 s). |
| stress | OK (fumee 14 s) | list (880 px) + stream 5000 + canvas 5000 formes dans un panneau 400x720 (zone 380x580 + graphe 380x100) + core ; ~4 fps (--full-damage) / ~1,2-1,7 fps (naturel). `heap_samples` : tas natif glibc (mallinfo2) + RSS, pas de tas gere. Run 600 s non lance. |

### Canvas : primitives et limites du renderer tiny-skia
- Degrade lineaire 2 couleurs sur cercles, rectangles, rects arrondis : **supporte** (Frame::fill avec `Style::Gradient(Gradient::Linear)` -> tiny_skia::LinearGradient ; verifie en capture). Coins arrondis reels (`Path::rounded_rectangle`, rayon 8), cercles courbes (pas des polygones).
- **Ombre portee : pas d'API dans `canvas::Frame`** (ni dans tiny-skia). Les ombres (1 forme sur 10, flou 8 px, offset 2,2, noir alpha 110/255) sont dessinees par un widget separe `ShadowLayer` via `Quad.shadow` (`fill_quad`, fond transparent) : ombre SDF par pixel calculee par le CPU dans `engine.rs`. Elle est **sous toutes les formes** (couche empilee sous le canvas, comme pour egui) et non sous la forme propre dans l'ordre z ; la forme est un quad aux bornes de la forme (cercle = rayon w/2). Cout ~90 ms/frame pour 500 ombres. Non verifie en detail visuellement.
- **Clip arrondi 1000x600 : non supporte** : le clip est rectangulaire (bornes du widget canvas de 1000x600) ; le fond arrondi (rayon 12) est un `container` derriere. `Frame::with_clip` n'a de plus **aucun effet dans le backend tiny-skia** (lu dans le code : `draft` -> `paste` concatene les primitives sans clip ni translation ; non utilise).
- Graphe : `Path` de 10 000 points (`line_to`) + `Frame::stroke` 1 px, un point ajoute par frame, retesselle completement a chaque frame (~20 ms/frame). Widget canvas separe 1000x100.
- Formes : mouvement analytique (onde triangulaire) comme egui ; positions/vitesses via fnv identiques.

### Texte 1 Mo (resultat)
- Widget `text_editor` (cosmic-text) en lecture seule : toute `Action::Edit` est ignoree dans `update`, deplacements/selection/molette appliques ; `Wrapping::None` (lignes <= 205 car. non repliees -> lignes visuelles = 9 599 lignes logiques, ce qui permet un script exact), 14 px, interligne 18 px ; defilement script 3 000 px/s aller-retour par `Action::Scroll{lines}` (entier) ; a t=5 s : `Action::SelectAll` + `operation::focus` (le surlignage de selection n'est dessine que si le widget a le focus ; verifie en capture logs/shots/text-selectall.png).
- **`Content::with_text` (1 Mo) bloque le thread UI ~1,5-1,6 s** (`ops.layout_ms` ; fait dans le constructeur, avant la fenetre), `time_to_first_display_ms` ~1,7 s (debut de main -> 2e frame). RSS ~256 Mo en defilement (1 Mo de texte), 6 threads.
- Defilement : ~165 fps (naturel), ~130 fps (--full-damage), p95 9-13 ms, apres selection de tout (surlignage des lignes visibles) inclus. Wgpu : ~90-116 fps.
- Rendu verifie en capture : CJK, hebreu, arabe (shaping + alignement a droite des lignes RTL), emoji couleur (fusees, visages), accents. Defaut observe : le surlignage de selection n'apparait pas sur les lignes RTL alignees a droite. Pas d'IME testee.

### Liste (details)
- Rangees dessinees directement par `fill_quad`/`fill_text` (`Shaping::Basic`, `Wrapping::None`, sous-titre = `sub` x4, clippe par la couche de la ligne). Police par defaut de iced (fontdb/systeme) ; prix en `Font::MONOSPACE` aligne a droite.
- Cout par frame (naturel) domine par les remplissages de quads (voir dommage) ; un artefact de mesure avait ete constate avec un clip local incorrect (corrige).

## Ecarts a la spec / honnetete
1. Deux modes de mesure (naturel / `--full-damage`) : voir plus haut ; le second est un contournement d'un comportement du renderer, pas du code « lent exprès ».
2. Canvas : ombres hors canvas (quads), sous toutes les formes ; clip rectangulaire ; pas de clip arrondi ; `Frame::with_clip` inoperant.
3. Liste : pas de `Scrollable`/scrollbar ; virtualisation par widget custom ; pas d'interaction souris testee.
4. Images : zoom/pan = transformation manuelle de la grille ; defilement vertical aller-retour 3 000 px/s, pan horizontal sinus 4 s, zoom triangle 1->3->1 sur 6 s (comme egui). Pas de mipmaps (filtre lineaire).
5. nav : frames comptees en continu (la sequence boucle, il n'y a pas de temps mort) ; fond de modale = scrim noir alpha 150 + boite 420x240 rayon 12.
6. `final_offset` = offset de la derniere frame ; checksum list/stream/idle = fnv(sum_heights), core = XOR des `core_heavy`.
7. Fenetre 1280x720 non redimensionnable, position (0,0), `scale_factor` force a 1.0 ; sans gestionnaire de fenetre sous Xvfb : barre de titre dessinee dans l'app.
8. `frame_ms` mesure post-present approximatif (message de subscription traite apres le present) : voir section mesure ; pas d'horodatage de presentation reelle.
9. Les chiffres de fumee (1 execution, 8 s) ne sont pas des mesures officielles.

## Fumee brute (tiny-skia principal, 1 execution, 8 s sauf stress 14 s ; logs/smoke-tinyskia.txt)
| scenario | naturel fps (p95 ms, max ms) | --full-damage fps (p95, max) |
|---|---|---|
| list | 25,7 (54 ; 60) — 13 a 25 selon les essais | 227 (6,1 ; 21) |
| stream 10000/s | 27,4 (49 ; 59) | 208 (7,2 ; 13) |
| canvas 5000 | 0,21 (3 frames, ~9 s/frame) | 4,3 (304 ; 353) |
| canvas 20000 | non lance (naturel) | 1,1 (1161) |
| core | 16,1 (60 ; max 1869) | 27,4 (53 ; 56) |
| stress | 1,2 (19 frames) | 3,9 (311 ; 326) |
| text | 164 (9,4 ; 47) | 128 (13,4 ; 23) |
| images | 8,1 (213 ; 235) | 51 (27 ; 62) |
| nav | 51,7 (100 ; 213) | 100 (14 ; 21) |
Harnais (list --full-damage, 8 s) : 188 fps, CPU 90 % d'un coeur, RSS 17 Mo, 3 frames > 16,7 ms. idle : CPU 0,5 %, RSS 16,7 Mo.
wgpu/lavapipe (logs/smoke-wgpu.txt ; naturel / --full-damage) : list 133 / 96 fps ; stream 93 / 111 ; canvas 5000 2,9 / 3,0 ; core 8,7 ; stress 2,2 / 2,3 ; text 87-116 ; images 68 / 71 ; nav 54 / 53. Le rendu wgpu ne souffre pas de la fragmentation de dommage mais est plafonne par llvmpipe/lavapipe et entre en concurrence avec les threads core (core 8,7 fps).

## Deploiement / dependances d'execution
- Binaire tiny-skia : 6 325 264 o (6,3 Mo) ; binaire wgpu : 11 989 336 o (12,0 Mo). Aucun asset embarque (police systeme via fontdb : DejaVu/Noto trouvees ; polices CJK/emoji/arabe resolues par fallback systeme, aucune police embarquee ni chargee a la main).
- `ldd` (identique pour les 2) : libc.so.6 (2,1 Mo), libm.so.6 (0,95 Mo), libgcc_s.so.1 (0,18 Mo). Charges par dlopen a l'execution du binaire tiny-skia (/proc/PID/maps, logs/runtime-libs-tinyskia.txt) : libX11 (1,3 Mo), libxkbcommon(-x11), libxcb(+xkb), libXext, libXi, libXrender, libXcursor, libXfixes, libXau, libXdmcp, libbsd, libmd, libX11-xcb : ~6,0 Mo au total, **aucun Mesa/LLVM/OpenGL/Vulkan** (rendu 100 % CPU dans le binaire).
- Variante wgpu : + libvulkan, libvulkan_lvp (lavapipe), libgallium-25.2.8 (43 Mo), libLLVM.so.20.1 (144 Mo), libEGL... comme egui (Mesa).
- Dossier de deploiement applicatif = le binaire seul (+ polices systeme).
- Lignes de code applicatif : 1 187 (src/main.rs 605, widgets.rs 299, shapes.rs 159, data.rs 124 copie de la logique deterministe d'egui).
- RSS observee : idle 16,7 Mo ; list 17 Mo ; text ~256 Mo ; stress ~29 Mo.

## Bugs / surprises
- Aucune erreur de build ; API 0.14 : `iced::application(boot, update, view)` (boot = closure -> (State, Task)), `window::frames()`, `Widget::{layout(&mut self, ...), update, draw}`, `canvas::Program::draw(&self, state, renderer, theme, bounds, cursor)`, `Space::new()` sans argument, `operation::focus(id)`.
- Texte deborde de la liste (barre d'etat) tant que le clip n'est pas porte par une couche : `clip_bounds` de `fill_text` non applique s'il est inclus dans la couche (voir plus haut) ; corrige.
- Premier rendu canvas naturel : 4,7 a 9,8 s par frame (fragmentation de dommage) ; `Frame::with_clip` n'a pas d'effet en tiny-skia ; pas d'ombre dans `canvas::Frame`.
- wgpu : message stderr `XDG_RUNTIME_DIR is invalid or not set in the environment.` (bénin).
- Le pare-feu reseau n'a rien bloque.
