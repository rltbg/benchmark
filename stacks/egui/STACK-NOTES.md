# egui / eframe 0.37.0 : notes de pile

## Versions
rustc/cargo 1.97.0 ; eframe/egui/epaint/egui_glow/egui-winit/egui-wgpu 0.37.0 (derniere stable sur index.crates.io) ; winit 0.30.13 ; glutin 0.32.3 ; glow 0.17.0 ; wgpu 30.0.1 (variante secondaire) ; parley 0.11.1 / skrifa 0.44 (texte) ; image 0.25.10 (feature png seule) ; benchcore via dependance `path = ../../core` (rlib, appels Rust->extern "C" lies statiquement, LTO possible ; pas de passage par la .so).
Mesa 25.2.8 llvmpipe (GL 4.5 core, LLVM 20.1.2) ; lavapipe = adapter wgpu "llvmpipe (LLVM 20.1.2) / Vulkan / Cpu" (decouvert sans VK_ICD_FILENAMES).

## Build
- `./build.sh` (glow, principal) -> `target/release/egui_bench` ; `./build.sh wgpu` -> `target-wgpu/release/egui_bench`. CARGO_BUILD_JOBS=2.
- Profile : opt-level 3, lto = "thin", codegen-units 1, panic = abort (tenu en temps : OK).
- Temps : glow, build froid complet 2 min 25 s (user 4 min 08) ; wgpu froid 3 min 18 ; rebuild incremental de l'app seule 34-49 s. Logs : logs/build-attempt1.log (froid glow), build-wgpu.log, build-glow2.log, build-wgpu2.log. Aucune erreur de compilation ni avertissement bloquant (compile du premier coup).
- Features eframe utilisees : `x11`, `default_fonts`, (`glow` | `wgpu`). Sans accesskit, clipboard, wayland, persistence, system_fonts.
- Hotes reseau : seulement index.crates.io / static.crates.io (rien de bloque).

