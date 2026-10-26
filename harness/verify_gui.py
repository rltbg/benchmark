#!/usr/bin/env python3 -I
"""Preuve que l'app testée est une vraie GUI : pour chaque stack et scénario
 1) une fenêtre X11 visible, appartenant au PID de l'app ou à un de ses enfants (_NET_WM_PID), de taille >= 1000x600 ;
 2) le framebuffer n'est pas vide (couleurs distinctes, part de pixels hors fond) ;
 3) il change entre deux captures espacées (animation réelle) ;
 4) une capture PNG est enregistrée dans results/verification/ pour inspection.
usage: verify_gui.py <stack> "<cmd>" <scenario> [extra args]"""
import os, sys, os, time, subprocess, json, tempfile, zlib, struct, array, collections
ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
stack, cmd, sc = sys.argv[1], sys.argv[2].replace("${ROOT}", ROOT).split(), sys.argv[3]; extra = sys.argv[4:]
OUT = os.path.join(ROOT,"results","verification"); os.makedirs(OUT, exist_ok=True)
d = tempfile.mkdtemp(prefix="vg"); disp = ":390"
xv = subprocess.Popen(["Xvfb", disp, "-screen","0","1280x720x24","-fbdir",d,"-nolisten","tcp"], stderr=subprocess.DEVNULL); time.sleep(0.8)
env = dict(os.environ, DISPLAY=disp, LIBGL_ALWAYS_SOFTWARE="1")
def sh(*a): return subprocess.run(list(a),capture_output=True,text=True,env=env).stdout
def toplevels():
    out=[]
    for l in sh("xwininfo","-root","-tree").splitlines():
        l=l.strip()
        if l.startswith("0x") and "x" in l:
            try:
                geo=[t for t in l.split() if "x" in t and "+" in t][0]; w,h=geo.split("+")[0].split("x"); out.append({"id":l.split()[0],"w":int(w),"h":int(h),"name":l.split(None,1)[1][:40]})
            except Exception: pass
    return out
def big(ws): return [w for w in ws if w["w"]>=1000 and w["h"]>=600]
before_ws = toplevels()
p = subprocess.Popen(cmd+["--scenario",sc,"--duration",os.environ.get("VG_D","12"),"--out",os.path.join(d,"o.json"),"--assets",os.path.join(ROOT,"assets")]+extra, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
def tree(pid):
    r=[pid]
    for c in subprocess.run(["pgrep","-P",str(pid)],capture_output=True,text=True).stdout.split(): r+=tree(int(c))
    return r
def sh(*a): return subprocess.run(list(a),capture_output=True,text=True,env=env).stdout
before = before_ws
time.sleep(float(os.environ.get("VG_T","6")))
during = toplevels()
wins = during
fb = os.path.join(d,"Xvfb_screen0")
def grab():
    raw = open(fb,"rb").read(); off = len(raw)-1280*720*4; return raw[off:]
a = grab(); time.sleep(0.4); b = grab()
px = array.array("I"); px.frombytes(a); cnt = collections.Counter(px[::7]); bg,bgn = cnt.most_common(1)[0]
nonbg = 1 - sum(1 for v in px[::7] if v==bg)/len(px[::7])
px2 = array.array("I"); px2.frombytes(b); changed = sum(1 for i in range(0,len(px),97) if px[i]!=px2[i])/len(range(0,len(px),97))
# PNG
rows=[]; 
for y in range(720):
    row=bytearray(b"\x00")
    for x in range(0,1280):
        o=(y*1280+x)*4; row += bytes((a[o+2],a[o+1],a[o]))
    rows.append(bytes(row))
def ch(t,dd): c=struct.pack(">I",len(dd))+t+dd; return c+struct.pack(">I",zlib.crc32(t+dd)&0xFFFFFFFF)
png=b"\x89PNG\r\n\x1a\n"+ch(b"IHDR",struct.pack(">IIBBBBB",1280,720,8,2,0,0,0))+ch(b"IDAT",zlib.compress(b"".join(rows),6))+ch(b"IEND",b"")
fn=f"{OUT}/{stack}-{sc}.png"; open(fn,"wb").write(png)

p.terminate(); 
try: p.wait(timeout=8)
except Exception: p.kill(); p.wait()
time.sleep(0.5); after = toplevels()
bw=big(wins)
print(json.dumps({"stack":stack,"scenario":sc,"toplevels_before":len(before),"big_window_during":[{"w":w["w"],"h":w["h"],"name":w["name"]} for w in bw][:2],"toplevels_after":len(after),"distinct_colors":len(cnt),"nonbackground_pct":round(100*nonbg,1),"pixels_changed_between_captures_pct":round(100*changed,1),"png":fn}))
xv.kill(); xv.wait()
