# Brief v3 — implémenter les bancs de limites dans UNE stack

Lis d'abord `docs/BENCH-SPEC-v3.md` (spec v3, à respecter à la lettre), puis `docs/BENCH-SPEC.md` (v2 : marqueurs, cadence, définition de frame, JSON) et `docs/AGENT-BRIEF.md` (règles d'équité et de rigueur v2, toujours valables). Dépôt : `/home/user/benchmark` (branche `bench-v3`, **ne change jamais de branche**, ne `git checkout` pas ; ne commit pas — je m'en charge). Ton dossier de travail : `bench3/<stack>/` (crée-le ; point de départ : le code de `stacks/<stack>/` de la passe précédente, que tu copies puis fais évoluer — **sans** aucune trace de F\* ni de cœur F#/« sans FFI » : le cœur est celui de `core/` (C-ABI, v2 : `core/benchcore.h`), appelé par le FFI de ta stack).

## Machine et contraintes
- 4 vCPU, 16 Go, **~9 Go de disque libres au total** (partagés avec deux autres implémentations qui compilent en même temps) : un dossier de build par stack, nettoie `target/`, `obj/`, caches inutiles ; ne clone pas de gros dépôts dans le dépôt (utilise `/opt/src/` pour les sources de référence, avec `--depth 1`, sparse si possible).
- Au plus 2 jobs de compilation. Pas d'`apt` sans nécessité absolue. Pas de `pkill Xvfb` ; ton Xvfb à toi sur un display :200–:699 ; `harness/run_bench.py` choisit un display libre. Réseau ouvert (registres, docs officielles, clones git anonymes) sauf pages github.com/API et crates.io web.
- **Tu ne fais pas de mesures officielles** : des fumées (≤ 15 s) seulement, pour valider chaque scénario à petit `N` et à grand `N`. Les mesures de capacité seront lancées par moi, machine au calme.
- Outils déjà installés : Rust 1.99.1, .NET SDK 10.0.403 (`/opt/dotnet10`), Qt 6.12.0 (`/opt/Qt/6.12.0/gcc_64`), cmake 4.4.4 (`/opt/cmake-4.4.4-linux-x86_64`), ninja, JDK 25, Xvfb, xdotool, Mesa llvmpipe, **perf** (`/usr/lib/linux-tools/*/perf`, événements logiciels `cpu-clock` seulement), valgrind, gdb. Vérifie en début de travail que les versions sont encore les plus récentes stables (comme en v2) et note-les.

## Ce que tu livres (`bench3/<stack>/`)
1. L'app (un binaire, tous les scénarios de la spec, `--scenario/--n/--variant`), `build.sh` reproductible, `run.sh` (wrapper exécutable : pose les variables d'environnement par défaut et `exec` le binaire avec les arguments reçus ; c'est la commande de `harness/stacks3.tsv`).
2. **Priorités** — Niveau 1 (obligatoire) : L1, L2, L3, T1, T3, G1 (5 variantes), G3, U1, U2, U3, F1, F2, F3 et le carré de latence partout. Niveau 2 : L4, T2, T4, G2, G4, G5. Les scénarios non pris en charge par la stack : `unsupported` + raison.
3. `OPTIMIZATION-LOG.md` (spec §1.2) : pour chaque scénario, v0 naïve → versions optimisées, gains mesurés en fumée, justification par la documentation ou **le code source du projet** (chemin:ligne, commit). Lis le code des composants qui servent (listes, scène/rendu, moteur de liaison, texte) : le but est d'écrire l'implémentation que les mainteneurs du projet écriraient, pas la plus évidente.
4. `NOTES.md` : versions exactes, tableau scénario × (implémenté / partiel / non supporté), pièges, limites, écarts à la spec, erreurs exactes rencontrées.
5. Vérification : pour chaque scénario, `python3 -I harness/verify_gui.py <stack> "<commande>" <scénario> --n <petit N>` et **lis la capture PNG** (outil Read) ; `ops.sum_heights` / `ops.checksum_data` justes ; `markers: True` ; rien ne doit planter à grand `N` (un échec propre et documenté vaut mieux qu'un plantage : si le processus meurt, c'est un résultat de capacité, mais pas un bug de ton code).
6. Fournis aussi, pour mon harnais, les noms exacts de `variant` acceptés par chaque scénario.

## Règles d'or
- Équité : même contenu visuel et mêmes données que les autres stacks ; aucune réduction de travail ; mêmes critères de « tient 60 Hz ». Si la stack ne sait pas faire quelque chose (ex. flou en rendu logiciel), c'est `unsupported`/`not_rendered`, pas un contournement.
- Ne prétends rien que tu n'aies exécuté ; garde les erreurs exactes ; documente les API instables utilisées.
- Rapport final ≤ 40 lignes : état par scénario, commande de lancement, versions, 3 plus grosses difficultés, fumées brutes (FPS à petit et grand N), ce qui reste à faire.
