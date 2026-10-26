#!/bin/bash
# Command of harness/stacks3.tsv for the Qt stack: sets the default environment and execs the deployed binary with the received arguments.
#   qt        : ${ROOT}/bench3/qt/run.sh                      (Qt Quick "software" scene-graph backend)
#   qt-opengl : ${ROOT}/bench3/qt/run.sh --backend opengl     (RHI OpenGL on Mesa llvmpipe)
HERE="$(cd "$(dirname "$0")" && pwd)"
export BENCH_ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
export BENCH_PACING="${BENCH_PACING:-60}"
export QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-xcb}"
export QT_QPA_UPDATE_IDLE_TIME="${QT_QPA_UPDATE_IDLE_TIME:-0}"   # documented in QWindow::requestUpdate(); Scene Graph docs recommend it for benchmarking
case " $* " in *" --backend opengl"*) ;; *) export QT_QUICK_BACKEND="${QT_QUICK_BACKEND:-software}";; esac   # software adaptation = primary backend
exec "$HERE/deploy/bin/qtbench" "$@"