## Lancement (harnais)
`--cmd "/home/user/eval/stacks/egui/target/release/egui_bench"` ; variante `--cmd ".../target-wgpu/release/egui_bench"` (wgpu ne necessite pas LIBGL_ALWAYS_SOFTWARE ; Vulkan = lavapipe automatiquement).
Options : `--scenario/--duration/--out/--rate/--shapes/--assets` conformes. Drapeaux de diagnostic hors spec (ne PAS utiliser en mesure officielle) : `--no-shadows`, `--no-graph`.
Chaque scenario : 2 s de warm-up non mesure (meme scene, script a t=0) puis `duration` s mesurees ; le JSON est ecrit puis `process::exit(0)` (les threads de fond sont arretes par drapeau).
Scripts utilitaires : `smoke.sh <scenario> <duree> [args]` (Xvfb prive :200-599, tue seulement son PID), `shot.sh` (capture d'ecran via `import`).

## Mesure des frames (a lire)
- eframe 0.37 : l'app implemente `App::logic()` (avant) puis `App::ui()` ; rendu = tessellation, `glow` draw, `swap_buffers`, `post_rendering`. Aucun hook post-swap n'est expose.
- `frame_ms[k]` = intervalle entre les DEBUTS de `logic()` de deux frames comptees consecutives. Ce debut survient apres le retour de `swap_buffers` de la frame precedente (+ un aller-retour boucle d'evenements winit), donc l'intervalle contient update + tessellation + upload + raster llvmpipe + swap. Ce n'est pas une "presentation reelle" mesuree au pixel (sous Xvfb elle n'est pas observable) mais le meilleur proxy disponible ; ops contient aussi `ui_end_interval_p50/p95_ms` (intervalle entre fins de `ui()`, identique en regime permanent) et `frame_cpu_usage_ms_mean` (`frame.info().cpu_usage` = temps CPU de l'app hors attente swap).
- Seules les frames dont le contenu a change sont comptees ; `ops.unchanged_frames_excluded` donne les exclues (2 sur list/stream/text : premieres frames post-warm-up). eframe ne produit pas de frames sans changement en redessin continu.
- Vsync : `NativeOptions.glow_options.vsync = false` (glutin SwapInterval::DontWait) ; wgpu : `PresentMode::AutoNoVsync`. Observe sous Xvfb/GLX + llvmpipe : AUCUNE quantification a 60 Hz ; text atteint 216-262 fps (p50 ~3,7-4,3 ms), list ~110-120 fps. Le swap est une copie logicielle (XPutImage/DRI sw) synchrone, donc le debit est borne par le CPU/raster, pas par un signal d'affichage. (Impossible de confirmer si GLX_EXT_swap_control est present : eframe ne journalise rien sans logger.)

## Scenarios
| scenario | etat | remarques |
|---|---|---|
| idle | OK | coquille (barre titre, liste 1re page, barre d'etat) dessinee une fois ; aucun repaint ; un thread dort puis ecrit le JSON. CPU mesure 0 tick sur 3 s apres demarrage ; RSS ~115 Mo (llvmpipe + 11 threads). |
| list | OK | sum_heights = 8799584 verifie. 8 lignes/frame. |
| stream | OK | 1000 et 10000/s : generes ~= appliques (+ < 1 lot en attente), 0 drop. Thread producteur a cadence temps-reel (accumulateur, sleep 1 ms), `sync_channel(100000)` + `try_send`. |
| canvas | OK mais ECARTS (ci-dessous) | 5000 formes : ~7 fps ; 20000 : ~2 fps (llvmpipe). |
| text | PARTIEL | voir ci-dessous. select-all non supporte. |
| images | OK | 600 PNG decodes par 2 threads `image`, textures 256x256 gardees (pas de reduction a 128), upload max 6/frame. |
| nav | OK | 20 transitions 300 ms (push/pop, glissement, smoothstep) + modales fondu 200 ms (9 faites en 8 s), 1 thread de calcul. La sequence boucle jusqu'a `duration` (transitions_done compte tout). |
| core | OK | calls_per_s ~1,3e8 (1 call = triplet), heavy_calls 8 en 8 s, 1000 formes sans ombre/graphe ; frames ~34 fps car le thread du triplet + core_heavy saturent 2 coeurs + llvmpipe. max_frame_stall_ms ~50-60. |
| stress | OK (fumee 14 s seulement) | list + stream 5000 + canvas 5000 dans un panneau 400x720 (zone 380x580, graphe 380x100) + core. ~8 fps. `heap_samples` toutes les 10 s : `managed_heap_kb` = tas natif glibc (mallinfo2 uordblks+hblkhd, pas de runtime gere) + `rss_kb`. Run 600 s non lance. |

## Ecarts a la spec (resultats sur la pile)
1. Liste : ScrollArea (`show_viewport`, hauteurs uniformes seulement) utilisee pour la barre et le clip ; la virtualisation reelle est faite a la main : sommes prefixes `prefix[i]` (u32) + `partition_point` (recherche binaire) ; les positions ecran sont calculees en f64 car f32 perd la precision de 1 px vers 8,8 Mpx. Defilement impose par `vertical_scroll_offset`. Pas d'interaction souris testee.
2. Canvas :
   - Degrades lineaires : pas d'API ; un seul `Mesh` (u32 indices) pour toutes les formes, couleurs de sommets = fonction lineaire de la position -> interpolation exacte (rect 4 sommets, cercle 20 segments en eventail, rect arrondi rayon 8 a 4x5 points). Les bords des Mesh ne sont PAS anti-aliases (egui n'applique le feathering qu'aux Shape tessellees, qui n'ont que des fills unis) ; les cercles sont des polygones a 20 cotes.
   - Ombre : `epaint::Shadow {offset [2,2], blur 8, color noir a 110}.as_shape()` -> RectShape avec `blur_width` (support natif par forme, pas seulement fenetres). Cercles = rect a rayon w/2. Ecart d'ordre : toutes les ombres sont dessinees AVANT le mesh des formes (donc sous toutes les formes), car un seul Mesh est utilise.
   - Clip arrondi 1000x600 : egui ne clippe que par rectangle (`clip_rect`/scissor) -> clip RECTANGULAIRE ; seul le fond de la zone a des coins arrondis (rayon 12). Non emule.
   - Graphe : `Shape::line` de 10000 points (trait 1 px), un point ajoute par frame, retesselle completement a chaque frame.
   - Positions : reflexion analytique (onde triangulaire) en fonction du temps, vitesse 60-240 px/s issue de fnv.
   - Ventilation du cout (5000 formes, fumee 6 s, glow) : complet 7,2 fps ; sans ombres 7,7 ; sans graphe 7,6 ; sans les deux 9,4. Le gros du cout est le remplissage llvmpipe du Mesh (surdessin ~12x) + ~23 ms CPU de construction des sommets/tessellation ; le graphe et les ombres pesent peu.
3. Texte 1 Mo : un `Label` selectionnable tronque par ligne (9598 lignes), `ScrollArea::show_rows` (hauteur uniforme = `text_style_height(Body)`), defilement 3000 px/s en aller-retour. Selection : limitee a la souris sur les lignes visibles ; "tout selectionner" non realisable (Ctrl+A ne s'applique qu'a un label) -> `ops.select_all="unsupported"`, pas simule. Pas de retour a la ligne (les lignes <= 205 car. sont tronquees a 1280 px). `layout_ms` = lecture + decoupage (~4 ms) ; il n'y a pas de mise en page globale du Mo (a la demande, ligne par ligne) ; `time_to_first_display_ms` ~60-65 ms = debut de `main` jusqu'au debut de la 2e frame, polices de repli comprises. Polices : egui n'embarque ni CJK ni arabe/hebreu : chargement runtime de Noto Sans CJK (TTC, ~19 Mo) et DejaVuSans depuis /usr/share/fonts. Rendu verifie en capture : CJK, hebreu, arabe (parley fait le shaping/bidi), emoji simples en monochrome ; les emoji ZWJ (famille) s'affichent en tofu. Pas d'IME testee.
4. Images : 256x256 gardees en texture (zoom 3x net) -> ~157 Mo de textures GPU/CPU potentiels en logiciel. Zoom/pan = transformation manuelle de la grille (pas de widget) : echelle triangle 1->3->1 par periode de 6 s, pan horizontal sinus 4 s, defilement vertical aller-retour 3000 px/s. Pas de mipmaps (minification bilineaire).
5. Core : appels Rust directs aux symboles `extern "C"` de la rlib benchcore (pas de FFI dynamique).
6. `final_offset` = offset de la derniere frame dessinee (script evalue en temps reel, donc legerement variable).
7. Checksum : list/stream/idle = fnv(sum_heights) ; core = XOR des resultats de core_heavy.
8. Redimensionnement/ decorations : fenetre 1280x720 non redimensionnable, sans gestionnaire de fenetre sous Xvfb (pas de decoration reelle) ; barre de titre dessinee dans l'app.

## Fumee brute (glow principal, une execution, 8 s sauf mention ; ne pas utiliser comme mesure officielle)
list 967 frames (~121 fps, p95 10,0 ms, max 23,6) ; stream 10000/s 895 frames (~112 fps) ; canvas 5000 : 58 frames (~7,2 fps, p95 167 ms) ; canvas 20000 : 17 frames (~2,1 fps) ; core 273 frames (~34 fps, calls_per_s 1,3e8) ; text ~216-262 fps ; images ~111 fps (600 decodees, 1179 ms de decodage cumule) ; nav ~91 fps ; stress (14 s) 118 frames (~8,4 fps).
wgpu/lavapipe (secondaire) : list ~86 fps ; stream ~92 ; canvas 5000 ~7,0 fps ; text ~122 ; images ~85 ; core ~33 ; nav ~88 -> un peu plus lent que glow/llvmpipe sauf canvas (equivalent) mais avec un cout CPU "app" 3-4 ms vs ~1,2.

## Deploiement / dependances d'execution
- Binaire glow : 7 185 536 o (7,2 Mo) ; binaire wgpu : 12 525 944 o (12,5 Mo). Aucune ressource embarquee hors polices egui par defaut.
- `ldd` du binaire glow : libc.so.6 (2,1 Mo), libm.so.6 (0,95 Mo), libgcc_s.so.1 (0,18 Mo) uniquement ; le reste est charge par dlopen (glutin/winit) : libGL.so.1, libX11, libxkbcommon(-x11), libXcursor/Xi/Xrandr via xcb..., et par Mesa : libgallium-25.2.8 (43 Mo), libLLVM.so.20.1 (144 Mo), libicudata (31 Mo) ... ~235 Mo de .so mappes en tout (relevé /proc/PID/maps), dont l'essentiel appartient a Mesa/LLVM (rendu logiciel), pas a egui.
- Dossier de deploiement applicatif = le binaire seul (+ polices systeme Noto/DejaVu pour le scenario text).
- Lignes de code applicatif : 992 (src/main.rs 400, scen.rs 270, canvas.rs 180, data.rs 124).

## Bugs / surprises
- Aucune erreur de build. API 0.37 : `App::ui(&mut Ui, &mut Frame)` + `App::logic`, `Panel::show`, `CentralPanel::no_frame()`, `glow_options.vsync` (pas `NativeOptions::vsync`).
- Premier rendu du texte : `add_sized` centrait les lignes (corrige avec `Label` direct).
- Eframe ne fournit pas de hook post-swap ni de timestamp de presentation.
