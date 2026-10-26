#!/bin/bash
# Micro-bench Skia raster pur (sans Compose) : 63 vignettes 128x128 dessinées avec canvas.scale(s) ; compare des versions de Skiko.
# usage: diag/run-skbench.sh <skiko-awt.jar> <skiko-awt-runtime-linux-x64.jar> <kotlin-stdlib.jar> <kotlinx-coroutines-core-jvm.jar>
D="$(cd "$(dirname "$0")" && pwd)"; ROOT="${BENCH_ROOT:-$(cd "$D/../../.." && pwd)}"
env -u JAVA_TOOL_OPTIONS "${JAVA_HOME_25:-/usr/lib/jvm/java-25-openjdk-amd64}/bin/java" -cp "$1:$2:$3:$4" "$D/SkBench.java" "$ROOT/assets/images/img000.png"
