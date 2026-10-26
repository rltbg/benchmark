#!/usr/bin/env python3 -I
"""Synthèse v2 : génère results/SYNTHESE.md à partir des JSON bruts du harnais v2.
Aucune valeur n'est saisie à la main. Médianes sur les exécutions réussies ; « p95 » = 95e centile par rang."""
import json, math, os, statistics as st

ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
R = os.environ.get("RES", os.path.join(ROOT, "results"))
MAIN = ["slint", "qt-software", "avalonia-aot", "avalonia-jit", "egui", "nsh", "compose-jvm", "flutter"]
SEC = ["slint-uncapped", "slint-software", "qt-software-uncapped", "qt-opengl", "egui-uncapped", "egui-glow", "compose-jvm-uncapped", "flutter-skia"]


def pct(v, p):
    s = sorted(v)
    return s[min(len(s), max(1, math.ceil(p / 100 * len(s)))) - 1] if s else None


def runs(tag):
    f = f"{R}/{tag}.summary.json"
    if not os.path.exists(f):
        return None
    return [r for r in json.load(open(f))["runs"] if r["rc"] == 0]


def med(rs, key, fn=None):
    v = [(fn(r) if fn else r.get(key)) for r in rs or []]
    v = [x for x in v if isinstance(x, (int, float))]
    return st.median(v) if v else None


def p95(rs, key):
    v = [r.get(key) for r in rs or [] if isinstance(r.get(key), (int, float))]
    return pct(v, 95)


def miss25(r):  # % de frames qui manquent une échéance 60 Hz d'au moins une demi-frame
    return 100 * r["dropped_gt25"] / r["frames"] if r.get("frames") else None


def ops(r, k):
    return (r.get("ops") or {}).get(k)


def f(x, nd=1, unit=""):
    if x is None:
        return "–"
    return f"{x:.{nd}f}{unit}"


out = []
w = out.append


def head(cols):
    w("| " + " | ".join(cols) + " |"); w("|" + "---|" * len(cols))


# ---------------------------------------------------------------- 1. tableau principal
w("\n#### 1. Mesure principale à 60 Hz (phase A : 10 exécutions, médianes)\n")
head(["Stack", "PSS idle (Mo)", "CPU idle (% cœur)", "list : FPS / frames manquées (> 25 ms) / CPU %",
      "stream 10k/s : FPS / manquées / CPU %", "canvas 5000 : FPS", "core : M triplets/s (FPS UI)", "input : latence p50 / p95 (ms)"])
for s in MAIN:
    idle, l, sm, cv, co, inp = (runs(f"{s}-{x}") for x in ("idle", "list", "stream-r10000", "canvas-s5000", "core", "input"))
    cps = med(co, None, lambda r: ops(r, "calls_per_s"))
    w(f"| {s} | {f(med(idle,'pss_kb_median') and med(idle,'pss_kb_median')/1024,0)} | {f(med(idle,'cpu_pct_core_mean'),2)} | "
      f"{f(med(l,'fps_mean'),0)} / {f(med(l,None,miss25),1,' %')} / {f(med(l,'cpu_pct_core_mean'),0)} | "
      f"{f(med(sm,'fps_mean'),0)} / {f(med(sm,None,miss25),1,' %')} / {f(med(sm,'cpu_pct_core_mean'),0)} | "
      f"{f(med(cv,'fps_mean'),1)} | {f(cps and cps/1e6,0)} ({f(med(co,'fps_mean'),0)}) | "
      f"{f(med(inp,'input_latency_ms_p50'),1)} / {f(med(inp,'input_latency_ms_p95'),1)} |")

# ---------------------------------------------------------------- 2. démarrage et empreinte
w("\n#### 2. Démarrage (premier pixel, 10 exécutions)\n")
cold = {}
if os.path.exists(f"{R}/coldstart.jsonl"):
    for line in open(f"{R}/coldstart.jsonl"):
        d = json.loads(line); cold[(d["stack"], d["mode"][:5])] = d["first_pixel_ms"]
head(["Stack", "froid (caches vidés) : médiane / p95 (ms)", "chaud : médiane / p95 (ms)"])
for s in MAIN:
    c, h = cold.get((s, "froid")), cold.get((s, "chaud"))
    w(f"| {s} | {f(c and c['median'],0)} / {f(c and c['p95'],0)} | {f(h and h['median'],0)} / {f(h and h['p95'],0)} |")

# ---------------------------------------------------------------- 3. autres scénarios
w("\n#### 3. Autres scénarios (phase B : 5 exécutions, FPS médian)\n")
head(["Stack", "stream 1k/s", "canvas 20 000", "text 1 Mo (FPS ; 1er affichage ms)", "images 600", "nav"])
for s in MAIN:
    t = runs(f"{s}-text")
    c20 = runs(f"{s}-canvas-s20000")
    c20f = med(c20, "fps_mean")
    c20s = ("**0 frame**" if (c20 and c20f is None) else f(c20f, 1))
    w(f"| {s} | {f(med(runs(f'{s}-stream-r1000'),'fps_mean'),0)} | {c20s} | "
      f"{f(med(t,'fps_mean'),0)} ; {f(med(t,None,lambda r: ops(r,'time_to_first_display_ms')),0)} | "
      f"{f(med(runs(f'{s}-images'),'fps_mean'),0)} | {f(med(runs(f'{s}-nav'),'fps_mean'),0)} |")

