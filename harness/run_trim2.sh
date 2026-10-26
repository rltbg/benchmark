#!/bin/bash
# Plan réduit (décision : gain de temps) : B à 3 runs, D à 3 runs (list/canvas), stress 300 s x1 sur stacks clés.
source /tmp/lib_run.sh
cd /home/user/eval
MAIN="slint avalonia-aot avalonia-jit compose-jvm qt-software flutter egui-glow nsh-gtk"
for s in avalonia-aot avalonia-jit compose-jvm qt-software flutter egui-glow nsh-gtk; do run $s stream 14 3 --rate 1000; run $s canvas 14 3 --shapes 20000; run $s text 14 3; run $s images 14 3; run $s nav 14 3; done
for s in slint-femtovg qt-opengl egui-wgpu; do run $s text 14 2; run $s images 14 2; run $s nav 14 2; done
for s in $MAIN; do run $s list 14 3 --cpus 0; run $s canvas 14 3 --shapes 5000 --cpus 0; done
for s in slint avalonia-aot compose-jvm qt-software flutter egui-glow nsh-gtk; do run $s stress 300 1; done
echo "=== DONE $(date +%T)"
