#!/usr/bin/env bash
# usage (racine du depot) : spikes/avalonia/ci/android-build.sh   -> APK dans work/apk-<abi>.apk ; journaux dans res/
set -u
source "$(dirname "$0")/common.sh"
root="$(pwd)"; res="$root/res"; work="$root/work"; mkdir -p "$res" "$work"
sa="$root/spikes/avalonia"
{ echo "== dotnet workload install android"; dotnet workload install android; echo "rc=$?"; } > "$res/workload-install.log" 2>&1
{ dotnet --info; dotnet workload list; echo "JAVA_HOME=${JAVA_HOME:-}"; java -version 2>&1; echo "ANDROID_HOME=${ANDROID_HOME:-} ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-}"; } > "$res/versions.txt" 2>&1
sdk="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}"
: > "$res/builds.txt"
head_used=fsharp
for abi in x64 arm64; do
  rm -rf "$sa/Spike.Android/bin" "$sa/Spike.Android/obj"
  timed secs dotnet publish "$sa/Spike.Android/Spike.Android.fsproj" -c Release -r "android-$abi" -p:AndroidPackageFormat=apk \
    -p:InstallAndroidDependencies=true -p:AcceptAndroidSDKLicenses=True ${sdk:+-p:AndroidSdkDirectory=$sdk} > "$res/build-fsharp-$abi.log" 2>&1
  rc=$?; echo "fsharp-$abi rc=$rc ${secs}s" | tee -a "$res/builds.txt"
  apk=$(find "$sa/Spike.Android/bin" -name '*-Signed.apk' 2>/dev/null | head -1); [ -z "$apk" ] && apk=$(find "$sa/Spike.Android/bin" -name '*.apk' 2>/dev/null | head -1)
  if [ $rc -ne 0 ] || [ -z "$apk" ]; then
    head_used=csharp-fallback
    first_errors "$res/build-fsharp-$abi.log" 10 > "$res/fsharp-head-errors-$abi.txt"
    rm -rf "$sa/heads-cs/Spike.Android.Cs/bin" "$sa/heads-cs/Spike.Android.Cs/obj"
    timed secs dotnet publish "$sa/heads-cs/Spike.Android.Cs/Spike.Android.Cs.csproj" -c Release -r "android-$abi" -p:AndroidPackageFormat=apk \
      -p:InstallAndroidDependencies=true -p:AcceptAndroidSDKLicenses=True ${sdk:+-p:AndroidSdkDirectory=$sdk} > "$res/build-csharp-$abi.log" 2>&1
    rc=$?; echo "csharp-$abi rc=$rc ${secs}s" | tee -a "$res/builds.txt"
    apk=$(find "$sa/heads-cs/Spike.Android.Cs/bin" -name '*-Signed.apk' 2>/dev/null | head -1); [ -z "$apk" ] && apk=$(find "$sa/heads-cs/Spike.Android.Cs/bin" -name '*.apk' 2>/dev/null | head -1)
  fi
  [ -n "$apk" ] && cp "$apk" "$work/apk-$abi.apk"
  echo "$secs" > "$work/secs-$abi"
done
echo "$head_used" > "$work/head"
ls -la "$work" | tee -a "$res/builds.txt"
