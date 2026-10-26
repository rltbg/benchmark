# Stack Qt 6.12 (Qt Quick / QML + C++) — notes (passe v2, 2026-10-26)

## Versions (vérifiées le 2026-10-26)
| Outil | Version | Source |
|---|---|---|
| Qt (qtbase, qtdeclarative, qtsvg, icu, qttranslations) | **6.12.0** (dernière stable ; LTS) | https://download.qt.io/official_releases/qt/ (dossiers 6.12/6.12.0 = plus récents), `aqt list-qt linux desktop` → 6.12.0 |
| aqtinstall | 3.3.0 (dernière sur PyPI) | https://pypi.org/pypi/aqtinstall/json |
| CMake | **4.4.4** (dernière stable, tarball officiel, sha256 vérifié) | https://cmake.org/files/LatestRelease/ |
| Ninja | 1.13.2 (PyPI `ninja`, dernière) | https://pypi.org/pypi/ninja/json |
| g++ | 13.3.0 (Ubuntu 24.04) — **pas la dernière GCC** : pas d'apt, compilateur système gardé | |
| libxcb-cursor0 | 0.1.4-1build1 (Ubuntu noble, .deb récupéré + sha256 de l'index Packages) | archive.ubuntu.com |
| cœur Rust | `core/target/release/libbenchcore.a`, lien **statique** | dépôt |

Installation : `python3 -m aqt install-qt linux desktop 6.12.0 linux_gcc_64 -O /opt/Qt --archives icu qtbase qtdeclarative qtsvg qttranslations` (1,6 Go ; les archives par défaut + modules optionnels ne sont pas nécessaires ; log `logs/aqt-install.log`). Champ JSON `version` : `Qt 6.12.0 (runtime 6.12.0, aqtinstall linux_gcc_64 from download.qt.io), g++ 13.3.0, CMake 4.4.4, Ninja 1.13.2`.

## Build / lancement
- `./build.sh` : CMake 4.4.4 + Ninja `-j2`, Release, contre `/opt/Qt/6.12.0/gcc_64` (variable `QT_DIR`), puis **déploiement officiel Qt** (`qt_generate_deploy_qml_app_script` + `cmake --install`) dans `deploy/`. Build complet propre : **29,8 s** (0 warning), install 0,6 s. Log : `logs/build.log`.
- Aucun chemin absolu dans le code : `BENCH_ROOT` (env ou -D), sinon dérivé de l'emplacement du script/CMakeLists ; à l'exécution `--assets`, sinon `$BENCH_ROOT/assets`, sinon premier ancêtre du binaire contenant `core/benchcore.h`. Cœur lié statiquement → `BENCHCORE_LIB` sans objet.
- Lignes `harness/stacks.tsv` (je ne modifie pas le fichier, hors de mon dossier) :
  - `qt-software	${ROOT}/stacks/qt/deploy/bin/qtbench	`
  - `qt-opengl	${ROOT}/stacks/qt/deploy/bin/qtbench --backend opengl	`
  (le binaire `build/qtbench` ne démarre pas seul : `libxcb-cursor.so.0` n'est que dans `deploy/lib`.)
- Options hors spec : `--backend software|opengl`, `--canvas-impl painted|render|qml`, `--paced`, `--shot f.png`.

## Revue contre la doc officielle Qt 6.12 (écarts/bugs trouvés → correction)
1. **`QAnimationDriver` n'est pas une API publique** : la classe est marquée `\internal` (source qtbase `src/corelib/animation/qabstractanimation.cpp`, absente de https://doc.qt.io/qt-6/classes.html, page `qanimationdriver.html` inexistante) ; la doc du scene graph ne prévoit un driver custom qu'avec `QQuickRenderControl` (https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html#what-does-advance-animations-refer-to). → **supprimé**. Conséquence documentée : en backend software et en boucle `basic` (RHI/llvmpipe), les animations Qt Quick avancent sur le timer de 16 ms → **nav plafonné à ~60 fps** (pas d'API publique pour le changer).
2. **`QT_QPA_UPDATE_IDLE_TIME=0` est public et documenté** : https://doc.qt.io/qt-6/qwindow.html#requestUpdate (« can be overridden using the QT_QPA_UPDATE_IDLE_TIME environment variable ») et la doc scene graph le recommande « for low-level benchmarking ». → **gardé** (`--paced` = défaut Qt 5 ms : list 149 fps vs ~700–1100).
3. nav : l'ancien code était une pile écrite à la main (les notes JSON prétendaient « StackView/Popup ») → **vrai `StackView`** (`pushItem`/`popCurrentItem`, Qt 6.7+, transitions `pushEnter/pushExit/popEnter/popExit` 300 ms) + **`Popup` modal** (`enter`/`exit` fondu 200 ms, `opened`/`closed`) de QtQuick.Controls.Basic (https://doc.qt.io/qt-6/qml-qtquick-controls-stackview.html, …-popup.html). Plus de tick re-planifié à 1 ms : en nav seules les frames rendues par Qt (scène modifiée par une animation) sont comptées.
4. **ListView, hauteurs variables** : la doc dit que `contentHeight` est estimé et que positionner par `contentY` est « unreliable » (https://doc.qt.io/qt-6/qml-qtquick-listview.html#variable-delegate-size-and-section-labels, #positionViewAtIndex-method). Ancien code : `contentY` absolu borné par l'estimation → ligne affichée ≠ offset exact. Essai `positionViewAtIndex()` + reste : **faux de ~280 px** (délégués réutilisés redimensionnés après coup) → rejeté. Correction : défilement par **deltas exacts** (pas ≤ ½ viewport) + réalignement par `itemAtIndex()` (public) de la ligne exacte ; contrôle `ops.top_row_y_error_px` = 0–0,1 px après 8–20 s (avant : 3 lignes / 290 px de dérive à 20 s).
5. Sous-titre répété 6× (contraire à la spec) → exactement `"Quote " + i + " — " + (fnv(i+7)%100000)/100.0` (formatage le plus court).
6. File bornée : un lot qui ne rentrait que partiellement était entièrement jeté → on accepte ce qui tient, on jette/compte le reste.
7. `setContextProperty` (déconseillé : https://doc.qt.io/qt-6/qtqml-cppintegration-contextproperties.html) → singleton `App` (`QML_SINGLETON` + **`QQmlEngine::setExternalSingletonInstance`**, nouveau en 6.12 : https://doc.qt.io/qt-6/qqmlengine.html#setExternalSingletonInstance).
8. qrc + `qmlRegisterType` → `qt_add_qml_module` + `QML_ELEMENT` (QML compilé par qmlcachegen) + `QQuickView::loadFromModule` (https://doc.qt.io/qt-6/qt-add-qml-module.html, https://doc.qt.io/qt-6/qquickview.html#loadFromModule).
9. Images : `QQuickImageProvider` = un seul thread de chargement par moteur ; la doc conseille `QQuickAsyncImageProvider` + `QThreadPool` (https://doc.qt.io/qt-6/qquickimageprovider.html#asynchronous-image-loading) → fait, décodage par `QImageReader::setScaledSize` dans le pool (chronométré dans le worker).
10. `QSGRenderNode` (impl `render`) en software : la région de clip arrive comme `QRegion` et n'était pas appliquée, ni `inheritedOpacity()` (https://doc.qt.io/qt-6/qsgrendernode.html#render) → appliquées. `QSGRenderNode` + `PainterResource` et `QQuickPaintedItem` sont bien supportés en 6.12 software ; `ShaderEffect` (donc `MultiEffect`) et particules ne le sont pas (https://doc.qt.io/qt-6/qtquick-visualcanvas-adaptations-software.html).
11. **Flou d'ombre** : QPainter n'a pas d'API de flou ; l'API officielle de Qt Quick (`MultiEffect.shadowBlur`) est un shader non rendu par l'adaptation software → méthode gardée : sprite pré-flouté (3 box blur ≈ 8 px) mis en cache par (type, taille), offset (2,2), 1 forme / 10. Vérifié à l'œil. `ops.not_rendered: []`.
12. `QQuickPaintedItem` : cible `Image` gardée (`FramebufferObject` n'accélère qu'en OpenGL depuis 6.9, AA moindre : https://doc.qt.io/qt-6/qquickpainteditem.html#RenderTarget-enum).
13. TextEdit : `textFormat: TextEdit.PlainText` explicite (https://doc.qt.io/qt-6/qml-qtquick-textedit.html#textFormat-prop) ; TextEdit + Flickable = idiome de la doc TextEdit (TextArea n'est pas défilant lui-même).
14. Variante OpenGL : `QSurfaceFormat::setSwapInterval(0)` (public) pour ne pas bloquer sur vsync ; sous llvmpipe Qt choisit la boucle `basic` (doc scene graph).
15. `QML Canvas` (fumée) : `Connections { target: bench }` cassé après passage au singleton (frames vides comptées : 175 000 « fps ») → `target: App`.
16. Spec v2 : marqueurs `BENCH:measure_start/end` (fflush), scénario `input` (Rectangle 200×200 en (100,100), `Keys.onPressed` Key_Space sans auto-repeat, fenêtre en (0,0), `requestActivate`), `version`, `unsupported: null`, `ops.not_rendered`, `ops.key_events` ; pas de `ops.gc` (C++ natif).
17. Dépendance découverte : le plugin xcb exige `libxcb-cursor.so.0` depuis Qt 6.5 (`qt.qpa.plugin: From 6.5.0, xcb-cursor0 or libxcb-cursor0 is needed…`, rc 134) ; absente, pas d'apt → embarquée dans `deploy/lib` (build.sh la récupère et vérifie le sha256).

## Définition de frame
`frame_ms` = intervalle entre deux `QQuickWindow::frameSwapped` (software : émis juste après `QBackingStore::flush` de la région sale vers X/xcb-SHM ; OpenGL : après `swapBuffers` de la boucle basic). Scénarios continus : chaque `frameSwapped` poste un tick (script + `update()`), seules les frames suivant un tick comptent. idle : aucun redessin (0 frame). nav : uniquement les frames rendues par Qt (animations, plafond ~60 Hz, cf. 1).

## Limites / écarts restants
- nav plafonné ~60 fps (timer d'animation 16 ms, non contournable publiquement).
- text : ~33–42 fps en software, coût dans la phase *sync* (~17 ms/frame, `qt.scenegraph.time.renderloop`) dû aux **emoji couleur** (même fichier sans emoji : 195 fps ; ASCII : 398 fps).
- stress : `updates_applied` peut dépasser `generated` d'un lot (mises à jour générées avant t=0 appliquées après).
- Machine partagée très chargée (load 6–8) : chiffres bruités. Deux runs harnais perdus par collision de display `:90` avec un autre agent (`could not connect to display :90`, `X11 connection broke`), relancés OK.
- Compilateur non mis à jour (pas d'apt).

## Fumée harnais (`run_bench.py --duration 8|10|15 --runs 1`, non officiel) — `logs/harness-smoke.txt`
software : idle 0 frame, RSS 57 Mo, CPU 0 % ; list 674 fps (p95 5,6 ms), RSS 61 Mo ; stream 1k 1047 fps / 10k 851 fps, 0 drop ; canvas 5k 14,2 fps, 20k 3,9 fps ; text 33 fps (RSS 83 Mo) ; images 267 fps (1686 décodages, 4990 ms) ; nav 61 fps, 24 transitions/10 s ; core 51 fps, 90,8 M calls/s, heavy 9, stall max 194 ms ; stress(10 s) 13,2 fps ; **input 1108 fps, latence p50 2,2 ms, 40/40 événements, 0 perdu**. Tous : `markers: True`, `sum_heights` 8 799 584.
OpenGL/llvmpipe : list 210 fps (RSS 166 Mo) ; canvas 5k 8,5 ; nav 41 ; text 59 ; input 211 fps, latence p50 13,7 ms, 0 perdu.
Fumées directes (`smoke.sh`, `logs/smoke/`) : canvas `render` (QSGRenderNode) 13,9 fps vs `painted` 11–14 ; Canvas QML 1000 formes 2,9 fps ; `--paced` list 149 fps.
Vérification visuelle (`results/verification/qt-software-*.png` + captures `--shot`) : lignes/pastilles/prix, dégradés, ombres, clip arrondi, graphe, CJK/arabe/hébreu/emoji, sélection totale à t=5 s, vignettes zoomées, StackView + Popup, carré noir/blanc : conformes.

## Déploiement (mesuré)
- `deploy/` complet (script officiel Qt) : **101,1 Mo** (97 Mio) : bin 6,4 Mo, lib 86 Mo, plugins 4,6 Mo (wayland, eglfs, tls… inutiles ici), qml 0,26 Mo.
- **Réellement nécessaire** (union des fichiers mappés dans `/proc/<pid>/maps` sur tous les scénarios software + OpenGL, + fichiers `qmldir`, testé seul dans un dossier minimal : nav software et canvas OpenGL OK) : **93,3 Mo, 35 fichiers** = binaire 6,40 Mo (QML compilé + cœur Rust inclus) + 15 libs Qt 6.12 47,6 Mo (Core, Gui, Qml, QmlMeta, QmlModels, QmlWorkerScript, Quick, QuickTemplates2, QuickControls2(+Impl, Basic, BasicStyleImpl), XcbQpa, OpenGL, Network, DBus) + **ICU 73 39,1 Mo** + libxcb-cursor 24 Ko + plugins 127 Ko (`platforms/libqxcb`, `xcbglintegrations/libqxcb-glx-integration` (OpenGL seulement), `platforminputcontexts/compose`) + QML 44 Ko (`QtQuick/Templates`, `QtQuick/Controls/Basic` plugins + qmldir). Liste : `logs/deploy-used-files.txt`.
- `ldd deploy/bin/qtbench` : 59 entrées, 13 dans `deploy/`, reste système (libstdc++, GL/EGL/GLX, X11/xcb, xkbcommon, freetype, fontconfig, glib, dbus, krb5/gssapi via QtNetwork…) : `logs/ldd-qtbench.txt`.

## Code
C++ 718 lignes (main.cpp, bench.h, model.h, canvas.h, common.h) + QML 299 lignes (9 fichiers) + CMake 44 + build.sh 29.
