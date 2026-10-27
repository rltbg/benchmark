
#### 1. Capacité à 60 Hz (plus grand N qui tient : fps ≥ 58,5, ≤ 1 % de frames > 25 ms, p99 ≤ 33 ms ; ui-tree : p95 redimensionnement ≤ 100 ms)

« ≥ X » = limite de balayage atteinte sans échec ; « < X » = échec dès le plus petit point essayé ; entre parenthèses, le premier N qui échoue.

| Scénario | slint | qt | avalonia-aot |
|---|---|---|---|
| ffi-callbacks | 155 k (160 k) | ≥ 10.24 M | ≥ 10.24 M |
| gfx-chart | 100 k (1 M) | 10 k (100 k) | < 100 k |
| gfx-images | 17 (19) | 80 (145) | 40 (52) |
| gfx-layers | 27 (28) | 160 (168) | 15 (16) |
| gfx-paths | – | – | 50 (84) |
| gfx-shapes | 2 k (2.625 k) | 1.5 k (1.562 k) | 3 k (3.125 k) |
| gfx-shapes-featuregradient | 1 k (1.312 k) | 718 (750) | 656 (687) |
| gfx-shapes-featureshadow | 375 (406) | 1.375 k (1.437 k) | 250 (656) |
| list-mutate | – | 800 k (840 k) | 77.5 k (80 k) |
| list-reset | ≥ 1 M | ≥ 1 M | ≥ 1 M |
| list-rich | 48 (96) | < 6 | 24 (48) |
| list-scale | ≥ 6.4 M | ≥ 6.4 M | ≥ 6.4 M |
| text-big | 1 (10) | < 1 | < 1 |
| text-labels | 1 k (6.25 k) | – | – |
| text-rich | ≥ 64 k | – | – |
| ui-anim | 2.6 k (2.7 k) | 225 (231) | 2 k (2.1 k) |
| ui-bind | 1 k (1.093 k) | – | < 288 |
| ui-tree | 10 k (100 k) | 1 k (10 k) | 1 k (10 k) |

#### 2. Latence externe touche→pixel (I1) et molette→pixel (I2) : p50 / p95 en ms, 3 exécutions


Touche :

| Charge | slint | qt | avalonia-aot | slint-direct |
|---|---|---|---|---|
| idle | 17.4 / 18.3 | 1.4 / 2.2 | 26.5 / 33.4 | 1.4 / 1.8 |
| fling | 12.2 / 18.0 | 1.2 / 5.0 | 27.1 / 34.8 | 3.5 / 5.1 |
| heavy | 17.4 / 18.4 | 1.4 / 3.4 | 26.4 / 33.3 | 1.2 / 1.7 |
| mutate | 12.9 / 18.5 | 1.1 / 4.3 | 26.5 / 33.1 | 3.5 / 5.7 |
| cpuhog | 17.3 / 18.3 | 0.9 / 1.3 | 26.7 / 33.7 | 1.0 / 1.6 |

Molette :

| Charge | slint | qt | avalonia-aot | slint-direct |
|---|---|---|---|---|
| idle | 19.5 / 33.8 | 19.5 / 232.8 | 29.1 / 871.2 | 19.5 / 34.9 |
| fling | 19.5 / 26.4 | 75.7 / 349.3 | 28.3 / 35.1 | 19.8 / 35.5 |
| heavy | 19.4 / 26.2 | 35.1 / 67.5 | 28.5 / 36.1 | 19.7 / 35.3 |
| mutate | 8.3 / 32.3 | 46.2 / 283.8 | 29.6 / 890.5 | 9.6 / 25.8 |
| cpuhog | 19.9 / 28.4 | 35.1 / 925.4 | 32.1 / 864.8 | 20.0 / 34.4 |

#### 3. Dégradation par quota CPU (fps médian / frames manquées > 25 ms)

| Scénario | slint | qt | avalonia-aot |
|---|---|---|---|
| list-scale 1 M, quota 1.0 cœur | 62.2 / 0 % | 60.0 / 0 % | 61.5 / 0 % |
| list-scale 1 M, quota 0.5 cœur | 62.2 / 0 % | 60.0 / 0 % | 61.3 / 0 % |
| gfx-shapes 1000, quota 1.0 cœur | 62.2 / 0 % | 59.8 / 0 % | 61.3 / 0 % |
| gfx-shapes 1000, quota 0.5 cœur | 61.1 / 2 % | 58.0 / 3 % | 60.8 / 1 % |
| text-labels 2000, quota 1.0 cœur | 59.6 / 1 % | 12.2 / 100 % | 17.9 / 100 % |
| text-labels 2000, quota 0.5 cœur | 30.8 / 33 % | 5.7 / 100 % | 8.7 / 100 % |

#### 4. FFI et édition de texte

| Mesure | slint | qt | avalonia-aot |
|---|---|---|---|
| ffi-calls : fps UI | 62.3 | 52.5 | 61.2 |
| ffi-block (fil de fond) : frame max (ms) | 27.7 | 37.9 | 25.5 |
| ffi-block (fil UI) : frame max (ms) | 1006.8 | 1019.7 | 1002.2 |
| T3 édition 1 Mo : latence touche p50 / p95 (ms) | 836.2 / 898.0 | 27.1 / 36.4 | 30.2 / 36.1 |

- slint : ffi-calls 1284.4 M appels/s (médiane des ops.calls_per_s)

- avalonia-aot : ffi-calls 695.7 M appels/s (médiane des ops.calls_per_s)

#### 5. Soak 600 s (list-mutate 20 k/s, 1 exécution)

| Stack | fps | frames manquées > 25 ms | PSS 1er quart → dernier quart (Mo) | CPU % cœur |
|---|---|---|---|---|
| slint | 62.2 | 0.1 % | 47 → 48 | 19 |
| qt | 60.0 | 0.1 % | 65 → 66 | 18 |
| avalonia-aot | 61.3 | 0.1 % | 148 → 205 | 18 |

#### 6. Mémoire minimale (cgroup v1 : pages facturées au cgroup ; bissection 24..1024 Mo, résolution 16 Mo ; 1024 = jamais tenu)

| Scénario | slint | qt | avalonia-aot | slint-direct |
|---|---|---|---|---|
| list-scale 100000 | 24–39 Mo | 180–195 Mo | 24–39 Mo | 24–39 Mo |
| text-big 1 | 117–133 Mo | 1008–1024 Mo | 1008–1024 Mo | 117–133 Mo |

#### 7. Démarrage jusqu'au premier pixel de ui-tree (5 exécutions, médiane / p95 en ms)

| Stack | froid N=1 k | froid N=100 k | chaud N=1 k | chaud N=100 k |
|---|---|---|---|---|
| slint | 167 / 193 | 4542 / 5238 | 88 / 109 | 4646 / 5082 |
| qt | 275 / 309 | 15184 / 16017 | 197 / 219 | 15182 / 16526 |
| avalonia-aot | 206 / 295 | 13919 / 14329 | 120 / 130 | 13667 / 13987 |
| slint-direct | 171 / 263 | 4450 / 4563 | 93 / 104 | 4310 / 4499 |
