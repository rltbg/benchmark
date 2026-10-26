#!/bin/bash
# Build JIT local de mise au point (non livre) : out dans dev/ ; libbenchcore.so resolue dynamiquement.
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export DOTNET_ROOT=/opt/dotnet10 PATH=/opt/dotnet10:$PATH AVALONIA_TELEMETRY_OPTOUT=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
cd "$HERE"
dotnet build -c Release -r linux-x64 --self-contained false -m:2 --artifacts-path artifacts/dev -o dev 2>&1 | grep -E "error|Warn|warn FS|Elapsed" | grep -v "IL[0-9]" | sort -u | head -60
