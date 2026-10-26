#!/bin/bash
# Short smoke run of every scenario (not an official measurement). Starts its own Xvfb on :77.
cd "$(dirname "$0")"
Xvfb :77 -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XV=$!
sleep 1; export DISPLAY=:77
DUR=8 ./smoke.sh idle idle; DUR=14 ./smoke.sh list list; DUR=8 ./smoke.sh stream1k stream --rate 1000; DUR=8 ./smoke.sh stream10k stream --rate 10000
DUR=8 ./smoke.sh canvas5k canvas; DUR=8 ./smoke.sh canvas5k-render canvas --canvas-impl render
DUR=8 ./smoke.sh canvas20k canvas --shapes 20000; DUR=8 ./smoke.sh canvas20k-render canvas --shapes 20000 --canvas-impl render
DUR=8 ./smoke.sh canvas1k-qmlcanvas canvas --shapes 1000 --canvas-impl qml; DUR=8 ./smoke.sh canvas1k-painted canvas --shapes 1000
DUR=8 ./smoke.sh core core; DUR=10 ./smoke.sh stress stress; DUR=8 ./smoke.sh text text; DUR=8 ./smoke.sh images images; DUR=8 ./smoke.sh nav nav
export LIBGL_ALWAYS_SOFTWARE=1
DUR=8 ./smoke.sh rhi-list list --backend opengl; DUR=8 ./smoke.sh rhi-stream10k stream --rate 10000 --backend opengl; DUR=8 ./smoke.sh rhi-canvas5k canvas --backend opengl
DUR=8 ./smoke.sh rhi-images images --backend opengl; DUR=8 ./smoke.sh rhi-nav nav --backend opengl; DUR=8 ./smoke.sh rhi-text text --backend opengl
DUR=8 ./smoke.sh paced-list list --paced
kill $XV
