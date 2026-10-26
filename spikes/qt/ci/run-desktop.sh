#!/usr/bin/env bash
# Lance l'app de bureau en parcours automatique, attend SPIKE:done, mesure démarrage à froid et RAM.
# usage: run-desktop.sh <exécutable> <dossier-sortie> <étiquette> [variables d'env supplémentaires K=V ...]
# Sortie : <dossier>/run-<étiquette>.log, <dossier>/<étiquette>-tab*.png, <dossier>/run-<étiquette>.json
set -u
exe="$1"; out="$2"; label="$3"; shift 3
pname=$(basename "$exe" .exe)
mkdir -p "$out/shots-$label"; log="$out/run-$label.log"; : > "$log"
rss_kb() {   # RSS résident en Ko
  case "$(uname -s)" in
    Linux) awk '/VmRSS/{print $2}' "/proc/$1/status" 2>/dev/null ;;
    Darwin) ps -o rss= -p "$1" 2>/dev/null | tr -d ' ' ;;
    *) powershell -NoProfile -Command "[int]((Get-Process -Name '$pname' -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum/1024)" 2>/dev/null | tr -d '\r' ;;
  esac
}
now_ms() { local n; n=$(date +%s%3N); case "$n" in *N) n=$(python3 -c 'import time;print(int(time.time()*1000))');; esac; echo "$n"; }
t0=$(now_ms)
env SPIKE_AUTORUN=1 SPIKE_SHOT_DIR="$out/shots-$label" "$@" "$exe" > "$log" 2>&1 &
pid=$!
ready_ms=""; peak=0; deadline=$(( $(date +%s) + 90 ))
while kill -0 "$pid" 2>/dev/null && [ "$(date +%s)" -lt "$deadline" ]; do
  if [ -z "$ready_ms" ] && grep -q '^SPIKE:ready' "$log"; then ready_ms=$(( $(now_ms) - t0 )); fi
  r=$(rss_kb "$pid"); [ -n "${r:-}" ] && [ "$r" -gt "$peak" ] 2>/dev/null && peak=$r
  if grep -q '^SPIKE:done' "$log"; then break; fi
  sleep 0.2
done
sleep 1; r=$(rss_kb "$pid"); [ -n "${r:-}" ] && [ "$r" -gt "$peak" ] 2>/dev/null && peak=$r
kill "$pid" 2>/dev/null; sleep 1; kill -9 "$pid" 2>/dev/null
case "$(uname -s)" in MINGW*|MSYS*) taskkill //F //IM "$pname.exe" >/dev/null 2>&1 ;; esac
done_marker=$(grep -c '^SPIKE:done' "$log")
for f in "$out/shots-$label"/*.png; do [ -f "$f" ] && cp "$f" "$out/$label-$(basename "$f")"; done
rm -rf "$out/shots-$label"
printf '{"label":"%s","cold_start_to_ready_ms":%s,"rss_peak_kb":%s,"done":%s}\n' "$label" "${ready_ms:-null}" "$peak" "$done_marker" | tee "$out/run-$label.json"
grep '^SPIKE:' "$log" | head -40
[ "$done_marker" -ge 1 ]
