#!/usr/bin/env bash
# Exécuté DANS l'émulateur (reactivecircus/android-emulator-runner) : installe l'APK, lance, mesure.
# usage: android-run.sh <apk> <dossier-sortie>
set -u
apk="$1"; out="$2"; mkdir -p "$out"
adb wait-for-device
adb shell getprop ro.build.version.release | tee "$out/android-version.txt"
adb shell getprop ro.product.cpu.abi | tee "$out/android-abi.txt"
adb install -r "$apk" 2>&1 | tee "$out/install.log"
BT=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)
pkg=$("$BT/aapt2" dump packagename "$apk"); echo "package=$pkg"
adb logcat -c
# démarrage à froid : am start -W (TotalTime) avec l'extra autorun
adb shell am force-stop "$pkg"
act=$(adb shell cmd package resolve-activity --brief -c android.intent.category.LAUNCHER "$pkg" | tail -1 | tr -d '\r')
echo "activité=$act"
adb shell am start -W -n "$act" --ez autorun true 2>&1 | tr -d '\r' | tee "$out/am-start.txt"
# attend SPIKE:done (90 s)
for i in $(seq 1 90); do
  adb logcat -d -s SPIKE:I 2>/dev/null | grep -q 'SPIKE:done' && break
  [ "$i" = 6 ] && adb exec-out screencap -p > "$out/android-early.png"
  sleep 1
done
adb logcat -d -s SPIKE:I > "$out/run.log" 2>&1
adb exec-out screencap -p > "$out/android-final.png"
adb shell dumpsys meminfo "$pkg" > "$out/meminfo.txt" 2>&1
adb logcat -d > "$out/logcat-full.log" 2>&1
grep -E 'SPIKE:' "$out/run.log" | head -40
# captures par onglet, calées sur les marqueurs (relance : le parcours dure ~8 s après SPIKE:autorun start)
adb shell am force-stop "$pkg"; adb logcat -c
adb shell am start -n "$act" --ez autorun true >/dev/null 2>&1
waitm() { for i in $(seq 1 $2); do adb logcat -d -s SPIKE:I | grep -q "$1" && return 0; sleep 0.5; done; return 1; }
waitm 'SPIKE:autorun start' 80; sleep 1.5; adb exec-out screencap -p > "$out/android-tab1-list.png"
sleep 3; adb exec-out screencap -p > "$out/android-tab2-canvas.png"
waitm 'SPIKE:heavy done' 40; sleep 0.5; adb exec-out screencap -p > "$out/android-tab3-core.png"
adb logcat -d -s SPIKE:I > "$out/run2.log" 2>&1
grep -q 'SPIKE:done' "$out/run.log"
