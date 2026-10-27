#!/usr/bin/env python3 -I
"""Recherche de capacité à 60 Hz (spec v3 §6).

usage: capacity.py --stack NAME --scenario ID [--variant k=v]... [--duration 12] [--outdir DIR]
                   [--cpu-quota Q] [--env K=V] [--n-start N] [--n-max N] [--values a,b,c]
Le critère « tient 60 Hz » : rc=0, marqueurs, fps >= 58.5, frames > 25 ms <= 1 %, p99 <= 33 ms
(+ applied/generated >= 0.99 et pertes explicites <= 0.1 % pour les scénarios de flux). Recherche : doublement, bissection
(±5 %), 3 confirmations, puis sonde à 2×N_max. Résumé : <outdir>/capacity/<stack>/<scn>.json"""
import argparse, json, os, sys, types

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "harness"))
import run_bench as rb  # noqa: E402

ROOT = rb.ROOT
# scénario -> (valeurs discrètes | (départ, max), contrôle ops optionnel)
SCENARIOS = {
    "list-scale":   dict(start=100_000, max=10_000_000),
    "list-rich":    dict(values=[6, 12, 24, 48, 96]),
    "list-mutate":  dict(start=10_000, max=10_000_000, flow=True),
    "list-reset":   dict(values=[10_000, 100_000, 1_000_000], stall_ms=100),
    "text-big":     dict(values=[1, 10, 50, 100], first_display_ms=2000),
    "text-labels":  dict(start=500, max=200_000),
    "text-rich":    dict(start=1_000, max=100_000),
    "gfx-shapes":   dict(start=250, max=200_000),
    "gfx-paths":    dict(start=100, max=100_000),
    "gfx-chart":    dict(values=[10_000, 100_000, 1_000_000]),
    "gfx-images":   dict(start=10, max=20_000),
    "gfx-layers":   dict(start=4, max=2_000),
    "ui-tree":      dict(values=[1_000, 10_000, 100_000, 500_000], resize=True),
    "ui-bind":      dict(start=1_000, max=1_000_000),
    "ui-anim":      dict(start=100, max=100_000),
    "ffi-callbacks": dict(start=10_000, max=20_000_000, flow=True),
}
FPS_MIN, MISS_MAX, P99_MAX = 58.5, 1.0, 33.0
RESIZE_P95_MAX = 100.0  # ui-tree : frame de redimensionnement interactive (p95 <= 100 ms)


