#!/usr/bin/env bash
# Linux x64 : build release (Skia, FemtoVG, cœur Rust) puis exécution sous Xvfb. usage: linux.sh <OUT>
set -uo pipefail
OUT=$(realpath -m "$1"); mkdir -p "$OUT"
cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
{ rustc -Vv; cargo -V; clang --version | head -1; gcc --version | head -1; lsb_release -ds; } > "$OUT/versions.txt" 2>&1
sudo apt-get update -qq
sudo apt-get install -y -qq libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxcb1-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev libgl1-mesa-dri xvfb imagemagick xdotool >> "$OUT/apt.log" 2>&1
export CARGO_TERM_COLOR=never

# --- build 1 : variante principale = renderer Skia + cœur Pulse (C) ---
t0=$(date +%s)
cargo build --release --locked 2>&1 | tee "$OUT/build.log"; rc=${PIPESTATUS[0]}
sec=$(( $(date +%s) - t0 ))
BIN=target/release/slintspike
[ $rc -eq 0 ] && cp "$BIN" "$OUT/slintspike-skia-pulse" && cargo tree --locked -e normal --depth 1 > "$OUT/cargo-tree.txt" 2>&1

# --- variantes de comparaison (taille seulement) ---
if [ $rc -eq 0 ]; then
  t1=$(date +%s)
  cargo build --release --locked --no-default-features --features core-pulse,femtovg 2>&1 | tee "$OUT/build-femtovg.log"
  [ -f "$BIN" ] && cp "$BIN" "$OUT/slintspike-femtovg-pulse"
  echo "build femtovg: $(( $(date +%s) - t1 )) s" >> "$OUT/notes.txt"
  t1=$(date +%s)
  cargo build --release --locked --no-default-features --features core-rust,skia 2>&1 | tee "$OUT/build-corerust.log"
  [ -f "$BIN" ] && cp "$BIN" "$OUT/slintspike-skia-rust"
  echo "build skia + coeur rust: $(( $(date +%s) - t1 )) s" >> "$OUT/notes.txt"
fi

status=ok; [ $rc -eq 0 ] || status=fail
arts=(); notes=()
if [ $rc -eq 0 ]; then
  arts=(--artifact "$OUT/slintspike-skia-pulse")
  for f in slintspike-femtovg-pulse slintspike-skia-rust; do [ -f "$OUT/$f" ] && arts+=(--artifact "$OUT/$f"); done
  notes+=(--note "artefacts dans l'ordre : skia+pulse (principal), femtovg+pulse, skia+coeur rust")
  notes+=(--note "$(cat "$OUT/notes.txt" 2>/dev/null | tr '\n' ';')")
fi

# --- exécution sous Xvfb (rendu logiciel Skia, aucune GPU) ---
if [ $rc -eq 0 ]; then
  Xvfb :99 -screen 0 1280x800x24 >/dev/null 2>&1 &
  XPID=$!; export DISPLAY=:99; sleep 2
  export SLINT_BACKEND=winit-skia-software SPIKE_AUTORUN=1 SPIKE_KEEP=1
  : > "$OUT/run.log"
  T0=$(now_ms)
  "$OUT/slintspike-skia-pulse" > "$OUT/run.log" 2>&1 &
  APID=$!
  if waitfor "$OUT/run.log" "SPIKE:ready" 60; then
    T1=$(now_ms); echo "cold_start_ms_to_ready=$((T1-T0))" > "$OUT/run-metrics.txt"
  else echo "ready absent" >> "$OUT/run-metrics.txt"; fi
  waitfor "$OUT/run.log" "SPIKE:tab index=1" 20 && { sleep 1.5; import -window root "$OUT/shot-1-list.png"; }
  waitfor "$OUT/run.log" "SPIKE:tab index=2" 20 && { sleep 2; import -window root "$OUT/shot-2-canvas.png"; echo "rss_kb_canvas=$(ps -o rss= -p $APID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  waitfor "$OUT/run.log" "SPIKE:done" 30 && { import -window root "$OUT/shot-3-core.png"; echo "rss_kb_end=$(ps -o rss= -p $APID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  # about
  kill $APID 2>/dev/null; wait $APID 2>/dev/null
  # variante coeur Rust : même autotest, marqueurs seulement
  env -u SPIKE_KEEP timeout 40 "$OUT/slintspike-skia-rust" > "$OUT/run-corerust.log" 2>&1
  kill $XPID 2>/dev/null
  grep -c '^SPIKE:' "$OUT/run.log" | sed 's/^/spike_lines=/' >> "$OUT/run-metrics.txt"
  grep -q 'SPIKE:done' "$OUT/run.log" || { status=fail; notes+=(--note "SPIKE:done absent du run.log"); }
  grep -q 'SPIKE:selftest ok' "$OUT/run.log" || { status=fail; notes+=(--note "selftest non ok"); }
  notes+=(--note "execution: Xvfb 1280x800, SLINT_BACKEND=winit-skia-software (rendu CPU), chiffres de CI et non d'appareil")
  notes+=(--note "metrics: $(tr '\n' ' ' < "$OUT/run-metrics.txt")")
fi
record linux "$status" "$sec" "$OUT" "${arts[@]}" "${notes[@]}"
exit 0
