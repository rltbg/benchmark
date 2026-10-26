# Banc comparatif de stacks UI natives multiplateformes (rendu logiciel, Linux)

Comparaison mesurée de Slint, Qt/QML, Avalonia (F#, NativeAOT et JIT), Compose Multiplatform (JVM), Flutter, egui et NSH (Rust, GTK 4) sur une app de référence identique, plus deux « portes » de vérification formelle : F\* → F# → NativeAOT et F\* (Low\*) → C via KaRaMeL.

**Lire d'abord : [`docs/RAPPORT-FINAL.md`](docs/RAPPORT-FINAL.md)** (résultats, limites, audit des versions, verdict, plan de migration depuis KMP).

> Toutes les mesures sont en **rendu logiciel sur une VM de 4 vCPU sans GPU**. Elles ne valent ni pour un GPU, ni pour iOS/Android, ni pour wasm. Un audit des versions a montré que plusieurs outils n'étaient pas à jour : voir §0 du rapport (remesure en cours).

## Arborescence
| Chemin | Contenu |
|---|---|
| `docs/RAPPORT-FINAL.md` | rapport final |
| `docs/BENCH-SPEC.md`, `docs/AGENT-BRIEF.md` | spécification de l'app de référence et consignes d'implémentation |
| `docs/research/` | fiches de recherche (architecture, maturité, licences, contributeurs ; sources avec URL et date) |
| `docs/fstar/` | rapports des portes F\* (F#/NativeAOT et Low\*→C/KaRaMeL) |
| `core/` | cœur C-ABI commun en Rust (`validate`, `parse`, `hash`, `heavy`), header C, micro-banc C |
| `harness/` | harnais de mesure externe (`run_bench.py`), pilotes (`run_all.sh`, `run_resume.sh`, `run_trim2.sh`), démarrage (`coldstart.py`), calibration (`calibrate.py`, `fbwatch.py`), preuve GUI (`verify_gui.py`), agrégation (`aggregate.py`, `synthese.py`), liste des binaires (`stacks.tsv`) |
| `stacks/<stack>/` | code de l'app de référence par stack (+ `STACK-NOTES.md` et `logs/`) |
| `fstar/` | spec F\* (`Core.fst`), F# extrait, projets .NET, journaux |
| `stacks/karamel/` | module Low\* (`Core_Low.fst`), C généré, tests, scripts de build |
| `results/` | JSON bruts de chaque exécution, résumés, `SYNTHESE.md`, `TABLES.md`, démarrage, calibration, preuves GUI (`verification/`) |
| `logs/`, `toolchains-logs/` | journaux réseau et d'installation des outils |

## Reproduire
1. Générer les assets (non versionnés, 111 Mo) : `python3 -I harness/gen_assets.py assets`
2. Construire le cœur : `cd core && cargo build --release`
3. Construire une stack : `stacks/<stack>/build.sh` (dépendances système : Xvfb, Mesa llvmpipe, GTK 3/4, Qt 6, polices Noto, voir `stacks/*/STACK-NOTES.md`)
4. Mesurer : `python3 -I harness/run_bench.py --stack NAME --cmd "<binaire>" --scenario list --duration 14 --runs 10`

## Hors périmètre / exclu
- Aucune version web/TypeScript : les variantes NSH TSX/QuickJS et GPUI ont été retirées du dépôt.
- iced : écarté (mobile hors scope pour ses auteurs) ; ses résultats bruts restent dans `results/iced-*`.
- Les binaires, caches de build, dépôts tiers (NSH, KaRaMeL, Flutter SDK) et assets générés ne sont pas versionnés.
