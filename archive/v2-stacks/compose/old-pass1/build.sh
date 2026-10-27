#!/bin/bash
# Build : distribution (jlink runtime + jars) et uber-jar. Max 2 workers.
set -e
cd "$(dirname "$0")"
mkdir -p logs
( time gradle --no-daemon --max-workers=2 createDistributable packageUberJarForCurrentOS ) 2>&1 | tee logs/build-final.log
echo "dist: build/compose/binaries/main/app/app/bin/app   uberjar: build/compose/jars/app-linux-x64-1.0.0.jar"
