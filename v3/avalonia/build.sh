#!/bin/bash
# Build NativeAOT (seule variante livree) : Avalonia 12.1.4 + F# + .NET 10 ; libbenchcore.a (core/, Rust C-ABI v2) liee statiquement.
# Sortie : out/ (avalonia-bench + libSkiaSharp.so + libHarfBuzzSharp.so). Logs : logs/publish-aot.log, logs/warnings.txt. Au plus 2 jobs (-m:2).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export DOTNET_ROOT="${DOTNET_ROOT_OVERRIDE:-/opt/dotnet10}" PATH="${DOTNET_ROOT_OVERRIDE:-/opt/dotnet10}:$PATH"
export AVALONIA_TELEMETRY_OPTOUT=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
cd "$HERE"
REQ_SDK=10.0.403
SDK=$(dotnet --version)
[ "$SDK" = "$REQ_SDK" ] || { echo "SDK $SDK != $REQ_SDK (exporter DOTNET_ROOT_OVERRIDE vers un SDK $REQ_SDK)"; exit 1; }
if [ ! -f "$BENCH_ROOT/core/target/release/libbenchcore.a" ]; then
  ( cd "$BENCH_ROOT/core" && CARGO_BUILD_JOBS=2 cargo build --release )
fi
mkdir -p logs
rm -rf out artifacts/aot
( time dotnet publish -c Release -r linux-x64 -p:PublishAot=true -m:2 --artifacts-path artifacts/aot -o out ) > logs/publish-aot.log 2>&1 || { echo "AOT FAIL (voir logs/publish-aot.log)"; tail -30 logs/publish-aot.log; exit 1; }
grep -E "warning|error" logs/publish-aot.log | sed 's/ \[\/.*//' | sort -u > logs/warnings.txt || true
tail -n 4 logs/publish-aot.log
