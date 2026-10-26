#!/usr/bin/env bash
# usage: run-desktop.sh <linux|windows|macos> <exe> <outdir>   (a lancer sous xvfb-run sur Linux)
# Lance l'app avec le parcours automatique, prend une capture ECRAN (outil de l'OS) a chaque marqueur SPIKE:shot,
# echantillonne le RSS (ps) sur Linux/macOS, attend SPIKE:done / la sortie du processus (SPIKE_EXIT=1).
set -u
plat="$1"; exe="$2"; out="$3"; mkdir -p "$out/inapp"; here="$(cd "$(dirname "$0")" && pwd)"
export SPIKE_AUTORUN=1 SPIKE_EXIT=1 SPIKE_LOG_FILE="$out/run.log" SPIKE_SHOT_DIR="$out/inapp"
: > "$out/run.log"
shot() {
  case "$plat" in
    linux) import -window root "$out/$1.png" ;;
    macos) screencapture -x "$out/$1.png" ;;
    windows) powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$here/shot-win.ps1")" -Path "$(cygpath -w "$out/$1.png")" ;;
  esac
}
t0=$(date +%s)
"$exe" > "$out/stdout.log" 2>&1 &
pid=$!
seen=" "; maxrss=0; tick=0
while kill -0 "$pid" 2>/dev/null; do
  for n in tab1-liste tab2-canvas tab3-coeur; do
    case "$seen" in *" $n "*) ;; *) if grep -q "SPIKE:shot $n" "$out/run.log" 2>/dev/null; then sleep 0.3; shot "$n" >> "$out/shot.log" 2>&1; seen="$seen$n "; fi ;; esac
  done
  tick=$((tick+1))
  if [ $((tick % 10)) -eq 0 ] && [ "$plat" != windows ]; then r=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' '); [ -n "${r:-}" ] && [ "$r" -gt "$maxrss" ] && maxrss=$r; fi
  if [ $(( $(date +%s) - t0 )) -gt 150 ]; then echo "TIMEOUT 150 s : processus tue" >> "$out/run.log"; kill "$pid" 2>/dev/null; break; fi
  sleep 0.2
done
wait "$pid" 2>/dev/null; echo "exit_code=$? wall_s=$(( $(date +%s) - t0 )) max_rss_kb=$maxrss" | tee "$out/exit.txt"
