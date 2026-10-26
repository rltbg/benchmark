#!/bin/sh
# Lanceur : SLINTAPP_RENDERER=skia-software (défaut) | software | femtovg ; SLINTAPP_THROTTLE=1 = pacing 60 Hz natif.
HERE=$(cd "$(dirname "$0")" && pwd)
exec "$HERE/app/target/release/slintapp" "$@"
