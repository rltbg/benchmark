#!/bin/bash
# poke.sh <dev|aot> <keys|resize|type|wheel> <scenario> <duration> [args] : lance l'app, envoie des evenements X apres le marqueur, affiche quelques ops
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="$(cd "$HERE/../.." && pwd)" DISPLAY=:200 LIBGL_ALWAYS_SOFTWARE=1 DOTNET_ROOT=/opt/dotnet10
v=$1; mode=$2; sc=$3; d=$4; shift 4
OUT=/tmp/poke-$sc.json; LOG=/tmp/poke-$sc.log; rm -f $OUT
if [ "$v" = dev ]; then BIN="/opt/dotnet10/dotnet $HERE/dev/avalonia-bench.dll"; else BIN="$HERE/out/avalonia-bench"; fi
$BIN --scenario $sc --duration $d --out $OUT --assets $BENCH_ROOT/assets "$@" > $LOG 2>&1 &
PID=$!
for i in $(seq 1 200); do grep -q measure_start $LOG && break; sleep 0.1; done
W=$(xdotool search --pid $PID 2>/dev/null | head -1)
xdotool mousemove 640 360; [ -n "$W" ] && xdotool windowfocus $W 2>/dev/null
sleep 0.5
case $mode in
 keys) for i in 1 2 3 4 5; do xdotool key space; sleep 0.2; done;;
 type) for i in $(seq 1 30); do xdotool key a; sleep 0.1; done;;
 wheel) for i in $(seq 1 10); do xdotool click 5; sleep 0.1; done;;
 resize) for i in $(seq 1 10); do xdotool windowsize $W 1100 650; sleep 0.25; xdotool windowsize $W 1280 720; sleep 0.25; done;;
esac
wait $PID
python3 -I - $OUT <<'PY'
import json,sys
d=json.load(open(sys.argv[1])); o=d['ops']; f=d['frame_ms']
print('frames',len(f),'rc ok', {k:o.get(k) for k in ('key_events','text_entered','resize_frame_ms','final_offset','nodes')})
PY
