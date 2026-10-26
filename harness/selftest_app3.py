#!/usr/bin/env python3 -I
"""Fausse app v3 pour tester capacity.py : à 60 Hz tant que n <= CAP (3000), puis frames proportionnellement plus longues."""
import sys, time, json
a = dict(zip(sys.argv[1::2], sys.argv[2::2])); d = float(a["--duration"]); n = int(a.get("--n", "1000")); CAP = 3000
time.sleep(0.5); print("BENCH:measure_start", flush=True)
period = 1 / 60 * max(1.0, n / CAP); fm = []; t0 = time.perf_counter(); last = t0
while time.perf_counter() - t0 < d:
    time.sleep(period); now = time.perf_counter(); fm.append((now - last) * 1000); last = now
print("BENCH:measure_end", flush=True)
json.dump({"stack": "selftest", "scenario": a["--scenario"], "frame_ms": fm, "ops": {"n": n, "pacing": "60"}}, open(a["--out"], "w"))
