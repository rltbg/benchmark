# Spécification v3 — bancs de limites : Slint / Qt / Avalonia AOT

**But.** Les bancs v2 mesuraient *une* charge fixe : en rendu logiciel, tout le monde s'effondrait sur le même scénario (canvas), ce qui ne dit rien de la conception des stacks. La v3 cherche, pour chaque scénario, **la capacité maximale tenue à 60 Hz** (le plus grand paramètre d'échelle `N` pour lequel la stack tient son budget de frame), puis étudie *comment* chacune casse au-delà. Trois stacks seulement : Slint (Rust), Qt (C++/QML), Avalonia (F#, NativeAOT). **Aucun F\*, aucun cœur F# : le cœur natif commun (Rust, interface C) est appelé par le FFI de chaque stack.**

Plateforme de mesure : Linux x64, VM de 4 vCPU, **sans GPU**, Xvfb 1280×720 ; les chemins GPU (OpenGL/Vulkan) ne tournent que sur llvmpipe/lavapipe (émulation CPU) : ils testent le code de la stack, pas un GPU. Aucune conclusion sur GPU, mobile ou web n'est tirée de ces mesures.

## 1. Principes d'implémentation (équité)
1. **Même contenu visuel, mêmes données, mêmes scripts** pour les trois (voir §4). Les fonctions pures (`fnv`, `xorshift32`, hauteurs, couleurs) sont identiques.
2. **Idiomatique et performant** : chaque implémentation doit utiliser la voie que recommandent la documentation officielle, les guides de performance du projet et ses propres benchmarks (`slint-ui/slint/benchmarks`, `qt/qtdeclarative/tests/benchmarks`, `AvaloniaUI/Avalonia/tests/Avalonia.Benchmarks`) pour ce cas d'usage. **Journal d'optimisation obligatoire** (`bench3/<stack>/OPTIMIZATION-LOG.md`) : pour chaque scénario, la version naïve (v0), chaque optimisation tentée (v1, v2…) avec le gain mesuré en fumée et la source qui la justifie (URL de doc ou fichier:ligne du code du projet). Une optimisation qui ne tient pas la comparaison d'équité (triche, contournement d'API privée, réduction du contenu) est interdite.
3. **Aucune API privée ni réflexion sur l'interne** ; les API publiques marquées instables sont permises si elles sont documentées dans `NOTES`.
4. **Une capacité absente est un résultat** : `"unsupported": "raison"` dans le JSON et dans `NOTES` ; ne simule jamais. Une primitive non rendue → `ops.not_rendered`.
5. Release uniquement ; versions les plus récentes stables vérifiées au moment du build (notées dans `version`).
6. Le thread UI ne doit jamais être bloqué volontairement ; tout travail de fond va sur d'autres threads ; les mises à jour du modèle passent par le mécanisme thread-safe idiomatique de la stack.

## 2. Interface en ligne de commande et marqueurs (v2 inchangés, avec `--n`)
`APP --scenario <ID> [--n N] [--variant k=v]… --duration S --out FICHIER.json --assets DIR`
- Warm-up de 2 s, puis `BENCH:measure_start` sur stdout (flush) ; fin : `BENCH:measure_end`, JSON écrit, code 0. Le harnais mesure CPU/RSS/PSS entre les marqueurs.
- `BENCH_PACING=60` (défaut, cadence native ou API publique ; `ops.pacing`) ou `uncapped`.
- `frame_ms` = intervalle entre fins de présentation de frames dont le contenu a changé (v2 : `docs/BENCH-SPEC.md`).
- JSON : champs v2 (`stack`, `version`, `scenario`, `frame_ms`, `ops`, `checksum`, `notes`, `unsupported`) **plus** :
  - `ops.n` (paramètre reçu), `ops.variant` ;
  - `ops.ui_stall_ms_max` : plus long intervalle sans retour à la boucle d'événements, mesuré par un minuteur de 4 ms sur le thread UI (si l'API le permet ; sinon dérivé de `frame_ms`, à préciser dans `notes`) ;
  - `ops.setup_ms` : temps entre le début du processus (ou de `main`) et la première frame du scénario *avec* ses données construites ;
  - `ops.gc` pour .NET (`GC.CollectionCount(0/1/2)`, `GC.GetTotalPauseDuration`, `GC.GetTotalAllocatedBytes`), `ops.alloc_bytes` pour les autres si disponible ;
  - `ops.checksum_data` : empreinte du jeu de données de test (identique entre stacks).

