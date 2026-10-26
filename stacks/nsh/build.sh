#!/bin/bash
# Builds the NSH reference app (GTK 4 backend, Rust-only) and, optionally, the TSX variant.
#   ./build.sh          -> app/target/release/nshbench          (Rust-only, no QuickJS)
#   ./build.sh tsx      -> + app-tsx-target/release/nshbench + .nsh-out/bundle.nshc (QuickJS, TSX List/stream)
#   ./build.sh gpui     -> + gpui-app/target/release/nshbench-gpui (GPUI backend, Vulkan; ~15 min, 650 crates)
# Needs: GTK 4.14 dev files (pkg-config gtk4), clang (QuickJS only), network to index.crates.io/static.crates.io
# and git clone of github.com/longbridge/nsh (anonymous) into ./src. CARGO_BUILD_JOBS=2 as per the brief.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p logs
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
if [ ! -d src/.git ]; then
  git clone --depth 1 https://github.com/longbridge/nsh src
fi
# NB: src/rust-toolchain.toml makes rustup install `stable` + 7 mobile targets (static.rust-lang.org)
# the first time cargo runs inside src/. The app itself is a separate workspace (app/).
( cd app && time cargo build --release ) 2>&1 | tee logs/build-app.log
if [ "${1:-}" = "tsx" ]; then
  ( cd src && time cargo build --release -p nsh-cli ) 2>&1 | tee logs/build-nsh-cli.log
  mkdir -p .nsh-out
  ./src/target/release/nsh build tsx --release -o .nsh-out 2>&1 | tee logs/build-tsx-bundle.log
  ( cd app && time cargo build --release --features tsx --target-dir ../app-tsx-target ) 2>&1 | tee logs/build-app-tsx.log
fi
if [ "${1:-}" = "gpui" ]; then
  ( cd gpui-app && time cargo build --release ) 2>&1 | tee logs/build-gpui.log
fi
