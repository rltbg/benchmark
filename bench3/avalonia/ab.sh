#!/bin/bash
# ab.sh <fichier-de-configs> : fumees A/B (binaire AOT), une ligne "label|scenario|args". Sortie : une ligne resumee par config.
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="$(cd "$HERE/../.." && pwd)" DISPLAY=:200 LIBGL_ALWAYS_SOFTWARE=1
D=${AB_DURATION:-8}
while IFS='|' read -r label sc args; do
  [ -z "$label" ] && continue
  OUT=/tmp/ab-$$.json; rm -f $OUT
  timeout 120 $HERE/out/avalonia-bench --scenario $sc --duration $D --out $OUT --assets $BENCH_ROOT/assets $args > /tmp/ab-$$.log 2>&1
  rc=$?
  python3 -I - "$OUT" "$label" "$rc" <<'PY'
import json,sys
out,label,rc=sys.argv[1:4]
try: d=json.load(open(out))
except Exception: print(f"{label:46s} rc={rc} NO-JSON"); sys.exit()
f=sorted(d['frame_ms']);n=len(f);o=d['ops'];g=o.get('gc',{})
fps=1000*n/sum(f) if n else 0
extra=[]
for k in ('setup_ms','ui_stall_ms_max','updates_generated','updates_applied','calls_per_s','mb_per_s','time_to_first_display_ms','layout_ms','heavy_ui_ms','rebuild_ms'):
    if k in o:
        v=o[k]
        if isinstance(v,list): v=round(max(v),1) if v else v
        elif isinstance(v,float): v=round(v,1)
        extra.append(f"{k}={v}")
print(f"{label:46s} rc={rc} n={n:5d} fps={fps:5.1f} p50={f[n//2] if n else 0:6.1f} p99={f[int(n*.99)] if n else 0:6.1f} max={f[-1] if n else 0:6.1f} gt25={sum(1 for x in f if x>25):4d} gc={g.get('gen0')}/{g.get('gen1')}/{g.get('gen2')} pause={g.get('pause_ms_total')} alloc={round((g.get('alloc_bytes') or 0)/1e6)}MB "+" ".join(extra))
PY
done < "$1"
