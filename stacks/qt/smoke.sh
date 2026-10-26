#!/bin/bash
# usage: smoke.sh tag scenario [extra args...]  (DISPLAY must point to a running Xvfb 1280x720x24; DUR = duration s)
tag=$1; sc=$2; shift 2
mkdir -p logs/smoke
timeout 90 ./build/qtbench --scenario $sc --duration ${DUR:-6} --out logs/smoke/$tag.json --assets /home/user/eval/assets "$@" 2>&1 | grep -v XDG_RUNTIME | head -20
python3 -I - <<PY
import json
d=json.load(open("logs/smoke/$tag.json")); f=d["frame_ms"]
s=sorted(f)
print("%-18s frames=%-6d fps=%-8.1f p50=%.2f p95=%.2f max=%.1f"%("$tag",len(f),1000*len(f)/sum(f) if f else 0,s[len(s)//2] if f else 0,s[int(len(s)*.95)] if f else 0,max(f) if f else 0), {k:(round(v,1) if isinstance(v,float) else v) for k,v in d["ops"].items() if k not in("qt_version","sum_heights","graphics_api")})
PY
