#!/usr/bin/env python3 -I
import json, glob, os, statistics as st, sys, collections
R = "/home/user/eval/results"
def pct(v, p):
    s = sorted(v); k = min(len(s)-1, max(0, int(round(p/100*len(s)+0.5))-1)); return s[k]
rows = collections.defaultdict(dict)
for f in sorted(glob.glob(R + "/*.summary.json")):
    d = json.load(open(f)); s = d["summary"]; runs = [r for r in d["runs"] if r["rc"] == 0]
    if not runs: rows[s["scenario"]][s["tag"]] = {"stack": s["stack"], "n": 0, "fail": len(d["runs"])}; continue
    g = lambda k: [r[k] for r in runs if r.get(k) is not None]
    fr = [r["frames"] for r in runs if r.get("frames")]
    drop = [100*r["dropped_gt16_7"]/r["frames"] for r in runs if r.get("frames")]
    row = {"stack": s["stack"], "n": len(runs), "fail": len(d["runs"]) - len(runs), "tag": s["tag"]}
    for k in ("fps_mean", "frame_p95", "frame_p99", "frame_max", "rss_kb_median", "rss_kb_peak", "cpu_pct_core_mean", "cpu_pct_core_peak_1s", "wall_s"):
        v = g(k)
        if v: row[k] = (st.median(v), pct(v, 95))
    if drop: row["drop_pct"] = (st.median(drop), pct(drop, 95))
    ops = [r.get("ops") or {} for r in runs]
    for k in ("calls_per_s", "max_frame_stall_ms", "updates_dropped", "updates_applied", "startup_to_first_frame_ms"):
        v = [o[k] for o in ops if isinstance(o.get(k), (int, float))]
        if v: row[k] = (st.median(v), pct(v, 95))
    rows[s["scenario"]][s["tag"]] = row
def f(x, nd=1):
    return "–" if x is None else (f"{x[0]:.{nd}f} / {x[1]:.{nd}f}" if isinstance(x, tuple) else str(x))
out = []
for sc in ("idle", "list", "stream", "canvas", "core", "text", "images", "nav", "stress"):
    if sc not in rows: continue
    out.append(f"\n### {sc}\n")
    if sc == "idle":
        out.append("| tag | n | RSS idle Mo (méd / p95) | CPU % cœur (méd / p95) |\n|---|---|---|---|")
        for t, r in sorted(rows[sc].items()):
            if r["n"] == 0: out.append(f"| {t} | 0 (échecs {r['fail']}) | | |"); continue
            rs = r.get("rss_kb_median"); out.append(f"| {t} | {r['n']} | {f((rs[0]/1024, rs[1]/1024)) if rs else '–'} | {f(r.get('cpu_pct_core_mean'),2)} |")
        continue
    extra = {"core": ("calls_per_s", "M triplets/s"), "stream": ("updates_dropped", "drops")}.get(sc)
    out.append("| tag | n | FPS (méd / p95) | frame p95 ms | frame p99 ms | frames >16,7 ms % | RSS pic Mo | CPU % cœur moy | CPU pic 1 s |" + (f" {extra[1]} |" if extra else "") + "\n|---|---|---|---|---|---|---|---|---|" + ("---|" if extra else ""))
    for t, r in sorted(rows[sc].items()):
        if r["n"] == 0: out.append(f"| {t} | 0 (échecs {r['fail']}) |" + " |"*8); continue
        rp = r.get("rss_kb_peak"); rpm = f"{rp[0]/1024:.0f}" if rp else "–"
        e = ""
        if extra and r.get(extra[0]): e = f" {r[extra[0]][0]/1e6:.1f} |" if extra[0] == "calls_per_s" else f" {r[extra[0]][0]:.0f} |"
        elif extra: e = " – |"
        out.append(f"| {t} | {r['n']}{'+'+str(r['fail'])+'KO' if r['fail'] else ''} | {f(r.get('fps_mean'))} | {f(r.get('frame_p95'))} | {f(r.get('frame_p99'))} | {f(r.get('drop_pct'))} | {rpm} | {f(r.get('cpu_pct_core_mean'),0)} | {f(r.get('cpu_pct_core_peak_1s'),0)} |{e}")
open("/home/user/eval/results/TABLES.md", "w").write("\n".join(out) + "\n"); print("\n".join(out))
