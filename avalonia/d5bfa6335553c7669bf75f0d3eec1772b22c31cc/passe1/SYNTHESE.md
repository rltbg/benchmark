
#### Stacks retenues (phase A : 10 exécutions, médianes ; stress : 1 exécution)

| Stack | RAM idle (Mo) | CPU idle (% cœur) | list 100k : FPS (frame p95 ms) | stream 10k/s : FPS | canvas 5000 : FPS | core : M triplets/s (FPS UI) | 1er pixel froid / chaud (ms) | Stress 300 s : FPS, RSS début→fin (Mo) |
|---|---|---|---|---|---|---|---|---|
| slint | 23 | 0.16 | 528 (2.5) | 495 | 18.4 | 78 (201) | 88 / 46 | 40.6, 36→36 |
| qt-software | 52 | 0.25 | 798 (2.0) | 738 | 13.6 | 127 (67) | 134 / 66 | 13.9, 63→65 |
| avalonia-aot | 40 | 0.32 | 202 (6.8) | 190 | 8.5 | 37 (39) | 118 / 74 | 7.0, 71→92 |
| avalonia-jit | 79 | 5.66 | 178 (7.7) | 172 | 8.0 | 67 (35) | 867 / 813 | non mesuré |
| egui-glow | 114 | 2.91 | 197 (6.6) | 216 | 11.4 | 167 (63) | 154 / 99 | 10.2, 176→178 |
| nsh-gtk | 116 | 0.63 | 60 (17.9) | 60 | 4.0 | 106 (55) | 179 / 100 | 7.9, 149→149 |
| compose-jvm | 168 | 10.75 | 225 (6.7) | 216 | 4.7 | 25 (30) | 1688 / 1312 | 3.1, 318→467 |
| flutter | 243 | 1.63 | 50 (26.1) | 54 | 0.3 | 51 (2) | 443 / 317 | 0.3, 1662→1718 |

Secondaires (phase A : 3 exécutions) et hors périmètre plateformes (iced : mobile hors scope pour ses auteurs, 10 exécutions) :

| Variante | list FPS | stream 10k FPS | canvas 5000 FPS | core FPS UI | RAM idle Mo |
|---|---|---|---|---|---|
| slint-femtovg | 173 | 162 | 1.4 | 8 | 135 |
| qt-opengl | 250 | 240 | 13.8 | 48 | 156 |
| egui-wgpu | 136 | 133 | 11.4 | 51 | 145 |
| iced-natural | 26 | 26 | 0.3 | 17 | 16 |
| iced-fulldamage | 238 | 231 | 4.4 | 31 | 16 |

#### Autres scénarios (phase B : 3 exécutions ; FPS médian)

| Stack | stream 1k/s | canvas 20 000 | text 1 Mo | images 600 | nav | list 1 cœur | canvas 5000 1 cœur |
|---|---|---|---|---|---|---|---|
| slint | 552 | 8.4 | 246 | 282 | 810 | 545 | 21 |
| qt-software | 774 | 3.5 | 172 | 398 | 710 | 866 | 14.3 |
| avalonia-aot | 201 | 2.5 | 1.2 | 110 | 170 | 201 | 8.8 |
| avalonia-jit | 192 | 2.2 | 1.1 | 110 | 160 | 178 | 8.1 |
| egui-glow | 206 | 4.2 | 279 | 176 | 132 | 83 | 4.2 |
| nsh-gtk | 60 | 1.1 | 60 | 60 | 56 | 60 | 4.0 |
| compose-jvm | 201 | 1.3 | 118 | 28 | 245 | 145 | 4.7 |
| flutter | 55 | **aucune frame** | 35 | 54 | 33 | 36 | 0.6 |
