# OPTIMIZATION-LOG : Slint, bancs v3 (spec §1.2)

Méthode : v0 = version évidente ; vN = optimisations ; chaque gain est mesuré par fumée (6 s, `run_bench.py` 1 run, `skia-software`, cadence 60 Hz, même binaire, variante activée à l'exécution quand c'est possible : A/B sur la même compilation). La machine était partagée avec d'autres compilations : écarts < 10 % = bruit. Justifications : fichiers des crates 1.18.1 (`~/.cargo/registry/src/*/`, abrégés `core/` = `i-slint-core-1.18.1/`, `winit/` = `i-slint-backend-winit-1.18.1/`, `skia/` = `i-slint-renderer-skia-1.18.1/`) ; le dépôt `/opt/src/slint` (HEAD 372cf0e) contient les mêmes sources en sparse. Profils : `perf record -e cpu-clock -F 499 -p <pid>` (symboles conservés).

## Transversal : taille explicite des `Text` (variante `sizes=implicit` = v0)
- v0 : `Text { width: ..; text: .. }` sans hauteur. La hauteur est alors la hauteur préférée du texte, donc une mise en forme (parley/fontique) à chaque réévaluation de la géométrie, y compris pour les éléments hors écran (le profil de `ui-bind` N = 100 k montre `memcmp` 22 %, `fontique::match_font`, `FontRequest::query_fontique`, `read_fonts::table_data` ≈ 40 % du temps) : `core/items/text.rs` (`layout_info`, `font_request` → `WindowItem::resolved_font_request`).
- v1 : `height` explicite (`Bench.explicit ? 14px : self.preferred-height`) sur les Text d'étiquettes, de lignes et de puces.
- Gains mesurés : `ui-bind` N=100 k model 2,2 → 8,6 fps (× 3,9), derived 2,8 → 10,0 fps (× 3,6) ; `text-labels` N=5 000 31,2 → 48,1 fps ; `list-rich` K=96 (native) 33,6 → 41,3 fps. Sans effet sur L1 (62,1 fps dans les deux cas : 6 Text par ligne, 15 lignes).

## L1 list-scale
- v0 (v2) : anneau de 32 lignes + `VecModel::set_row_data` + préfixe `Vec<u32>` de N+1 entiers (40 Mo à 10 M) + prix `Vec<f64>` (80 Mo à 10 M).
- v1 `impl=pool` : sommes préfixes **éparses** (1 point / 64 lignes, 0,6 Mo à 10 M), origine par blocs de 4 096 lignes (f32 exact), pas de tableau de prix. Positions exactes. 1 M lignes : 62,1 fps, CPU 16,5 %, `rows_drawn` 2 979.
- v2 `impl=native` (défaut) : `ListView` officiel sur un `Model` paresseux (`row_data` construit la ligne, `row_count` = N, aucune donnée par ligne). 1 M lignes : 62,1 fps, CPU 16,2 % ; 10 M : 62,1 fps, `model_build_ms` 38 ms, RSS 28,5 Mo. Aucun gain de fps entre les deux (rendu = 15 lignes), mais pas de structure côté application et c'est ce que fait la doc (« seuls les éléments visibles sont instanciés »). Un `for` hors `ListView` refuserait > 1 048 576 lignes : `core/model/repeater.rs:172-190`.
- Limite connue du `ListView` : la hauteur des lignes non instanciées est estimée par la moyenne des instances (`core/model/repeater.rs:296-300`, `:424-426` `cached_item_height`), un saut de plus de 1,5 écran détruit toutes les instances et re-sonde la ligne par `floor(-content_y / hauteur_moyenne)` (`:323-336`) : l'offset en px n'est donc pas exact pour des hauteurs variables ; `pool` l'est. Les instances ne sont pas recyclées (pas de pool, `:449-487`).

