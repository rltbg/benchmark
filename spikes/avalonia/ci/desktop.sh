#!/usr/bin/env bash
# usage (depuis la racine du depot) : spikes/avalonia/ci/desktop.sh <linux|windows|macos> <rid> <variants:0|1>
# Construit (NativeAOT, puis repli self-contained+trim si l'AOT echoue), mesure les variantes de taille, lance, enregistre result.json dans res/.
set -u
plat="$1"; rid="$2"; variants="${3:-0}"
root="$(pwd)"; res="$root/res"; work="$root/work"; mkdir -p "$res" "$work"
proj="$root/spikes/avalonia/Spike.Desktop/Spike.Desktop.fsproj"; ci="$root/spikes/avalonia/ci"
PY=$(command -v python3 || command -v python); ext=""; [ "$plat" = windows ] && ext=".exe"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1 AVALONIA_TELEMETRY_OPTOUT=1
"$PY" -m pip install --quiet --break-system-packages brotli > /dev/null 2>&1 || "$PY" -m pip install --quiet brotli > /dev/null 2>&1 || true
{ echo "plateforme=$plat rid=$rid"; dotnet --info; } > "$res/versions.txt" 2>&1
notes=(); arts=(); status=ok
build() { # nom, args...
  local name="$1"; shift; local s=$(date +%s)
  dotnet publish "$proj" -c Release -r "$rid" "$@" -o "$work/$name" > "$res/build-$name.log" 2>&1; local rc=$?
  local d=$(( $(date +%s) - s )); echo "$name rc=$rc ${d}s" | tee -a "$res/builds.txt"
  eval "BUILD_S_$name=$d"; return $rc
}
build aot -p:PublishAot=true; aot_rc=$?
cp "$res/build-aot.log" "$res/build.log"
grep -E "warning|error" "$res/build-aot.log" | sed 's/ \[\/.*//' | sort -u > "$res/warnings.txt"
run_dir=aot; secs=$BUILD_S_aot
if [ $aot_rc -ne 0 ]; then
  status=fail; notes+=("NativeAOT ECHEC (voir build.log) ; repli self-contained + trim")
  build sctrim --self-contained true -p:PublishTrimmed=true; run_dir=sctrim
else
  arts+=(--artifact "$work/aot/avalonia-spike$ext" --artifact "$work/aot")
  notes+=("NativeAOT OK en ${BUILD_S_aot}s ; $(wc -l < "$res/warnings.txt") lignes warning/error uniques")
fi
build sctrim --self-contained true -p:PublishTrimmed=true && { arts+=(--artifact "$work/sctrim"); notes+=("variante self-contained+trim (IL, JIT) : ${BUILD_S_sctrim}s"); }
if [ "$variants" = 1 ]; then
  build fdd --self-contained false && { arts+=(--artifact "$work/fdd"); notes+=("variante framework-dependent : ${BUILD_S_fdd}s"); }
  build sc --self-contained true && { arts+=(--artifact "$work/sc"); notes+=("variante self-contained non trimee : ${BUILD_S_sc}s"); }
fi
exe="$work/$run_dir/avalonia-spike$ext"
if [ -f "$exe" ]; then
  [ "$plat" = macos ] && chmod +x "$exe"
  if [ "$plat" = linux ]; then xvfb-run -a -s "-screen 0 1280x800x24" bash "$ci/run-desktop.sh" linux "$exe" "$res"; else bash "$ci/run-desktop.sh" "$plat" "$exe" "$res"; fi
  grep -q "SPIKE:selftest ok" "$res/run.log" && grep -q "SPIKE:done" "$res/run.log" || { status=fail; notes+=("execution : marqueurs SPIKE:selftest ok / SPIKE:done absents"); }
  notes+=("execute : $run_dir ; $(cat "$res/exit.txt")")
  notes+=("$(grep -E 'SPIKE:(ready|startup_ms)' "$res/run.log" | tr '\n' ' ')")
else status=fail; notes+=("aucun executable produit"); fi
args=(); for n in "${notes[@]}"; do args+=(--note "$n"); done
"$PY" "$root/ci/record.py" --stack avalonia --platform "$plat" --status "$status" --seconds "$secs" "${arts[@]}" "${args[@]}" --out "$res"
