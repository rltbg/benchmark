#!/bin/sh
# Fumée courte via le harnais (pas une mesure officielle).
# usage: smoke.sh <scenario> [durée=8] [options run_bench…]   ex. smoke.sh list 8 --env BENCH_PACING=uncapped
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}
sc=${1:?scenario}; dur=${2:-8}; shift; [ $# -gt 0 ] && shift
exec python3 -I "$ROOT/harness/run_bench.py" --stack egui --cmd '${ROOT}/stacks/egui/target/release/egui_bench' \
  --scenario "$sc" --duration "$dur" --runs 1 --outdir "${TMPDIR:-/tmp}/egui-smoke" "$@"
