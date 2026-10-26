#!/bin/bash
# Pilote séquentiel des mesures officielles. Rien d'autre ne doit tourner sur la machine.
cd /home/user/eval
H="python3 -I harness/run_bench.py"
MAIN="slint avalonia-aot avalonia-jit compose-jvm qt-software flutter egui-glow nsh-gtk iced-natural iced-fulldamage"
SEC="slint-femtovg qt-opengl egui-wgpu"
getline() { grep -v '^#' harness/stacks.tsv | awk -F'\t' -v n="$1" '$1==n'; }
run() { # stack scenario duration runs [extra harness args...]
  local s=$1 sc=$2 d=$3 r=$4; shift 4
  local line cmd extra; line=$(getline $s); cmd=$(echo "$line" | cut -f2); extra=$(echo "$line" | cut -f3)
  echo "=== $(date +%T) $s $sc d=$d runs=$r $*"
  $H --stack $s --cmd "$cmd" --scenario $sc --duration $d --runs $r $extra "$@" 2>&1 | grep -E "^SUMMARY|rc': [^0]|Traceback" | cut -c1-300
}
phaseA() { for s in $MAIN; do run $s idle 20 10; run $s list 14 14 10 2>/dev/null; done; }
phaseA() {
  for s in $MAIN; do run $s idle 20 10; run $s list 14 10; run $s stream 14 10 --rate 10000; run $s canvas 14 10 --shapes 5000; run $s core 14 10; done
  for s in $SEC; do run $s idle 20 3; run $s list 14 3; run $s stream 14 3 --rate 10000; run $s canvas 14 3 --shapes 5000; run $s core 14 3; done
}
phaseB() {
  for s in $MAIN; do run $s stream 14 5 --rate 1000; run $s canvas 14 5 --shapes 20000; run $s text 14 5; run $s images 14 5; run $s nav 14 5; done
  for s in $SEC; do run $s text 14 3; run $s images 14 3; run $s nav 14 3; done
}
phaseD() { for s in $MAIN; do for sc in list stream canvas; do
   a=""; [ $sc = stream ] && a="--rate 10000"; [ $sc = canvas ] && a="--shapes 5000"
   run $s $sc 14 3 --cpus 0 $a; done; done; }
phaseC() { for s in $MAIN; do run $s stress 600 1; done; }
case "$1" in A) phaseA;; B) phaseB;; D) phaseD;; C) phaseC;; all) phaseA; phaseB; phaseD; phaseC;; esac
echo "=== DONE $(date +%T)"
