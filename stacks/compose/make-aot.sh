#!/bin/bash
# Variante démarrage (pas la principale) : cache AOT JDK 25 (JEP 483 « AOT class loading & linking », JEP 514 « AOTCacheOutput »
# en une passe ; successeur d'AppCDS/-XX:SharedArchiveFile). Le plugin Compose (createDistributable) n'expose pas d'option CDS ;
# jlink a --generate-cds-archive (archive CDS des classes JDK seulement). Ici : entraînement = 1 exécution du scénario list
# (4 s) avec le runtime jlink de la distribution, sous un Xvfb privé ; le cache est écrit à la sortie de la JVM.
# Usage : make-aot.sh   (puis BENCH_VARIANT=aot run.sh ...)
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$D/build/aot" "$D/logs"
DISP=":$((200 + RANDOM % 500))"
Xvfb "$DISP" -screen 0 1280x720x24 -nolisten tcp 2>/dev/null & XP=$!
trap 'kill $XP 2>/dev/null || true' EXIT
sleep 1
rm -f "$D/build/aot/app.aot" "$D/build/aot/app.aotconf" "$D/build/aot/app.aot.config"
# Mode 2 étapes (JEP 483) : le mode 1 étape (-XX:AOTCacheOutput, JEP 514) lance un processus enfant lib/runtime/bin/java,
# absent du runtime jlink généré par jpackage -> "Cannot run program .../bin/java" (logs/make-aot-onestep.log).
B="${JAVA_TOOL_OPTIONS:+$JAVA_TOOL_OPTIONS }"
{ time DISPLAY="$DISP" JAVA_TOOL_OPTIONS="$B-XX:AOTMode=record -XX:AOTConfiguration=$D/build/aot/app.aotconf" \
    "$D/run.sh" --scenario list --duration 4 --out "$D/build/aot/training.json"
  time DISPLAY="$DISP" JAVA_TOOL_OPTIONS="$B-XX:AOTMode=create -XX:AOTConfiguration=$D/build/aot/app.aotconf -XX:AOTCache=$D/build/aot/app.aot" \
    "$D/run.sh" --scenario list --duration 1 --out "$D/build/aot/create.json"; } > "$D/logs/make-aot.log" 2>&1 || true
ls -la "$D/build/aot/" | tee -a "$D/logs/make-aot.log"
