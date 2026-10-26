#!/bin/bash
# Pilote séquentiel des mesures v3 (machine au repos : rien d'autre ne doit tourner).
# usage: harness/run_plan3.sh "<phases>" "<stacks>"
#   cap    : recherche de capacité (harness/capacity.py) sur tous les scénarios à échelle
#   lat    : I1 touche->pixel et I2 molette->pixel, 5 charges (idle fling cpuhog heavy mutate), 3 exécutions
#   quota  : dégradation de list-scale 1M, gfx-shapes 1000 et text-labels 2000 à 1 cœur et ½ cœur
#   ffi    : ffi-calls (threads 1/4), ffi-block (bg/ui), ffi-callbacks (capacité)
#   edit   : T3 text-edit 1 Mo (latence par frappe)
#   soak   : list-mutate 20 k/s pendant 600 s (dérive PSS) — composite M1 non implémenté dans toutes les stacks
#   memmin : plus petite limite cgroup (bissection 24..1024 Mo) pour list-scale 100k et text-big 1
#   cold   : démarrage à froid/chaud de ui-tree 1 k et 100 k
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; export BENCH_ROOT="$ROOT"; cd "$ROOT"
PHASES="$1"; STACKS="$2"
OUT="$ROOT/results3"; mkdir -p "$OUT"
cmd_of() { grep -v '^#' harness/stacks3.tsv | awk -F'\t' -v n="$1" '$1==n{print $2}'; }
extra_of() { grep -v '^#' harness/stacks3.tsv | awk -F'\t' -v n="$1" '$1==n{print $3}'; }
has() { case " $PHASES " in *" $1 "*) return 0;; esac; return 1; }
rb() { # stack scenario duration runs tag [args...]
  local s=$1 sc=$2 d=$3 r=$4 tag=$5; shift 5
  echo "=== $(date +%T) $s $sc $tag $*"
  mkdir -p "$OUT/runs/$s"
  # shellcheck disable=SC2046
  python3 -I harness/run_bench.py --stack "$s" --cmd "$(cmd_of "$s")" --scenario "$sc" --duration "$d" --runs "$r" \
    --outdir "$OUT/runs/$s" $(extra_of "$s") "$@" 2>&1 | grep -E "^SUMMARY|Traceback|Error" | cut -c1-300
}
if has cap; then for s in $STACKS; do for sc in list-scale list-rich list-mutate list-reset text-big text-labels text-rich gfx-shapes gfx-paths \
    gfx-chart gfx-images gfx-layers ui-tree ui-bind ui-anim ffi-callbacks; do
  python3 -I harness/capacity.py --stack "$s" --scenario "$sc" 2>&1 | grep -E "RÉSULTAT|Traceback" ; done
  python3 -I harness/capacity.py --stack "$s" --scenario gfx-shapes --variant feature=gradient 2>&1 | grep -E "RÉSULTAT|Traceback"
  python3 -I harness/capacity.py --stack "$s" --scenario gfx-shapes --variant feature=shadow 2>&1 | grep -E "RÉSULTAT|Traceback"; done; fi
if has lat; then for s in $STACKS; do for load in idle fling heavy mutate cpuhog; do
  hog=0; [ $load = cpuhog ] && hog=3
  rb "$s" latency-load 14 3 "lat-key-$load" --n 100000 --variant load=$load --latency key --cpuhog $hog
  rb "$s" latency-wheel 14 3 "lat-wheel-$load" --n 100000 --variant load=$load --latency wheel --cpuhog $hog; done; done; fi
if has quota; then for s in $STACKS; do for q in 1 0.5; do
  rb "$s" list-scale 14 3 "q$q-list" --n 1000000 --cpu-quota $q
  rb "$s" gfx-shapes 14 3 "q$q-shapes" --n 1000 --cpu-quota $q
  rb "$s" text-labels 14 3 "q$q-labels" --n 2000 --cpu-quota $q; done; done; fi
if has ffi; then for s in $STACKS; do
  rb "$s" ffi-calls 14 3 ffi-calls --n 1000000
  rb "$s" ffi-block 14 3 ffi-block-bg --n 1000 --variant where=bg
  rb "$s" ffi-block 14 3 ffi-block-ui --n 1000 --variant where=ui; done; fi
if has edit; then for s in $STACKS; do
  rb "$s" text-edit 14 3 text-edit-1mb --n 1 --latency key; done; fi
if has soak; then for s in $STACKS; do
  rb "$s" list-mutate 600 1 soak --n 20000; done; fi
if has memmin; then for s in $STACKS; do for sc in "list-scale 100000" "text-big 1"; do set -- $sc
  lo=24; hi=1024
  while [ $((hi - lo)) -gt 16 ]; do mid=$(( (lo + hi) / 2 ))
    r=$(python3 -I harness/run_bench.py --stack "$s" --cmd "$(cmd_of "$s")" --scenario "$1" --n "$2" --duration 20 --runs 1 \
        --outdir "$OUT/runs/$s" $(extra_of "$s") --mem-limit $mid 2>&1 | grep -o "'ok_runs': [0-9]*\|\"ok_runs\": [0-9]*\|\"fps_mean\": {\"median\": [0-9.]*" | tr '\n' ' ')
    echo "memmin $s $1 $mid -> $r"
    case "$r" in *"ok_runs\": 1"*"fps_mean\": {\"median\": 5[89]"*|*"ok_runs\": 1"*"fps_mean\": {\"median\": 6"*) hi=$mid;; *) lo=$mid;; esac
  done; echo "MEMMIN $s $1 $2 : entre $lo et $hi Mo"; done; done; fi
if has cold; then for s in $STACKS; do for n in 1000 100000; do
  echo "cold $s ui-tree $n"; export CS_SCENARIO=ui-tree; python3 -I harness/coldstart.py "$s" "$(cmd_of "$s") --n $n" 5 --drop 2>&1 | tail -1
  python3 -I harness/coldstart.py "$s" "$(cmd_of "$s") --n $n" 5 2>&1 | tail -1; done; done; fi
echo "=== DONE $(date +%T)"
