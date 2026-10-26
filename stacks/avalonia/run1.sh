#!/bin/bash
# Fumee manuelle : run1.sh <aot|jit> <scenario> <duration> [args...]   (DISPLAY doit pointer vers un Xvfb 1280x720x24)
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
v=$1; sc=$2; d=$3; shift 3
OUT="${TMPDIR:-/tmp}/avalonia-$v-$sc.json"
LIBGL_ALWAYS_SOFTWARE=1 "$HERE/out/$v/avalonia-bench" --scenario "$sc" --duration "$d" --out "$OUT" --assets "$BENCH_ROOT/assets" "$@" 2>&1 | tail -15
python3 -I - "$OUT" <<'PY'
import json,sys;d=json.load(open(sys.argv[1]));f=sorted(d['frame_ms']);n=len(f)
if n: print('frames',n,'fps',round(1000*n/sum(f),1),'p50',f[n//2],'p95',f[int(n*.95)],'max',f[-1])
o=d['ops']; print({k:v for k,v in o.items() if k!='heap_samples'})
PY
