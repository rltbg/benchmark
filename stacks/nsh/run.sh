#!/bin/bash
# Launcher for the harness: run.sh <scenario args...>   (same CLI as the spec)
# GSK_RENDERER: documented GTK variable (https://docs.gtk.org/gtk4/running.html); cairo is the
# fastest renderer on llvmpipe here (STACK-NOTES.md), overridable with --env GSK_RENDERER=ngl.
# BENCH_PACING (spec v2.1) is read by the app itself ("uncapped" -> "60-only", see notes).
here="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$here/../.." && pwd)}"
export GSK_RENDERER="${GSK_RENDERER:-cairo}"
export GDK_BACKEND="${GDK_BACKEND:-x11}"
export GTK_A11Y="${GTK_A11Y:-none}"
exec "$here/app/target/release/nshbench" "$@"
