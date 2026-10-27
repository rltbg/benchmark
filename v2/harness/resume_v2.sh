#!/bin/bash
# Reprise du plan v2 après l'arrêt du conteneur (04:52) : egui input, phase A de NSH et des
# variantes, puis toutes les phases suivantes pour les stacks principales.
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
MAIN="slint qt-software avalonia-aot avalonia-jit compose-jvm flutter egui nsh"
SEC="slint-uncapped slint-software qt-software-uncapped qt-opengl egui-uncapped egui-glow compose-jvm-uncapped flutter-skia"
echo "=== $(date +%T) egui input d=14 runs=10 (reprise)"
python3 -I harness/run_bench.py --outdir v2/results --stack egui --cmd "$(grep -P '^egui\t' v2/harness/stacks.tsv | cut -f2)" --scenario input --duration 14 --runs 10 2>&1 | grep -E "^SUMMARY|'rc': -?[1-9]|Traceback" | cut -c1-400
bash v2/harness/run_plan.sh "A" "nsh" "$SEC"
bash v2/harness/run_plan.sh "B D cold cal verify C" "$MAIN" ""
echo "=== RESUME DONE $(date +%T)"
