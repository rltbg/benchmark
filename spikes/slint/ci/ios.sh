#!/usr/bin/env bash
# iOS simulateur arm64 : voie officielle XcodeGen + script cargo, repli .app manuel, puis simctl. usage: ios.sh <OUT>
set -uo pipefail
OUT=$(realpath -m "$1"); mkdir -p "$OUT"
cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
{ sw_vers; rustc -Vv; xcodebuild -version; xcrun simctl list runtimes; } > "$OUT/versions.txt" 2>&1
rustup target add aarch64-apple-ios-sim >> "$OUT/versions.txt" 2>&1
brew install xcodegen >> "$OUT/versions.txt" 2>&1; xcodegen --version >> "$OUT/versions.txt" 2>&1
t0=$(date +%s)
APP=""; ROUTE=""
# voie officielle (docs.slint.dev/guide/platforms/mobile/ios) : xcodegen -> xcodebuild appelle le script cargo
( cd ios && xcodegen && xcodebuild -project SlintSpike.xcodeproj -scheme SlintSpike -configuration Release \
    -sdk iphonesimulator -arch arm64 CODE_SIGNING_ALLOWED=NO -derivedDataPath build ) 2>&1 | tee "$OUT/build.log"
rcx=${PIPESTATUS[0]}
if [ $rcx -eq 0 ] && [ -d ios/build/Build/Products/Release-iphonesimulator/SlintSpike.app ]; then
  APP=$PWD/ios/build/Build/Products/Release-iphonesimulator/SlintSpike.app; ROUTE="xcodegen+xcodebuild"
else
  echo "=== voie XcodeGen en échec (code $rcx) : repli .app assemblé à la main ===" | tee -a "$OUT/build.log"
  cargo build --release --locked --target aarch64-apple-ios-sim 2>&1 | tee -a "$OUT/build.log"
  if [ -f target/aarch64-apple-ios-sim/release/slintspike ]; then
    APP=$PWD/manual/SlintSpike.app; mkdir -p "$APP"; cp target/aarch64-apple-ios-sim/release/slintspike "$APP/"
    cat > "$APP/Info.plist" <<'PL'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>SlintSpike</string>
<key>CFBundleIdentifier</key><string>dev.spike.SlintSpike</string>
<key>CFBundleExecutable</key><string>slintspike</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>MinimumOSVersion</key><string>13.0</string>
<key>UIDeviceFamily</key><array><integer>1</integer><integer>2</integer></array>
<key>CFBundleSupportedPlatforms</key><array><string>iPhoneSimulator</string></array>
<key>UILaunchScreen</key><dict/>
<key>UIApplicationSceneManifest</key><dict>
 <key>UIApplicationSupportsMultipleScenes</key><false/>
 <key>UISceneConfigurations</key><dict><key>UIWindowSceneSessionRoleApplication</key><array><dict>
  <key>UISceneConfigurationName</key><string>Slint</string>
  <key>UISceneDelegateClassName</key><string>SlintWindowSceneDelegate</string>
 </dict></array></dict>
</dict>
</dict></plist>
PL
    codesign --force --sign - "$APP" >> "$OUT/build.log" 2>&1
    ROUTE="manuel(cargo+Info.plist)"
  fi
fi
sec=$(( $(date +%s) - t0 ))
status=ok; [ -n "$APP" ] || status=fail
arts=(); notes=(--note "voie de build: $ROUTE")
if [ -n "$APP" ]; then
  cp -R "$APP" "$OUT/SlintSpike.app"
  arts=(--artifact "$OUT/SlintSpike.app/slintspike" --artifact "$OUT/SlintSpike.app")
  BID=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Info.plist")
  UDID=$(python3 - <<'PY'
import json,subprocess
d=json.loads(subprocess.check_output(["xcrun","simctl","list","devices","available","-j"]))["devices"]
c=[(rt,x["udid"],x["name"]) for rt,l in d.items() if "iOS" in rt for x in l if "iPhone" in x["name"]]
c.sort(key=lambda t:[int(p) if p.isdigit() else 0 for p in t[0].split("iOS-")[-1].split("-")])
print(c[-1][1] if c else "")
PY
)
  echo "simulateur: $UDID bundle: $BID" | tee -a "$OUT/versions.txt"
  xcrun simctl list devices | grep "$UDID" >> "$OUT/versions.txt"
  xcrun simctl boot "$UDID" 2>&1 | tee -a "$OUT/run-setup.log"; xcrun simctl bootstatus "$UDID" -b >> "$OUT/run-setup.log" 2>&1
  xcrun simctl install "$UDID" "$APP" 2>&1 | tee -a "$OUT/run-setup.log"
  T0=$(now_ms)
  xcrun simctl launch --console "$UDID" "$BID" --autorun > "$OUT/run.log" 2>&1 &
  LPID=$!
  if waitfor "$OUT/run.log" "SPIKE:ready" 120; then echo "cold_start_ms_launch_to_ready=$(( $(now_ms)-T0 ))" > "$OUT/run-metrics.txt"; fi
  PID=$(head -3 "$OUT/run.log" | sed -n 's/^[^:]*: \([0-9]*\)$/\1/p' | head -1)
  waitfor "$OUT/run.log" "SPIKE:tab index=1" 20 && { sleep 1.5; xcrun simctl io "$UDID" screenshot "$OUT/shot-1-list.png"; }
  waitfor "$OUT/run.log" "SPIKE:tab index=2" 20 && { sleep 2; xcrun simctl io "$UDID" screenshot "$OUT/shot-2-canvas.png"; [ -n "$PID" ] && echo "rss_kb_canvas=$(ps -o rss= -p $PID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  waitfor "$OUT/run.log" "SPIKE:done" 30 && { xcrun simctl io "$UDID" screenshot "$OUT/shot-3-core.png"; [ -n "$PID" ] && echo "rss_kb_end=$(ps -o rss= -p $PID | tr -d ' ')" >> "$OUT/run-metrics.txt"; }
  xcrun simctl terminate "$UDID" "$BID"; kill $LPID 2>/dev/null
  grep -q 'SPIKE:done' "$OUT/run.log" || { status=fail; notes+=(--note "SPIKE:done absent"); }
  grep -q 'SPIKE:selftest ok' "$OUT/run.log" || { status=fail; notes+=(--note "selftest non ok"); }
  notes+=(--note "execution: simulateur iOS sur runner macos-26 (pas un appareil), chiffres de simulateur")
  notes+=(--note "metrics: $(tr '\n' ' ' < "$OUT/run-metrics.txt" 2>/dev/null)")
fi
record ios-sim "$status" "$sec" "$OUT" "${arts[@]}" "${notes[@]}"
exit 0
