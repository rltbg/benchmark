#!/bin/bash
# usage: smoke.sh tag scenario [extra args...]   (DISPLAY = a running Xvfb 1280x720x24; DUR = duration in s)
# Direct run of the deployed binary (not an official measurement); also records which deployed files are mapped (deployment size).
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
tag=$1; sc=$2; shift 2
mkdir -p "$HERE/logs/smoke" "$HERE/logs/maps"
"$HERE/deploy/bin/qtbench" --scenario "$sc" --duration "${DUR:-6}" --out "$HERE/logs/smoke/$tag.json" --assets "$ROOT/assets" "$@" > "$HERE/logs/smoke/$tag.out" 2>&1 &
pid=$!
sleep $(( ${DUR:-6} / 2 + 2 )); grep -o "$HERE/deploy/[^ ]*" /proc/$pid/maps 2>/dev/null | sort -u > "$HERE/logs/maps/$tag.txt"
wait $pid; rc=$?
python3 -I - "$HERE/logs/smoke/$tag.json" "$tag" "$rc" <<'PY'
import json, sys
fn, tag, rc = sys.argv[1:4]
try: d = json.load(open(fn))
except Exception as e: print(tag, "rc", rc, "NO JSON", e); sys.exit()
f = d["frame_ms"]; s = sorted(f)
ops = {k: (round(v, 1) if isinstance(v, float) else v) for k, v in d["ops"].items() if k not in ("sum_heights",)}
print("%-18s rc=%s frames=%-6d fps=%-7.1f p50=%.2f p95=%.2f max=%.1f" % (tag, rc, len(f), 1000 * len(f) / sum(f) if f else 0,
      s[len(s) // 2] if f else 0, s[int(len(s) * .95)] if f else 0, max(f) if f else 0), ops)
PY
grep -v "locale\|UTF-8\|reconfigure\|for more information" "$HERE/logs/smoke/$tag.out" | grep -v "^BENCH:" | head -5
