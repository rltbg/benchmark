#!/bin/sh
# Build release de l'app iced.  ./build.sh        -> variante principale tiny-skia : target/release/iced_bench
#                               ./build.sh wgpu   -> variante secondaire wgpu (+ tiny-skia en repli) : target-wgpu/release/iced_bench
cd "$(dirname "$0")" || exit 1
mkdir -p logs
export CARGO_BUILD_JOBS=2
if [ "$1" = "wgpu" ]; then
  /usr/bin/time -v cargo build --release --features wgpu --target-dir target-wgpu 2>&1 | tee logs/build-wgpu.log
else
  /usr/bin/time -v cargo build --release 2>&1 | tee logs/build.log
fi
