# v3 — face-à-face Slint / Qt Quick / Avalonia NativeAOT (bancs de limites)

Trois apps qui poussent chaque framework à ses limites (listes, texte, graphisme, arbres d'UI, flux, FFI, latence, mémoire), toutes avec le même cœur Rust C-ABI (`../core/`). Rendu logiciel, VM 4 vCPU, sans GPU.

- `docs/RAPPORT-V3.md` : **à lire d'abord** (graphes, résultats, prédictions des analystes contre mesures, limites). `docs/BENCH-SPEC-v3.md` : spécification. `docs/deep/*.md` : analyses de sources par stack (prédictions datées). `docs/img/` : graphes SVG.
- `slint/`, `qt/`, `avalonia/` : les trois apps, chacune avec `build.sh`, `run.sh`, `NOTES.md` (versions, variantes, ce qui est partiel/non supporté), `OPTIMIZATION-LOG.md`.
- `harness/` : `capacity.py` (recherche de capacité à 60 Hz), `run_plan3.sh` (pilote reprenable de toutes les mesures), `synthese3.py` (génère `results/SYNTHESE.md`), `graphes3.py` (graphes), `stacks3.tsv`. Le harnais commun (`run_bench.py`, `coldstart.py`…) est dans `../harness/`.
- `results/` : JSON bruts (`capacity/`, `runs/`, `raw/`), `SYNTHESE.md`, `plan*.log`, captures de vérification GUI.

Reproduire : `python3 -I v3/harness/gen_assets_big.py assets` puis `python3 -I harness/gen_assets.py assets` ; `(cd core && cargo build --release)` ; `v3/<stack>/build.sh` ; `bash v3/harness/run_plan3.sh "lat quota ffi edit cap soak memmin cold foot" "slint qt avalonia-aot"` ; `python3 -I v3/harness/synthese3.py && python3 -I v3/harness/graphes3.py`.