# ---------------------------------------------------------------- 4. dégradation
w("\n#### 4. Dégradation (3 exécutions, FPS médian / frames manquées > 25 ms)\n")
head(["Stack", "list 1 cœur", "stream 10k 1 cœur", "canvas 1 cœur", "list ½ cœur", "canvas ½ cœur"])
for s in MAIN:
    cells = []
    for tag in (f"{s}-list-cpu0", f"{s}-stream-r10000-cpu0", f"{s}-canvas-s5000-cpu0", f"{s}-list-q0.5", f"{s}-canvas-s5000-q0.5"):
        rs = runs(tag)
        cells.append(f"{f(med(rs,'fps_mean'),0 if (med(rs,'fps_mean') or 0) >= 20 else 1)} / {f(med(rs,None,miss25),0,' %')}" if rs else "–")
    w(f"| {s} | " + " | ".join(cells) + " |")
w("\nPression mémoire (cgroup v1, 3 exécutions ; « pic facturé » = memory.max_usage_in_bytes ; OOM = processus tué) :\n")
head(["Stack", "list 128 Mo : FPS / pic facturé / OOM", "stream 128 Mo", "list 96 Mo", "stream 96 Mo"])
for s in MAIN:
    cells = []
    for tag in (f"{s}-list-m128", f"{s}-stream-r10000-m128", f"{s}-list-m96", f"{s}-stream-r10000-m96"):
        f_ = f"{R}/{tag}.summary.json"
        if not os.path.exists(f_):
            cells.append("–"); continue
        allr = json.load(open(f_))["runs"]
        oom = sum(1 for r in allr if r.get("oom")); ok = [r for r in allr if r["rc"] == 0]
        peak = med(allr, "mem_charged_peak_kb")
        cells.append(f"{f(med(ok,'fps_mean'),0)} / {f(peak and peak/1024,0,' Mo')} / {oom}/{len(allr)}")
    w(f"| {s} | " + " | ".join(cells) + " |")

# ---------------------------------------------------------------- 5. stress
w("\n#### 5. Stress 600 s (1 exécution)\n")
head(["Stack", "FPS", "frames manquées > 25 ms", "CPU % cœur", "PSS 1er quart → dernier quart (Mo)", "GC : collections / pause totale (ms)"])
for s in MAIN:
    rs = runs(f"{s}-stress")
    if not rs:
        w(f"| {s} | non mesuré | | | | |"); continue
    r = rs[0]; gc = ops(r, "gc") or {}
    gcs = f"{gc.get('collections')} / {gc.get('pause_ms_total')}" if gc else "n/a"
    w(f"| {s} | {f(r.get('fps_mean'),1)} | {f(miss25(r),0,' %')} | {f(r.get('cpu_pct_core_mean'),0)} | "
      f"{f((r.get('pss_kb_first_quarter') or 0)/1024,0)} → {f((r.get('pss_kb_last_quarter') or 0)/1024,0)} | {gcs} |")

# ---------------------------------------------------------------- 6. variantes
w("\n#### 6. Variantes (phase A : 3 exécutions) — marge sans plafond et autres renderers\n")
head(["Variante", "list FPS / CPU %", "stream 10k FPS", "canvas 5000 FPS", "input p50 (ms)", "PSS idle (Mo)"])
for s in SEC:
    l, sm, cv, inp, idle = (runs(f"{s}-{x}") for x in ("list", "stream-r10000", "canvas-s5000", "input", "idle"))
    if not any((l, sm, cv)):
        continue
    w(f"| {s} | {f(med(l,'fps_mean'),0)} / {f(med(l,'cpu_pct_core_mean'),0)} | {f(med(sm,'fps_mean'),0)} | {f(med(cv,'fps_mean'),1)} | "
      f"{f(med(inp,'input_latency_ms_p50'),1)} | {f(med(idle,'pss_kb_median') and med(idle,'pss_kb_median')/1024,0)} |")

# ---------------------------------------------------------------- 7. calibration et preuves
w("\n#### 7. Calibration externe (list) et preuves GUI\n")
if os.path.exists(f"{R}/calibration.jsonl"):
    head(["Stack", "FPS déclaré", "mises à jour visibles/s (framebuffer)", "rapport"])
    for line in open(f"{R}/calibration.jsonl"):
        d = json.loads(line); e = d["external"]
        ratio = e["changes_per_s"] / d["app_fps"] if d.get("app_fps") else None
        w(f"| {d['stack']} | {f(d.get('app_fps'),0)} | {f(e['changes_per_s'],0)} | {f(ratio,2,'×')} |")
if os.path.exists(f"{R}/verification.jsonl"):
    ver = [json.loads(l) for l in open(f"{R}/verification.jsonl")]
    ok = [d for d in ver if d["big_window_during"] and d["toplevels_before"] == 0 and d["toplevels_after"] == 0 and d["nonbackground_pct"] > 0]
    ko = [f"{d['stack']}/{d['scenario']}" for d in ver if d not in ok]
    w(f"\nPreuves GUI : {len(ok)}/{len(ver)} réussies (fenêtre ≥ 1000×600 créée puis détruite par l'app, contenu non vide). Échecs : {', '.join(ko) or 'aucun'}.")

open(f"{R}/SYNTHESE.md", "w").write("\n".join(out) + "\n")
print("\n".join(out))
