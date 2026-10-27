#!/usr/bin/env python3
"""Mesures porte 1 : démarrage à froid (10 runs, caches de pages vidés si possible),
RSS idle (10 processus), taille. Usage : measure.py <coreapp-AOT> <coreapp.dll> [dotnet]"""
import subprocess, time, statistics, os, sys, math
AOT = sys.argv[1]; DLL = sys.argv[2]; DOTNET = sys.argv[3] if len(sys.argv) > 3 else 'dotnet'
N = 10
def drop():
    try:
        os.sync(); open('/proc/sys/vm/drop_caches', 'w').write('3'); return True
    except Exception as e:
        return False
def p95(v):  # rang le plus proche : ceil(0.95 n) ; avec n=10 c'est le max
    s = sorted(v); return s[math.ceil(0.95 * len(s)) - 1]
def startup(cmd, dropc):
    ts = []
    for _ in range(N):
        if dropc: drop()
        t = time.perf_counter(); r = subprocess.run(cmd + ['once'], capture_output=True)
        ts.append((time.perf_counter() - t) * 1000)
        assert r.returncode == 0 and r.stdout.strip() == b'4F9F2CAB', r
    return ts
def rss(cmd):
    out = []
    for _ in range(N):
        p = subprocess.Popen(cmd + ['idle'], stdout=subprocess.PIPE)
        assert p.stdout.readline().strip() == b'ready'; time.sleep(1.0)
        d = dict(l.split(':', 1) for l in open(f'/proc/{p.pid}/status').read().splitlines() if ':' in l)
        out.append((int(d['VmRSS'].split()[0]), int(d['VmHWM'].split()[0])))
        p.kill(); p.wait()
    return out
print("loadavg avant:", open('/proc/loadavg').read().strip())
can = drop(); print("drop_caches possible:", can)
print("taille AOT (octets):", os.path.getsize(AOT))
for name, cmd in (('AOT', [AOT]), ('JIT (dotnet coreapp.dll)', [DOTNET, DLL])):
    for dropc in ([True, False] if can else [False]):
        ts = startup(cmd, dropc)
        print(f"{name} démarrage ms ({N} runs, caches {'vidés' if dropc else 'chauds'}): min={min(ts):.1f} "
              f"médiane={statistics.median(ts):.1f} p95={p95(ts):.1f} max={max(ts):.1f}")
        print("  runs:", [round(t, 1) for t in ts])
    r = rss(cmd)
    print(f"  RSS idle kB: médiane={statistics.median([a for a, b in r]):.0f} min={min(a for a, b in r)} "
          f"max={max(a for a, b in r)} ; pic VmHWM médiane={statistics.median([b for a, b in r]):.0f}")
print("loadavg après:", open('/proc/loadavg').read().strip())
