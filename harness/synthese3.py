#!/usr/bin/env python3 -I
"""Synthèse v3 : génère results3/SYNTHESE.md à partir des JSON bruts (capacité, latence, quota, FFI, soak, mémoire, démarrage).
Aucune valeur n'est saisie à la main. Médianes sur les exécutions réussies."""
import glob, json, os, re, statistics as st

ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
R = os.path.join(ROOT, "results3")
STACKS = ["slint", "qt", "avalonia-aot"]
ALL = STACKS + ["slint-direct"]
out = []; w = out.append


def head(cols):
    w("| " + " | ".join(cols) + " |"); w("|" + "---|" * len(cols))


def f(x, nd=1, unit=""):
    return "–" if x is None else f"{x:.{nd}f}{unit}"


def runs(s, tag):
    p = f"{R}/runs/{s}/{s}-{tag}.summary.json"
    if not os.path.exists(p): return None
    return [r for r in json.load(open(p))["runs"] if r.get("rc") == 0]


def med(rs, k):
    v = [r.get(k) for r in rs or [] if isinstance(r.get(k), (int, float))]
    return st.median(v) if v else None


def num(n):
    return "–" if n is None else (f"{n/1e6:g} M" if n >= 1e6 else f"{n/1e3:g} k" if n >= 1e3 else str(n))


# ------------------------------------------------------------ 1. capacité
w("\n#### 1. Capacité à 60 Hz (plus grand N qui tient : fps ≥ 58,5, ≤ 1 % de frames > 25 ms, p99 ≤ 33 ms ; ui-tree : p95 redimensionnement ≤ 100 ms)\n")
w("« ≥ X » = limite de balayage atteinte sans échec ; « < X » = échec dès le plus petit point essayé ; entre parenthèses, le premier N qui échoue.\n")
caps = {}
for p in glob.glob(f"{R}/capacity/*/*.json"):
    d = json.load(open(p)); caps[(d["stack"], os.path.basename(p)[:-5])] = d
scs = sorted({k[1] for k in caps})
head(["Scénario"] + STACKS)
for sc in scs:
    cells = []
    for s in STACKS:
        d = caps.get((s, sc))
        if not d: cells.append("–"); continue
        n = d["n_max"]
        if n is None: cells.append(f"< {num(d['first_fail'])}")
        else:
            cells.append(("≥ " if not d["bounded_above"] else "") + num(n) + (f" ({num(d['first_fail'])})" if d["first_fail"] else ""))
    w(f"| {sc} | " + " | ".join(cells) + " |")

# ------------------------------------------------------------ 2. latence
w("\n#### 2. Latence externe touche→pixel (I1) et molette→pixel (I2) : p50 / p95 en ms, 3 exécutions\n")
for kind, name in (("key", "latency-load"), ("wheel", "latency-wheel")):
    w(f"\n{'Touche' if kind == 'key' else 'Molette'} :\n")
    head(["Charge"] + ALL)
    for load in ("idle", "fling", "heavy", "mutate", "cpuhog"):
        cells = []
        for s in ALL:
            rs = runs(s, f"{name}-n100000-load{load}")
            cells.append(f"{f(med(rs,'input_latency_ms_p50'))} / {f(med(rs,'input_latency_ms_p95'))}" if rs else "–")
        w(f"| {load} | " + " | ".join(cells) + " |")

# ------------------------------------------------------------ 3. quota
w("\n#### 3. Dégradation par quota CPU (fps médian / frames manquées > 25 ms)\n")
head(["Scénario"] + STACKS)
for tag, name in (("list-scale-n1000000", "list-scale 1 M"), ("gfx-shapes-n1000", "gfx-shapes 1000"), ("text-labels-n2000", "text-labels 2000")):
    for q in ("1.0", "0.5"):
        cells = []
        for s in STACKS:
            rs = runs(s, f"{tag}-q{q}")
            if not rs: cells.append("–"); continue
            miss = med(rs, "dropped_gt25"); fr = med(rs, "frames")
            cells.append(f"{f(med(rs,'fps_mean'),1)} / {f(100*miss/fr if fr else None,0,' %')}")
        w(f"| {name}, quota {q} cœur | " + " | ".join(cells) + " |")

# ------------------------------------------------------------ 4. FFI, édition
w("\n#### 4. FFI et édition de texte\n")
head(["Mesure"] + STACKS)
for tag, name, key in (("ffi-calls-n1000000", "ffi-calls : fps UI", "fps_mean"), ("ffi-block-n1000-wherebg", "ffi-block (fil de fond) : frame max (ms)", "frame_max"),
                       ("ffi-block-n1000-whereui", "ffi-block (fil UI) : frame max (ms)", "frame_max"), ("text-edit-n1", "T3 édition 1 Mo : latence touche p50 / p95 (ms)", None)):
    cells = []
    for s in STACKS:
        rs = runs(s, tag)
        if not rs: cells.append("–"); continue
        cells.append(f"{f(med(rs,'input_latency_ms_p50'))} / {f(med(rs,'input_latency_ms_p95'))}" if key is None else f(med(rs, key), 1))
    w(f"| {name} | " + " | ".join(cells) + " |")
