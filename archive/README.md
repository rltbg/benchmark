# Archive (mise de côté, hors de la comparaison v3)

Contenu conservé tel quel, non maintenu, non utilisé par les bancs v3 :

- `fstar/`, `fstar-docs/` : les portes F* (F* → F# → .NET → NativeAOT, F* → KaRaMeL → C) et leurs documents.
- `v2-stacks/` : les implémentations de l'app de référence v2 (Slint, Qt, Avalonia avec cœurs F*/F#, Compose, Flutter, egui, NSH).

Les scripts v2 de `harness/` (`stacks.tsv`, `run_plan.sh`, `run_corevariants.sh`…) pointent encore vers l'ancien emplacement `stacks/` : il faut remplacer `stacks/` par `archive/v2-stacks/` pour les relancer. Les données v2 restent dans `results/` et le rapport v2 dans `docs/RAPPORT-FINAL.md`.
