#!/bin/sh
# Build release reproductible de l'app egui (un seul binaire, deux renderers) :
#   target/release/egui_bench                      -> wgpu (défaut eframe 0.37, Vulkan lavapipe)
#   EGUI_BENCH_RENDERER=glow target/release/egui_bench -> glow (OpenGL Mesa llvmpipe)
# Versions épinglées dans Cargo.toml (=x.y.z) + Cargo.lock (--locked). Toolchain : rustc 1.99.1.
# benchcore = dépendance path ../../core (rlib), compilée dans ce build : aucun chemin absolu.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
cd "$HERE"
mkdir -p logs
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
LOG=logs/build-last.log
{ rustc --version; cargo --version; } > "$LOG"
start=$(date +%s)
cargo +1.99.1 build --release --locked 2>&1 | tee -a "$LOG"
echo "build_seconds=$(( $(date +%s) - start ))" | tee -a "$LOG"
ls -l target/release/egui_bench | tee -a "$LOG"
