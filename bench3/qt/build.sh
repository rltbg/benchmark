#!/bin/bash
# Build the Qt Quick bench app (spec v3) against Qt 6.12.0 (aqtinstall, /opt/Qt by default), CMake 4.4.4 + Ninja 1.13.2, Release, -j2.
# Then deploy it with Qt's official CMake deployment script (qt_generate_deploy_qml_app_script) into ./deploy.
# Toolchain installation (one-off, reproducible, versions pinned):
#   python3 -m aqt install-qt linux desktop 6.12.0 linux_gcc_64 -O /opt/Qt --archives icu qtbase qtdeclarative qtsvg qttranslations
#   curl -O https://cmake.org/files/LatestRelease/cmake-4.4.4-linux-x86_64.tar.gz   (sha256 checked against cmake-4.4.4-SHA-256.txt) -> /opt
#   python3 -m pip install --target /opt/ninja-1.13.2 ninja==1.13.2
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
QT_DIR="${QT_DIR:-/opt/Qt/6.12.0/gcc_64}"
CMAKE="${CMAKE:-/opt/cmake-4.4.4-linux-x86_64/bin/cmake}"
NINJA="${NINJA:-/opt/ninja-1.13.2/bin/ninja}"
cd "$HERE"; mkdir -p logs thirdparty
if [ ! -e thirdparty/libxcb-cursor.so.0.0.0 ]; then   # needed by Qt's xcb plugin, missing on the machine (apt not allowed)
  deb=libxcb-cursor0_0.1.4-1build1_amd64.deb; tmp=$(mktemp -d)
  curl -sS -o "$tmp/$deb" "http://archive.ubuntu.com/ubuntu/pool/universe/x/xcb-util-cursor/$deb"
  echo "137cf52479b5a9d8c5926d70d311af04be41941a32ee777340d704cc458c06d8  $tmp/$deb" | sha256sum -c
  dpkg-deb -x "$tmp/$deb" "$tmp/x"; cp -a "$tmp"/x/usr/lib/x86_64-linux-gnu/libxcb-cursor.so.0* thirdparty/; rm -rf "$tmp"
fi
{
  echo "== $(date -Is) build with $QT_DIR / $($CMAKE --version | head -1) / ninja $($NINJA --version) / $(g++ --version | head -1)"
  time ( "$CMAKE" -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_MAKE_PROGRAM="$NINJA" -DCMAKE_PREFIX_PATH="$QT_DIR" \
           -DBENCH_ROOT="$BENCH_ROOT" && "$CMAKE" --build build -j2 )
  rm -rf deploy
  time "$CMAKE" --install build --prefix "$HERE/deploy"
} > logs/build.log 2>&1 || { tail -60 logs/build.log; exit 1; }
tail -8 logs/build.log
ls -la build/qtbench deploy/bin/qtbench
