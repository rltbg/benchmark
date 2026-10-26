# Journal d'optimisation — Qt Quick 6.12.0 (backend `software`), bancs v3

Fumées uniquement (6–8 s, `smoke.sh`, machine partagée avec d'autres compilations : chiffres bruités, **pas** des mesures officielles). Sources : `QD` = qtdeclarative v6.12.0 (`4332fbb`, `/opt/src/qtdeclarative`), `QB` = qtbase v6.12.0 (`025bdad`, `/opt/src/qtbase`), analyse `docs/deep/qt.md`. Les sondes brutes sont dans `logs/smoke/*.json`.

## Principes appliqués à tous les scénarios
| Choix | Justification |
|---|---|
| Backend `software` (`QT_QUICK_BACKEND=software` posé par `run.sh`; `QQuickWindow::setSceneGraphBackend("software")` par défaut dans `main.cpp`) | doc Qt Quick Software Adaptation (https://doc.qt.io/qt-6/qtquick-visualcanvas-adaptations-software.html) |
| `ListView` : `reuseItems: true`, `cacheBuffer: 600` **posés explicitement** (défauts Qt : `reuseItems=false`, `cacheBuffer=320 px`, `QD:qquickitemview.cpp:17-19`) ; variantes `reuse=0`, `cache=N` | `performance.qdoc` (doc Qt « Performance considerations and suggestions », section listes) |
| `Text.textFormat: PlainText` partout (évite `Qt::mightBeRichText` à chaque changement, `QD:qquicktext.cpp:1846`) | doc Text.textFormat |
| Modèle `QAbstractListModel` C++, `rowCount()` O(1), données calculées à la demande (jamais `ListModel`/JS) | `performance.qdoc` « ListModel … pas adapté aux grandes quantités » |
| Cadence 60 Hz : `QTimer::singleShot(chrono::nanoseconds, Qt::PreciseTimer)` sur une grille de 16,667 ms + `QT_QPA_UPDATE_IDLE_TIME=0` | doc `QWindow::requestUpdate`, doc scene graph (benchmarking) ; `QB:qplatformwindow.cpp:786`. Le tick (script + `update()`) n'existe que dans les scénarios scriptés ; ui-anim, gfx-layers, ui-tree, text-edit, idle laissent Qt (timer d'animation 16 ms, redimensionnement, frappe) produire les frames |
| Défilement scripté exact : `contentY` par deltas ≤ ½ viewport, sinon `positionViewAtIndex(row)` + reste, puis réalignement par `itemAtIndex()` (index des hauteurs par blocs de 1024 lignes, 80 Ko pour 10 M) | doc `ListView` « variable delegate size » (contentHeight estimé), `QD:qquicklistview.cpp:164,506-531,743-757` |
| Aucune ombre/flou émulé : `ops.not_rendered` | consigne coordinateur ; `adaptations.qdoc` |

## L1 `list-scale`
| Version | Changement | Résultat (fumée) |
|---|---|---|
| v0 (passe v2) | `contentY` absolu / deltas ≤ ½ viewport, `Text` sans `textFormat` | v2 : dérive de 290 px à 20 s |
| v1 | deltas exacts + réalignement `itemAtIndex` | erreur de la ligne du haut ≤ 0,5 px après 8 s |
| v2 | saut > ½ viewport (flings 150 000 px/s = 2 500 px/frame) par `positionViewAtIndex` + reste au lieu de 8 pas | 60,0 fps, p99 20–24 ms, 330 sauts + 14–30 réalignements ; N = 100 k / 1 M / 10 M : 60,0 fps partout, `setup_ms` 45–77 ms |
| essai `step=0.9` | seuil de saut 0,9 viewport | pas de gain (60 fps, p99 22 ms) ; gardé 0,5 |
| **piège float32** | départ en profondeur (`from=0.9`) : à 1 M lignes le rendu est irrégulier (pas de 8 px), à 10 M **corrompu** (lignes superposées/trous, quantification 64 px) — captures lues. Le défilement du banc ne dépasse pas ~190 000 px depuis le départ, donc à `from=0` le rendu est exact quel que soit N. Source : matrices `QMatrix4x4` float (`QB:qmatrix4x4.h:185`, `QD:qquickwindow.cpp:2231`) |

## L2 `list-rich`
Délégué dense, 8 items de base + 2 items (Rectangle+Text) par puce, `Repeater` dans le délégué. Fumée : K=6 : 46–50 fps (p99 41 ms), K=24 : 10,5 fps, K=96 : 2 fps. `perf record -e cpu-clock` : le coût est dans `QRegion::intersects/subtracted` + `memmove/realloc` de QtGui = gestion des régions sales/opaques du rendu logiciel par nœud (`QD:scenegraph/adaptations/software/qsgabstractsoftwarerenderer.cpp:140-160`), qui croît avec le nombre de nœuds par frame. Pas de levier applicatif sans réduire le contenu (interdit) : `reuseItems` déjà actif, `step` sans effet.

## L3 `list-mutate` / F2 `ffi-callbacks` (hand-off natif → UI, mesuré)
Producteur : fil C++ (L3) ou fil natif du cœur (F2) ; `UpdateSink` à trois stratégies (`handoff=`) ; application par lot à chaque tick ; `group=1` (défaut) = `dataChanged(..., {PriceRole})` par plage contiguë de lignes sales, `group=0` = un `dataChanged` par événement.
| Variante (F2, 100 k lignes) | 100 k/s | 1 M/s | 10 M/s |
|---|---|---|---|
| `handoff=ring` (SPSC lock-free vidé par frame) | 60,0 fps, p99 19,6, 0 drop | 60,0, p99 19,9, 0 drop | 59,8, p99 21,8, 0 drop |
| `handoff=batch` (vecteur côté producteur, 1 `invokeMethod` / ms) | 60,0 | 59,8, p99 22,8 | – |
| `handoff=invoke` (1 `invokeMethod(QueuedConnection)` par événement) | 60,0 | **effondrement : 2 frames, arrêt 4,5 s, 1,4 M événements jetés** | – |
| `ring`, `group=0` | 60,0 | 59,5, p99 24,6 | – |
Décision : `ring` par défaut (documenté : un `QMetaCallEvent` alloué + posté par événement sature la file d'événements du thread UI, `QD`/`QB:qcoreapplication.cpp` postEvent). L3 : N=100 k/s 60 fps (grouped ; ungrouped 59,6 + une frame de 62 ms), N=1 M/s : 47,9 fps. Source de l'idiome « regrouper » : `performance.qdoc` « batch up backend operations » ; coût de `dataChanged` O(\|cache\|) : `QMLM:qqmldmabstractitemmodeldata_p.h:106-150` (analyse `qt.md` §6).

## L4 `list-reset`
`beginResetModel/endResetModel` sur modèle implicite (O(1)) ; l'index des hauteurs de la génération suivante est construit sur un fil pendant l'attente. 100 k lignes : reset 0,8–1,8 ms, reconstruction (reset → frame) 3,4–5 ms ; 1 M lignes : 60,0 fps, 1 frame > 25 ms. Le reset ramène la vue au début (`QD:qquickitemview.cpp:1370-1388`).

## T1 `text-big`
| Version | Chargement | N=1 | N=10 | N=50 | N=100 |
|---|---|---|---|---|---|
| v0 `load=sync` | lecture + décodage + `setText` + mise en page sur le thread UI (avant la boucle) | 0,53 s | 4,8 s | – | – |
| v1 `load=async` (défaut) | lecture + `QString::fromUtf8` sur un fil, `setText` + `contentHeight` sur l'UI | 0,49 s / 24 fps | 5,0 s / 17 fps (RSS 253 Mo de tas) | **28 s** / 7 fps (tas 1 Go) | **33 s** / 4,5 fps (tas 1,16 Go) |
| v2 `load=chunked` | 256 Ko par tour de boucle via `QTextCursor::insertText` | – | **pas fini après > 200 s** (mise en page complète à chaque ajout : `QQuickTextEdit::updateSize()` → `idealWidth()`, `QD:qquicktextedit.cpp:3019-3070`) → abandonné | | |
`setUndoRedoEnabled(false)` (lecture seule). Le gel de la mise en page est réel et inévitable avec `TextEdit` : `ui_stall_ms_max_incl_warmup` = 5,3 s à 10 Mo. `setDocumentLayout` personnalisé non essayé (hors budget).

## T2 / U2 / U3 (items par étiquette)
T2 (`Repeater` + `Text`, modèle `dataChanged` global par frame) : N=1000 : 21 fps ; N=20 000 : 1,5 fps. U2 (N `QObject` à propriété `QProperty` bindable, `text: cell.value`) : N=1000 : 25 fps ; 100 k : 0,5 fps. U3 (N `Rectangle` avec `NumberAnimation on x` + `SequentialAnimation on opacity`) : N=1000 : 31 fps ; 20 k : 2,9 fps. Pas d'optimisation conforme trouvée (le coût est la mise à jour/rendu par item, cf. L2).

## T3 / T4
T3 : `TextEdit` éditable dans un `Flickable`, `textChanged` → carré ; 22 appuis → `key_events=22`, curseur au milieu du document de 1 Mo, 1ʳᵉ frame après ~0,6 s. T4 (`QTextCursor::insertText` + `QTextCharFormat`, pas de HTML) : 5 000 spans 14 fps, 100 000 spans : mise en page 24 s, 4,5 fps.

## G1 `gfx-shapes` (deux idiomes étiquetés)
| Idiome (`impl=`) | N | fps (fumée) |
|---|---|---|
| `painted` (QQuickPaintedItem, défaut) solid | 1 000 | 59,5 (p99 29) ; N=20 000 : 5,9 |
| `painted` gradient | 1 000 | 44,4 |
| `painted` shadow / clip / all | 1 000 | 57,5 / 55,8 / 45,3 (shadow = **non rendu**, voir `not_rendered`) |
| `render` (QSGRenderNode → QPainter du renderer) solid | 1 000 | 59,9 (p99 23) |
| `items` (1 `Rectangle` par forme, déplacés depuis C++) solid | 200 / 1 000 | 29,8 / 5,6 |
Gradient 1 forme = `QLinearGradient` ObjectBoundingMode ; clip rond = `QPainter::setClipPath` (réel) ; idiome `items` : `Item.clip` rectangulaire seulement → `not_rendered` ; ombre : jamais émulée (sprite pré-flouté de la passe v2 **supprimé**). Cible `Image` de `QQuickPaintedItem` (`FramebufferObject` inutile en software).

## G2–G5
G2 (N=100) 17,6 fps (trait plein, chemins reconstruits ou en cache : `rebuild=1` 17,1 ; `feature=dashed` 10,9) ; N=1000 : 1,8. G3 : 10 k → 59,4 ; 100 k → 21 ; 1 M → 13,9. Anneau de points à x fixe (aucun recalcul des points existants) tracé en 2 `drawPolyline` + 1 segment de jonction. G4 (N=100) 50,5 fps, N=1000 : 7 ; `big=4096` 58,7, `big=8192` 48,2 (décodage 600 PNG sur `QThreadPool`, 1,56 s cumulés). G5 : `layer.enabled` + `opacity: 0.5` donne un vrai groupe en software (capture lue), N=100 : 60,8 fps ; `effect=blur` (MultiEffect) : **les groupes disparaissent** (capture lue) → `not_rendered`.

## U1 `ui-tree`
`GridLayout` 5 colonnes (défaut) vs `tree=pos` (liaisons de position simples). 1 k : setup 32 ms ; 10 k : 4,8 s (grille) / 4,2 s (pos) ; 100 k : 17 s, tas 1,47 Go ; **500 k : pas de 1ʳᵉ frame après plusieurs minutes, > 6,5 Go de RSS, arrêté à la main (rc 144)** ≈ 15 Ko/nœud. Redimensionnement (xdotool, 20 allers-retours) : 1 k : médiane 99 ms/frame ; 10 k : **9,3 s** (grille) vs **3,5 s** (pos). La croissance super-linéaire vient du rendu logiciel (régions par nœud, `perf` : 40 % `clear_page_erms` + `QRegion::*`). QML interdit l'auto-instanciation statique d'un type : récursion par `Component` partagé (`App.nodeComp`).

## F1 / F3 / I / M
F1 : 1 thread puis 4 (phases de 3 s en fumée) : `core_noop`/`core_add` ≈ 3,4 M/s → 11 M/s, `core_hash` 1 Ko ≈ 285 → 930 Mo/s, `core_fill` idem ; UI 1 000 formes 49,8 fps. F3 : `core_heavy(100)` sur fil de fond : 60 fps, arrêt UI max 10 ms ; `where=ui` : arrêt 108 ms, p99 117 ms. I1/I2 : charges `idle|fling|cpuhog|heavy|mutate` sans anomalie (60 fps sous fling). M1 soak (6 s) 52,8 fps, 99,7 % appliquées ; M2 mem-min 19 fps (T1 N=1 défilant à 30 000 px/s coûteux).
