#!/bin/bash
# Pilote séquentiel des mesures v2. Rien d'autre ne doit tourner sur la machine pendant les mesures.
# usage: harness/run_plan.sh "<phases>" "<stacks principales>" "<stacks secondaires>"
#   phases : A B D cold cal verify C   (ex. "A B D cold cal verify C")
#   A      : idle, list, stream 10k, canvas 5000, core, input — 10 exécutions (secondaires : 3)
#   B      : stream 1k, canvas 20000, text, images, nav — 5 exécutions
#   D      : dégradation — 1 cœur (list, stream, canvas), ½ cœur (list, canvas), mémoire 512/256 Mo (list, stream) — 3 exécutions
#   cold   : démarrage à froid (caches vidés) et à chaud — 10 exécutions
#   cal    : calibration externe des FPS (list)
#   verify : preuve GUI + capture par scénario
#   C      : stress 600 s — 1 exécution
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; export BENCH_ROOT="$ROOT"; cd "$ROOT"
PHASES="$1"; MAIN="$2"; SEC="${3:-}"
H="python3 -I harness/run_bench.py"
line() { grep -v '^#' harness/stacks.tsv | awk -F'\t' -v n="$1" '$1==n'; }
cmd_of() { line "$1" | cut -f2; }
extra_of() { line "$1" | cut -f3; }
run() { # stack scenario duration runs [args...]
  local s=$1 sc=$2 d=$3 r=$4; shift 4
  echo "=== $(date +%T) $s $sc d=$d runs=$r $*"
  # shellcheck disable=SC2046
  $H --stack "$s" --cmd "$(cmd_of "$s")" --scenario "$sc" --duration "$d" --runs "$r" $(extra_of "$s") "$@" 2>&1 \
    | grep -E "^SUMMARY|'rc': -?[1-9]|Traceback|Error" | cut -c1-400
}
has() { case " $PHASES " in *" $1 "*) return 0;; esac; return 1; }
A() { local n=$1; shift; for s in "$@"; do
  run "$s" idle 20 "$n"; run "$s" list 14 "$n"; run "$s" stream 14 "$n" --rate 10000; run "$s" canvas 14 "$n" --shapes 5000
  run "$s" core 14 "$n"; run "$s" input 14 "$n"; done; }
B() { for s in "$@"; do run "$s" stream 14 5 --rate 1000; run "$s" canvas 14 5 --shapes 20000
  run "$s" text 14 5; run "$s" images 14 5; run "$s" nav 14 5; done; }
D() { for s in "$@"; do
  run "$s" list 14 3 --cpus 0; run "$s" stream 14 3 --rate 10000 --cpus 0; run "$s" canvas 14 3 --shapes 5000 --cpus 0
  run "$s" list 14 3 --cpu-quota 0.5; run "$s" canvas 14 3 --shapes 5000 --cpu-quota 0.5
  run "$s" list 14 3 --mem-limit 512; run "$s" stream 14 3 --rate 10000 --mem-limit 512
  run "$s" list 14 3 --mem-limit 256; run "$s" stream 14 3 --rate 10000 --mem-limit 256; done; }
has A && A 10 $MAIN && [ -n "$SEC" ] && A 3 $SEC
has B && B $MAIN
has D && D $MAIN
if has cold; then : > results/coldstart.jsonl; for s in $MAIN; do
  python3 -I harness/coldstart.py "$s" "$(cmd_of "$s")" 10 --drop >> results/coldstart.jsonl
  python3 -I harness/coldstart.py "$s" "$(cmd_of "$s")" 10 >> results/coldstart.jsonl; echo "cold $s $(date +%T)"; done; fi
if has cal; then : > results/calibration.jsonl; for s in $MAIN; do
  python3 -I harness/calibrate.py "$s" "$(cmd_of "$s")" list >> results/calibration.jsonl; echo "cal $s $(date +%T)"; done; fi
if has verify; then : > results/verification.jsonl; for s in $MAIN; do for sc in idle list stream canvas core input text images nav; do
  a=""; [ "$sc" = canvas ] && a="--shapes 1000"; [ "$sc" = stream ] && a="--rate 10000"
  VG_D=30 VG_T=8 python3 -I harness/verify_gui.py "$s" "$(cmd_of "$s")" "$sc" $a >> results/verification.jsonl; done; echo "verify $s $(date +%T)"; done; fi
has C && for s in $MAIN; do run "$s" stress 600 1; done
echo "=== DONE $(date +%T)"
