#!/bin/sh
# usage: shot.sh scenario outpng after_s [args]   (Xvfb prive, ne tue que son PID)
D=$((200 + $$ % 400)); sc=$1; png=$2; w=${3:-5}; shift 3
Xvfb :$D -screen 0 1280x720x24 -nolisten tcp >/dev/null 2>&1 & XP=$!
sleep 1
DISPLAY=:$D ${BIN:-./target/release/iced_bench} --scenario $sc --duration 30 --out /tmp/iced-shot.json --assets /home/user/eval/assets "$@" > /tmp/iced-shot.log 2>&1 & AP=$!
sleep $w
DISPLAY=:$D import -window root $png
kill $AP; kill $XP
