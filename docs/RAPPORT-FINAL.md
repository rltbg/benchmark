# Banc comparatif de stacks UI natives multiplateformes — rapport final (passe 2)

Mesures du 2026-10-26 sur une VM Linux x86_64 (4 vCPU, 16 Go, **sans GPU**), rendu logiciel sous Xvfb 1280×720. Tout ce qui est donné comme mesuré a été exécuté ; les JSON et journaux bruts sont dans `results/`. Les résultats du premier passage (versions en partie obsolètes, code non relu) sont archivés dans `results/passe1/` et ne sont plus utilisés.

## 1. Résumé

- **Ce qui est mesuré** : le coût CPU, RAM, latence et démarrage de 8 stacks pour une app de référence identique (liste virtualisée de 100 000 lignes à hauteur variable, flux de 10 000 mises à jour/s, canvas de 5 000 à 20 000 formes, texte de 1 Mo, 600 images, navigation, appels C-ABI, saisie clavier, stress de 10 minutes), **à 60 Hz pour toutes les stacks**, plus une dégradation CPU et mémoire. **Ce n'est pas** une mesure sur GPU, iOS, Android ou wasm.
- **À 60 Hz en défilement**, Slint, Qt, Avalonia (AOT) et NSH tiennent la cadence avec 11 à 17 % d'un cœur. Compose en consomme 57 %, egui 92 %, et Flutter 140 % sans tenir 60 FPS (Impeller émulé sur CPU).
- **Mémoire** : Slint 24 Mo de PSS au repos, Avalonia AOT 43, Qt 49, NSH 88, egui 123, Compose 163, Flutter 212. Sous une limite cgroup de 128 ou 96 Mo, **Compose et Flutter sont tués (OOM)** ; les six autres tiennent leurs 60 FPS.
- **Démarrage à froid** (premier pixel) : Slint 99 ms, Avalonia AOT 131, Qt 185, NSH 189, egui 239, Flutter 469, Avalonia JIT 962, Compose 1 711.
- **Stress de 10 minutes** : mémoire stable pour Slint, Qt, egui et NSH ; +51 Mo pour Avalonia AOT ; +84 Mo pour Compose avec 287 GC et 2,1 s de pause cumulée.
- **Latence clavier → pixel** (médiane, mesurée de l'extérieur) : Qt 1,2 ms, egui 9,1, Slint 12,4, NSH 14,9, Compose 22,1, Avalonia 27,4, Flutter 31,8.
- **Portes F\*** : F\* → F# → NativeAOT est **utilisable sans contournement** avec F\* 2026 (nouveau backend F#) ; **Low\* n'existe plus dans F\* 2026**, la voie C passe désormais par **Pulse → KaRaMeL → C** (utilisable, débit 0,5 à 0,6× du Rust écrit à la main).
- **Classement conditionnel** (§7) : 1. Slint, 2. Qt, 3. Avalonia AOT, puis NSH, Compose, Flutter ; egui comme repère (pas d'iOS).

## 2. Ce qui a changé depuis le premier passage

1. **Versions à jour** (vérifiées le 2026-10-26 dans les registres officiels) : Qt 6.4.2 → **6.12.0**, .NET SDK 10.0.113 → **10.0.403**, Rust 1.97 → **1.99.1**, JDK 21 → **25** (FFM final), Gradle 8.14 → **9.8.2**, F\* 2025.03.25 → **2026.10.25**, NSH (HEAD non publié) → **0.2.0 de crates.io**. Déjà à jour : Slint 1.18.1, egui 0.37.0, Avalonia 12.1.4, Compose Multiplatform 1.12.2, Flutter 3.47.8, KaRaMeL 98ffca5.
2. **Revue de chaque implémentation contre la documentation officielle** : chaque stack avait au moins un défaut qui faussait le premier passage, par exemple une liste Qt qui dérivait de 290 px, un GC gen2 par frame dans Avalonia, des frames comptées deux fois dans egui, des pertes du flux impossibles à compter dans Compose, des prix faux dans NSH, une sélection jamais peinte dans Flutter. Le détail, avec les URL de documentation, est dans `stacks/*/STACK-NOTES.md` (section « Revue »).
3. **Plus de contournement par API privée** (Avalonia et Qt en utilisaient).
4. **Cadence commune à 60 Hz** : sans elle, une stack plafonnée à 60 Hz et une stack qui redessine sans arrêt n'ont pas des CPU % comparables. La marge sans plafond est mesurée en variante.
5. **Harnais v2** : CPU, RSS et PSS mesurés entre des marqueurs émis par l'app, descendants comptés, percentiles corrigés, latence d'entrée externe, dégradation par cgroup, preuve que chaque app est une vraie fenêtre.

## 3. Périmètre et versions

| Stack | Version et rendu mesurés | Langage(s) |
|---|---|---|
| Slint | 1.18.1, winit 0.30.13, **skia-software** (le renderer logiciel natif ne dessine ni ombres ni formes arrondies à dégradé) | Rust + DSL `.slint` |
| Qt | 6.12.0 (aqtinstall), Qt Quick backend software | C++ + QML |
| Avalonia | 12.1.4 + AvaloniaEdit 12.0.0, Skia logiciel, NativeAOT et JIT (.NET 10.0.403) | F# |
| Compose Multiplatform | 1.12.2, Kotlin 2.4.21, Skiko 0.150.2 SOFTWARE_FAST, JDK 25 (jlink) | Kotlin |
| Flutter | 3.47.8, Dart 3.13.7, Impeller (OpenGL ES émulé par llvmpipe) ; variante Skia documentée | Dart |
| egui | 0.37.0, renderer par défaut wgpu (Vulkan émulé par lavapipe) ; variante glow | Rust |
| NSH | 0.2.0 (crates.io), Rust seul, backend GTK 4 (GSK cairo) | Rust |

Exclus : iced (mobile hors périmètre pour ses auteurs), toute version web/TypeScript (variantes TSX et GPUI de NSH, Tauri, Dioxus WebView).

## 4. Méthode

- Spécification : `docs/BENCH-SPEC.md` (v2.1). Données déterministes ; chaque app prouve qu'elle travaille sur le même contenu (somme des hauteurs = 8 799 584).
- Harnais `harness/run_bench.py` : Xvfb neuf par exécution ; CPU, RSS et PSS (`/proc`, 10 Hz) entre `BENCH:measure_start` et `BENCH:measure_end` ; une seule stack à la fois sur la machine.
- **Cadence** : `BENCH_PACING=60` pour toutes les stacks (cadence native ou API publique ; Slint = 62,5 Hz, sa cadence native).
- **Frames manquées** : frames de plus de 25 ms, c'est-à-dire une échéance de 60 Hz manquée d'au moins une demi-frame. Le seuil de 16,7 ms est inutilisable à 60 Hz, car il compte la gigue des timers.
- **Latence d'entrée** : le harnais donne le focus à la fenêtre, envoie Espace par XTest et mesure le délai jusqu'au changement des pixels d'un carré dans le framebuffer (40 appuis par exécution).
- **Plan exécuté** :
  - phase A en 10 exécutions (idle, list, stream, canvas, core, input) ;
  - phase B en 5 exécutions ;
  - dégradation en 3 exécutions : 1 cœur, ½ cœur par quota CFS, mémoire à 128 et 96 Mo par cgroup ;
  - démarrage en 10 exécutions à froid (caches vidés) et 10 à chaud ;
  - calibration externe ; preuves GUI (72/72) ; stress de 600 s en 1 exécution.
- **Incident** : le conteneur a été arrêté de 04:52 à 06:52 pendant la phase A (session jugée inactive) ; les mesures ont repris au scénario interrompu, et aucune donnée partielle n'a été conservée.
- **Pression mémoire** : les limites de 512 et 256 Mo prévues ne contraignaient aucune stack (pic facturé ≤ 181 Mo, les bibliothèques partagées déjà en cache n'étant pas facturées au cgroup). Elles ont été remplacées par 128 et 96 Mo.

## 5. Résultats

#### 1. Mesure principale à 60 Hz (phase A : 10 exécutions, médianes)

| Stack | PSS idle (Mo) | CPU idle (% cœur) | list : FPS / frames manquées (> 25 ms) / CPU % | stream 10k/s : FPS / manquées / CPU % | canvas 5000 : FPS | core : M triplets/s (FPS UI) | input : latence p50 / p95 (ms) |
|---|---|---|---|---|---|---|---|
| slint | 24 | 0.00 | 62 / 0.0 % / 12 | 62 / 0.1 % / 15 | 10.8 | 82 (58) | 12.4 / 18.2 |
| qt-software | 49 | 0.00 | 60 / 0.1 % / 11 | 60 / 0.1 % / 15 | 13.3 | 116 (46) | 1.2 / 3.0 |
| avalonia-aot | 43 | 0.00 | 61 / 0.2 % / 14 | 61 / 0.2 % / 17 | 10.6 | 111 (54) | 27.4 / 34.4 |
| avalonia-jit | 77 | 0.43 | 61 / 0.1 % / 17 | 61 / 0.2 % / 19 | 10.4 | 86 (52) | 27.8 / 34.9 |
| egui | 123 | 0.00 | 60 / 0.2 % / 92 | 60 / 0.1 % / 93 | 8.1 | 79 (25) | 9.1 / 15.7 |
| nsh | 88 | 0.00 | 60 / 0.1 % / 17 | 60 / 0.3 % / 20 | 0.2 | 94 (55) | 14.9 / 21.9 |
| compose-jvm | 163 | 0.15 | 60 / 0.1 % / 57 | 60 / 0.2 % / 76 | 3.5 | 52 (21) | 22.1 / 31.7 |
| flutter | 212 | 0.00 | 55 / 6.1 % / 140 | 56 / 4.4 % / 177 | 12.1 | 47 (36) | 31.8 / 46.5 |

#### 2. Démarrage (premier pixel, 10 exécutions)

| Stack | froid (caches vidés) : médiane / p95 (ms) | chaud : médiane / p95 (ms) |
|---|---|---|
| slint | 99 / 125 | 56 / 82 |
| qt-software | 185 / 243 | 71 / 77 |
| avalonia-aot | 131 / 156 | 74 / 78 |
| avalonia-jit | 962 / 1137 | 788 / 886 |
| egui | 239 / 269 | 126 / 152 |
| nsh | 189 / 228 | 107 / 128 |
| compose-jvm | 1711 / 1852 | 1394 / 1491 |
| flutter | 469 / 928 | 301 / 327 |

#### 3. Autres scénarios (phase B : 5 exécutions, FPS médian)

| Stack | stream 1k/s | canvas 20 000 | text 1 Mo (FPS ; 1er affichage ms) | images 600 | nav |
|---|---|---|---|---|---|
| slint | 62 | 2.3 | 62 ; 2124 | 62 | 58 |
| qt-software | 60 | 3.7 | 23 ; 549 | 60 | 62 |
| avalonia-aot | 61 | 2.7 | 59 ; 156 | 61 | 61 |
| avalonia-jit | 61 | 2.8 | 58 ; 764 | 61 | 61 |
| egui | 60 | 2.2 | 33 ; 904 | 60 | 60 |
| nsh | 60 | **0 frame** | 60 ; 160 | 60 | 54 |
| compose-jvm | 60 | 0.8 | 59 ; 1554 | 28 | 59 |
| flutter | 58 | **0 frame** | 32 ; 202 | 51 | 29 |

#### 4. Dégradation (3 exécutions, FPS médian / frames manquées > 25 ms)

| Stack | list 1 cœur | stream 10k 1 cœur | canvas 1 cœur | list ½ cœur | canvas ½ cœur |
|---|---|---|---|---|---|
| slint | 62 / 0 % | 62 / 0 % | 8.1 / 100 % | 62 / 0 % | 4.1 / 100 % |
| qt-software | 60 / 0 % | 60 / 0 % | 12.2 / 100 % | 60 / 0 % | 7.0 / 100 % |
| avalonia-aot | 61 / 1 % | 61 / 0 % | 9.4 / 100 % | 61 / 0 % | 4.7 / 100 % |
| avalonia-jit | 61 / 0 % | 61 / 0 % | 9.2 / 100 % | 57 / 3 % | 4.5 / 100 % |
| egui | 57 / 2 % | 56 / 1 % | 2.7 / 100 % | 30 / 34 % | 1.4 / 100 % |
| nsh | 60 / 0 % | 60 / 0 % | 0.2 / 100 % | 60 / 0 % | 0.1 / 100 % |
| compose-jvm | 59 / 2 % | 56 / 4 % | 3.1 / 100 % | 29 / 31 % | 1.5 / 100 % |
| flutter | 34 / 84 % | 25 / 98 % | 4.7 / 100 % | 17.6 / 58 % | **0 frame** |

Pression mémoire (cgroup v1, 3 exécutions ; « pic facturé » = memory.max_usage_in_bytes ; OOM = processus tué) :

| Stack | list 128 Mo : FPS / pic facturé / OOM | stream 128 Mo | list 96 Mo | stream 96 Mo |
|---|---|---|---|---|
| slint | 62 / 10 Mo / 0/3 | 62 / 13 Mo / 0/3 | 62 / 10 Mo / 0/3 | 62 / 13 Mo / 0/3 |
| qt-software | 60 / 18 Mo / 0/3 | 60 / 21 Mo / 0/3 | 60 / 18 Mo / 0/3 | 60 / 21 Mo / 0/3 |
| avalonia-aot | 61 / 29 Mo / 0/3 | 61 / 29 Mo / 0/3 | 61 / 29 Mo / 0/3 | 61 / 29 Mo / 0/3 |
| avalonia-jit | 61 / 49 Mo / 0/3 | 61 / 48 Mo / 0/3 | 61 / 49 Mo / 0/3 | 61 / 49 Mo / 0/3 |
| egui | 59 / 69 Mo / 0/3 | 60 / 70 Mo / 0/3 | 60 / 68 Mo / 0/3 | 60 / 70 Mo / 0/3 |
| nsh | 60 / 41 Mo / 0/3 | 60 / 41 Mo / 0/3 | 60 / 41 Mo / 0/3 | 60 / 41 Mo / 0/3 |
| compose-jvm | – / 128 Mo / 3/3 | – / 128 Mo / 3/3 | – / 96 Mo / 3/3 | – / 96 Mo / 3/3 |
| flutter | – / 128 Mo / 3/3 | – / 128 Mo / 3/3 | – / 96 Mo / 3/3 | – / 96 Mo / 3/3 |

#### 5. Stress 600 s (1 exécution)

| Stack | FPS | frames manquées > 25 ms | CPU % cœur | PSS 1er quart → dernier quart (Mo) | GC : collections / pause totale (ms) |
|---|---|---|---|---|---|
| slint | 9.5 | 100 % | 209 | 39 → 39 | n/a |
| qt-software | 12.2 | 100 % | 197 | 61 → 62 | n/a |
| avalonia-aot | 7.4 | 100 % | 209 | 89 → 140 | 0 / 0.0 |
| avalonia-jit | 7.3 | 100 % | 210 | 130 → 181 | 0 / 0.0 |
| egui | 6.4 | 100 % | 336 | 301 → 303 | n/a |
| nsh | 0.8 | 50 % | 210 | 396 → 397 | n/a |
| compose-jvm | 3.3 | 100 % | 215 | 375 → 459 | 287 / 2113 |
| flutter | 8.1 | 100 % | 335 | 829 → 829 | n/a |

#### 6. Variantes (phase A : 3 exécutions) — marge sans plafond et autres renderers

| Variante | list FPS / CPU % | stream 10k FPS | canvas 5000 FPS | input p50 (ms) | PSS idle (Mo) |
|---|---|---|---|---|---|
| slint-uncapped | 543 / 75 | 532 | 9.0 | 3.6 | 22 |
| slint-software | 62 / 14 | 62 | 18.9 | 13.2 | 18 |
| qt-software-uncapped | 956 / 87 | 906 | 13.6 | 2.2 | 48 |
| qt-opengl | 60 / 79 | 60 | 7.7 | 3.7 | 132 |
| egui-uncapped | 129 / 196 | 128 | 8.2 | 13.9 | 127 |
| egui-glow | 60 / 76 | 60 | 9.0 | 5.7 | 94 |
| compose-jvm-uncapped | 309 / 121 | 306 | 3.3 | 9.9 | 152 |
| flutter-skia | 55 / 123 | 57 | 17.8 | 27.4 | 186 |

#### 7. Calibration externe (list) et preuves GUI

| Stack | FPS déclaré | mises à jour visibles/s (framebuffer) | rapport |
|---|---|---|---|
| slint | 62 | 88 | 1.42× |
| qt-software | 60 | 79 | 1.32× |
| avalonia-aot | 61 | 167 | 2.72× |
| avalonia-jit | 61 | 165 | 2.70× |
| compose-jvm | 60 | 154 | 2.57× |
| flutter | 53 | 73 | 1.39× |
| egui | 58 | 78 | 1.35× |
| nsh | 60 | 88 | 1.48× |

Preuves GUI : 72/72 réussies (fenêtre ≥ 1000×600 créée puis détruite par l'app, contenu non vide). Échecs : aucun.


### Empreinte disque (mesurée, `du -sbL`)
| Stack | Taille | Contenu |
|---|---|---|
| NSH | 19,6 Mo | binaire, + bibliothèques GTK 4 du système (~37 Mo) |
| egui | 19,7 Mo | binaire ; Mesa et X11 chargés par dlopen |
| Flutter | 20,8 Mo | bundle, + GTK 3 du système |
| Slint | 21,3 Mo | binaire avec Skia statique |
| Avalonia AOT | 37,8 Mo | dossier publié |
| Qt | 96,4 Mo | `deploy/` : binaire, bibliothèques Qt 6.12, ICU, plugins |
| Avalonia JIT | 102,2 Mo | dossier publié, autonome |
| Compose | 135,0 Mo | distribution jpackage avec runtime JDK 25 (jlink) |

Tailles wasm : non mesurées.

### Comment lire ces chiffres
- **Rendu logiciel** : le canvas (dégradés, ombres floues, clip arrondi) écrase toutes les stacks, de 0,2 à 13 FPS. Sur GPU, l'ordre peut changer complètement, surtout pour Flutter, Compose, egui et Slint-Skia.
- **Flutter et egui** passent par une émulation GPU (Impeller GLES sur llvmpipe, wgpu sur lavapipe) : leur CPU élevé mesure cette émulation, pas leur comportement sur un appareil. En variante, egui-glow consomme 76 % au lieu de 92 %, et flutter-skia monte le canvas de 12 à 18 FPS.
- **Flutter dessine son canvas par atlas** (sprites pré-rendus, technique recommandée par sa documentation) : c'est plus favorable que le dessin direct des autres stacks.
- **Slint** : 62,5 Hz (timer natif de 16 ms) au lieu de 60 Hz, soit environ 4 % de frames de plus.
- **Calibration** : le framebuffer change 1,3 à 2,7 fois par frame déclarée (Avalonia et Compose présentent en plusieurs bandes). Les FPS déclarés restent la mesure, et la calibration confirme que l'écran est bien mis à jour.
- **Latence de Qt** : Qt redessine immédiatement après une touche, en dehors de la grille de 60 Hz. Les autres stacks attendent la frame suivante, d'où 8 ms de plus en moyenne.
- **Stress** : une seule exécution par stack ; la dérive d'Avalonia (+51 Mo sans aucun GC) et celle de Compose (+84 Mo) demandent une confirmation sur plusieurs exécutions.

### Portes F\* (détails : `docs/fstar/RAPPORT-FSHARP.md`, `docs/fstar/RAPPORT-KRML.md`)
| Porte | Résultat |
|---|---|
| **F\* 2026.10.25 → F# → .NET 10 NativeAOT** | spec 177 lignes vérifiée (Z3 5.3.0) ; nouveau backend F# (Custard) : 0 warning, octets en `uint8` natif ; 9 201 tests OK en JIT et AOT ; binaire 2,35 Mo, **démarrage 8,6 ms à froid, 3,1 ms à chaud, RSS 4,3 Mo** (machine calme), hash de 64 octets en ~90 ns. Source réécrite pour deux changements de F\* (`FStar.Mul` supprimé, `U8.v` interdit dans le code extrait). **Utilisable.** |
| **F\* (Low\*) → C** | **KO** : Low\* ne se vérifie plus avec F\* 2026.10.25 (`Error 134 FStar.HyperStack.ST`). Tout code Low\* existant doit être porté. |
| **F\* (Pulse) → KaRaMeL → C** | 228 lignes vérifiées en 5,6 s → 66 lignes de C sans warning ; 2 634 vérifications et 2 millions d'entrées différentielles contre le cœur Rust, 0 divergence (y compris sous ASan/UBSan) ; débit sur machine calme : **70 M triplets/s en appel direct et 60 M via l'ABI commune, contre 119 M pour le Rust écrit à la main**. KaRaMeL construit avec `make LOWSTAR=false`. **Utilisable.** |

## 6. Constats par stack

- **Slint** : la plus légère (24 Mo de PSS, 99 ms de démarrage, 21 Mo sur disque) ; mémoire parfaitement stable sous stress ; 10 Mo facturés sous pression mémoire. Faiblesses : premier affichage du texte de 1 Mo en 2,1 s ; emoji en monochrome ; pas d'API canvas (un `Rectangle` par forme) ; liste à hauteur variable à virtualiser soi-même. Licence GPL, Royalty-free (attribution obligatoire) ou commerciale.
- **Qt 6.12** : meilleure tenue sous charge (canvas 13,3 FPS, stress 12,2, ½ cœur 7,0 FPS en canvas), latence la plus basse, mémoire stable. Faiblesses : texte avec emoji couleur à 23 FPS, 96 Mo sur disque, deux langages, licence LGPL ou commerciale.
- **Avalonia AOT** : 43 Mo de PSS, 131 ms de démarrage, 38 Mo sur disque, un seul langage (F#), texte de 1 Mo fluide via AvaloniaEdit. Faiblesses : latence clavier de 27 ms, dérive de +51 Mo en stress (à confirmer), plafond de 60 Hz non réglable publiquement.
- **NSH 0.2.0** : CPU bas (17 %), mais **canvas avec ombres documentées injouable** (0,2 FPS, gel du thread GTK) et stress à 0,8 FPS ; projet de quelques semaines.
- **Compose 1.12.2 (JVM)** : tient 60 Hz en défilement, mais à 57-76 % d'un cœur ; 163 Mo de PSS ; 1,7 s de démarrage (0,3-0,4 s avec cache AOT du JDK 25, en variante) ; **OOM à 128 Mo** ; dérive de +84 Mo et 2,1 s de pauses GC en stress ; régression du scénario images liée à Skia/Skiko.
- **Flutter 3.47.8** : en rendu logiciel, ne tient pas 60 FPS en défilement (55, 6 % de frames manquées) ; 212 Mo de PSS ; **OOM à 128 Mo** ; 829 Mo en stress. Non représentatif d'un appareil avec GPU.
- **egui 0.37** (repère, pas d'iOS) : 92 % d'un cœur à 60 Hz avec wgpu émulé (76 % en glow) ; mémoire stable ; texte de 1 Mo sélectionnable.

## 7. Classement et recommandations

Priorités : 1) performance (CPU, RAM, disque, démarrage, tenue sous charge), 2) six plateformes natives + wasm, 3) un seul langage, 4) cœur vérifié, 5) DX.

| Stack | Perf sous charge | Empreinte | Plateformes déclarées | wasm | Langage | Maturité | Risque principal |
|---|---|---|---|---|---|---|---|
| Slint | très bonne | **la plus faible** | 6 | oui, déconseillé pour les apps web générales | Rust + DSL | bonne | licence ; renderer logiciel natif incomplet |
| Qt 6.12 | **la meilleure** | moyenne (96 Mo disque) | 6 | officiel | C++ + QML | très élevée | licence ; deux langages ; taille |
| Avalonia AOT | bonne | faible | 6 (mobile : .NET 10) | annoncé | F# | élevée | dérive en stress à confirmer ; latence |
| NSH | bonne sauf canvas | moyenne | 6 | DOM | Rust | **très faible** | maturité ; canvas |
| Compose (JVM) | moyenne | lourde | Android, iOS, desktop stables | Beta | Kotlin | élevée | JVM, mémoire, OOM |
| Flutter | faible en logiciel (artefact probable) | lourde | 6 | oui | Dart | très élevée | non évaluable sans GPU |
| egui (repère) | CPU élevé | moyenne | **pas d'iOS** | oui | Rust | moyenne | couverture mobile |

**Classement sur les preuves actuelles :**
1. **Slint** : meilleur rapport performance/empreinte, cœur Rust lié directement.
2. **Qt** : la plus robuste sous charge.
3. **Avalonia AOT** : un seul langage, et la seule stack où la porte F\* → F# donne un cœur vérifié dans le même langage que l'UI.
4. NSH, puis Compose, puis Flutter.

**Ce qui ferait changer le classement :**
- **Un GPU réel** : remonte probablement Flutter, Compose et egui, à remesurer.
- **Une contrainte de licence** : peut exclure Slint ou Qt selon l'usage.
- **Une cible web prioritaire** : favorise Compose, Flutter ou Qt.
- **Une équipe Kotlin d'abord et mobile d'abord** : favorise Compose.
- **Une dérive confirmée d'Avalonia** sur plusieurs runs de 10 min : la ferait reculer.

**Par type d'app** (proposition, non validée sur appareil) :
- **Outil léger** : Slint ; Avalonia AOT si l'équipe est .NET/F#.
- **App riche** (grandes listes, texte, graphiques) : Qt, ou Slint ; Avalonia si le canvas reste modéré.
- **Mobile d'abord** : non tranché par ces mesures (aucun appareil) ; mesurer Compose, Flutter, Slint et Avalonia sur appareil avec le protocole §9.

## 8. Plan de migration depuis KMP (proposition, non testée sur mobile)
1. **Inventaire** : séparer dans `commonMain` la logique métier pure (validation, parsing, hash, règles) de l'UI et des API plateforme.
2. **Contrat C-ABI** du cœur (modèle : `core/benchcore.h`). Coût mesuré depuis la JVM via FFM : ~52 M triplets/s pendant que l'UI anime.
3. **Cœur vérifié** :
   - F\* → Pulse → C : 66 lignes de C, 0,5 à 0,6× du Rust à la main ;
   - ou F\* → F#, si l'UI cible est Avalonia ;
   - ou Rust non vérifié.
4. **Branchement dans KMP d'abord**, sans toucher l'UI : FFM ou JNI sur JVM et Android, cinterop sur iOS.
5. **Tranche verticale** dans la stack cible (Slint, Qt ou Avalonia) sur le même cœur, puis migration écran par écran.
6. **Bascule** quand la parité et les budgets du §9 sont tenus sur appareils réels.

## 9. Non mesuré ici, et protocole pour vos appareils
Non mesuré : GPU, iOS et Android réels, IME (CJK, emoji, bidi), énergie, thermique, latence tactile jusqu'au pixel, wasm, Windows, macOS.

Protocole : mêmes apps et même spec (`docs/BENCH-SPEC.md`), 10 exécutions, médiane et p95, `BENCH_PACING=60`.
- **iOS** : Instruments (Time Profiler, Core Animation FPS, Allocations).
- **Android** : `dumpsys gfxinfo <pkg> framestats`, Perfetto, `dumpsys meminfo`, `bundletool` pour la taille installée.
- **Windows** : WPR/WPA et PresentMon. **macOS** : Instruments.
- **Web** : DevTools Performance, tailles brotli du `.wasm` et du JS.
- **IME** : saisie réelle CJK avec composition, emoji ZWJ, arabe et hébreu (curseur, sélection).
- **Latence** : caméra à 240 i/s ou photodiode.
- **Stress** : 10 minutes avec échantillonnage mémoire.

## 10. Annexes
- Recherche (architecture, maturité, licences, contributeurs ; sources avec URL et date, consultées le 2026-10-25) : `docs/research/*.md`.
- Spécification et brief : `docs/BENCH-SPEC.md`, `docs/AGENT-BRIEF.md`.
- Revue et versions par stack : `stacks/*/STACK-NOTES.md`.
- Données brutes : `results/*.json`, `results/*.stdout`, `results/plan*.log`, `results/coldstart.jsonl`, `results/calibration.jsonl`, `results/verification.jsonl` (+ captures PNG).
- Reproduction : `harness/run_plan.sh`, `harness/resume_v2.sh`, `harness/run_mempressure.sh`, `harness/run_bench.py`, `harness/coldstart.py`, `harness/calibrate.py`, `harness/verify_gui.py`, `harness/synthese.py`, `harness/stacks.tsv`.
