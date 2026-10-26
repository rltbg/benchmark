#!/bin/bash
# Porte 1 (passe 2) : F* 2026.10.25 -> F# (Custard) -> .NET 10.0.403 JIT + NativeAOT.
# Les sorties de build vont dans $BUILD (hors dépôt). Usage : ./build.sh
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
FSTAR_HOME=${FSTAR_HOME:-/opt/fstar-2026.10.25/fstar}
DOTNET_ROOT=${DOTNET_ROOT:-/opt/dotnet10}
BUILD=${BUILD:-/opt/krml/p1/build}
export DOTNET_ROOT PATH=$FSTAR_HOME/bin:$DOTNET_ROOT:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
"$FSTAR_HOME/bin/fstar.exe" --version | head -1; dotnet --version
mkdir -p "$BUILD/spec" "$BUILD/gen" "$BUILD/app"
cp "$HERE/spec/Core.fst" "$BUILD/spec/"
cp "$HERE/fsharp-project/app/Program.fs" "$HERE/fsharp-project/app/app.fsproj" "$BUILD/app/"
cd "$BUILD/spec"
F="--z3version 5.3.0 --cache_checked_modules --cache_dir cache"
echo "== 1. vérification F*";  fstar.exe $F Core.fst
# Racines explicites : seul le code calculatoire est extrait (pas de noextract nécessaire).
E=""; for d in magic version max_payload header_len validate parse encode checked_add \
               fnv_offset fnv_prime fnv_step fnv_fold hash hash_frame; do E="$E --custard_entry Core.$d"; done
echo "== 2. extraction --codegen FSharp"; fstar.exe $F --codegen FSharp $E Core.fst -o "$BUILD/gen/Core.fs"
diff -q "$BUILD/gen/Core.fs" "$HERE/extracted/Core.fs" && echo "extraction identique à extracted/Core.fs"
cd "$BUILD/app"
echo "== 3. build JIT";      dotnet build -c Release -m:2
echo "== 4. tests JIT";      dotnet bin/Release/net10.0/coreapp.dll
echo "== 5. publish AOT";    dotnet publish -c Release -r linux-x64 -m:2 -p:PublishAot=true -o "$BUILD/aot-out"
echo "== 6. tests AOT";      "$BUILD/aot-out/coreapp"
ls -l "$BUILD/aot-out/coreapp"
