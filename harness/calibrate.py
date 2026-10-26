#!/usr/bin/env python3 -I
"""Calibration externe : FPS déclaré par l'app vs mises à jour visibles du framebuffer (fbwatch) pendant `list`.
usage: calibrate.py <stack> "<cmd>" [scenario=list] [extra args]"""
import os, sys, os, time, subprocess, json, tempfile
ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
stack, cmd = sys.argv[1], sys.argv[2].replace("${ROOT}", ROOT).split(); sc = sys.argv[3] if len(sys.argv)>3 else "list"; extra = sys.argv[4:]
d = tempfile.mkdtemp(prefix="cal"); disp=":380"
xv = subprocess.Popen(["Xvfb",disp,"-screen","0","1280x720x24","-fbdir",d,"-nolisten","tcp"], stderr=subprocess.DEVNULL); time.sleep(0.8)
env = dict(os.environ, DISPLAY=disp, LIBGL_ALWAYS_SOFTWARE="1"); out=os.path.join(d,"o.json")
p = subprocess.Popen(cmd+["--scenario",sc,"--duration","14","--out",out,"--assets",os.path.join(ROOT,"assets")]+extra, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(5)  # warm-up (2 s) + marge
w = subprocess.run(["python3","-I",os.path.join(ROOT,"harness","fbwatch.py"),d,"5"],capture_output=True,text=True).stdout
p.wait(); xv.kill(); xv.wait()
j = json.load(open(out)); fm = j.get("frame_ms",[])
print(json.dumps({"stack":stack,"scenario":sc,"app_fps":round(1000*len(fm)/sum(fm),1) if fm else None,"external":json.loads(w)}))
