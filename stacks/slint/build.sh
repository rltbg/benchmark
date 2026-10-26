#!/bin/bash
# Build reproductible du binaire release slintapp (Slint 1.18.1, winit/X11 ; renderers skia-software
# [défaut], software, femtovg). Versions épinglées dans app/Cargo.toml (=x.y.z) et app/Cargo.lock.
# Le cœur benchcore est une dépendance path (../../../core) compilée dans ce build : pas de chemin absolu.
set -eo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE/app"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
mkdir -p "$HERE/logs"
rustc --version | tee "$HERE/logs/build-last.log"
cargo --version | tee -a "$HERE/logs/build-last.log"
start=$(date +%s)
cargo build --release --locked 2>&1 | tee -a "$HERE/logs/build-last.log"
echo "build_seconds=$(( $(date +%s) - start ))" | tee -a "$HERE/logs/build-last.log"
ls -l target/release/slintapp
