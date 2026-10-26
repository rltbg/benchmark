#!/usr/bin/env bash
# macOS arm64 : build release + empaquetage .app écrit à la main + exécution. usage: macos.sh <OUT>
set -uo pipefail
OUT=$(realpath -m "$1"); mkdir -p "$OUT"
cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
{ sw_vers; rustc -Vv; cargo -V; xcodebuild -version; } > "$OUT/versions.txt" 2>&1
t0=$(date +%s)
cargo build --release --locked 2>&1 | tee "$OUT/build.log"; rc=${PIPESTATUS[0]}
sec=$(( $(date +%s) - t0 ))
APP="$OUT/SlintSpike.app"
if [ $rc -eq 0 ]; then
  mkdir -p "$APP/Contents/MacOS"
  cp target/release/slintspike "$APP/Contents/MacOS/slintspike"
  cat > "$APP/Contents/Info.plist" <<'PL'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>SlintSpike</string>
<key>CFBundleIdentifier</key><string>dev.spike.slintspike</string>
<key>CFBundleExecutable</key><string>slintspike</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PL
  codesign --force --sign - "$APP" >> "$OUT/build.log" 2>&1   # signature ad hoc locale (pas de signature de magasin)
fi
status=ok; [ $rc -eq 0 ] || status=fail
arts=(); notes=()
if [ $rc -eq 0 ]; then
  arts=(--artifact "$OUT/SlintSpike.app/Contents/MacOS/slintspike" --artifact "$APP")
  export SLINT_BACKEND=winit-skia-software SPIKE_AUTORUN=1 SPIKE_KEEP=1
  T0=$(now_ms)
  "$APP/Contents/MacOS/slintspike" > "$OUT/run.log" 2>&1 &
  APID=$!
  if waitfor "$OUT/run.log" "SPIKE:ready" 90; then echo "cold_start_ms_to_ready=$(( $(now_ms)-T0 ))" > "$OUT/run-metrics.txt"; fi
  waitfor "$OUT/run.log" "SPIKE:tab index=1" 20 && { sleep 1.5; screencapture -x "$OUT/shot-1-list.png"; }
  waitfor "$OUT/run.log" "SPIKE:tab index=2" 20 && { sleep 2; screencapture -x "$OUT/shot-2-canvas.png"; echo "rss_kb_canvas=$(ps -o rss= -p $APID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  waitfor "$OUT/run.log" "SPIKE:done" 30 && { screencapture -x "$OUT/shot-3-core.png"; echo "rss_kb_end=$(ps -o rss= -p $APID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  kill $APID 2>/dev/null
  grep -q 'SPIKE:done' "$OUT/run.log" || { status=fail; notes+=(--note "SPIKE:done absent"); }
  grep -q 'SPIKE:selftest ok' "$OUT/run.log" || { status=fail; notes+=(--note "selftest non ok"); }
  notes+=(--note "execution: session GUI du runner macos-26, SLINT_BACKEND=winit-skia-software, chiffres de CI")
  notes+=(--note "metrics: $(tr '\n' ' ' < "$OUT/run-metrics.txt")")
  notes+=(--note "Info.plist écrit à la main (14 lignes) ; signature ad hoc seulement")
fi
record macos "$status" "$sec" "$OUT" "${arts[@]}" "${notes[@]}"
exit 0
