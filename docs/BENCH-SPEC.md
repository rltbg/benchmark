# Spécification de l'app de référence — v2 (identique dans chaque stack)

Fenêtre 1280×720 logique, échelle 1, Xvfb 24 bits, **rendu logiciel (pas de GPU)**. Release uniquement.

## Chemins (portables)
- Racine du dépôt : variable d'environnement `BENCH_ROOT` ; à défaut, chaque script la déduit de son propre emplacement (`git rev-parse --show-toplevel` ou chemin relatif). **Aucun chemin absolu codé en dur** dans le code.
- Assets : `--assets DIR` (générés par `python3 -I harness/gen_assets.py assets`).
- Cœur C-ABI : `core/` (`benchcore.h`, `target/release/libbenchcore.{a,so}`). Lien statique ou dynamique au choix de la stack ; si dynamique, le chemin se résout via `BENCHCORE_LIB` puis `$BENCH_ROOT/core/target/release/libbenchcore.so`.

## Interface de ligne de commande
`APP --scenario <idle|list|stream|canvas|text|images|nav|core|input|stress> --duration <s> --out <fichier.json> [--rate N] [--shapes N] [--assets DIR]`

Le binaire ouvre la fenêtre, fait 2 s de warm-up non mesuré, puis :
1. imprime sur stdout la ligne exacte `BENCH:measure_start` (puis flush) ;
2. exécute UNIQUEMENT le scénario pendant `duration` secondes ;
3. imprime `BENCH:measure_end` (flush), écrit le JSON, quitte avec code 0.

Le harnais mesure CPU, RSS et PSS **entre ces deux marqueurs**, de l'extérieur (`/proc`, 10 Hz). L'app ne rapporte ni CPU ni RAM.

## Définition d'une frame (uniforme)
- Scénarios animés : **redessin continu**, sans plafond si la stack le permet ; si la stack impose un plafond (vsync, 60 Hz) non désactivable par une API **publique**, le garder et le documenter dans `notes` (ne pas contourner par API privée).
- `frame_ms[i]` = temps entre deux **fins de présentation** consécutives (retour de swap/present/commit, ou le signal post-rendu le plus tardif exposé publiquement). Documenter l'événement exact dans `notes`.
- Ne compter que les frames dont le **contenu a changé**. Ne jamais gonfler les FPS.

## Cadence (variable `BENCH_PACING`) — ajout v2.1
Sans cadence commune, une stack plafonnée à 60 Hz et une stack sans plafond ne sont pas comparables (la seconde consomme ~100 % d'un cœur quoi qu'il arrive).
- `BENCH_PACING=60` (**défaut, mesure principale**) : animation cadencée à 60 Hz. Utiliser la cadence native de la stack si elle existe (vsync, frame clock, timer de rendu) ; sinon une API publique (timer, `request_repaint_after`, `QT_QPA_UPDATE_IDLE_TIME`…) visant une frame toutes les 16,667 ms. Une frame qui dépasse son échéance est une frame manquée (mesurée par le harnais : frames > 16,7 ms et > 25 ms).
- `BENCH_PACING=uncapped` : le plus vite possible, uniquement par API publique. Si la stack ne le permet pas, garder 60 Hz et écrire `ops.pacing = "60-only"`.
- Écrire dans `ops.pacing` le mode effectif (`"60"`, `"uncapped"`, `"60-only"`) et le mécanisme utilisé dans `notes`.

