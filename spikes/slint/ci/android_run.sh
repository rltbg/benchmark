#!/usr/bin/env bash
# Exécuté DANS l'émulateur (android-emulator-runner) : installe, lance, relève marqueurs, captures, mémoire. usage: android_run.sh <OUT>
set -uo pipefail
OUT=$(realpath -m "$1"); cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
APK="$OUT/slintspike.apk"
BT=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)
PKG=$("$BT/aapt2" dump badging "$APK" | sed -n "s/^package: name='\([^']*\)'.*/\1/p")
ACT=$("$BT/aapt2" dump badging "$APK" | sed -n "s/^launchable-activity: name='\([^']*\)'.*/\1/p" | head -1)
echo "pkg=$PKG activity=$ACT" | tee "$OUT/run-setup.log"
adb shell getprop ro.build.version.release >> "$OUT/run-setup.log"; adb shell getprop ro.product.cpu.abi >> "$OUT/run-setup.log"
adb install -r "$APK" 2>&1 | tee -a "$OUT/run-setup.log"
adb logcat -c
adb logcat -v time > "$OUT/logcat-full.log" 2>&1 &
adb logcat -s SPIKE:I -v raw > "$OUT/run.log" 2>&1 &
sleep 1
adb shell am start -W -n "$PKG/$ACT" --es autorun 1 2>&1 | tee "$OUT/am-start.log"
T0=$(now_ms)
waitfor "$OUT/run.log" "SPIKE:ready" 120 && echo "ready_ms_after_am_start_returned=$(( $(now_ms)-T0 ))" > "$OUT/run-metrics.txt"
grep -E 'TotalTime|WaitTime|ThisTime' "$OUT/am-start.log" | tr '\n' ' ' >> "$OUT/run-metrics.txt"; echo >> "$OUT/run-metrics.txt"
waitfor "$OUT/run.log" "SPIKE:tab index=1" 20 && { sleep 1.5; adb exec-out screencap -p > "$OUT/shot-1-list.png"; }
waitfor "$OUT/run.log" "SPIKE:tab index=2" 20 && { sleep 2; adb exec-out screencap -p > "$OUT/shot-2-canvas.png"; adb shell dumpsys meminfo "$PKG" > "$OUT/meminfo-canvas.txt"; }
waitfor "$OUT/run.log" "SPIKE:done" 30 && { adb exec-out screencap -p > "$OUT/shot-3-core.png"; adb shell dumpsys meminfo "$PKG" > "$OUT/meminfo-end.txt"; }
for f in canvas end; do echo "total_pss_kb_$f=$(grep -m1 'TOTAL PSS' "$OUT/meminfo-$f.txt" 2>/dev/null | awk '{print $3}')" >> "$OUT/run-metrics.txt"; done
adb shell dumpsys SurfaceFlinger --list > /dev/null 2>&1
adb shell am force-stop "$PKG"
sleep 1; pkill -f "adb logcat" || true
