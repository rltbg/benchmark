#!/bin/sh
# Lanceur : SLINTAPP_RENDERER=skia-software (défaut) | software | femtovg ; BENCH_PACING=60 (défaut, cadence native Slint) | uncapped ; alias SLINTAPP_THROTTLE.
HERE=$(cd "$(dirname "$0")" && pwd)
exec "$HERE/app/target/release/slintapp" "$@"
