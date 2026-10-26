#!/usr/bin/env bash
# usage (racine du depot) : spikes/avalonia/ci/web-build.sh -> work/web-jit (wwwroot publie, sans AOT) et work/web-aot (RunAOTCompilation)
set -u
source "$(dirname "$0")/common.sh"
root="$(pwd)"; res="$root/res"; work="$root/work"; mkdir -p "$res" "$work"; sa="$root/spikes/avalonia"
{ echo "== dotnet workload install wasm-tools"; dotnet workload install wasm-tools; echo "rc=$?"; } > "$res/workload-install.log" 2>&1
{ dotnet --info; dotnet workload list; node --version; google-chrome --version; } > "$res/versions.txt" 2>&1
: > "$res/builds.txt"
pub() { # <nom> <projet> <extra...> : publie puis copie wwwroot dans work/<nom>
  local name="$1" proj="$2"; shift 2
  rm -rf "$(dirname "$proj")/bin" "$(dirname "$proj")/obj"
  timed secs dotnet publish "$proj" -c Release "$@" > "$res/build-$name.log" 2>&1; local rc=$?
  echo "$name rc=$rc ${secs}s" | tee -a "$res/builds.txt"; eval "SECS_${name//-/_}=$secs"
  local idx; idx=$(find "$(dirname "$proj")/bin" -path '*publish*' -name index.html 2>/dev/null | head -1)
  [ -z "$idx" ] && idx=$(find "$(dirname "$proj")/bin" -name index.html 2>/dev/null | head -1)
  rm -rf "$work/$name"; [ -n "$idx" ] && cp -R "$(dirname "$idx")" "$work/$name"
  [ $rc -eq 0 ] && [ -n "$idx" ]
}
head_used=fsharp; fs="$sa/Spike.Browser/Spike.Browser.fsproj"; cs="$sa/heads-cs/Spike.Browser.Cs/Spike.Browser.Cs.csproj"; proj="$fs"
if ! pub fsharp-jit "$fs"; then
  first_errors "$res/build-fsharp-jit.log" 10 > "$res/fsharp-head-errors.txt"
  # variante B : projet F# a la maniere du gabarit officiel F# d'Avalonia (Microsoft.NET.Sdk + WasmMainJSPath) -- voir NOTES.md
  head_used=csharp-fallback; proj="$cs"
  pub csharp-jit "$cs" || true
  mv "$work/csharp-jit" "$work/web-jit" 2>/dev/null; SECS_jit=${SECS_csharp_jit:-0}
else mv "$work/fsharp-jit" "$work/web-jit"; SECS_jit=$SECS_fsharp_jit; fi
echo "$head_used" > "$work/head"; echo "${SECS_jit:-0}" > "$work/secs-jit"
# variante AOT wasm (Mono AOT -> WebAssembly)
if pub aot-try "$proj" -p:RunAOTCompilation=true; then mv "$work/aot-try" "$work/web-aot"; echo "$SECS_aot_try" > "$work/secs-aot"
else first_errors "$res/build-aot-try.log" 10 > "$res/aot-errors.txt"; fi
ls -la "$work" | tee -a "$res/builds.txt"
