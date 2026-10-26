#!/usr/bin/env bash
# Reproducible Flutter Linux release build (Dart AOT). Max 2 parallel compile jobs.
# Pinned: Flutter 3.47.8 stable (framework 737ba8f885dc774b53b442759999a312c9461e4e), package:ffi 2.2.0.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
FLUTTER_TAG=3.47.8
FLUTTER_REV=737ba8f885dc774b53b442759999a312c9461e4e
if [ ! -d "$HERE/sdk/.git" ]; then
  git clone --depth 1 -b "$FLUTTER_TAG" https://github.com/flutter/flutter "$HERE/sdk"
fi
got="$(git -C "$HERE/sdk" rev-parse HEAD)"
[ "$got" = "$FLUTTER_REV" ] || { echo "SDK at $got, expected $FLUTTER_REV ($FLUTTER_TAG)" >&2; exit 1; }
export PATH="$HERE/sdk/bin:$PATH"
export PUB_CACHE="$HERE/pub-cache"
export CI=true FLUTTER_SUPPRESS_ANALYTICS=true
export CMAKE_BUILD_PARALLEL_LEVEL=2 NINJAFLAGS="-j2"
flutter config --no-analytics --enable-linux-desktop >/dev/null
dart --disable-analytics >/dev/null || true
cd "$HERE/app"
# CLEAN=1: full rebuild (flutter clean also drops .dart_tool/flutter_build, whose stamps
# otherwise make a build fail when only build/ was deleted)
if [ "${CLEAN:-0}" = 1 ]; then flutter clean >/dev/null; fi
flutter pub get --enforce-lockfile 2>/dev/null || flutter pub get
mkdir -p "$HERE/logs"
FFI_VER="$(awk '/^  ffi:/{f=1} f&&/version:/{gsub(/"/,"",$2);print $2;exit}' pubspec.lock)"
set +e
/usr/bin/time -v flutter build linux --release --dart-define=BENCH_FFI_VERSION="$FFI_VER" -v > "$HERE/logs/build-release.log" 2>&1
rc=$?
set -e
grep -E "Built build|Build process failed|Elapsed|Maximum resident" "$HERE/logs/build-release.log" || true
[ $rc = 0 ] || { echo "flutter build failed (rc=$rc), see logs/build-release.log" >&2; exit $rc; }
test -x build/linux/x64/release/bundle/benchapp && test -f build/linux/x64/release/bundle/lib/libapp.so
du -sb build/linux/x64/release/bundle
