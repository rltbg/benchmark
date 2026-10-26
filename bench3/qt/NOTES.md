# Qt Quick / QML + C++ — notes (bancs v3, 2026-10-26)

## Versions (vérifiées le 2026-10-26)
Qt **6.12.0** (dernière stable : `download.qt.io/official_releases/qt/6.12/` ne contient que 6.12.0 ; aqtinstall linux_gcc_64, `/opt/Qt/6.12.0/gcc_64`), CMake **4.4.4** (cmake.org LatestRelease), Ninja **1.13.2** (PyPI), g++ 13.3.0 (système, pas d'apt), cœur Rust 1.99.1 (`core/target/release/libbenchcore.a`, lien statique, C-ABI v2 `core/benchcore.h`), libxcb-cursor0 0.1.4 (embarquée, requise par le plugin xcb). Champ JSON `version` : `Qt 6.12.0 (runtime 6.12.0, …), g++ 13.3.0, CMake 4.4.4, Ninja 1.13.2`. Sources lues : qtdeclarative `4332fbb` (v6.12.0), qtbase `025bdad` (`/opt/src`).

## Commande, build, variantes
- Build : `bench3/qt/build.sh` (CMake+Ninja `-j2`, Release, déploiement officiel Qt dans `deploy/`, ≈ 1 min). `harness/stacks3.tsv` : `qt ${ROOT}/bench3/qt/run.sh` ; `qt-opengl … run.sh --backend opengl` (extra, hors périmètre « software uniquement »).
- `run.sh` : `BENCH_ROOT`, `BENCH_PACING=60`, `QT_QPA_PLATFORM=xcb`, `QT_QPA_UPDATE_IDLE_TIME=0`, `QT_QUICK_BACKEND=software` (sauf `--backend opengl`) puis `exec deploy/bin/qtbench "$@"`. Aucun `QT_LOGGING_RULES` (le diagnostic `qt.scenegraph.time.renderloop`/`qt.scenegraph.softwarecontext.renderer` est à proscrire pendant les mesures).
- CLI : `--scenario ID --n N --variant k=v … --duration S --out F --assets DIR [--backend software|opengl] [--paced] [--shot f.png]`. Un binaire, tous les scénarios. `_exit(0)` après l'écriture du JSON (pas de destruction de 10⁵–10⁶ items).

## Noms de `variant` acceptés (pour le harnais)
| Scénario | Variantes |
|---|---|
| list-scale | `from=<0..0.99>` (départ en profondeur, test float32), `reuse=0\|1` (défaut 1), `cache=<px>` (600), `step=<0..1>` (0,5) |
| list-rich | `N` = `K` ∈ {6,12,24,48,96} ; mêmes `reuse/cache/step` |
| list-mutate | `handoff=ring\|invoke\|batch` (défaut ring), `group=1\|0` (défaut 1) |
| list-reset | – |
| text-big | `load=async\|sync\|chunked` (défaut async ; chunked non viable) |
| text-labels, text-edit, text-rich | – (N = étiquettes / car·s⁻¹ côté harnais / spans) |
| gfx-shapes | `feature=solid\|gradient\|shadow\|clip\|all` (solid), `impl=painted\|render\|items` (painted) |
| gfx-paths | `feature=dashed`, `rebuild=1` |
| gfx-chart | – ; gfx-images : `big=4096\|8192` ; gfx-layers : `effect=blur` |
| ui-tree (= startup-scale) | `tree=pos` (liaisons au lieu de `GridLayout`) |
| ui-bind, ui-anim | – |
| ffi-calls | `fn=all\|noop\|add\|hash\|fill` (all), `threads=1\|4` (défaut : 1 puis 4, mi-mesure) ; N = taille du tampon |
| ffi-callbacks | `handoff=ring\|invoke\|batch`, `group=1\|0` |
| ffi-block | `where=bg\|ui` (alias `--variant ui`) ; N = ms |
| latency-load / latency-wheel | `load=idle\|fling\|cpuhog\|heavy\|mutate` (cpuhog = externe, app idle) ; wheel : `ListView.interactive` |
| soak, mem-min | – |

## Scénario × état
| Scénario | État | Remarques |
|---|---|---|
| L1 list-scale | implémenté | 100 k–10 M testés (60 fps, setup ≤ 77 ms) ; rendu corrompu > 1 M lignes si défilement profond (float32), non atteint par le script |
| L2 list-rich | implémenté | K=6 : 46–50 fps, K ≥ 24 < 15 fps (rendu logiciel) |
| L3 list-mutate | implémenté | ring vs invoke vs batch ; insert/remove/move ≥ 1ʳᵉ ligne visible ; file bornée 1 M |
| L4 list-reset | implémenté | reset 1–2 ms (100 k), 1 M OK |
| T1 text-big | implémenté (partiel : gel de mise en page) | 1 Mo 0,5 s ; 10 Mo 5 s ; 50 Mo 28 s ; 100 Mo 33 s ; `selectAll` à t = 5 s OK |
| T2 text-labels | implémenté | grille couvrant la fenêtre, police ≤ 14 px et ≥ 3 px selon N (illisible à grand N) |
| T3 text-edit | implémenté | carré commuté par `textChanged` ; testé xdotool 22/22 |
| T4 text-rich | implémenté | API `QTextCursor`, polices DejaVu/Noto, CJK/emoji/arabe/hébreu (capture lue) |
| G1 gfx-shapes | implémenté ; `shadow` **non rendu** (`not_rendered`), clip rond réel (painted/render) ou non rendu (items) | 3 idiomes |
| G2 gfx-paths | implémenté | |
| G3 gfx-chart | implémenté | 1 000 points/frame, polyligne de N points entière |
| G4 gfx-images | implémenté | images 256² décodées une fois (`QThreadPool`), `big` 4096/8192 |
| G5 gfx-layers | partiel : groupes OK (`layer.enabled`), `blur` **non rendu** | capture lue |
| U1 ui-tree | implémenté | 500 k : pas de 1ʳᵉ frame, > 6,5 Go (arrêté) |
| U2 ui-bind | implémenté | `QProperty` bindable + liaison QML |
| U3 ui-anim | implémenté | animations QML natives (2 par item) |
| F1 ffi-calls | implémenté | |
| F2 ffi-callbacks | implémenté | |
| F3 ffi-block | implémenté | |
| I1/I2 | implémenté (côté app) | le harnais fournit touches/molette/charges externes |
| M1 soak, M2 mem-min, M3 startup-scale | implémentés côté app ; la mesure (PSS, cgroup, froid/chaud) est du ressort du harnais | |
| Carré de latence | partout sauf idle | `Rectangle` 200×200 en (100,100), z = 1000, `Keys.onPressed` Espace ; T3 : sur `textChanged` |

## Écarts à la spec / limites / pièges
- **Frames** : `frame_ms` = intervalle entre `QQuickWindow::frameSwapped` (software : après `QBackingStore::flush` xcb/SHM). Scénarios scriptés : seules les frames suivant un tick comptent ; ui-anim, gfx-layers, ui-tree, text-edit : toutes les frames Qt (`ops.frame_driver`: `qt`, `ops.pacing`: `60-native`). **ui-tree** : statique, donc `frame_ms` ne contient que des frames post-redimensionnement ; utiliser `ops.resize_frame_ms[]` (durée redimensionnement → fin de frame) ; text-edit : frames de frappe + clignotement du curseur.
- `ops.ui_stall_ms_max` = plus long écart entre deux tops d'un `QTimer` 4 ms précis, fenêtre de mesure seulement ; `ops.ui_stall_ms_max_incl_warmup` inclut le chargement (T1 : 5,3 s à 10 Mo). `ops.setup_ms` = initialisation statique du processus → 1ʳᵉ frame après création des données (inclut la mise en page de T1/T4 et le rendu initial de U1). `ops.alloc_bytes` = null (pas de compteur public peu coûteux) ; `heap_inuse_bytes_end` (mallinfo2) fourni.
- `ops.checksum_data` = XOR de `fnv(i)` pour i < N (lignes, formes, étiquettes, nœuds, chemins, images), hex 8 ; identique entre stacks si elles adoptent la même définition (non fixée par la spec : à aligner). T1/T3 : sans `checksum_data` (`text_bytes` fourni).
- `ops.sum_heights` = somme exacte calculée côté modèle (`HeightIndex`), jamais lue sur la vue (`contentHeight` est estimé). Contrôle : `top_row_y_error_px` ≤ 0,5 px.
- Hauteur dense (`hd`) uniquement pour L2 ; L3/L4/F2/F3/I = hauteurs standard.
- L2 : puces réparties sur 600–1250 px (pas ≤ 44 px, largeur step−2) ; sous-titre 9 px sur 2 lignes avec ellipse. Contenu de chaque puce = 2 caractères base 36.
- T2 : N étiquettes dans une grille couvrant la fenêtre (illisibles au-delà de ~500).
- L3 : prix par position courante (`index % rowCount`), insertions avec nouvel id ≥ N ; `updates_applied` peut dépasser `generated` de ≈ 0,3 % (événements générés avant t = 0, appliqués après).
- F2 : `applied` = événements consommés puis appliqués à l'état du modèle ; plusieurs événements sur la même ligne dans une frame sont **coalescés** (dernière valeur gagnante, un `dataChanged` par plage).
- Pas de `ops.gc` (C++). API instables/internes : aucune ; `Q_PROPERTY BINDABLE`, `QQuickPaintedItem`, `QSGRenderNode` (`PainterResource`) sont publics.
- Variante OpenGL (llvmpipe) : non revalidée en v3 (hors périmètre « software uniquement ») ; `impl=render` ne fonctionne qu'en software.
- Machine partagée et chargée : toutes les valeurs sont des fumées.
- Erreurs rencontrées : `Meta object features not supported for nested classes` (moc, struct imbriquée dans une classe `Q_OBJECT`) ; `Parse error at "signals"` (membre nommé `signals`) ; `TreeNode is instantiated recursively` / `Cyclic dependency between TreeNode.qml and TreeAny.qml` (QML interdit l'auto-instanciation statique → `Component` partagé) ; segfault `impl=items` si `shapesReady()` arrive avant `registerItem` (corrigé) ; `QQmlExpression … depends on non-bindable properties: Bench::nodeComp` (propriété sans NOTIFY, corrigé).

## Vérification (captures lues, `results/verification/qt-*.png`)
list-scale, list-rich (puces, barre, bascule), list-mutate, list-reset, text-big, text-labels, text-edit, text-rich (CJK/emoji/arabe/hébreu), gfx-shapes (all), gfx-paths, gfx-chart, gfx-images, gfx-layers (+ blur : vide), ui-tree, ui-bind, ui-anim, ffi-calls, soak, mem-min : conformes, carré noir présent. `harness/run_bench.py` : `markers: True` (list-scale 60,0 fps ; ui-anim 22 fps avec 500 items), `sum_heights` 8 799 584 à 100 k lignes.

## Fumées brutes (6–8 s, non officielles) : voir `OPTIMIZATION-LOG.md`
