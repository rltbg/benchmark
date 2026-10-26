#!/bin/sh
# Build du binaire release unique slintapp (winit/X11 + rendu logiciel, FemtoVG compilé aussi).
# Prérequis : /home/user/eval/core/target/release/libbenchcore.a déjà construit.
set -e
cd "$(dirname "$0")/app"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}
cargo build --release 2>&1 | tee ../logs/build-last.log
ls -l target/release/slintapp
# Lancement (logiciel, défaut) : app/target/release/slintapp --scenario list --duration 10 --out x.json
# Variante FemtoVG (OpenGL ES via llvmpipe) : SLINTAPP_RENDERER=femtovg LIBGL_ALWAYS_SOFTWARE=1 app/target/release/slintapp ...
# Pacing natif Slint (timer 60 Hz, non conforme à la spec) : SLINTAPP_THROTTLE=1
