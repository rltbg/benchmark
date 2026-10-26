#!/usr/bin/env python3 -I
"""Fausse app pour tester le harnais : 2 s de warm-up, marqueurs, ~100 % d'un cœur pendant la mesure, alloue --alloc Mo."""
import sys, time, json
a = dict(zip(sys.argv[1::2], sys.argv[2::2])); d = float(a["--duration"]); alloc = int(a.get("--alloc", "0"))
time.sleep(2); print("BENCH:measure_start", flush=True)
buf = bytearray(alloc * 1024 * 1024)
for i in range(0, len(buf), 4096): buf[i] = 1
t0 = time.perf_counter(); fm = []; last = t0
while time.perf_counter() - t0 < d:
    x = 0
    for _ in range(20000): x += 1
    now = time.perf_counter(); fm.append((now - last) * 1000); last = now
print("BENCH:measure_end", flush=True)
json.dump({"stack": "selftest", "scenario": a["--scenario"], "frame_ms": fm, "ops": {}}, open(a["--out"], "w"))
