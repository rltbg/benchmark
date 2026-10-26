#!/usr/bin/env python3 -I
"""Harnais v2 : lance APP sous Xvfb, N fois ; mesure CPU / RSS / PSS de l'extérieur (/proc, 10 Hz)
entre les marqueurs BENCH:measure_start et BENCH:measure_end imprimés par l'app ; dégradation par
taskset / cgroup (quota CPU, limite mémoire) ; latence d'entrée externe pour le scénario `input`.

usage: run_bench.py --stack NAME --cmd "APP [args]" --scenario S --duration D --runs N
                    [--rate R] [--shapes N] [--cpus 0] [--cpu-quota 0.5] [--mem-limit 256] [--env K=V]
`${ROOT}` dans --cmd est remplacé par la racine du dépôt."""
import argparse, json, math, mmap, os, random, statistics, subprocess, sys, tempfile, threading, time

ROOT = os.environ.get("BENCH_ROOT") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLK = os.sysconf("SC_CLK_TCK")
START, END = "BENCH:measure_start", "BENCH:measure_end"


def pct(v, p):
    """Percentile par rang le plus proche (nearest-rank) : rang = ceil(p/100 * n)."""
    if not v:
        return None
    s = sorted(v)
    return s[min(len(s), max(1, math.ceil(p / 100 * len(s)))) - 1]


def children(pid):
    """pid et tous ses descendants (par PPid de /proc ; /proc/PID/task/PID/children n'existe pas sur tous les noyaux)."""
    kids = {}
    for d in os.listdir("/proc"):
        if d.isdigit():
            try:
                ppid = int(open(f"/proc/{d}/stat").read().rsplit(")", 1)[1].split()[1])
                kids.setdefault(ppid, []).append(int(d))
            except (OSError, IndexError, ValueError):
                pass
    out, todo = [], [pid]
    while todo:
        q = todo.pop(); out.append(q); todo += kids.get(q, [])
    return out


def proc_sample(pid):
    try:
        st = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
        ticks = int(st[11]) + int(st[12])
        rss = pss = 0
        for line in open(f"/proc/{pid}/smaps_rollup"):
            if line.startswith("Rss:"):
                rss = int(line.split()[1])
            elif line.startswith("Pss:"):
                pss = int(line.split()[1])
        return ticks, rss, pss
    except (OSError, IndexError, ValueError):
        return None


class Cgroup:
    """cgroup v1 (memory, cpu) créé pour une exécution ; supprimé ensuite."""
    def __init__(self, name, mem_mb=None, cpu_quota=None):
        self.paths = []
        if mem_mb:
            p = f"/sys/fs/cgroup/memory/{name}"; os.makedirs(p, exist_ok=True)
            open(f"{p}/memory.limit_in_bytes", "w").write(str(int(mem_mb * 1024 * 1024)))
            self.paths.append(p); self.mem = p
        else:
            self.mem = None
        if cpu_quota:
            p = f"/sys/fs/cgroup/cpu/{name}"; os.makedirs(p, exist_ok=True)
            open(f"{p}/cpu.cfs_period_us", "w").write("100000")
            open(f"{p}/cpu.cfs_quota_us", "w").write(str(int(cpu_quota * 100000)))
            self.paths.append(p)

    def enter(self):  # appelé dans le fils avant exec
        for p in self.paths:
            open(f"{p}/cgroup.procs", "w").write(str(os.getpid()))

    def stats(self):
        if not self.mem:
            return {}
        d = {"mem_failcnt": int(open(f"{self.mem}/memory.failcnt").read())}
        for line in open(f"{self.mem}/memory.oom_control"):
            k, v = line.split()
            if k == "oom_kill":
                d["oom_kills"] = int(v)
        return d

    def remove(self):
        for p in self.paths:
            for _ in range(20):
                try:
                    os.rmdir(p); break
                except OSError:
                    time.sleep(0.1)


