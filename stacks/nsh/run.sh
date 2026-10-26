#!/bin/bash
# Launcher for the harness: run.sh <scenario args...>   (same CLI as the spec)
# GSK_RENDERER defaults to cairo (see STACK-NOTES.md: cairo vs ngl on llvmpipe).
here="$(cd "$(dirname "$0")" && pwd)"
export GSK_RENDERER="${GSK_RENDERER:-cairo}"
export GDK_BACKEND="${GDK_BACKEND:-x11}"
export GTK_A11Y="${GTK_A11Y:-none}"
exec "$here/app/target/release/nshbench" "$@"
