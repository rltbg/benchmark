#!/bin/bash
# Build the Qt 6 (Qt Quick) bench app: CMake + Ninja, Release (-O2), system Qt 6.4.2, max 2 jobs.
set -e
cd "$(dirname "$0")"
mkdir -p logs
{ time (cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build -j2 -v) ; } > logs/build.log 2>&1 || { tail -40 logs/build.log; exit 1; }
tail -5 logs/build.log
ls -la build/qtbench
