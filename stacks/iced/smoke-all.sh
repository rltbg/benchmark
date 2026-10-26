#!/bin/sh
# usage: smoke-all.sh [binaire] [fichier-sortie] ; fumee 8 s par scenario, modes naturel et --full-damage (hors spec)
BIN=${1:-./target/release/iced_bench}; OUT=${2:-logs/smoke.txt}; export BIN
: > $OUT
for fd in "" "--full-damage"; do
  for s in "list 8" "stream 8 --rate 10000" "canvas 8 --shapes 5000" "core 8" "stress 14" "text 8" "images 8" "nav 8"; do
    echo "### [$fd] $s" >> $OUT
    ./smoke.sh $s $fd 2>&1 | cut -c1-1200 >> $OUT
  done
done
