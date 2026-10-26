#!/bin/bash
# Build Avalonia 12.1.4 + F# + .NET 10 : variante AOT (NativeAOT) et variante JIT (self-contained, meme code).
# Sorties : out/aot/ (libbenchcore.so dynamique), out/aot-static/ (libbenchcore.a lie statiquement, DirectPInvoke), out/jit/ (libbenchcore.so copiee a cote du binaire). Logs bruts dans logs/.
set -e
cd "$(dirname "$0")"
export AVALONIA_TELEMETRY_OPTOUT=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
CORE=/home/user/eval/core/target/release/libbenchcore.so
mkdir -p logs
rm -rf out artifacts
( time dotnet publish -c Release -r linux-x64 -p:PublishAot=true -m:2 --artifacts-path artifacts/aot -o out/aot ) > logs/publish-aot.log 2>&1 || { echo "AOT FAIL (voir logs/publish-aot.log)"; }
( time dotnet publish -c Release -r linux-x64 --self-contained true -p:PublishAot=false -m:2 --artifacts-path artifacts/jit -o out/jit ) > logs/publish-jit.log 2>&1 || { echo "JIT FAIL (voir logs/publish-jit.log)"; }
( time dotnet publish -c Release -r linux-x64 -p:PublishAot=true -p:LinkCoreStatic=true -m:2 --artifacts-path artifacts/aots -o out/aot-static ) > logs/publish-aot-static.log 2>&1 || { echo "AOT-static FAIL"; }
for d in out/aot out/jit; do [ -d $d ] && cp $CORE $d/; done
ls -la out/aot out/jit | head -80