## 3. Cœur C-ABI v2 (partagé, FFI)
`core/benchcore.h` (inchangé : `core_validate`, `core_parse`, `core_hash`, `core_heavy`) **plus** `core_noop`, `core_add`, `core_fill(out,n,seed)`, et le **producteur natif** `core_producer_start(cb, user, rate, range)` / `core_producer_stop` / `core_producer_produced` (thread natif qui rappelle `cb(user, index, price)` à la cadence `rate`, flux xorshift32 identique au banc). Chaque stack lie le cœur par son mécanisme FFI idiomatique (Slint : `extern "C"` ; Qt : fonctions C liées statiquement ; Avalonia : `DllImport` blittable, `DisableRuntimeMarshalling`, `DirectPInvoke` en AOT, `UnmanagedCallersOnly` pour les rappels).

## 4. Données et fonctions communes
`fnv(i)` = FNV-1a 32 bits des 4 octets little-endian de `i`. Hauteur de ligne standard `h(i) = 48 + 16*(fnv(i) % 6)` ; **hauteur dense** `hd(i) = 24 + 4*(fnv(i) % 6)`. Couleur d'une ligne `rgb(fnv(i)&255, fnv(i)>>8&255, fnv(i)>>16&255)`. Flux de mises à jour : xorshift32 graine 0x2545F491 ; index = `x % range`, prix = `x/100.0`. Texte : `assets/text1mb.txt` (+ générateur `harness/gen_assets.py --text-mb S` pour 10/50/100 Mo), images `assets/images/img000..599.png`, + `assets/big4096.png` (4096×4096) et `assets/big8192.png` (8192×8192) générés de façon déterministe. Cellule de liste riche : voir L2. Fenêtre 1280×720, échelle 1.

## 5. Scénarios et paramètres d'échelle (ID → ce qui est joué ; `N` = paramètre balayé)
Chaque scénario dure 12 s de mesure (sauf mention). **Carré de latence permanent** : dans TOUS les scénarios (sauf `idle`), un carré 200×200 est dessiné en (100,100) au-dessus du reste ; il alterne noir #000000 / blanc #FFFFFF à chaque appui sur Espace, par le chemin d'événements normal de la stack (`ops.key_events` compte les appuis). Le harnais mesure de l'extérieur le délai appui→pixel (I1, T3) sur n'importe quel scénario. Redimensionnement : la fenêtre doit se redimensionner normalement (U1) ; pas de taille fixe. « Tient 60 Hz » : voir §6.

**Listes (virtualisées)**
- **L1 `list-scale`** — `N` lignes (100 k → 10 M, hauteurs standard), défilement 30 000 px/s constant 4 s puis 6 flings alternés de 150 000 px/s (`v(t)=v0·exp(-2t)`, 1,5 s). Mesure : `ops.setup_ms` (création du modèle de `N` lignes), RSS, frames. `ops.sum_heights` doit valoir la somme exacte.
- **L2 `list-rich`** — 100 k lignes à hauteur dense, `K` éléments par ligne : base de 6 (pastille ronde, titre, sous-titre 2 lignes avec ellipse, barre de progression, bascule = rect arrondi + cercle, prix) + `(K-6)/2` « puces » (rect arrondi coloré + texte court). `K` ∈ {6, 12, 24, 48, 96}. Défilement 20 000 px/s. `N` = `K`.
- **L3 `list-mutate`** — 100 k lignes, défilement 3 000 px/s ; un producteur de fond émet `N` mises à jour de prix/s (valeur) + `N/100` opérations structurelles/s (insertion, suppression, déplacement à un index ≥ première ligne visible), file bornée 1 M, application par lots à chaque frame. Mesure `updates_generated/applied/dropped`, frames. Tient si ≥ 99,9 % appliquées et 60 Hz.
- **L4 `list-reset`** — toutes les 2 s, remplacement complet du modèle par `N` lignes (10 k → 1 M) pendant le défilement ; mesure du plus long arrêt (`ops.ui_stall_ms_max`) et du temps de reconstruction.

