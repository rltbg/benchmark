# v2 — app de référence sur 8 stacks (rendu logiciel, Linux)

Comparaison Slint, Qt/QML, Avalonia (JIT et NativeAOT), Compose Multiplatform (JVM), Flutter, egui et NSH sur une app identique (scénarios idle, list, stream, canvas, text, images, nav, core, input, stress), avec un cœur Rust C-ABI commun.

- `docs/RAPPORT-FINAL.md` : rapport (résultats, limites, audit des versions, verdict, plan de migration depuis KMP). `docs/BENCH-SPEC.md`, `docs/AGENT-BRIEF.md` : spécification et consignes.
- `<stack>/` (slint, qt, avalonia, compose, flutter, egui, nsh, iced) : code de l'app par stack, avec `STACK-NOTES.md`.
- `harness/` : pilotes v2 (`run_plan.sh`, `resume_v2.sh`, `run_mempressure.sh`, `run_corevariants.sh`), `stacks.tsv`, `synthese.py`, `aggregate.py`. Le harnais de mesure commun est dans `../harness/`.
- `results/` : JSON bruts de chaque exécution et `SYNTHESE.md` (régénérable : `python3 -I v2/harness/synthese.py`, vérifié identique après la réorganisation).

Les binaires et caches de build (`target/`, `out/`, `deploy/`…) ont été supprimés pour libérer du disque : il faut reconstruire chaque stack (`v2/<stack>/build.sh`) avant de relancer les mesures. Les pilotes ont été adaptés aux nouveaux chemins mais **pas relancés** depuis la réorganisation.