## JSON de sortie
```
{"stack":"slint","version":"<versions exactes>","scenario":"list","duration_s":10.0,"frames":N,"frame_ms":[...],
 "ops":{...},"checksum":"<hex>","notes":"...","unsupported":null}
```
- `ops.gc` (runtimes managés uniquement) : `{"collections": n, "pause_ms_total": x, "pause_ms_max": y}` (.NET : `GC.CollectionCount`, `GC.GetTotalPauseDuration` ; JVM : `GarbageCollectorMXBean` ; Dart : `null` si indisponible).
- Si un scénario est impossible dans la stack : `"unsupported": "raison"` (c'est un résultat, ne pas simuler).

## Modèle de données commun
- `fnv(i)` = FNV-1a 32 bits des 4 octets little-endian de i.
- Ligne i (0 ≤ i < 100 000) : hauteur `h(i) = 48 + 16*(fnv(i) % 6)` px. Titre `"Item #" + i` ; sous-titre `"Quote " + i + " — " + (fnv(i+7) % 100000)/100.0`, clippé ; pastille ronde 32 px `rgb(fnv(i)&255, fnv(i)>>8&255, fnv(i)>>16&255)` ; prix à droite, mis à jour par le flux.
- `ops.sum_heights` doit valoir **8 799 584**. Liste VIRTUALISÉE (pas 100 000 widgets).

## Scénarios
- **idle** : coquille d'app (barre de titre, première page de liste, barre d'état), aucune animation ni redessin continu ; dormir `duration`. Base RAM/CPU.
- **list** : défilement scripté déterministe (offset px) : 4 s à 3 000 px/s vers le bas, puis 6 flings alternés bas/haut de vitesse initiale 20 000 px/s, `v(t)=v0·exp(-2t)` pendant 1,5 s chacun, jusqu'à `duration` ; clamp aux bornes. `ops.final_offset`, `ops.rows_drawn`.
- **stream** : list + `--rate R` (1 000 ou 10 000) mises à jour/s par un thread de fond (xorshift32 seed 0x2545F491 : index = x % 100000, prix = x / 100.0), file bornée 100k vers le thread UI (drop + compte si pleine), application par lots à chaque frame. `ops.updates_generated/applied/dropped`.
- **canvas** : `--shapes N` (5 000 ; 20 000) formes en rebond (vitesse issue de fnv), 1/3 rectangles, 1/3 cercles, 1/3 rect. arrondis, dégradé linéaire 2 couleurs, 1 sur 10 avec ombre portée (flou 8 px, offset 2,2), le tout clippé dans un rect. arrondi 1000×600 ; plus une polyligne de 10 000 points qui défile (1 point/frame) dans 1000×100 en bas. API canvas/scène native (pas un widget par forme sauf impossibilité documentée). Toute primitive non rendue par le moteur → `ops.not_rendered: [...]`.
- **text** : text1mb.txt dans une vue scrollable en lecture seule avec sélection ; défilement 3 000 px/s aller-retour ; « tout sélectionner » à t=5 s si l'API existe. `ops.time_to_first_display_ms`, `ops.layout_ms`.
- **images** : 600 PNG 256×256 en vignettes 128×128, décodés hors thread UI ; défilement 3 000 px/s + zoom/pan 1→3→1 sur 6 s. `ops.images_decoded`, `ops.decode_total_ms`.
- **nav** : 3 écrans ; 20 transitions animées de 300 ms (push/pop glissé) + modales (fondu 200 ms), avec un thread de fond occupant ≈1 cœur. `ops.transitions_done`.
- **core** : worker en boucle serrée `core_validate`+`core_parse`+`core_hash` (trame « hello », 8 octets) via la C-ABI (`ops.calls_per_s`, 1 call = 1 triplet, la boucle ne doit pas être optimisée par le compilateur : résultat accumulé et publié) ; un autre thread appelle `core_heavy(100)` toutes les 1 s ; l'UI anime 1 000 formes. `ops.heavy_calls`, `ops.max_frame_stall_ms`.
- **input** (nouveau) : le scénario list tourne ; un carré 200×200 en (100,100) est dessiné par-dessus, et **change de couleur à chaque appui sur Espace** (alterne noir #000000 / blanc #FFFFFF). Le harnais envoie les touches (XTest) et mesure de l'extérieur le délai jusqu'au changement des pixels du carré. L'app ne fait que basculer la couleur à la réception de l'événement clavier, par le chemin d'événements normal de la stack. `ops.key_events`.
- **stress** : list + stream(5 000) + canvas(5 000 formes, panneau latéral 400×720) + core simultanés ; `duration` 600 s ; `ops.heap_samples` toutes les 10 s si runtime managé.

## Dégradation (appliquée par le harnais, sans changer l'app)
- CPU : `--cpus 0` (1 cœur, taskset) et `--cpu-quota 0.5` (½ cœur, cgroup CFS).
- Mémoire : `--mem-limit 256` / `512` Mo (cgroup v1 `memory.limit_in_bytes`) ; un OOM est un résultat (`oom: true`).

## Hors cloud (protocole manuel)
IME réelle (CJK/emoji/bidi), GPU, iOS/Android, énergie, thermique, latence touch→pixel sur écran réel, wasm.
