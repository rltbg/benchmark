#!/usr/bin/env python3 -I
"""Graphes v3 (SVG sans dépendance) à partir de results3/ → docs/img/*.svg. Aucune valeur saisie à la main."""
import glob, json, os, re, statistics as st
from xml.sax.saxutils import escape

ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
R = os.path.join(ROOT, "results3"); IMG = os.path.join(ROOT, "docs", "img"); os.makedirs(IMG, exist_ok=True)
STACKS = ["slint", "qt", "avalonia-aot"]
COL = {"slint": "#2f6fb0", "qt": "#3a9a4f", "avalonia-aot": "#d9822b", "slint-direct": "#8fb5de"}
NAME = {"slint": "Slint", "qt": "Qt Quick", "avalonia-aot": "Avalonia AOT", "slint-direct": "Slint (winit direct)"}
INK, GRID = "#222", "#ddd"


def fmt(v):
    if v is None: return "–"
    if v >= 1e6: return f"{v/1e6:.3g} M"
    if v >= 1e3: return f"{v/1e3:.3g} k"
    return f"{v:.3g}"


def hbar(path, title, subtitle, groups, series, log=False, unit="", note=""):
    """groups = [(label, {serie: (valeur|None, étiquette)})]"""
    W, left, right, top = 900, 230, 90, 70
    bh, gap, ggap = 16, 3, 16
    n = len(series); gh = n * bh + (n - 1) * gap
    H = top + len(groups) * (gh + ggap) + 70
    vals = [v for _, d in groups for (v, _l) in d.values() if v]
    if not vals: return
    import math
    vmax = max(vals); vmin = min(vals)
    if log:
        lo = 10 ** math.floor(math.log10(vmin) - 0.0); hi = 10 ** math.ceil(math.log10(vmax))
        pos = lambda v: (math.log10(max(v, lo)) - math.log10(lo)) / (math.log10(hi) - math.log10(lo))
    else:
        lo, hi = 0, vmax * 1.08; pos = lambda v: v / hi
    pw = W - left - right
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" font-family="Helvetica,Arial,sans-serif" font-size="12">',
         f'<rect width="{W}" height="{H}" fill="#fff"/>',
         f'<text x="16" y="26" font-size="17" font-weight="bold" fill="{INK}">{escape(title)}</text>',
         f'<text x="16" y="46" fill="#555">{escape(subtitle)}</text>']
    ticks = ([10 ** k for k in range(int(math.log10(lo)), int(math.log10(hi)) + 1)] if log
             else [hi * i / 5 / 1.08 * 1.08 for i in range(0, 6)])
    for t in ticks:
        x = left + pw * pos(t)
        o.append(f'<line x1="{x:.1f}" y1="{top-8}" x2="{x:.1f}" y2="{H-60}" stroke="{GRID}"/>')
        o.append(f'<text x="{x:.1f}" y="{H-44}" text-anchor="middle" fill="#666">{fmt(t)}</text>')
    y = top
    for label, d in groups:
        o.append(f'<text x="{left-10}" y="{y + gh/2 + 4:.1f}" text-anchor="end" fill="{INK}">{escape(label)}</text>')
        for i, s in enumerate(series):
            v, lab = d.get(s, (None, "–"))
            yy = y + i * (bh + gap)
            w = pw * pos(v) if v else 0
            if v: o.append(f'<rect x="{left}" y="{yy}" width="{w:.1f}" height="{bh}" fill="{COL[s]}" rx="2"/>')
            o.append(f'<text x="{left + w + 5:.1f}" y="{yy + 12}" fill="#444">{escape(lab)}</text>')
        y += gh + ggap
    lx = left
    for s in series:
        o.append(f'<rect x="{lx}" y="{H-30}" width="12" height="12" fill="{COL[s]}" rx="2"/><text x="{lx+17}" y="{H-20}" fill="{INK}">{escape(NAME[s])}</text>')
        lx += 160
    o.append(f'<text x="{W-16}" y="{H-8}" text-anchor="end" fill="#777" font-size="11">{escape(note or unit)}</text>')
    o.append("</svg>")
    open(os.path.join(IMG, path), "w").write("\n".join(o))


def cap(s, sc):
    p = f"{R}/capacity/{s}/{sc}.json"
    if not os.path.exists(p): return (None, "–")
    d = json.load(open(p)); n = d["n_max"]
    if n is None: return (None, "< " + fmt(d["first_fail"]))
    return (n, ("≥ " if not d["bounded_above"] else "") + fmt(n))


def summ(s, tag):
    p = f"{R}/runs/{s}/{s}-{tag}.summary.json"
    return [r for r in json.load(open(p))["runs"] if r.get("rc") == 0] if os.path.exists(p) else []


def med(rs, k):
    v = [r.get(k) for r in rs if isinstance(r.get(k), (int, float))]
    return st.median(v) if v else None


# 1. capacité (échelle log, plus grand = mieux)
rows = [("Listes : mises à jour /s", "list-mutate"), ("Formes pleines", "gfx-shapes"), ("Formes à dégradé", "gfx-shapes-featuregradient"),
        ("Formes à ombre", "gfx-shapes-featureshadow"), ("Étiquettes de texte", "text-labels"), ("Texte riche (spans)", "text-rich"),
        ("Animations simultanées", "ui-anim"), ("Arbre redimensionnable (nœuds)", "ui-tree"), ("Images 256² vignettes", "gfx-images"),
        ("Calques avec opacité", "gfx-layers"), ("Chemins vectoriels", "gfx-paths"), ("Lignes riches (éléments/ligne)", "list-rich")]
