#!/usr/bin/env bash
# usage : android-run.sh <apk> <outdir>   (dans le script de android-emulator-runner : emulateur deja demarre)
set -u
apk="$1"; out="$2"; pkg=net.rltbg.spike.avalonia; mkdir -p "$out"
adb devices > "$out/adb-devices.txt"; adb shell getprop ro.build.version.sdk > "$out/api-level.txt"; adb shell getprop ro.product.cpu.abi >> "$out/api-level.txt"
adb install -r "$apk" > "$out/install.log" 2>&1 || { echo "INSTALL FAILED" >> "$out/install.log"; exit 1; }
adb logcat -c
act=$(adb shell cmd package resolve-activity --brief "$pkg" | tail -1 | tr -d '\r')
echo "activite=$act" > "$out/am-start.txt"
t0=$(date +%s)
adb shell am start -W -n "$act" --es autorun 1 >> "$out/am-start.txt" 2>&1
seen=" "; last=0
while :; do
  adb logcat -d -s SPIKE:I 2>/dev/null | sed -n -E 's/^.* [VDIWEF] SPIKE *: //p' | tr -d '\r' > "$out/run.log"
  for n in tab1-liste tab2-canvas tab3-coeur; do
    case "$seen" in *" $n "*) ;; *) if grep -q "SPIKE:shot $n" "$out/run.log"; then sleep 0.3; adb exec-out screencap -p > "$out/$n.png"; seen="$seen$n "; fi ;; esac
  done
  if grep -q "SPIKE:done" "$out/run.log"; then break; fi
  [ $(( $(date +%s) - t0 )) -gt 150 ] && { echo "TIMEOUT 150 s" >> "$out/run.log"; break; }
  sleep 1
done
adb shell dumpsys meminfo "$pkg" > "$out/meminfo-end.txt" 2>&1
adb shell ps -A -o PID,RSS,NAME 2>/dev/null | grep "$pkg" > "$out/ps-end.txt"
adb logcat -d > "$out/logcat-full.log" 2>&1
adb logcat -d -b crash > "$out/logcat-crash.log" 2>&1
adb exec-out screencap -p > "$out/final.png"
echo "wall_s=$(( $(date +%s) - t0 ))" > "$out/exit.txt"
