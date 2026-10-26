#!/bin/bash
# Short direct smoke of every scenario/variant (not an official measurement). Starts its own Xvfb on :251 (killed by PID only).
cd "$(dirname "$0")"
Xvfb :251 -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XV=$!
sleep 1; export DISPLAY=:251
{
DUR=6 ./smoke.sh idle idle; DUR=8 ./smoke.sh list list; DUR=8 ./smoke.sh stream1k stream --rate 1000; DUR=8 ./smoke.sh stream10k stream --rate 10000
DUR=8 ./smoke.sh canvas5k canvas; DUR=8 ./smoke.sh canvas5k-render canvas --canvas-impl render; DUR=8 ./smoke.sh canvas20k canvas --shapes 20000
DUR=6 ./smoke.sh canvas1k-qmlcanvas canvas --shapes 1000 --canvas-impl qml; DUR=6 ./smoke.sh canvas1k canvas --shapes 1000
DUR=8 ./smoke.sh core core; DUR=10 ./smoke.sh stress stress; DUR=8 ./smoke.sh text text; DUR=8 ./smoke.sh images images; DUR=10 ./smoke.sh nav nav; DUR=8 ./smoke.sh input input
export LIBGL_ALWAYS_SOFTWARE=1
for s in list canvas images nav text input; do DUR=8 ./smoke.sh gl-$s $s --backend opengl; done
DUR=8 ./smoke.sh paced-list list --paced
} 2>&1 | tee logs/smoke.txt
kill $XV
