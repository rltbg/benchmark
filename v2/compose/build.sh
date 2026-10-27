#!/bin/bash
# Build release reproductible : distribution jpackage/jlink (runtime JDK 25) + uber-jar. Versions épinglées dans build.gradle.kts.
# Outils : Gradle 9.8.2 ($GRADLE, défaut /opt/gradle-9.8.2/bin/gradle), JDK 25 ($JAVA_HOME, défaut /usr/lib/jvm/java-25-openjdk-amd64).
# Max 2 workers (machine partagée).
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"; cd "$D"
export JAVA_HOME="${JAVA_HOME_25:-/usr/lib/jvm/java-25-openjdk-amd64}"
GRADLE="${GRADLE:-/opt/gradle-9.8.2/bin/gradle}"
"$GRADLE" --version | grep -q '^Gradle 9.8.2$' || { echo "Gradle 9.8.2 requis ($GRADLE)"; exit 1; }
"$JAVA_HOME/bin/java" -version 2>&1 | grep -q 'version "25' || { echo "JDK 25 requis ($JAVA_HOME)"; exit 1; }
mkdir -p logs
( time "$GRADLE" --no-daemon --max-workers=2 -Dorg.gradle.java.installations.paths="$JAVA_HOME" \
    createDistributable packageUberJarForCurrentOS ) 2>&1 | tee logs/build-final.log
echo "dist: build/compose/binaries/main/app/app/bin/app   uberjar: build/compose/jars/app-linux-x64-1.0.0.jar"
