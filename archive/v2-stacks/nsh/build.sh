#!/bin/bash
# Builds the NSH reference app: Rust-only app on NSH's GTK 4 backend (no QuickJS, no WebKitGTK).
#   ./build.sh              -> app/target/release/nshbench
# Pinned: nsh-core / nsh-base / nsh-platform-linux =0.2.0 from crates.io (= git tag v0.2.0),
# exact dependency versions in app/Cargo.lock (cargo --locked). Toolchain: NSH's
# rust-toolchain.toml says channel "stable" (no nightly); this build pins that stable, 1.99.1.
# Needs: GTK >= 4.14 dev files (pkg-config gtk4), network to index.crates.io / static.crates.io.
# The benchmark core (../../core) is a path dependency (rlib, compiled into target/).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-1.99.1}"
mkdir -p "$here/logs"
cd "$here/app"
{ rustc --version; cargo --version; pkg-config --modversion gtk4; } | tee "$here/logs/build-toolchain.txt"
( time cargo build --release --locked ) 2>&1 | tee "$here/logs/build-app.log"
