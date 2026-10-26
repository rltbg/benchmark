#!/bin/bash
cd /home/user/eval; : > results/coldstart.jsonl; : > results/calibration.jsonl
S="slint avalonia-aot avalonia-jit compose-jvm qt-software flutter egui-glow nsh-gtk"
get() { grep -v '^#' harness/stacks.tsv | awk -F'\t' -v n="$1" '$1==n {print $2}'; }
for s in $S; do c=$(get $s)
  python3 -I harness/coldstart.py $s "$c" 10 --drop >> results/coldstart.jsonl 2>>results/final.err
  python3 -I harness/coldstart.py $s "$c" 10 >> results/coldstart.jsonl 2>>results/final.err
  echo "cold $s done $(date +%T)"
done
for s in $S; do c=$(get $s); python3 -I harness/calibrate.py $s "$c" list >> results/calibration.jsonl 2>>results/final.err; echo "cal $s $(date +%T)"; done
echo "=== DONE"
