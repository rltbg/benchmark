#!/bin/bash
# Lanceur pour le harnais : run.sh --scenario S --duration D --out F [...]
# Principal : lanceur jpackage build/compose/binaries/main/app/app/bin/app (runtime jlink JDK 25, G1 par défaut ;
#             options JVM dans lib/app/app.cfg : -Dskiko.renderApi=SOFTWARE_FAST --enable-native-access=ALL-UNNAMED -Dsun.java2d.uiScale=1).
# Variantes (jamais la principale) :
#   BENCH_VARIANT=tuned  -> -Xms512m -Xmx1g -XX:+UseSerialGC -XX:-UsePerfData
#   BENCH_VARIANT=aot    -> -XX:AOTCache=build/aot/app.aot (cache AOT/CDS JDK 25, JEP 483/514 ; créé par make-aot.sh)
#   BENCH_VSYNC=1        -> -Dskiko.vsync.enabled=true (limiteur Skiko au taux de rafraîchissement, défaut Skiko)
#   BENCH_MODE=jar       -> uber-jar + JDK 25 système ($JAVA_HOME_25)
# Le runtime jlink de jpackage n'a pas de bin/java : les options de variante passent par JAVA_TOOL_OPTIONS (cumulatif).
D="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$D/../.." && pwd)}"
export BENCHCORE_LIB="${BENCHCORE_LIB:-$BENCH_ROOT/core/target/release/libbenchcore.so}"
export SKIKO_RENDER_API="${SKIKO_RENDER_API:-SOFTWARE_FAST}"
APP="$D/build/compose/binaries/main/app/app"
X=""
case "${BENCH_VARIANT:-default}" in
  default) ;;
  tuned) X="-Xms512m -Xmx1g -XX:+UseSerialGC -XX:-UsePerfData" ;;
  aot) X="-XX:AOTCache=$D/build/aot/app.aot" ;;
  *) echo "BENCH_VARIANT inconnu: $BENCH_VARIANT" >&2; exit 2 ;;
esac
[ "${BENCH_VSYNC:-0}" = 1 ] && X="$X -Dskiko.vsync.enabled=true"
if [ "${BENCH_MODE:-dist}" = "jar" ]; then
  J="${JAVA_HOME_25:-/usr/lib/jvm/java-25-openjdk-amd64}/bin/java"
  exec "$J" -Dskiko.renderApi=SOFTWARE_FAST --enable-native-access=ALL-UNNAMED -Dsun.java2d.uiScale=1 $X \
    -cp "$D/build/compose/jars/app-linux-x64-1.0.0.jar" bench.MainKt "$@"
fi
[ -n "$X" ] && export JAVA_TOOL_OPTIONS="${JAVA_TOOL_OPTIONS:+$JAVA_TOOL_OPTIONS }$X"
exec "$APP/bin/app" "$@"
