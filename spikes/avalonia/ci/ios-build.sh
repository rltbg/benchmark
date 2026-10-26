#!/usr/bin/env bash
# usage (racine du depot) : spikes/avalonia/ci/ios-build.sh -> work/Spike.app (Mono) et work/Spike-aot.app (NativeAOT, experimental)
set -u
source "$(dirname "$0")/common.sh"
root="$(pwd)"; res="$root/res"; work="$root/work"; mkdir -p "$res" "$work"; sa="$root/spikes/avalonia"
{ echo "== dotnet workload install ios (jeu de workloads 10.0.401 : iOS 26.5.x ; le jeu 10.0.403 donne iOS 27.1 qui exige Xcode 27.1, absent du runner)"; dotnet workload install ios --version 10.0.401; echo "rc=$?"; } > "$res/workload-install.log" 2>&1
{ dotnet --info; dotnet workload list; xcode-select -p; xcodebuild -version; ls -d /Applications/Xcode*; xcrun simctl list runtimes; } > "$res/versions.txt" 2>&1
: > "$res/builds.txt"
build_ios() { # <nom> <projet> <extra args...>
  local name="$1" proj="$2"; shift 2
  for attempt in 1 2 3; do
    rm -rf "$(dirname "$proj")/bin" "$(dirname "$proj")/obj"
    timed secs dotnet build "$proj" -c Release -r iossimulator-arm64 "$@" > "$res/build-$name.log" 2>&1; rc=$?
    echo "$name tentative=$attempt rc=$rc ${secs}s" | tee -a "$res/builds.txt"
    [ $rc -eq 0 ] && break
    # le workload exige une version precise de Xcode : "requires Xcode X.Y"
    v=$(grep -oE "requires Xcode [0-9]+(\.[0-9]+)*" "$res/build-$name.log" | head -1 | grep -oE "[0-9]+(\.[0-9]+)*")
    [ -z "$v" ] && break
    x=$(ls -d /Applications/Xcode_${v}*.app /Applications/Xcode-${v}*.app 2>/dev/null | head -1)
    [ -z "$x" ] && break
    echo "selection de Xcode : $x" | tee -a "$res/builds.txt"; sudo xcode-select -s "$x/Contents/Developer"
  done
  return $rc
}
head_used=fsharp
build_ios fsharp "$sa/Spike.iOS/Spike.iOS.fsproj"; rc=$?
app=$(find "$sa/Spike.iOS/bin" -maxdepth 6 -name '*.app' -type d 2>/dev/null | head -1)
if [ $rc -ne 0 ] || [ -z "$app" ]; then
  head_used=csharp-fallback; first_errors "$res/build-fsharp.log" 10 > "$res/fsharp-head-errors.txt"
  build_ios csharp "$sa/heads-cs/Spike.iOS.Cs/Spike.iOS.Cs.csproj"; rc=$?
  app=$(find "$sa/heads-cs/Spike.iOS.Cs/bin" -maxdepth 6 -name '*.app' -type d 2>/dev/null | head -1)
fi
echo "$secs" > "$work/secs"; echo "$head_used" > "$work/head"
[ -n "$app" ] && cp -R "$app" "$work/Spike.app"
# variante NativeAOT (experimentale sur iOS) : meme projet, PublishAot
if [ -d "$work/Spike.app" ]; then
  if [ "$head_used" = fsharp ]; then proj="$sa/Spike.iOS/Spike.iOS.fsproj"; else proj="$sa/heads-cs/Spike.iOS.Cs/Spike.iOS.Cs.csproj"; fi
  build_ios nativeaot "$proj" -p:PublishAot=true -p:PublishAotUsingRuntimePack=true; arc=$?
  echo "$secs" > "$work/secs-aot"
  aapp=$(find "$(dirname "$proj")/bin" -maxdepth 6 -name '*.app' -type d 2>/dev/null | head -1)
  [ $arc -eq 0 ] && [ -n "$aapp" ] && cp -R "$aapp" "$work/Spike-aot.app"
  [ $arc -ne 0 ] && first_errors "$res/build-nativeaot.log" 10 > "$res/nativeaot-errors.txt"
fi