for s in STACKS:
    rs = runs(s, "ffi-calls-n1000000")
    ops = [(r.get("ops") or {}) for r in rs or []]
    cps = [o.get("calls_per_s") for o in ops if o.get("calls_per_s")]
    if cps: w(f"\n- {s} : ffi-calls {st.median(cps)/1e6:.1f} M appels/s (médiane des ops.calls_per_s)")

# ------------------------------------------------------------ 5. soak, mémoire, démarrage
w("\n#### 5. Soak 600 s (list-mutate 20 k/s, 1 exécution)\n")
head(["Stack", "fps", "frames manquées > 25 ms", "PSS 1er quart → dernier quart (Mo)", "CPU % cœur"])
for s in STACKS:
    rs = runs(s, "list-mutate-n20000")
    if not rs: w(f"| {s} | – | | | |"); continue
    r = rs[0]; fr = r.get("frames") or 0
    w(f"| {s} | {f(r.get('fps_mean'))} | {f(100*r.get('dropped_gt25',0)/fr if fr else None,1,' %')} | "
      f"{f((r.get('pss_kb_first_quarter') or 0)/1024,0)} → {f((r.get('pss_kb_last_quarter') or 0)/1024,0)} | {f(r.get('cpu_pct_core_mean'),0)} |")
w("\n#### 6. Mémoire minimale (cgroup v1 : pages facturées au cgroup ; bissection 24..1024 Mo, résolution 16 Mo ; 1024 = jamais tenu)\n")
head(["Scénario"] + ALL)
mm = {}
for line in open(f"{R}/plan.log", errors="replace"):
    m = re.match(r"MEMMIN (\S+) (\S+) (\d+) : entre (\d+) et (\d+) Mo", line)
    if m: mm[(m[1], f"{m[2]} {m[3]}")] = (int(m[4]), int(m[5]))
for sc in ("list-scale 100000", "text-big 1"):
    w(f"| {sc} | " + " | ".join(("%d–%d Mo" % mm[(s, sc)]) if (s, sc) in mm else "–" for s in ALL) + " |")
w("\n#### 7. Démarrage jusqu'au premier pixel de ui-tree (5 exécutions, médiane / p95 en ms)\n")
cold = {}
for line in open(f"{R}/plan.log", errors="replace"):
    if line.startswith('{"stack"'):
        try: d = json.loads(line)
        except ValueError: continue
        cold.setdefault((d["stack"], d["mode"][:5]), []).append(d["first_pixel_ms"])
head(["Stack", "froid N=1 k", "froid N=100 k", "chaud N=1 k", "chaud N=100 k"])
for s in ALL:
    c = []
    for mode in ("froid", "chaud"):
        for i in range(2):
            v = cold.get((s, mode), [])
            c.append(f"{v[i]['median']:.0f} / {v[i]['p95']:.0f}" if len(v) > i else "–")
    w(f"| {s} | " + " | ".join(c) + " |")


# ------------------------------------------------------------ 8. empreinte CPU / RAM
w("\n#### 8. Empreinte CPU / RAM (mesurée de l'extérieur par /proc entre les marqueurs ; médiane de 5 exécutions, 3 pour ui-tree)\n")
w("PSS = mémoire réellement attribuée au processus (pages partagées divisées entre leurs utilisateurs) ; pic = maximum sur la mesure. CPU en % d'un cœur (100 = un cœur plein ; 4 cœurs disponibles).\n")
head(["Situation", "Mesure"] + STACKS)
FOOT = (("idle", "À vide (fenêtre ouverte, rien ne bouge)"), ("list-scale-n100000", "Liste de 100 k lignes défilant"), ("gfx-shapes-n500", "500 formes animées"),
        ("text-labels-n200", "200 étiquettes de texte animées"), ("ui-tree-n1000", "Arbre statique 1 k nœuds"), ("ui-tree-n100000", "Arbre statique 100 k nœuds"))
for tag, lab in FOOT:
    rows = {s: runs(s, tag) for s in STACKS}
    for key, name, conv, nd in (("pss_kb_median", "PSS médian (Mo)", 1024, 0), ("pss_kb_peak", "PSS pic (Mo)", 1024, 0), ("cpu_pct_core_mean", "CPU moyen (% cœur)", 1, 0)):
        if tag.startswith("ui-tree") and key == "cpu_pct_core_mean": continue
        w(f"| {lab if key == 'pss_kb_median' else ''} | {name} | " + " | ".join(
            f(med(rows[s], key) / conv if med(rows[s], key) is not None else None, nd) for s in STACKS) + " |")
w("\nTaille sur disque de ce qu'il faut livrer (binaire + bibliothèques propres à l'app, hors bibliothèques système) :\n")
import subprocess
def du(p):
    p = os.path.join(ROOT, p)
    if not os.path.exists(p): return None
    return int(subprocess.run(["du", "-sk", p], capture_output=True, text=True).stdout.split()[0]) / 1024
head(["", "Slint", "Qt Quick", "Avalonia AOT"])
w(f"| Taille (Mo) | {f(du('bench3/slint/app/target/release/slintapp'),0)} | {f(du('bench3/qt/deploy'),0)} | {f(du('bench3/avalonia/out'),0)} |")

open(f"{R}/SYNTHESE.md", "w").write("\n".join(out) + "\n")
print("\n".join(out))
