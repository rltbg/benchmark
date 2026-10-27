#!/bin/sh
# Lanceur : SLINTAPP_RENDERER=skia-software (défaut, winit-skia-software) | software (moteur logiciel natif) ;
# BENCH_PACING=60 (défaut, cadence native Slint) | uncapped. SLINT_DEBUG_PERFORMANCE ne doit pas être posé pendant les mesures.
HERE=$(cd "$(dirname "$0")" && pwd)
unset SLINT_DEBUG_PERFORMANCE
export SLINTAPP_RENDERER="${SLINTAPP_RENDERER:-skia-software}"
export BENCH_PACING="${BENCH_PACING:-60}"
exec "$HERE/app/target/release/slintapp" "$@"
