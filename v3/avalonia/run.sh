#!/bin/bash
# Commande de lancement (v3/harness/stacks3.tsv) : pose l'environnement par defaut puis exec le binaire NativeAOT avec les arguments recus.
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}" AVALONIA_TELEMETRY_OPTOUT=1 DOTNET_TieredPGO=0
exec "$HERE/out/avalonia-bench" "$@"
