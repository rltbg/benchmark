
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
