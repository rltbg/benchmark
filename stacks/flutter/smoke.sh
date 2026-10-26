#!/usr/bin/env bash
# usage: smoke.sh SCENARIO DURATION TAG [extra app args...]   (env passes through)
HERE="$(cd "$(dirname "$0")" && pwd)"
S=$1; D=$2; TAG=$3; shift 3
N=$((200+RANDOM%400))
Xvfb :$N -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XP=$!
sleep 1
DISPLAY=:$N LIBGL_ALWAYS_SOFTWARE=1 timeout 120 "$HERE/app/build/linux/x64/release/bundle/benchapp" --scenario $S --duration $D --out "$HERE/logs/smoke-$TAG.json" --assets /home/user/eval/assets "$@" > "$HERE/logs/smoke-$TAG.stdout" 2>&1
echo "rc=$?"
kill $XP
python3 -I - "$HERE/logs/smoke-$TAG.json" <<'PY'
import json,sys
try: d=json.load(open(sys.argv[1]))
except Exception as e: print("no json",e); sys.exit()
f=sorted(d['frame_ms']); o=d['ops']
print(d['scenario'],"frames",d['frames'],"fps %.1f"%(1000*len(f)/sum(f) if f else 0),"p50 %.1f p95 %.1f max %.1f"%((f[len(f)//2],f[int(len(f)*.95)],f[-1]) if f else (0,0,0)))
for k in ('build_ms','raster_ms'):
    if k in o and 'p50' in o[k]: print(" ",k,"p50 %.2f p95 %.2f"%(o[k]['p50'],o[k]['p95']))
print(" ",{k:v for k,v in o.items() if not isinstance(v,(dict,list)) and k not in('clock_check_main_to_firstframe_ms',)})
PY
