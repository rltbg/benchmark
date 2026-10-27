#!/bin/sh
# usage: smoke.sh scenario duration [extra args]   (Xvfb prive, ne tue que son PID)
D=$((200 + $$ % 400)); sc=$1; dur=$2; shift 2
Xvfb :$D -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XP=$!
sleep 1
DISPLAY=:$D LIBGL_ALWAYS_SOFTWARE=1 ${BIN:-./target/release/iced_bench} --scenario $sc --duration $dur --out /tmp/iced-$sc.json --assets /home/user/eval/assets "$@" > /tmp/iced-$sc.log 2>&1
echo "rc=$?"; kill $XP
python3 -I - <<PY
import json
d=json.load(open("/tmp/iced-$sc.json")); f=d["frame_ms"]
print("$sc frames",d["frames"], "fps", (1000*len(f)/sum(f) if f else 0), "p95", sorted(f)[int(len(f)*.95)] if f else None, "max", max(f) if f else None)
print(d["ops"] if len(str(d["ops"]))<900 else str(d["ops"])[:900])
PY
head -5 /tmp/iced-$sc.log
