#!/usr/bin/env bash
# usage : ios-run.sh <app> <outdir>   (simulateur iPhone, runtime iOS le plus recent)
set -u
app="$1"; out="$2"; bid=net.rltbg.spike.avalonia; mkdir -p "$out"
UDID=$(xcrun simctl list devices available -j | python3 -c "
import json,sys
d=json.load(sys.stdin)['devices']
rts=sorted([k for k in d if 'iOS' in k], key=lambda k:[int(x) for x in k.split('iOS-')[1].split('-')])
for k in reversed(rts):
    for dev in d[k]:
        if 'iPhone' in dev['name']: print(dev['udid']); sys.exit()
")
echo "UDID=$UDID" > "$out/sim.txt"; xcrun simctl list devices | grep "$UDID" >> "$out/sim.txt"
xcrun simctl boot "$UDID"; xcrun simctl bootstatus "$UDID" -b > "$out/boot.log" 2>&1
xcrun simctl install "$UDID" "$app" > "$out/install.log" 2>&1 || { echo "INSTALL FAILED" >> "$out/install.log"; exit 1; }
t0=$(python3 -c 'import time;print(time.time())')
xcrun simctl launch --terminate-running-process --stdout="$out/stdout.log" --stderr="$out/stderr.log" "$UDID" "$bid" --autorun > "$out/launch.txt" 2>&1
pid=$(grep -oE '[0-9]+$' "$out/launch.txt" | tail -1)
seen=" "; ready=""; maxrss=0; tick=0; s0=$(date +%s)
while :; do
  cat "$out/stdout.log" "$out/stderr.log" 2>/dev/null | grep -a "SPIKE:" | awk '!x[$0]++ || /fps/' > "$out/run.log"
  [ -z "$ready" ] && grep -q "SPIKE:ready" "$out/run.log" && ready=$(python3 -c "import time;print(round((time.time()-$t0)*1000))")
  for n in tab1-liste tab2-canvas tab3-coeur; do
    case "$seen" in *" $n "*) ;; *) if grep -q "SPIKE:shot $n" "$out/run.log"; then sleep 0.3; xcrun simctl io "$UDID" screenshot "$out/$n.png" > /dev/null 2>&1; seen="$seen$n "; fi ;; esac
  done
  tick=$((tick+1)); if [ -n "${pid:-}" ] && [ $((tick % 3)) -eq 0 ]; then r=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' '); [ -n "${r:-}" ] && [ "$r" -gt "$maxrss" ] && maxrss=$r; fi
  grep -q "SPIKE:done" "$out/run.log" && break
  [ $(( $(date +%s) - s0 )) -gt 150 ] && { echo "TIMEOUT 150 s" >> "$out/run.log"; break; }
  sleep 1
done
[ -n "${pid:-}" ] && { footprint "$pid" > "$out/footprint.txt" 2>&1; ps -o pid,rss,vsz,command -p "$pid" > "$out/ps-end.txt" 2>&1; }
xcrun simctl io "$UDID" screenshot "$out/final.png" > /dev/null 2>&1
echo "pid=${pid:-} cold_start_to_ready_ms=${ready:-none} max_rss_kb=$maxrss wall_s=$(( $(date +%s) - s0 ))" | tee "$out/exit.txt"
xcrun simctl terminate "$UDID" "$bid" > /dev/null 2>&1; xcrun simctl shutdown "$UDID" > /dev/null 2>&1