def evaluate(res, spec):
    """(ok, métriques) pour une exécution du harnais."""
    m = {k: res.get(k) for k in ("rc", "fps_mean", "frame_p95", "frame_p99", "frame_max", "frames", "cpu_pct_core_mean",
                                 "pss_kb_peak", "rss_kb_peak", "markers", "oom", "unsupported")}
    fr = res.get("frames") or 0
    m["missed_gt25_pct"] = 100 * res.get("dropped_gt25", 0) / fr if fr else None
    ops = res.get("ops") or {}
    for k in ("updates_generated", "updates_applied", "updates_dropped", "setup_ms", "ui_stall_ms_max", "time_to_first_display_ms", "pacing", "not_rendered"):
        if k in ops: m[k] = ops[k]
    reasons = []
    if spec.get("resize"):  # arbre statique : seules les frames après redimensionnement comptent (ops.resize_frame_ms)
        rf = sorted(ops.get("resize_frame_ms") or [])
        m["resize_p95_ms"] = rf[min(len(rf), max(1, -(-95 * len(rf) // 100))) - 1] if rf else None
        if res.get("unsupported"): reasons.append("unsupported")
        if res.get("rc") != 0: reasons.append(f"rc={res.get('rc')}")
        if not rf: reasons.append("aucune frame de redimensionnement")
        elif m["resize_p95_ms"] > RESIZE_P95_MAX: reasons.append(f"redim. p95 {m['resize_p95_ms']:.0f} ms > {RESIZE_P95_MAX:.0f}")
        return (not reasons), m, reasons
    if res.get("unsupported"): reasons.append("unsupported")
    if res.get("rc") != 0: reasons.append(f"rc={res.get('rc')}")
    if not res.get("markers"): reasons.append("marqueurs absents")
    if not fr: reasons.append("0 frame")
    else:
        if (res.get("fps_mean") or 0) < FPS_MIN: reasons.append(f"fps {res.get('fps_mean'):.1f} < {FPS_MIN}")
        if m["missed_gt25_pct"] > MISS_MAX: reasons.append(f"manquées {m['missed_gt25_pct']:.1f} % > {MISS_MAX}")
        if (res.get("frame_p99") or 0) > P99_MAX: reasons.append(f"p99 {res.get('frame_p99'):.1f} ms > {P99_MAX}")
    if spec.get("flow"):
        g, ap_ = ops.get("updates_generated"), ops.get("updates_applied")
        # le retard d'une frame de drainage en fin de mesure (≈ 0,2 % à 10 k/s) n'est pas une perte : seuil 99 % appliquées,
        # et les pertes explicites (file pleine, ops.updates_dropped) doivent rester ≤ 0,1 %
        dr = ops.get("updates_dropped") or 0
        if not g or ap_ is None or ap_ / g < 0.99: reasons.append(f"appliquées {ap_}/{g} < 99 %")
        elif dr / g > 0.001: reasons.append(f"perdues {dr}/{g} > 0,1 %")
    if spec.get("stall_ms") and (ops.get("ui_stall_ms_max") or 0) > spec["stall_ms"]: reasons.append(f"arrêt {ops.get('ui_stall_ms_max')} ms > {spec['stall_ms']}")
    if spec.get("first_display_ms") and (ops.get("time_to_first_display_ms") or 0) > spec["first_display_ms"]:
        reasons.append(f"1er affichage {ops.get('time_to_first_display_ms')} ms > {spec['first_display_ms']}")
    return (not reasons), m, reasons


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--stack", required=True); ap.add_argument("--scenario", required=True, choices=sorted(SCENARIOS))
    ap.add_argument("--variant", action="append", default=[]); ap.add_argument("--duration", type=float, default=12)
    ap.add_argument("--outdir", default=os.path.join(ROOT, "v3/results")); ap.add_argument("--env", action="append", default=[])
    ap.add_argument("--cpu-quota", type=float); ap.add_argument("--cmd")
    ap.add_argument("--n-start", type=int); ap.add_argument("--n-max", type=int); ap.add_argument("--values")
    ap.add_argument("--confirm", type=int, default=3)
    a = ap.parse_args()
    spec = dict(SCENARIOS[a.scenario])
    if a.n_start: spec["start"] = a.n_start
    if a.n_max: spec["max"] = a.n_max
    if a.values: spec["values"] = [int(x) for x in a.values.split(",")]
    cmd = a.cmd
    if not cmd:
        for line in open(os.path.join(ROOT, "v3", "harness", "stacks3.tsv")):
            if line.startswith("#") or not line.strip(): continue
            c = line.rstrip("\n").split("\t")
            if c[0] == a.stack: cmd = c[1]; extra_env = (c[2].replace("--env ", "").split() if len(c) > 2 and c[2] else []); break
        else: sys.exit(f"stack inconnue dans stacks3.tsv: {a.stack}")
    else:
        extra_env = []
    vtag = "".join("-" + v.replace("=", "") for v in a.variant) + (f"-q{a.cpu_quota}" if a.cpu_quota else "")
    name = f"{a.scenario}{vtag}"
    raw = os.path.join(a.outdir, "raw", a.stack, name); os.makedirs(raw, exist_ok=True)
    probes = []

    def probe(n, i=0):
        ns = types.SimpleNamespace(stack=a.stack, cmd=cmd, scenario=a.scenario, duration=a.duration, runs=1, rate=None, shapes=None, cpus=None,
                                   cpu_quota=a.cpu_quota, mem_limit=None, outdir=raw, assets=os.path.join(ROOT, "assets"),
                                   env=list(a.env) + [e for e in extra_env], n=n, variant=list(a.variant),
                                   latency=None, cpuhog=0, resize=100 if spec.get("resize") else 0)
        res = rb.one_run(ns, i, f"n{n}")
        res.pop("samples", None)
        ok, m, reasons = evaluate(res, spec)
        rec = dict(n=n, ok=ok, reasons=reasons, **{k: v for k, v in m.items() if v is not None})
        probes.append(rec)
        print(f"[{a.stack} {name}] n={n} -> {'OK' if ok else 'ÉCHEC'} fps={m.get('fps_mean') and round(m['fps_mean'],1)} "
              f"manquées={m.get('missed_gt25_pct') and round(m['missed_gt25_pct'],1)}% {'; '.join(reasons)}", flush=True)
        return ok

    lo = hi = None; bounded = True
    if "values" in spec:
        for v in spec["values"]:
            if probe(v): lo = v
            else: hi = v; break
        else: bounded = False
    else:
        n = spec["start"]
        while True:
            if probe(n): lo = n; n *= 2
            else: hi = n; break
            if n > spec["max"]:
                bounded = False; break
        if lo is None:  # échec dès le départ : recherche vers le bas (÷2) jusqu'à un point qui tient, au plus 6 fois
            m = hi // 2
            for _ in range(6):
                if m < 1: break
                if probe(m): lo = m; break
                hi = m; m //= 2
        if lo is not None and hi is not None:
            while hi - lo > max(1, int(lo * 0.05)):
                mid = (lo + hi) // 2
                if probe(mid): lo = mid
                else: hi = mid
    # confirmations
    confirmed = None
    if lo is not None:
        cand = lo
        for _ in range(4):
            if all(probe(cand, i + 1) for i in range(a.confirm)): confirmed = cand; break
            below = [p["n"] for p in probes if p["ok"] and p["n"] < cand]
            cand = max(below) if below else None
            if cand is None: break
    over = None
    if confirmed is not None and bounded and "values" not in spec:
        over = confirmed * 2
        probe(over, 9)
    summ = dict(stack=a.stack, scenario=a.scenario, variant=a.variant, cpu_quota=a.cpu_quota, cmd=cmd,
                n_max=confirmed, bounded_above=bounded, first_fail=hi, probes=probes,
                at_nmax=next((p for p in reversed(probes) if p["n"] == confirmed), None),
                at_2x=next((p for p in probes if p["n"] == over), None))
    d = os.path.join(a.outdir, "capacity", a.stack); os.makedirs(d, exist_ok=True)
    json.dump(summ, open(os.path.join(d, f"{name}.json"), "w"), indent=1)
    print(f"RÉSULTAT {a.stack} {name}: N_max={confirmed}{'' if bounded else ' (non borné : limite de balayage atteinte)'} premier échec={hi}")


if __name__ == "__main__":
    main()
