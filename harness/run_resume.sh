#!/bin/bash
# Reprise après arrêt du pilote : complète la phase A puis lance B, D, C.
source /tmp/lib_run.sh
cd /home/user/eval
MAIN="slint avalonia-aot avalonia-jit compose-jvm qt-software flutter egui-glow nsh-gtk iced-natural iced-fulldamage"
resumeA() {
  run egui-glow canvas 14 10 --shapes 5000; run egui-glow core 14 10
  for s in nsh-gtk iced-natural iced-fulldamage compose-jvm; do run $s idle 20 10; run $s list 14 10; run $s stream 14 10 --rate 10000; run $s canvas 14 10 --shapes 5000; run $s core 14 10; done
  for s in slint-femtovg qt-opengl egui-wgpu; do run $s idle 20 3; run $s list 14 3; run $s stream 14 3 --rate 10000; run $s canvas 14 3 --shapes 5000; run $s core 14 3; done
}
resumeA; phaseB; phaseD; phaseC
echo "=== DONE $(date +%T)"
