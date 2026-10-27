#!/bin/bash
# Fumee via le harnais (run_bench.py), 1 run, duree 8 s : harness-smoke.sh <outdir> ; liste "scenario|args" dans v3/avalonia/logs/harness-smoke.cfg
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT=${1:-/tmp/avalonia3-smoke}; mkdir -p $OUT
while IFS='|' read -r sc args; do
  [ -z "$sc" ] && continue
  n=""; vars=""
  set -- $args
  while [ $# -gt 0 ]; do case $1 in --n) n="--n $2"; shift 2;; --variant) vars="$vars --variant $2"; shift 2;; *) shift;; esac; done
  python3 -I $ROOT/harness/run_bench.py --stack avalonia-aot --cmd "\${ROOT}/v3/avalonia/run.sh" --scenario $sc --duration 8 --runs 1 --outdir $OUT $n $vars 2>&1 | grep -E " run 0 " | sed -E "s/'window': 'markers', //" | cut -c1-330
done < $ROOT/v3/avalonia/logs/harness-smoke.cfg
