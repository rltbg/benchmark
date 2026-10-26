#!/usr/bin/env bash
# Build the Flutter Linux release bundle (max 2 parallel jobs).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
if [ ! -d "$HERE/sdk" ]; then
  git clone --depth 1 -b stable https://github.com/flutter/flutter "$HERE/sdk"
fi
export PATH="$HERE/sdk/bin:$PATH"
export PUB_CACHE="$HERE/pub-cache"
export CI=true FLUTTER_SUPPRESS_ANALYTICS=true
export CMAKE_BUILD_PARALLEL_LEVEL=2 NINJAFLAGS="-j2"
flutter config --no-analytics --enable-linux-desktop >/dev/null
dart --disable-analytics >/dev/null || true
cd "$HERE/app"
flutter pub get
mkdir -p "$HERE/logs"
/usr/bin/time -v flutter build linux --release -v 2>&1 | tee "$HERE/logs/build-release.log"
ls -la build/linux/x64/release/bundle
