#!/bin/bash
# Build Avalonia 12.1.4 + F# + .NET 10 : variante NativeAOT (libbenchcore.a liee statiquement) et variante JIT
# (self-contained, libbenchcore.so resolue a l'execution via BENCHCORE_LIB puis $BENCH_ROOT/core/target/release).
# Sorties : out/aot/, out/jit/. Logs bruts dans logs/. Au plus 2 jobs (-m:2).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export DOTNET_ROOT="${DOTNET_ROOT_OVERRIDE:-/opt/dotnet10}" PATH="${DOTNET_ROOT_OVERRIDE:-/opt/dotnet10}:$PATH"
export AVALONIA_TELEMETRY_OPTOUT=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
cd "$HERE"
REQ_SDK=10.0.403
SDK=$(dotnet --version)
[ "$SDK" = "$REQ_SDK" ] || { echo "SDK $SDK != $REQ_SDK (exporter DOTNET_ROOT_OVERRIDE vers un SDK $REQ_SDK)"; exit 1; }
[ -f "$BENCH_ROOT/core/target/release/libbenchcore.a" ] || { echo "libbenchcore.a absente : construire core/ d'abord"; exit 1; }
mkdir -p logs
rm -rf out artifacts
st=0
( time dotnet publish -c Release -r linux-x64 -p:PublishAot=true -m:2 --artifacts-path artifacts/aot -o out/aot ) > logs/publish-aot.log 2>&1 || { echo "AOT FAIL (voir logs/publish-aot.log)"; st=1; }
( time dotnet publish -c Release -r linux-x64 --self-contained true -p:PublishAot=false -m:2 --artifacts-path artifacts/jit -o out/jit ) > logs/publish-jit.log 2>&1 || { echo "JIT FAIL (voir logs/publish-jit.log)"; st=1; }
grep -E "warning|error" logs/publish-aot.log logs/publish-jit.log | sort -u > logs/warnings.txt || true
tail -n 4 logs/publish-aot.log logs/publish-jit.log
exit $st
