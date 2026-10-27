#!/bin/bash
# Fumee manuelle : smoke.sh <dev|aot> <scenario> <duration> [args...]  (DISPLAY :200 par defaut)
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export DISPLAY="${DISPLAY:-:200}" LIBGL_ALWAYS_SOFTWARE=1 DOTNET_ROOT=/opt/dotnet10
v=$1; sc=$2; d=$3; shift 3
OUT="${TMPDIR:-/tmp}/avalonia3-$v-$sc.json"
rm -f "$OUT"
if [ "$v" = dev ]; then BIN="/opt/dotnet10/dotnet $HERE/dev/avalonia-bench.dll"; else BIN="$HERE/out/avalonia-bench"; fi
timeout $((${d%.*}+60)) $BIN --scenario "$sc" --duration "$d" --out "$OUT" --assets "$BENCH_ROOT/assets" "$@" 2>&1 | tail -${TAILN:-6}
python3 -I - "$OUT" <<'PY'
import json,sys
try: d=json.load(open(sys.argv[1]))
except Exception as e: print("NO JSON",e); sys.exit(1)
f=sorted(d['frame_ms']);n=len(f)
if n: print('frames',n,'fps',round(1000*n/sum(f),1),'p50',f[n//2],'p95',f[int(n*.95)],'p99',f[int(n*.99)],'max',f[-1],'>25ms',sum(1 for x in f if x>25))
o=d['ops']; g=o.get('gc',{})
print({k:v for k,v in o.items() if k not in ('gc','resize_frame_ms','ui_stall_hist_lt8_16_33_50_100_250_1000_ge1000','rebuild_ms')})
print('gc',{k:g.get(k) for k in ('gen0','gen1','gen2','pause_ms_total','pause_ms_max','alloc_bytes')})
if d.get('unsupported'): print('UNSUPPORTED',d['unsupported'])
PY
