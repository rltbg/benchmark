#!/usr/bin/env bash
# usage (racine) : record-generic.sh <plateforme> <build_secs> <dossier-run (avec run.log)> [--artifact X]...
# Etat = ok seulement si l'execution a produit SPIKE:selftest ok ET SPIKE:done. Ecrit res/result.json.
set -u
source "$(dirname "$0")/common.sh"
root="$(pwd)"; res="$root/res"; work="$root/work"
plat="$1"; secs="$2"; rundir="$3"; shift 3
status=fail; notes=()
head=$(cat "$work/head" 2>/dev/null || echo "?"); notes+=("tete de plateforme : $head")
[ -s "$res/builds.txt" ] && notes+=("builds : $(grep -E ' rc=' "$res/builds.txt" | tr '\n' ';')")
for f in "$res"/*errors*.txt; do [ -s "$f" ] && notes+=("$(basename "$f") : $(head -c 700 "$f" | tr '\n' '|')"); done
if [ -f "$rundir/run.log" ]; then
  if grep -q "SPIKE:selftest ok" "$rundir/run.log" && grep -q "SPIKE:done" "$rundir/run.log"; then status=ok; fi
  notes+=("marqueurs : $(grep -cE '^SPIKE:' "$rundir/run.log") lignes ; $(grep -E 'SPIKE:(ready|selftest|startup_ms|heavy)' "$rundir/run.log" | head -5 | tr '\n' ' ')")
  notes+=("fps canvas : $(grep -E 'SPIKE:fps' "$rundir/run.log" | sed 's/.*value=//' | tr '\n' ',')")
  notes+=("mem : $(grep -E 'SPIKE:mem' "$rundir/run.log" | tr '\n' '|')")
else notes+=("aucune execution (pas de run.log)"); fi
for d in "$res"/run-aot "$res"/run-nativeaot; do
  [ -d "$d" ] && notes+=("variante $(basename "$d") : $(grep -E 'SPIKE:(ready|selftest|done|startup_ms)' "$d/run.log" 2>/dev/null | tr '\n' ' ') $(cat "$d/exit.txt" 2>/dev/null | tr '\n' ' ') $(tr -d '\n ' < "$d/metrics.json" 2>/dev/null | head -c 500)")
done
[ -f "$rundir/exit.txt" ] && notes+=("exec : $(cat "$rundir/exit.txt" | tr '\n' ' ')")
[ -f "$rundir/metrics.json" ] && notes+=("web metrics : $(tr -d '\n ' < "$rundir/metrics.json" | head -c 900)")
notes+=("emulateur/simulateur/rendu logiciel : pas des chiffres d'appareil")
args=(); for n in "${notes[@]}"; do args+=(--note "$n"); done
"$PY" "$root/ci/record.py" --stack avalonia --platform "$plat" --status "$status" --seconds "$secs" "$@" "${args[@]}" --out "$res"