## L2 list-rich (K = 6 … 96)
- v0 `impl=native` + sizes implicit, K=96 : 33,6 fps, p99 45 ms.
- v1 sizes explicit : 41,3 fps.
- v2 `impl=pool` (défaut du scénario) : les instances de ligne sont **réutilisées** (`row_changed` met à jour les propriétés, au lieu de détruire et recréer 100 éléments par ligne) : 52,8 fps (p99 32 ms, 24 frames > 25 ms contre 106). Dernière fumée du binaire final : 45,9 fps (bruit machine). Justification : `core/model/repeater.rs:449-487` (instances détruites/créées par le `ListView`), `:533-548` (`row_changed` → `update` sur l'instance existante).
- Capacité observée : K=24 61,5 fps ; K=48 58,5 fps ; K=96 ≈ 46–53 fps (profil : mise en forme du texte, `malloc/free`, `drop` d'`ItemTree`).

## L3 list-mutate
- Une seule conception : `ListView` + `Model` personnalisé (`Vec<Rec>` dans un `RefCell` + `ModelNotify::{row_changed,row_added,row_removed}`), producteur de fond → file SPSC sans verrou (2^20 opérations, `drop` compté) → lot complet appliqué une fois par frame. `VecModel` ne fournit pas de « move » et `insert/remove` y sont un `Vec::insert/remove` (memmove) comme dans mon modèle (`core/model.rs` VecModel). 
- Correction de comptage (pas une optimisation) : sans dernier lot, `applied/generated` = 99,67 % (retard d'une frame) ; avec arrêt du producteur + dernier drainage : 100 %.
- Mesures : 1 k/s … 1 M/s tiennent (1 M/s : 61,0 fps, p99 26 ms, max_batch 34 667) ; 10 M/s : 7,4 fps, 61 % des opérations perdues (file de 1 Mi pleine, `max_batch` 682 719). Non testé : structure chunkée (O(√n)) pour `insert/remove` : le `Vec` n'est pas dominant jusqu'à 1 M/s (CPU 50 %).

## L4 list-reset
- v0 `build=ui` (reconstruction sur le fil UI) vs v1 `build=bg` (défaut ; `Vec<u32>` d'identifiants de N éléments rempli dans un fil de fond puis `ModelRc` échangé sur le fil UI) : à 1 M lignes la reconstruction coûte ≈ 2 ms et l'échange ≈ 3 ms dans les deux cas ; `ui_stall_ms_max` 11 ms (ui) / 30 ms (bg, bruit d'une frame à 20 ms). Pas de gain mesurable : le modèle paresseux rend le remplacement quasi gratuit ; le `ListView` ne recrée que les instances visibles (`core/model/repeater.rs:612-616` `reset` vide les instances).

## T1 text-big
- Une seule conception : `TextEdit` lecture seule, texte entier dans un `SharedString`. Mesures : 1 Mo 1,8 s (setup), 10 Mo 19 s / 1,0 Go, 50 Mo 117 s / 4,6 Go ; une fois affiché, le défilement tient 60 Hz (62,1 fps à 10 Mo). La mise en forme du texte entier a lieu dans `show()`, pas pendant le dessin (1re frame 5 ms). Non testé : découpage en paragraphes dans un `ListView` (perdrait la sélection globale/`select-all()`, contraire à la spec).

## T2 text-labels
- v0 sizes implicit : N=5 000, 31,2 fps ; v1 hauteur explicite : 48,1 fps (cf. transversal). Hors fenêtre les étiquettes sont écartées au rendu, mais restent visitées (coût O(N) par frame, `partial_renderer` `compute_dirty_regions`).

## T3 text-edit
- v0 `wrap=word` : p50 938 ms par touche (1 Mo). v1 `wrap=none` : 935 ms (aucun gain). `direct` : 934 ms. Profil : `parley::shape::shape_item`, `harfrust` (HarfBuzz), `fontique::charmap` ≈ 40 % : le document entier est re-mis en forme à chaque édition (pas de cache de paragraphes par modification). Dépendance à la taille (variante `docsize`) : 16 Ko 22 ms, 64 Ko 61 ms, 256 Ko 246 ms, 1 Mo ≈ 940 ms (≈ 1 ms/Ko). C'est un résultat de capacité, pas un défaut d'implémentation.

## T4 text-rich (partiel)
- `StyledText::from_markdown` (API publique) : 62 fps jusqu'à 100 k spans ; 1er affichage 3,4 s, `ui_stall` 986 ms à 100 k. Polices/tailles par span non exprimables (cf. `core/styled_text.rs`, `builtin_elements.rs` StyledTextItem) → `ops.not_rendered`.

## G1 gfx-shapes
- v0 `drive=model` : Rust recalcule x,y de N formes et appelle `set_row_data` à chaque frame. v1 `drive=bind` (défaut) : une propriété `t` écrite par frame, `x`/`y` sont des liaisons (`L - abs(L - mod(x0 + vx·t, 2L))`).
- N=5 000 solid : 25,9 fps (model) vs 26,3 fps (bind) : pas de gain, le temps est dans la rastérisation Skia (`aaa_fill_path`, `blit_trapezoid_row`, `SkRasterPipeline`, ≈ 40 %), pas dans les liaisons. Gardé `bind` (moins de travail Rust, CPU identique).
- Capacité par variante (fumée N=250 → 5 000) : solid 62,2 → 29,8 fps ; gradient 62,2 → 15,9 ; shadow 62,0 → 5,6 (ombre : 1 couche floutée par forme) ; clip 62,2 → 20,6 ; all 61,9 (250). La fonction qui s'effondre en premier : l'ombre portée, puis le clip arrondi et le dégradé.
- Moteur `software` : mêmes fps mais ombres, dégradés arrondis et clip arrondi non dessinés → `ops.not_rendered` (`i-slint-renderer-software` `draw_box_shadow` TODO, `lib.rs:3059`).

## G2 gfx-paths
- Aucune optimisation trouvée : coût = rastérisation de trait Skia (`SafeRLEAdditiveBlitter::blitAntiH`, `blit_trapezoid_row`, mémoire) ; 100 chemins 41 fps, 1 000 chemins 3,6 fps. Les chemins sont déplacés par liaison (pas de re-parse des commandes). `dashed` : non supporté.

## G3 gfx-chart
- v0 `render=path` : la polyligne est une chaîne de commandes `M/L` reconstruite et reparsée chaque frame : N=100 k 59,7 fps (16,3 ms de construction), N=1 M 6,2 fps (159 ms).
- v1 `render=pixbuf` (défaut ; approche de l'exemple `plotter` de Slint : `SharedPixelBuffer` + `Image`) : N=100 k 62,2 fps (5,8 ms), N=1 M 17,4 fps (51 ms, Bresenham 1 px sans anti-aliasing). Gain × 2,8 à 3 sur la construction.

## G4 gfx-images
- v0 `premul=off` (`Image::from_rgba8`) vs v1 `Image::from_rgba8_premultiplied` : N=200 4,5 fps dans les deux cas (la rastérisation `bilerp_clamp_8888` + `blit_row_s32a_blend` fait 80 % du temps : rotation + opacité + échelle). Gardé premultiplié (évite un `premultiply` par pixel au 1er usage). Décodage 600 PNG sur 3 fils hors fil UI : 480 ms ; `slint::Image` non `Send` : `from_rgba8_premultiplied` sur le fil UI (`core/graphics/image.rs`), `Image` gardées par l'application (cache de chemin plafonné à 5 Mio : `core/graphics/image/cache.rs`).
- `big` : 4096² puis 8192² : chargement 163 ms / 666 ms, 42 fps, RSS 681 Mo.

## G5 gfx-layers
- Aucune optimisation : chaque groupe `opacity: 0.5` est une couche hors écran ; 50 groupes 34,6 fps, 200 groupes 8,5 fps. `blur` : non supporté.

## U1 ui-tree
- v0 `layout=layout` (défaut, idiomatique) : `VerticalLayout` de `HorizontalLayout` dans un `Flickable` (sans lui le minimum du layout imposerait une taille mini à la fenêtre). v1 `layout=manual` : mêmes éléments, `x`/`width` par liaison `root.width / 10 * k`, `y = c * 23px`.
- N=100 k feuilles : setup 4,8 s → 0,77 s, RSS 712 → 273 Mo, frame de redimensionnement 312 → 178 ms (33 redimensionnements). N=10 k : 62 ms (limité par le pas de 60 ms) pour les deux. 500 k feuilles (layout) : setup 43,9 s, RSS 3,5 Go.

## U2 ui-bind
- v0 `bind=model` + hauteur implicite : 100 k, 2,2 fps. v1 hauteur explicite : 8,6 fps. v2 `bind=derived` (une propriété écrite, N liaisons `Bench.tick + i`) : 10,0 fps à 100 k, 30,8 fps à 10 k vs 36,1 (model) : pas de gain réel (le coût est la mise en forme des étiquettes visibles + la visite O(N) du graphe, pas le transfert Rust→UI). Défaut = `model` (N changements de propriété par frame, le plus proche du « N propriétés changent »).

## U3 ui-anim
- Une seule conception (moteur d'animation : `animate x/y/opacity { iteration-count: -1; direction: alternate }` ; `Bench.go` passe à `true` après la 1re frame). 1 k éléments (3 k propriétés animées) : 61,6 fps, CPU 45 % ; 10 k : 11,6 fps ; 100 k : 1,2 fps. Aucun code Rust par frame.

## F1 ffi-calls
- Pointeurs `extern "C"` opacifiés par `black_box` (sinon la LTO inline `core_noop`/`core_add` et la mesure serait du code inlinable). Une fonction par tranche de 3 s ; débit = Σ_fils appels / temps de phase. 1 fil : 1,28 G appels/s (toutes fonctions) ; 4 fils 1 Mo : FPS UI 58,9, p99 30 ms (4 fils + UI sur 4 vCPU).

## F2 ffi-callbacks
- v0 `src=invoke` : `slint::invoke_from_event_loop` par événement (`core/api.rs:1286`, `Box` + mpsc + `write(eventfd)` : `winit/event_loop.rs`, calloop ping). 100 k/s : 62,3 fps, CPU 27 % ; 1 M/s : 62,4 fps, CPU 111 %, 99,98 % appliquées ; 5 M/s : le rappel bloque le producteur natif, débit émis 34 % du demandé (`rate_fraction` 0,34), CPU 172 %.
- v1 `src=queue` (défaut) : file SPSC sans verrou drainée une fois par frame, chaque événement appliqué individuellement : 10 M/s : 62,2 fps, CPU 35 %, 100 % appliquées ; 20 M/s : 62,3 fps, CPU 45 %, 100 %, 0 perdue.
- v2 `src=coalesce` : valeur la plus récente par ligne (atomiques + drapeau sale), événements écrasés comptés comme appliqués : 10 M/s : CPU 46 % (pas meilleur que `queue` à ce débit : balayer 100 k drapeaux par frame coûte autant que drainer 160 k événements).

## F3 ffi-block
- Fil de fond : `core_heavy(10 … 1 000)` ne perturbe pas l'UI (62,2 fps, stall 11 ms ; CPU 113 % = 2 threads). Variante `ui` : gel = durée de l'appel (100 ms → stall 104 ms, 1 000 ms → 1 005 ms).

## I1 / I2 latence touche→pixel et molette→pixel
- v0 (cadence native) : p50 17,3 ms (idle), 11,6 (fling), 17,4 (heavy), 12,6 (mutate) ; molette 20,3 ms. Cause : `TimerBasedFrameThrottle::request_throttled_redraw` démarre un `Timer` de 16 ms avant le premier `request_redraw` de winit quand il n'est pas déjà armé (`winit/frame_throttle.rs:47-70`) : tout événement attend 16 ms avant sa frame.
- v1 `direct` (variante, API publique instable `WinitWindowAccessor::with_winit_window(|w| w.request_redraw())` appelée dans le rappel de l'événement) : touche 1,4 ms (idle), 3,6 (fling), 1,4 (heavy), 3,0 (mutate) ; molette 4,2 ms. Non activé par défaut (n'est pas le comportement de Slint ; à décider par le coordinateur ; `SLINTAPP_DIRECT=1`).
