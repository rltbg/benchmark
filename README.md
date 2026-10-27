# Banc comparatif de stacks UI natives multiplateformes

Trois campagnes, séparées par dossier. Toutes les mesures sont en **rendu logiciel sur VM 4 vCPU sans GPU** : elles ne valent ni pour un GPU, ni pour iOS/Android, ni pour le web.

| Dossier | Contenu | Rapport |
|---|---|---|
| [`v1/`](v1/README.md) | premier passage et portes F\* (F\* → F# → NativeAOT, F\* → C via KaRaMeL) | `v1/fstar-docs/` |
| [`v2/`](v2/README.md) | app de référence sur 8 stacks (Slint, Qt, Avalonia, Compose, Flutter, egui, NSH…) | [`v2/docs/RAPPORT-FINAL.md`](v2/docs/RAPPORT-FINAL.md) |
| [`v3/`](v3/README.md) | **face-à-face Slint / Qt Quick / Avalonia AOT**, bancs de limites, empreinte CPU/RAM | [`v3/docs/RAPPORT-V3.md`](v3/docs/RAPPORT-V3.md) |

Partagé : `core/` (cœur Rust C-ABI + header), `harness/` (harnais de mesure externe commun : `run_bench.py`, `coldstart.py`, `calibrate.py`, `verify_gui.py`, `fbwatch.py`, `gen_assets.py`), `docs/research/` (fiches de recherche avec sources et dates), `assets/` (générés, non versionnés), `logs/`, `toolchains-logs/`.

Les binaires, caches de build, dépôts tiers et assets générés ne sont pas versionnés.
