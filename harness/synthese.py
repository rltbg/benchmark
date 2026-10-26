#!/usr/bin/env python3 -I
"""Génère results/SYNTHESE.md (tableaux du rapport) à partir des JSON bruts. Aucune valeur n'est saisie à la main."""
import math, os, json, glob, statistics as st, os
ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
R = os.environ.get("RES", os.path.join(ROOT,"results"))
MAIN = ["slint","qt-software","avalonia-aot","avalonia-jit","egui-glow","nsh-gtk","compose-jvm","flutter"]
SEC  = ["slint-femtovg","qt-opengl","egui-wgpu"]
OUTSCOPE = ["iced-natural","iced-fulldamage"]
def runs(tag):
    f = f"{R}/{tag}.summary.json"
    if not os.path.exists(f): return None
    return [r for r in json.load(open(f))["runs"] if r["rc"] == 0]
def med(rs, k, scale=1.0):
    v = [r[k] for r in rs if r.get(k) is not None]
    return st.median(v)/scale if v else None
def p95(rs, k):
    v = sorted(r[k] for r in rs if r.get(k) is not None)
    return v[min(len(v)-1, max(0, math.ceil(.95*len(v))-1))] if v else None
def fmt(x, nd=1): return "–" if x is None else f"{x:.{nd}f}"
def fps(stack, sc, suffix=""):
    rs = runs(f"{stack}-{sc}{suffix}")
    if not rs: return None, None, 0
    ok = [r for r in rs if r.get("fps_mean")]
    return (med(ok, "fps_mean"), med(ok, "frame_p95"), len(ok)) if ok else (None, None, 0)
def ops_med(stack, sc, key, suffix=""):
    rs = runs(f"{stack}-{sc}{suffix}") or []
    v = [(r.get("ops") or {}).get(key) for r in rs]; v = [x for x in v if isinstance(x, (int, float))]
    return st.median(v) if v else None
cold = {}
for l in open(f"{R}/coldstart.jsonl"):
    d = json.loads(l); cold[(d["stack"], "froid" if d["mode"].startswith("froid") else "chaud")] = d["first_pixel_ms"]
out = []
def table1(stacks, title):
    out.append(f"\n#### {title}\n")
    out.append("| Stack | RAM idle (Mo) | CPU idle (% cœur) | list 100k : FPS (frame p95 ms) | stream 10k/s : FPS | canvas 5000 : FPS | core : M triplets/s (FPS UI) | 1er pixel froid / chaud (ms) | Stress 300 s : FPS, RSS début→fin (Mo) |")
    out.append("|---|---|---|---|---|---|---|---|---|")
    for s in stacks:
        idle = runs(f"{s}-idle")
        ram = med(idle, "rss_kb_median", 1024) if idle else None; cpu = med(idle, "cpu_pct_core_mean") if idle else None
        l = fps(s, "list"); sm = fps(s, "stream", "-r10000"); cv = fps(s, "canvas", "-s5000"); co = fps(s, "core")
        cps = ops_med(s, "core", "calls_per_s")
        c = cold.get((s, "froid")); h = cold.get((s, "chaud"))
        stl = runs(f"{s}-stress")
        stx = "non mesuré"
        if stl:
            r = stl[0]; stx = f"{r['frames']/300:.1f}, {r['rss_kb_first_quarter']/1024:.0f}→{r['rss_kb_last_quarter']/1024:.0f}"
        out.append(f"| {s} | {fmt(ram,0)} | {fmt(cpu,2)} | {fmt(l[0],0)} ({fmt(l[1],1)}) | {fmt(sm[0],0)} | {fmt(cv[0],1)} | {fmt(cps/1e6 if cps else None,0)} ({fmt(co[0],0)}) | {fmt(c['median'],0) if c else '–'} / {fmt(h['median'],0) if h else '–'} | {stx} |")
table1(MAIN, "Stacks retenues (phase A : 10 exécutions, médianes ; stress : 1 exécution)")
out.append("\nSecondaires (phase A : 3 exécutions) et hors périmètre plateformes (iced : mobile hors scope pour ses auteurs, 10 exécutions) :")
def table_sec(stacks):
    out.append("\n| Variante | list FPS | stream 10k FPS | canvas 5000 FPS | core FPS UI | RAM idle Mo |\n|---|---|---|---|---|---|")
    for s in stacks:
        l = fps(s, "list"); sm = fps(s, "stream", "-r10000"); cv = fps(s, "canvas", "-s5000"); co = fps(s, "core"); idle = runs(f"{s}-idle")
        out.append(f"| {s} | {fmt(l[0],0)} | {fmt(sm[0],0)} | {fmt(cv[0],1)} | {fmt(co[0],0)} | {fmt(med(idle,'rss_kb_median',1024) if idle else None,0)} |")
table_sec(SEC + OUTSCOPE)
out.append("\n#### Autres scénarios (phase B : 3 exécutions ; FPS médian)\n")
out.append("| Stack | stream 1k/s | canvas 20 000 | text 1 Mo | images 600 | nav | list 1 cœur | canvas 5000 1 cœur |\n|---|---|---|---|---|---|---|---|")
for s in MAIN:
    row = [fps(s,"stream","-r1000")[0], fps(s,"canvas","-s20000")[0], fps(s,"text")[0], fps(s,"images")[0], fps(s,"nav")[0], fps(s,"list","-cpu0")[0], fps(s,"canvas","-s5000-cpu0")[0]]
    out.append(f"| {s} | " + " | ".join("**aucune frame**" if (x is None and i==1 and s=='flutter') else fmt(x,1 if (x or 99)<20 else 0) for i,x in enumerate(row)) + " |")
open(f"{R}/SYNTHESE.md","w").write("\n".join(out)+"\n"); print("\n".join(out))
