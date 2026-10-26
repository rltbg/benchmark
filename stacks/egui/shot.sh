#!/bin/sh
# usage: shot.sh scenario outpng [after_s] [args]
D=$((200 + $$ % 400)); sc=$1; png=$2; w=${3:-5}; shift 3
Xvfb :$D -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XP=$!
sleep 1
DISPLAY=:$D LIBGL_ALWAYS_SOFTWARE=1 ./target/release/egui_bench --scenario $sc --duration 30 --out /tmp/egui-shot.json --assets /home/user/eval/assets "$@" > /tmp/egui-shot.log 2>&1 & AP=$!
sleep $w
DISPLAY=:$D import -window root $png
kill $AP; kill $XP
