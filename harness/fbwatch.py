#!/usr/bin/env python3 -I
"""Compteur EXTERNE de mises à jour visibles : Xvfb -fbdir expose le framebuffer en fichier mmap ;
on échantillonne des pixels répartis (grille) à ~1 kHz et on compte les changements d'état.
usage: fbwatch.py <fbdir> <secondes> -> JSON {changes, sampled, changes_per_s, gaps_ms_p50/p95/p99, max_gap_ms}"""
import sys, time, mmap, json, os, struct
fb = os.path.join(sys.argv[1], "Xvfb_screen0"); dur = float(sys.argv[2])
f = open(fb, "rb"); hdr = 0
# format XWD : en-tête + colormap puis pixels ; on lit tout le fichier une fois pour trouver l'offset des pixels
sz = os.path.getsize(fb); W, H = 1280, 720; bpp = 4
off = sz - W * H * bpp
m = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
step_x, step_y = 41, 37          # sous-échantillonnage ~ 700 points
idx = [off + (y * W + x) * bpp for y in range(0, H, step_y) for x in range(0, W, step_x)]
def state(): return bytes(m[i] ^ m[i+1] ^ (m[i+2]) for i in idx)
last = state(); t0 = time.perf_counter(); lastchange = t0; gaps = []; changes = 0; n = 0
while time.perf_counter() - t0 < dur:
    s = state(); n += 1; now = time.perf_counter()
    if s != last:
        changes += 1; gaps.append((now - lastchange) * 1000); lastchange = now; last = s
    time.sleep(0.0005)
gaps.sort(); q = lambda p: gaps[min(len(gaps)-1, int(p*len(gaps)))] if gaps else None
print(json.dumps({"changes": changes, "samples": n, "changes_per_s": changes / dur, "gap_ms_p50": q(.5), "gap_ms_p95": q(.95), "gap_ms_p99": q(.99), "max_gap_ms": max(gaps) if gaps else None}))
