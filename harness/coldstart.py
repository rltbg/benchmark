#!/usr/bin/env python3 -I
"""Démarrage externe uniforme : temps entre exec() et le premier changement du framebuffer Xvfb (premier pixel).
usage: coldstart.py <stack> "<cmd>" [runs=10] [--drop] [--env K=V]  -> JSON sur stdout"""
import math, os, sys, os, time, mmap, subprocess, json, statistics as st, tempfile
ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
def free_display(n):
    import os
    while os.path.exists(f"/tmp/.X11-unix/X{n}") or os.path.exists(f"/tmp/.X{n}-lock"): n += 1
    return f":{n}"
stack, cmd = sys.argv[1], sys.argv[2].replace("${ROOT}", ROOT).split(); runs = int(sys.argv[3]) if len(sys.argv) > 3 else 10
drop = "--drop" in sys.argv; envx = [a.split("=",1) for a in sys.argv if "=" in a and a.split("=")[0].isupper()]
def dropc():
    try: os.sync(); open("/proc/sys/vm/drop_caches","w").write("3")
    except Exception: pass
def pct(v,p): s=sorted(v); return s[min(len(s)-1,max(0,math.ceil(p/100*len(s))-1))]
res = []
for i in range(runs):
    d = tempfile.mkdtemp(prefix="cs"); disp = free_display(300+i)
    xv = subprocess.Popen(["Xvfb", disp, "-screen","0","1280x720x24","-fbdir",d,"-nolisten","tcp"], stderr=subprocess.DEVNULL); time.sleep(0.8)
    fb = os.path.join(d,"Xvfb_screen0"); f = open(fb,"rb"); m = mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ)
    W,H=1280,720; off=os.path.getsize(fb)-W*H*4; idx=[off+(y*W+x)*4 for y in range(0,H,23) for x in range(0,W,29)]
    st0 = bytes(m[j]^m[j+1]^m[j+2] for j in idx)
    if drop: dropc()
    env = dict(os.environ, DISPLAY=disp, LIBGL_ALWAYS_SOFTWARE="1"); env.update(dict(envx))
    out = os.path.join(d,"o.json")
    t0 = time.perf_counter()
    p = subprocess.Popen(cmd+["--scenario","idle","--duration","6","--out",out,"--assets",os.path.join(ROOT,"assets")], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    tfirst = None
    while time.perf_counter()-t0 < 20:
        if bytes(m[j]^m[j+1]^m[j+2] for j in idx) != st0: tfirst = (time.perf_counter()-t0)*1000; break
        time.sleep(0.002)
    p.kill(); p.wait(); xv.kill(); xv.wait()
    if tfirst: res.append(tfirst)
print(json.dumps({"stack":stack,"mode":"froid(caches vidés)" if drop else "chaud","runs":len(res),"first_pixel_ms":{"median":st.median(res) if res else None,"p95":pct(res,95) if res else None,"min":min(res) if res else None,"max":max(res) if res else None,"all":[round(x) for x in res]}}))
