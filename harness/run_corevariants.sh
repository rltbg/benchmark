#!/bin/bash
# Avalonia AOT : cœur Rust (P/Invoke) vs code F# extrait par F* vs F# écrit à la main (même binaire).
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
C='${ROOT}/stacks/avalonia/out/aot/avalonia-bench'
for impl in rust fstar fsharp; do
  echo "=== $(date +%T) core impl=$impl"
  python3 -I harness/run_bench.py --stack avalonia-aot-core-$impl --cmd "$C" --scenario core --duration 14 --runs 10 --env BENCH_CORE_IMPL=$impl 2>&1 | grep -E "^SUMMARY|'rc': -?[1-9]|Traceback" | cut -c1-300
done
for impl in fstar fsharp; do
  echo "=== $(date +%T) stress impl=$impl"
  python3 -I harness/run_bench.py --stack avalonia-aot-$impl --cmd "$C" --scenario stress --duration 600 --runs 1 --env BENCH_CORE_IMPL=$impl 2>&1 | grep -E "^SUMMARY|'rc': -?[1-9]|Traceback" | cut -c1-300
done
echo "=== CORE DONE $(date +%T)"
