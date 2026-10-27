#!/bin/bash
# usage: DISPLAY=:210 [DUR=8] smoke.sh tag scenario [extra args...]   (direct run of run.sh, NOT an official measurement)
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}"
tag=$1; sc=$2; shift 2
mkdir -p "$HERE/logs/smoke"
timeout 240 "$HERE/run.sh" --scenario "$sc" --duration "${DUR:-8}" --out "$HERE/logs/smoke/$tag.json" --assets "$ROOT/assets" "$@" > "$HERE/logs/smoke/$tag.out" 2>&1
rc=$?
python3 -I - "$HERE/logs/smoke/$tag.json" "$tag" "$rc" <<'PY'
import json, sys
fn, tag, rc = sys.argv[1:4]
try: d = json.load(open(fn))
except Exception as e: print(tag, "rc", rc, "NO JSON", e); sys.exit()
f = d["frame_ms"]; s = sorted(f)
skip = ("sum_heights", "variant", "resize_frame_ms", "reset_call_ms", "rebuild_ms", "ffi_phases", "soak")
ops = {k: (round(v, 1) if isinstance(v, float) else v) for k, v in d["ops"].items() if k not in skip and not isinstance(v, (list, dict))}
print("%-22s rc=%s frames=%-6d fps=%-6.1f p50=%.2f p95=%.2f p99=%.2f max=%.1f >25ms=%d" % (tag, rc, len(f), 1000 * len(f) / sum(f) if f else 0,
      s[len(s) // 2] if f else 0, s[int(len(s) * .95)] if f else 0, s[min(len(s)-1,int(len(s) * .99))] if f else 0, max(f) if f else 0, sum(1 for x in f if x > 25)), ops)
for k in ("resize_frame_ms", "reset_call_ms", "rebuild_ms", "ffi_phases", "soak"):
    if k in d["ops"]: v = d["ops"][k]; print("   ", k, v if not isinstance(v, list) or len(v) < 12 else (len(v), "min/med/max", min(v), sorted(v)[len(v)//2], max(v)))
PY
grep -v "locale\|UTF-8\|reconfigure\|for more information\|^BENCH:" "$HERE/logs/smoke/$tag.out" | head -6
