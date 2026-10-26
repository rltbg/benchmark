#!/usr/bin/env bash
# Enregistre result.json Android. usage: android_record.sh <OUT> <secondes_build>
set -uo pipefail
OUT=$(realpath -m "$1"); SEC=$2; cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
status=ok; arts=(); notes=()
if [ -f "$OUT/slintspike.apk" ]; then
  arts=(--artifact "$OUT/slintspike.apk"); [ -f "$OUT/libslintspike-arm64-v8a.so" ] && arts+=(--artifact "$OUT/libslintspike-arm64-v8a.so")
else status=fail; notes+=(--note "aucun APK produit"); fi
notes+=(--note "voie de build: $(cat "$OUT/build-route.txt" 2>/dev/null)")
if [ -f "$OUT/run.log" ]; then
  grep -q 'SPIKE:done' "$OUT/run.log" || { status=fail; notes+=(--note "SPIKE:done absent"); }
  grep -q 'SPIKE:selftest ok' "$OUT/run.log" || { status=fail; notes+=(--note "selftest non ok"); }
  notes+=(--note "execution: émulateur Android x86_64 (KVM, swiftshader), chiffres d'émulateur et non d'appareil")
  notes+=(--note "metrics: $(tr '\n' ' ' < "$OUT/run-metrics.txt" 2>/dev/null)")
else notes+=(--note "pas d'exécution (run.log absent)"); [ "$status" = ok ] && status=fail; fi
record android "$status" "$SEC" "$OUT" "${arts[@]}" "${notes[@]}"
