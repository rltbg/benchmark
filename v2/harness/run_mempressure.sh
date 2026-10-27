#!/bin/bash
# Pression mémoire réelle : les limites 512/256 Mo ne contraignaient aucune stack (pic facturé au
# cgroup ≤ 181 Mo, les bibliothèques partagées déjà en cache n'étant pas facturées). Limites 128 et 96 Mo.
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"; cd "$ROOT"
for s in slint qt-software avalonia-aot avalonia-jit compose-jvm flutter egui nsh; do c=$(grep -P "^$s\t" v2/harness/stacks.tsv | cut -f2)
  for m in 128 96; do for sc in list stream; do a=""; [ $sc = stream ] && a="--rate 10000"
    echo "=== $(date +%T) $s $sc mem=$m"
    python3 -I harness/run_bench.py --outdir v2/results --stack $s --cmd "$c" --scenario $sc --duration 14 --runs 3 --mem-limit $m $a 2>&1 | grep -E "^SUMMARY|Traceback" | cut -c1-300
  done; done; done
echo "=== MEM DONE $(date +%T)"