**Texte**
- **T1 `text-big`** — document de `N` Mo (1, 10, 50, 100) en lecture seule, sélectionnable, défilement 30 000 px/s aller-retour ; « tout sélectionner » à t = 5 s (si l'API existe). Mesure `time_to_first_display_ms`, `layout_ms`, RSS, frames.
- **T2 `text-labels`** — `N` étiquettes de texte visibles (grille), dont le texte change **à chaque frame** (compteur + 8 caractères dérivés de `fnv`) : débit de mise en forme/tracé de texte.
- **T3 `text-edit`** — document éditable de 1 Mo ; le harnais tape (XTest) des caractères au rythme `N` car/s (10, 30, 100, 300) au milieu du document ; carré 200×200 en (100,100) qui change de couleur à chaque touche **après** application à l'éditeur (comme `input`) → latence externe touche→pixel pendant l'édition.
- **T4 `text-rich`** — texte riche de `N` spans (1 000 → 100 000) : polices/tailles/couleurs/gras alternés, CJK, emoji, arabe/hébreu ; défilement 10 000 px/s.

**Graphisme (CPU)** — canvas natif de la stack (pas un widget par forme sauf impossibilité, à documenter).
- **G1 `gfx-shapes`** — `N` formes animées (rebond), 4 variantes `variant=feature` : `solid` (aplat), `gradient` (dégradé linéaire 2 couleurs), `shadow` (aplat + ombre portée flou 8 px), `clip` (aplat, le tout dans un rect arrondi 1000×600), plus `all`. Capacité par variante (isole la fonction qui s'effondre).
- **G2 `gfx-paths`** — `N` chemins de Bézier cubiques (8 segments) tracés (largeur 2 px) ; variante `dashed`. Capacité.
- **G3 `gfx-chart`** — courbe temps réel à `N` points (10 k, 100 k, 1 M), 1 000 points ajoutés par frame, tracé en polyligne 1 px dans 1000×300.
- **G4 `gfx-images`** — `N` images (256², décodées une fois) dessinées chaque frame avec échelle/rotation/opacité pseudo-aléatoires ; variante `big` : image 4096² puis 8192² en pan/zoom/rotation. Capacité.
- **G5 `gfx-layers`** — `N` groupes semi-transparents superposés (opacité 0,5) ; variante `blur` (flou 4 px sur chaque groupe). Une absence de support est un résultat.

**Arbre d'interface, liaisons, animations**
- **U1 `ui-tree`** — arbre statique de `N` éléments (1 k, 10 k, 100 k, 500 k : conteneurs de 10 enfants, feuilles = rect + texte court). Mesure `setup_ms`, RSS, puis **tempête de redimensionnement** : le harnais redimensionne la fenêtre 100 fois (1280×720 ↔ 1100×650) ; `ops.resize_frame_ms[]` (durée de chaque frame après redimensionnement).
- **U2 `ui-bind`** — `N` étiquettes liées à des propriétés réactives ; **à chaque frame** `N` propriétés changent (compteur) par le mécanisme de liaison idiomatique de la stack (pas de `setText` manuel si une liaison existe). Capacité.
- **U3 `ui-anim`** — `N` éléments animés *par le moteur d'animation de la stack* (position + opacité, interpolation, boucle infinie), pas par code par frame. Capacité.

**FFI et fils**
- **F1 `ffi-calls`** — appels du cœur depuis 1 puis 4 threads de fond : `core_noop`, `core_add`, `core_hash` sur tampons de `N` octets (8, 1 Ko, 64 Ko, 1 Mo ; tampon créé une fois côté stack), `core_fill` dans un tampon de la stack ; pendant ce temps l'UI anime 1 000 formes `solid`. Mesure appels/s, Mo/s, FPS UI.
- **F2 `ffi-callbacks`** — `core_producer_start` avec `rate` = `N`/s (10 k → 10 M) ; le rappel natif transmet (index, prix) au thread UI par le mécanisme idiomatique ; liste de 100 k lignes visible. Tient si ≥ 99 % des événements sont reflétés (ou coalescés explicitement, à documenter) et 60 Hz.
- **F3 `ffi-block`** — `core_heavy(N ms)` (10, 100, 1000) sur un fil de fond **pendant** que la liste défile : `ops.ui_stall_ms_max` doit rester petit. Une variante `ui` (appel sur le thread UI) mesure le gel.

**Latence (mesurée de l'extérieur : harnais + framebuffer, comme `input` en v2)**
- **I1 `latency-load`** — touche→pixel avec 5 charges de fond : `idle`, `fling` (liste en fling), `cpuhog` (3 processus `stress` à 100 % hors de l'app), `heavy` (`core_heavy(100)` en boucle sur un fil de fond), `mutate` (L3 à 100 k/s). p50/p95/p99/max sur 100 appuis.
- **I2 `latency-wheel`** — molette (XTest boutons 4/5) → premier pixel changé, dans la liste, avec les mêmes charges.

**Mémoire et durée**
- **M1 `soak`** — scénario composite pendant 30 min : L3 (20 k/s) + G1 `gradient` (500 formes) + F2 (50 k/s) + T3 (5 car/s). PSS toutes les 10 s ; dérive en Mo/h (régression linéaire sur les 20 dernières minutes), GC .NET, pics de frame.
- **M2 `mem-min`** — plus petite limite cgroup mémoire (bissection 24 → 1024 Mo) sous laquelle le composite de 60 s (L1 `N=100k` + T1 `N=1`) tourne sans OOM ni perte de 60 Hz.
- **M3 `startup-scale`** — démarrage jusqu'à la première frame utile pour U1 `N` ∈ {1 k, 100 k} ; à froid (caches vidés) et à chaud.

**Dégradation** : G1 `gradient`, L2, U2 et F2 répétés sous quota CPU 1 cœur, ½ cœur (CFS), ¼ de cœur (cgroup v1 `cpu.cfs_quota_us`).

## 6. Critère de capacité et recherche (harnais `harness/capacity.py`)
- **Tient 60 Hz** à `N` : sur la fenêtre de mesure, `fps ≥ 58,5` (Slint tourne à 62,5 Hz : le seuil est atteint a fortiori), **frames > 25 ms ≤ 1 %**, **p99 ≤ 33 ms**, `rc = 0`, marqueurs présents ; pour L3/F2 : `applied/generated ≥ 0,999`.
- **Recherche** : `N0` du scénario → doublement jusqu'à échec → bissection jusqu'à ±5 % → **3 confirmations consécutives** au `N_max` retenu (sinon on recule d'un pas) → on mesure aussi à `2×N_max` (forme de la dégradation : le FPS s'effondre-t-il ou décroît-il doucement ?). Chaque sonde : 1 exécution de 12 s ; le harnais écrit chaque sonde (`results3/capacity/<stack>/<scn>/<N>.json`) et le résumé (`N_max`, métriques à `N_max` et `2×N_max`, CPU %, PSS).
- Exécution séquentielle, machine au calme (aucune compilation en cours), un seul scénario à la fois.

## 7. Livrables de chaque stack (`bench3/<stack>/`)
Code de l'app (un binaire, tous les scénarios) ; `build.sh` reproductible ; commande de lancement pour `harness/stacks3.tsv` ; `NOTES.md` (versions exactes, ce qui est implémenté / partiel / non supporté et pourquoi, erreurs exactes) ; `OPTIMIZATION-LOG.md` (§1.2) ; captures de vérification (`harness/verify_gui.py`) *lues* pour chaque scénario ; fumée harnais de chaque scénario (`markers: True`, `sum_heights`/`checksum_data` corrects).
