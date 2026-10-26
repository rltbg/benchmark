#!/usr/bin/env bash
# Web (wasm) : wasm-pack (doc officielle), page HTML minimale, Chrome headless via playwright-core. usage: web.sh <OUT>
set -uo pipefail
OUT=$(realpath -m "$1"); mkdir -p "$OUT"
cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
{ rustc -Vv; cargo -V; clang --version | head -1; node -v; google-chrome --version; } > "$OUT/versions.txt" 2>&1
rustup target add wasm32-unknown-unknown >> "$OUT/versions.txt" 2>&1
curl -sSL https://github.com/wasm-bindgen/wasm-pack/releases/download/v0.15.0/wasm-pack-v0.15.0-x86_64-unknown-linux-musl.tar.gz | tar xz -C /tmp
WP=$(find /tmp -name wasm-pack -type f | head -1); "$WP" --version >> "$OUT/versions.txt"
t0=$(date +%s)
"$WP" build --release --target web --out-dir pkg 2>&1 | tee "$OUT/build.log"; rc=${PIPESTATUS[0]}
sec=$(( $(date +%s) - t0 ))
status=ok; [ $rc -eq 0 ] && [ -f pkg/slintspike_bg.wasm ] || status=fail
arts=(); notes=()
if [ "$status" = ok ]; then
  mkdir -p "$OUT/site"; cp -r pkg "$OUT/site/pkg"; rm -f "$OUT/site/pkg/.gitignore"; cp web/index.html "$OUT/site/"
  arts=(--artifact "$OUT/site/pkg/slintspike_bg.wasm" --artifact "$OUT/site/pkg/slintspike.js" --artifact "$OUT/site")
  ls -l "$OUT/site/pkg" > "$OUT/site-listing.txt"
  python3 -m http.server 8123 --directory "$OUT/site" > /dev/null 2>&1 &
  SPID=$!; sleep 1
  mkdir -p /tmp/pw && cd /tmp/pw && npm init -y >/dev/null && npm i playwright-core@1.64.1 >> "$OUT/versions.txt" 2>&1
  cp "$GITHUB_WORKSPACE/spikes/slint/ci/web_run.mjs" /tmp/pw/
  OUT="$OUT" CHROME="$(command -v google-chrome)" node /tmp/pw/web_run.mjs 2>&1 | tee "$OUT/web_run.out"
  kill $SPID 2>/dev/null
  grep -q 'SPIKE:done' "$OUT/run.log" || { status=fail; notes+=(--note "SPIKE:done absent"); }
  grep -q 'SPIKE:selftest ok' "$OUT/run.log" || { status=fail; notes+=(--note "selftest non ok"); }
  notes+=(--note "execution: Chrome headless (WebGL via SwiftShader logiciel), chiffres de CI")
  notes+=(--note "metrics: $(tr '\n' ' ' < "$OUT/run-metrics.txt" 2>/dev/null)")
fi
record web "$status" "$sec" "$OUT" "${arts[@]}" "${notes[@]}"
exit 0
