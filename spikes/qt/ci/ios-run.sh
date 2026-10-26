#!/usr/bin/env bash
# Simulateur iOS (macos-26) : démarre un iPhone, installe le .app, lance avec --autorun, capture.
# usage: ios-run.sh <app> <dossier-sortie> <étiquette> [variables du processus enfant, préfixées par SIMCTL_CHILD_ ...]
set -u
app="$1"; out="$2"; label="$3"; mkdir -p "$out"
log="$out/run-$label.log"
xcrun simctl list runtimes available | tee "$out/simctl-runtimes.txt"
# premier iPhone disponible sur le dernier runtime iOS
udid=$(xcrun simctl list devices available -j | python3 -c "
import json,sys
d=json.load(sys.stdin)['devices']
for rt in sorted(d, reverse=True):
    if 'iOS' in rt:
        for x in d[rt]:
            if x['name'].startswith('iPhone'): print(x['udid']); sys.exit(0)")
echo "udid=$udid"
xcrun simctl boot "$udid" 2>&1 || true
xcrun simctl bootstatus "$udid" -b 2>&1 | tail -2
xcrun simctl install "$udid" "$app" || exit 1
bid=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Info.plist"); echo "bundle=$bid"
: > "$log"
t0=$(python3 -c 'import time;print(int(time.time()*1000))')
# --console-pty relaie stdout/stderr de l'app (marqueurs SPIKE: sur stderr)
xcrun simctl launch --console-pty "$udid" "$bid" --autorun > "$log" 2>&1 &
lp=$!
ready=""; as=""; for i in $(seq 1 600); do
  if [ -z "$ready" ] && grep -q 'SPIKE:ready' "$log"; then ready=$(( $(python3 -c 'import time;print(int(time.time()*1000))') - t0 )); fi
  if [ -z "$as" ] && grep -q 'SPIKE:autorun start' "$log"; then as=$i; fi   # captures calées sur le marqueur (pas de 0,2 s)
  [ -n "$as" ] && [ "$i" = $((as+8)) ] && xcrun simctl io "$udid" screenshot "$out/$label-tab1-list.png" >/dev/null 2>&1
  [ -n "$as" ] && [ "$i" = $((as+23)) ] && xcrun simctl io "$udid" screenshot "$out/$label-tab2-canvas.png" >/dev/null 2>&1
  grep -q 'SPIKE:done' "$log" && break
  sleep 0.2
done
sleep 0.3; xcrun simctl io "$udid" screenshot "$out/$label-tab3-core.png" >/dev/null 2>&1
pid=$(xcrun simctl spawn "$udid" launchctl list | awk -v b="$bid" '$3 ~ b {print $1}' | head -1)
footprint=""; [ -n "$pid" ] && footprint=$( (footprint "$pid" 2>/dev/null || ps -o rss= -p "$pid") | tr '\n' ' ')
echo "{\"label\":\"$label\",\"cold_start_to_ready_ms\":${ready:-null},\"pid\":\"$pid\",\"footprint\":\"$footprint\"}" | tee "$out/run-$label.json"
xcrun simctl terminate "$udid" "$bid" 2>/dev/null; kill $lp 2>/dev/null
xcrun simctl spawn "$udid" log show --last 2m --predicate 'eventMessage CONTAINS "SPIKE:"' --style compact > "$out/oslog-$label.log" 2>&1
grep 'SPIKE:' "$log" | head -40
grep -q 'SPIKE:done' "$log"
