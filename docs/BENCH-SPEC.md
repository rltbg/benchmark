# Spécification de l'app de référence (identique dans chaque stack)

Fenêtre 1280×720 logique, échelle 1, Xvfb 24 bits, **rendu logiciel (pas de GPU)**. Release uniquement.
Données et assets communs : `/home/user/eval/assets` (text1mb.txt, images/img000..599.png), cœur C-ABI : `/home/user/eval/core` (`benchcore.h`, `target/release/libbenchcore.{a,so}`).

## Interface de ligne de commande de chaque binaire
`APP --scenario <idle|list|stream|canvas|text|images|nav|core|stress> --duration <s> --out <fichier.json> [--rate N] [--shapes N] [--assets DIR]`
Le binaire démarre, ouvre la fenêtre, exécute UNIQUEMENT le scénario demandé pendant `duration` secondes (après 2 s de warm-up non mesuré), écrit le JSON puis quitte avec code 0.
Pas de pacing vsync/timer : en scénario animé, **redessin continu** (chaque frame demande la suivante immédiatement), on mesure l'intervalle entre fins de frame consécutives.
CPU et RSS sont mesurés **de l'extérieur** par le harnais (`harness/run_bench.py`, lecture /proc à 10 Hz) ; l'app ne les rapporte pas.

## JSON de sortie
```
{"stack":"slint","scenario":"list","duration_s":10.0,"frames":N,"frame_ms":[...tous les intervalles en ms...],
 "ops":{...compteurs spécifiques...},"checksum":"<hex>","notes":"..."}
```
`frame_ms` = temps entre deux fins de frame (swap/present/raster terminé le plus tard observable). Documente dans `notes` ce qui est exactement mesuré.

## Modèle de données commun
- `fnv(i)` = FNV-1a 32 bits des 4 octets little-endian de i.
- Ligne i (0 ≤ i < 100 000) : hauteur `h(i) = 48 + 16*(fnv(i) % 6)` px (48..128). Titre `"Item #" + i` ; sous-titre = texte `"Quote " + i + " — " + (fnv(i+7) % 100000)/100.0` répété/tronqué (peu importe, clippé à la hauteur) ; pastille ronde 32 px colorée `rgb(fnv(i)&255, fnv(i)>>8&255, fnv(i)>>16&255)` ; prix à droite aligné, mis à jour par le flux.
- **Somme des hauteurs** de la liste = valeur attendue `sumH` (calculée par le harnais ; chaque app l'écrit dans ops.sum_heights pour preuve).
- Liste VIRTUALISÉE : seuls les éléments visibles (+ marge raisonnable) sont instanciés/dessinés. Pas de 100 000 widgets.

## Scénarios
- **idle** : coquille d'app (barre de titre, liste affichant la première page, barre d'état), AUCUNE animation, aucun redessin continu. Dormir `duration` s. (frame_ms vide.) Mesure RAM/CPU base.
- **list** : liste 100k variable. Défilement scripté (offset en px, déterministe) : 4 s à 3 000 px/s vers le bas, puis 6 « flings » alternés bas/haut, chacun vitesse initiale 20 000 px/s décroissant `v(t)=v0*exp(-2t)` pendant 1,5 s, jusqu'à `duration`. Clamp aux bornes. `ops.final_offset` + `ops.rows_drawn`.
- **stream** : comme list + `--rate R` (1000 ou 10000) mises à jour de prix par seconde, générées par un thread de fond (PRNG xorshift32 seed 0x2545F491 : index = x % 100000, prix = x / 100.0), transmises au thread UI par file/canal ; le thread UI les applique par lots à chaque frame ; les lignes visibles se redessinent avec le nouveau prix. `ops.updates_generated`, `ops.updates_applied`, `ops.updates_dropped` (jamais de blocage du producteur : file bornée 100k, drop+compte si pleine).
- **canvas** : `--shapes N` (5000 par défaut, test aussi 20000) formes qui bougent (rebond dans la fenêtre, vitesse fixe issue de fnv), 1/3 rectangles, 1/3 cercles, 1/3 rect arrondis, remplissage dégradé linéaire 2 couleurs, 1 forme sur 10 avec ombre portée (flou 8 px, offset 2,2), l'ensemble clippé dans un rect arrondi de 1000×600 ; plus un graphe temps réel : polyligne de 10 000 points qui défile (un point ajouté par frame) dans une zone 1000×100 en bas. Dessin via l'API canvas/immédiate/scene native de la stack (pas un widget par forme, sauf impossibilité à documenter).
- **text** : 1 Mo de texte (text1mb.txt) dans une vue scrollable en lecture seule avec sélection ; script : défilement continu 3 000 px/s aller-retour + à t=5 s « tout sélectionner » (si API) ; `ops.time_to_first_display_ms`, `ops.layout_ms`. L'IME n'est pas automatisable : voir protocole manuel.
- **images** : grille de 600 images (img000..599.png 256×256) en vignettes 128×128, décodées en asynchrone hors thread UI, défilement scripté (3 000 px/s) + zoom/pan scripté (échelle 1→3→1 sur 6 s) sur la grille ; `ops.images_decoded`, `ops.decode_total_ms`.
- **nav** : 3 écrans ; 20 transitions animées enchaînées de 300 ms (push/pop avec glissement) et ouvertures/fermetures de modale (fondu 200 ms), pendant qu'un thread de fond occupe ≈1 cœur (boucle de calcul). `ops.transitions_done`.
- **core** : thread worker appelant en boucle serrée `core_validate` + `core_parse` + `core_hash` (trame « hello », 8 octets) via le C-ABI de benchcore ; `ops.calls_per_s` (1 « call » = 1 triplet). Un autre thread de fond appelle `core_heavy(100)` toutes les 1 s. Pendant ce temps, le thread UI anime 1 000 formes en continu (canvas allégé). `ops.heavy_calls`, `ops.max_frame_stall_ms`.
- **stress** : list+stream(rate 5000)+canvas(5000 formes, panneau latéral 400×720)+core simultanés ; `duration` par défaut 600 s ; `ops` additionnel : toutes les 10 s un échantillon `{"t":..,"managed_heap_kb":..}` si le runtime en a un.

## Ce qui est hors de portée du cloud (protocole manuel à fournir)
IME réelle (CJK/emoji/bidi) ; GPU ; iOS/Android réels ; énergie ; latence d'entrée de bout en bout (touch→pixel) ; thermique.
sumH attendue (100k lignes) = 8799584