class FrameBuffer:
    """Accès au framebuffer de Xvfb (-fbdir) pour la latence d'entrée."""
    W, H = 1280, 720

    def __init__(self, d):
        f = open(os.path.join(d, "Xvfb_screen0"), "rb")
        self.m = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
        self.off = os.path.getsize(os.path.join(d, "Xvfb_screen0")) - self.W * self.H * 4
        # 10x10 points dans le carré 200x200 en (100,100), marge de 20 px
        self.idx = [self.off + ((120 + 16 * j) * self.W + (120 + 16 * i)) * 4 for j in range(10) for i in range(10)]

    def square(self):
        return bytes(self.m[k] ^ self.m[k + 1] ^ self.m[k + 2] for k in self.idx)


def input_latency(fb, disp, stop, result, n=40, seed=7):
    """Envoie jusqu'à n appuis sur Espace (XTest) à intervalles aléatoires et mesure le délai jusqu'au changement
    des pixels du carré. S'arrête dès que `stop` est levé (fin de la mesure de l'app) : un appui envoyé après
    n'est ni compté ni déclaré perdu. Tourne dans un thread pour ne pas bloquer l'échantillonnage CPU/RAM."""
    rnd = random.Random(seed)
    env = dict(os.environ, DISPLAY=disp)
    xd = subprocess.Popen(["xdotool", "-"], stdin=subprocess.PIPE, env=env, text=True)
    # sans gestionnaire de fenêtres, certains toolkits ignorent le clavier tant qu'ils n'ont pas reçu FocusIn :
    # le harnais donne le focus (XSetInputFocus) à la plus grande fenêtre de premier niveau, pour toutes les stacks
    wid = None
    for l in subprocess.run(["xwininfo", "-root", "-tree"], capture_output=True, text=True, env=env).stdout.splitlines():
        l = l.strip()
        if l.startswith("0x") and "1280x720" in l:
            wid = l.split()[0]; break
    xd.stdin.write("mousemove 640 360\n" + (f"windowfocus {wid}\n" if wid else "")); xd.stdin.flush(); time.sleep(0.5)
    lat, lost = [], 0
    for _ in range(n):
        if stop.wait(rnd.uniform(0.15, 0.35)):
            break
        before = fb.square(); t0 = time.perf_counter()
        xd.stdin.write("key space\n"); xd.stdin.flush()
        while True:
            now = time.perf_counter()
            if fb.square() != before:
                lat.append((now - t0) * 1000); break
            if stop.is_set():
                break  # la mesure s'est terminée pendant l'attente : appui non compté
            if now - t0 > 1.0:
                lost += 1; break
            time.sleep(0.00025)
    xd.stdin.close(); xd.wait()
    result["lat"], result["lost"] = lat, lost