g = [(lab, {s: cap(s, sc) for s in STACKS}) for lab, sc in rows]
hbar("capacite.svg", "Capacité à 60 Hz : combien d'éléments avant de décrocher ?", "Plus la barre est longue, mieux c'est (échelle logarithmique). « < x » : décroche dès le plus petit essai.",
     g, STACKS, log=True, note="VM 4 vCPU, rendu logiciel, sans GPU")

# 2. latence touche→pixel (p50, idle) et molette
def lat(s, name, load="idle", key="input_latency_ms_p50"):
    v = med(summ(s, f"{name}-n100000-load{load}"), key)
    return (v, "–" if v is None else f"{v:.1f} ms")
S4 = STACKS + ["slint-direct"]
g = [("Touche, machine au repos", {s: lat(s, "latency-load") for s in S4}),
     ("Touche, 3 processus à 100 % CPU", {s: lat(s, "latency-load", "cpuhog") for s in S4}),
     ("Touche, liste en défilement rapide", {s: lat(s, "latency-load", "fling") for s in S4}),
     ("Molette, machine au repos", {s: lat(s, "latency-wheel") for s in S4}),
     ("Molette, 3 processus à 100 % CPU", {s: lat(s, "latency-wheel", "cpuhog") for s in S4}),
     ("Molette, p95, au repos", {s: lat(s, "latency-wheel", "idle", "input_latency_ms_p95") for s in S4})]
hbar("latence.svg", "Latence d'entrée : du geste au pixel qui change", "Médiane (sauf dernière ligne : p95). Plus court = mieux (échelle logarithmique). Mesuré de l'extérieur, sur le framebuffer.",
     g, S4, log=True, unit="ms")

# 3. édition d'1 Mo
g = [("Latence par frappe, texte de 1 Mo", {s: lat(s, "text-edit-n1") for s in STACKS})]
g.append(("… p95", {s: lat(s, "text-edit-n1", "", "input_latency_ms_p95") if False else (med(summ(s, "text-edit-n1"), "input_latency_ms_p95"), "") for s in STACKS}))
g[1] = ("… p95", {s: (v, "–" if v is None else f"{v:.0f} ms") for s in STACKS for v in [med(summ(s, "text-edit-n1"), "input_latency_ms_p95")]} if False else
        {s: (lambda v: (v, "–" if v is None else f"{v:.0f} ms"))(med(summ(s, "text-edit-n1"), "input_latency_ms_p95")) for s in STACKS})
g[0] = ("Médiane", {s: (lambda v: (v, "–" if v is None else f"{v:.0f} ms"))(med(summ(s, "text-edit-n1"), "input_latency_ms_p50")) for s in STACKS})
hbar("edition-1mo.svg", "Taper dans un document de 1 Mo", "Délai entre une frappe et le changement affiché. Plus court = mieux (échelle logarithmique).", g, STACKS, log=True, unit="ms")

# 4. quota CPU
g = []
for q, ql in (("1.0", "1 cœur"), ("0.5", "½ cœur")):
    g.append((f"2000 étiquettes, {ql}", {s: (lambda v: (v, "–" if v is None else f"{v:.0f} fps"))(med(summ(s, f"text-labels-n2000-q{q}"), "fps_mean")) for s in STACKS}))
for q, ql in (("1.0", "1 cœur"), ("0.5", "½ cœur")):
    g.append((f"1000 formes, {ql}", {s: (lambda v: (v, "–" if v is None else f"{v:.0f} fps"))(med(summ(s, f"gfx-shapes-n1000-q{q}"), "fps_mean")) for s in STACKS}))
hbar("quota-cpu.svg", "Quand le processeur se raréfie", "Images par seconde avec le CPU bridé par cgroup (60 = parfait). Plus long = mieux.", g, STACKS, unit="fps")

# 5. démarrage
cold = {}
for line in open(f"{R}/plan.log", errors="replace"):
    if line.startswith('{"stack"'):
        try: d = json.loads(line)
        except ValueError: continue
        cold.setdefault((d["stack"], d["mode"][:5]), []).append(d["first_pixel_ms"]["median"])
g = []
for i, lab in ((0, "1 000 nœuds"), (1, "100 000 nœuds")):
    g.append((f"{lab}", {s: (lambda v: (v, "–" if v is None else f"{v/1000:.2f} s" if v >= 1000 else f"{v:.0f} ms"))(cold.get((s, "chaud"), [None, None])[i] if len(cold.get((s, "chaud"), [])) > i else None) for s in STACKS}))
hbar("demarrage.svg", "Temps de démarrage jusqu'au premier pixel", "Application déjà en cache disque, arbre statique de N nœuds. Plus court = mieux (échelle log).", g, STACKS, log=True, unit="axe en millisecondes")

# 6. mémoire (PSS du soak)
g = []
for lab, key in (("Début du soak", "pss_kb_first_quarter"), ("Fin du soak", "pss_kb_last_quarter")):
    g.append((lab, {s: (lambda v: (v, "–" if v is None else f"{v:.0f} Mo"))((lambda x: x / 1024 if x else None)(med(summ(s, "list-mutate-n20000"), key))) for s in STACKS}))
hbar("soak-memoire.svg", "Mémoire pendant 10 minutes de mises à jour continues", "PSS mesuré de l'extérieur (1er et dernier quart de la mesure). Une barre plus longue en bas = la mémoire grossit.", g, STACKS, unit="Mo")
print("ok", sorted(os.listdir(IMG)))
