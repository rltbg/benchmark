#!/bin/bash
# Lanceur pour le harnais (usage: run.sh --scenario S --duration D --out F ...).
# BENCH_VARIANT=default|tuned   BENCH_MODE=dist (défaut, runtime jlink)|jar (uber-jar + JDK système)
# Rendu : SKIKO_RENDER_API=SOFTWARE_FAST (variable d'env, défaut ci-dessous ; aussi -Dskiko.renderApi dans app.cfg)
D="$(cd "$(dirname "$0")" && pwd)"
APP="$D/build/compose/binaries/main/app/app"
export SKIKO_RENDER_API="${SKIKO_RENDER_API:-SOFTWARE_FAST}"
COMMON="--enable-preview --enable-native-access=ALL-UNNAMED -Dsun.java2d.uiScale=1"
TUNED="-Xms512m -Xmx1g -XX:+UseSerialGC -XX:-UsePerfData -Xshare:auto"
if [ "${BENCH_MODE:-dist}" = "jar" ]; then
  J=java; CP="$D/build/compose/jars/app-linux-x64-1.0.0.jar"
  [ "${BENCH_VARIANT:-default}" = tuned ] && O="$TUNED" || O=""
  exec $J $COMMON $O $BENCH_EXTRA -cp "$CP" bench.MainKt "$@"
fi
if [ "${BENCH_VARIANT:-default}" = tuned ]; then
  # le runtime jlink de jpackage n'a pas de bin/java : on passe les flags tunés par JAVA_TOOL_OPTIONS (cumulatif avec l'env existant)
  export JAVA_TOOL_OPTIONS="$JAVA_TOOL_OPTIONS $TUNED"
fi
# défaut : lanceur jpackage (bin/app) ; options JVM dans lib/app/app.cfg (--enable-preview, etc.), JVM par défaut (G1)
exec "$APP/bin/app" "$@"