def one_run(a, i, tag):
    out = f"{a.outdir}/{tag}.run{i}.json"; so = f"{a.outdir}/{tag}.run{i}.stdout"
    for f in (out,):
        if os.path.exists(f): os.remove(f)
    n = 90 + i % 9
    while os.path.exists(f"/tmp/.X11-unix/X{n}") or os.path.exists(f"/tmp/.X{n}-lock"):
        n += 10  # display occupé par un autre processus : en prendre un libre
    disp = f":{n}"; fbdir = tempfile.mkdtemp(prefix="fb")
    xargs = ["Xvfb", disp, "-screen", "0", "1280x720x24", "-nolisten", "tcp"] + (["-fbdir", fbdir] if a.scenario == "input" else [])
    xv = subprocess.Popen(xargs, stderr=subprocess.DEVNULL); time.sleep(0.8)
    env = dict(os.environ, DISPLAY=disp, LIBGL_ALWAYS_SOFTWARE="1", BENCH_ROOT=ROOT)
    for e in a.env:
        k, v = e.split("=", 1); env[k] = v
    cmd = a.cmd.replace("${ROOT}", ROOT).split() + ["--scenario", a.scenario, "--duration", str(a.duration), "--out", out, "--assets", a.assets]
    if a.rate: cmd += ["--rate", str(a.rate)]
    if a.shapes: cmd += ["--shapes", str(a.shapes)]
    if a.cpus: cmd = ["taskset", "-c", a.cpus] + cmd
    cg = Cgroup(f"bench-{os.getpid()}-{i}", a.mem_limit, a.cpu_quota) if (a.mem_limit or a.cpu_quota) else None
    t0 = time.perf_counter()
    p = subprocess.Popen(cmd, env=env, stdout=open(so, "w"), stderr=subprocess.STDOUT, preexec_fn=cg.enter if cg else None)
    samples, last, marks = [], {}, {}
    lat = lost = None; inp = None; stop = threading.Event(); result = {}
    while p.poll() is None:
        now = time.perf_counter() - t0
        if now > a.duration + 120:
            p.kill(); break
        rss = pss = 0
        for q in children(p.pid):
            r = proc_sample(q)
            if r:
                last[q] = r[0]; rss += r[1]; pss += r[2]
        samples.append((now, sum(last.values()) / CLK, rss, pss))
        # marqueurs dans stdout
        try:
            txt = open(so, errors="replace").read()
            if START in txt and "start" not in marks: marks["start"] = now
            if END in txt and "end" not in marks: marks["end"] = now; stop.set()
        except OSError:
            pass
        if a.scenario == "input" and "start" in marks and inp is None:
            inp = threading.Thread(target=input_latency, args=(FrameBuffer(fbdir), disp, stop, result), daemon=True); inp.start()
        time.sleep(0.1)
    rc = p.wait(); wall = time.perf_counter() - t0
    stop.set()
    if inp is not None:
        inp.join(timeout=5); lat, lost = result.get("lat"), result.get("lost")
    # l'app peut imprimer le marqueur de fin puis quitter entre deux lectures : relire une dernière fois
    try:
        txt = open(so, errors="replace").read()
        if START in txt and "start" not in marks: marks["start"] = wall
        if END in txt and "end" not in marks: marks["end"] = wall
    except OSError:
        pass
    cgs = cg.stats() if cg else {}
    if cg: cg.remove()
    xv.terminate(); xv.wait()
    res = {"run": i, "rc": rc, "wall_s": wall, "markers": "start" in marks and "end" in marks, **cgs}
    res["oom"] = bool(cgs.get("oom_kills")) or rc == -9
    if "start" in marks: res["startup_to_measure_s"] = marks["start"]
    if os.path.exists(out):
        try:
            d = json.load(open(out)); fm = d.get("frame_ms") or []
        except (OSError, ValueError) as ex:
            d, fm = {}, []; res["json_error"] = str(ex)
        res.update(frames=len(fm), ops=d.get("ops"), checksum=d.get("checksum"), unsupported=d.get("unsupported"), version=d.get("version"))
        if fm:
            res.update(fps_mean=1000 * len(fm) / sum(fm), frame_p50=pct(fm, 50), frame_p95=pct(fm, 95), frame_p99=pct(fm, 99),
                       frame_max=max(fm), dropped_gt16_7=sum(1 for x in fm if x > 16.7), dropped_gt8_3=sum(1 for x in fm if x > 8.3), dropped_gt25=sum(1 for x in fm if x > 25.0),
                       dropped_ms_gt16_7=sum(x - 16.7 for x in fm if x > 16.7))
    if lat is not None:
        res.update(input_latency_ms_p50=pct(lat, 50), input_latency_ms_p95=pct(lat, 95), input_latency_ms_max=max(lat) if lat else None,
                   input_events=len(lat), input_lost=lost, input_latency_all=[round(x, 2) for x in lat])
    # fenêtre de mesure : entre les marqueurs (sinon toute l'exécution, signalé)
    ws, we = marks.get("start"), marks.get("end")
    win = [s for s in samples if ws is not None and we is not None and ws <= s[0] <= we] or samples
    res["window"] = "markers" if (ws is not None and we is not None and len(win) > 2) else "whole_run"
    if len(win) >= 2:
        rss = [s[2] for s in win]; pss = [s[3] for s in win]; q = max(1, len(win) // 4)
        dt = win[-1][0] - win[0][0]
        res.update(rss_kb_median=statistics.median(rss), rss_kb_peak=max(rss), pss_kb_median=statistics.median(pss), pss_kb_peak=max(pss),
                   pss_kb_first_quarter=statistics.median(pss[:q]), pss_kb_last_quarter=statistics.median(pss[-q:]),
                   cpu_pct_core_mean=100 * (win[-1][1] - win[0][1]) / dt if dt > 0 else None)
        peaks = [(win[j][1] - win[j - 10][1]) * 100 / (win[j][0] - win[j - 10][0]) for j in range(10, len(win))]
        res["cpu_pct_core_peak_1s"] = max(peaks) if peaks else None
    res["samples"] = [[round(s[0], 1), round(s[1], 2), s[2], s[3]] for s in samples[::10]]
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--stack", required=True); ap.add_argument("--cmd", required=True)
    ap.add_argument("--scenario", required=True); ap.add_argument("--duration", type=float, default=10)
    ap.add_argument("--runs", type=int, default=10); ap.add_argument("--rate", type=int); ap.add_argument("--shapes", type=int)
    ap.add_argument("--cpus"); ap.add_argument("--cpu-quota", type=float); ap.add_argument("--mem-limit", type=float)
    ap.add_argument("--outdir", default=os.path.join(ROOT, "results")); ap.add_argument("--assets", default=os.path.join(ROOT, "assets"))
    ap.add_argument("--env", action="append", default=[])
    a = ap.parse_args(); os.makedirs(a.outdir, exist_ok=True)
    tag = (f"{a.stack}-{a.scenario}" + (f"-r{a.rate}" if a.rate else "") + (f"-s{a.shapes}" if a.shapes else "")
           + (f"-cpu{a.cpus}" if a.cpus else "") + (f"-q{a.cpu_quota}" if a.cpu_quota else "") + (f"-m{int(a.mem_limit)}" if a.mem_limit else ""))
    runs = []
    for i in range(a.runs):
        r = one_run(a, i, tag); runs.append(r)
        print(tag, "run", i, {k: (round(v, 2) if isinstance(v, float) else v) for k, v in r.items() if k not in ("samples", "ops", "input_latency_all")}, flush=True)
    keys = ["fps_mean", "frame_p50", "frame_p95", "frame_p99", "frame_max", "dropped_gt16_7", "dropped_gt8_3", "dropped_gt25", "rss_kb_median", "rss_kb_peak",
            "pss_kb_median", "pss_kb_peak", "cpu_pct_core_mean", "cpu_pct_core_peak_1s", "input_latency_ms_p50", "input_latency_ms_p95", "startup_to_measure_s", "wall_s"]
    def agg(k):
        v = [r[k] for r in runs if r.get(k) is not None and r["rc"] == 0]
        return {"median": statistics.median(v), "p95": pct(v, 95), "min": min(v), "max": max(v), "n": len(v)} if v else None
    summ = {"tag": tag, "stack": a.stack, "scenario": a.scenario, "runs": len(runs),
            "ok_runs": sum(1 for r in runs if r["rc"] == 0 and "json_error" not in r), "oom_runs": sum(1 for r in runs if r.get("oom")),
            "markers_ok": sum(1 for r in runs if r.get("markers")), "agg": {k: agg(k) for k in keys}}
    json.dump({"summary": summ, "runs": runs}, open(f"{a.outdir}/{tag}.summary.json", "w"), indent=1)
    print("SUMMARY", json.dumps(summ))


if __name__ == "__main__":
    main()
