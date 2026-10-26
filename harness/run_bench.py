#!/usr/bin/env python3 -I
"""Harnais : lance APP sous Xvfb, N fois, échantillonne CPU/RSS (/proc, 10 Hz), agrège.
usage: run_bench.py --stack NAME --cmd "APP args-before-scenario" --scenario S --duration D --runs N [--rate R] [--shapes N] [--cpus 0-3] [--outdir DIR]"""
import argparse, json, os, subprocess, time, statistics, signal, shutil, sys
def pct(v, p):
    if not v: return None
    s = sorted(v); k = min(len(s)-1, max(0, int(round(p/100*len(s)+0.5))-1)); return s[k]
def read_proc(pid):
    try:
        st = open(f"/proc/{pid}/stat").read().rsplit(")",1)[1].split()
        ticks = int(st[11]) + int(st[12])
        rss = 0
        for l in open(f"/proc/{pid}/status"):
            if l.startswith("VmRSS:"): rss = int(l.split()[1])
        return ticks, rss
    except Exception: return None
def tree(pid):
    pids=[pid]
    try:
        for l in subprocess.run(["pgrep","-P",str(pid)],capture_output=True,text=True).stdout.split():
            pids += tree(int(l))
    except Exception: pass
    return pids
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--stack", required=True); ap.add_argument("--cmd", required=True)
    ap.add_argument("--scenario", required=True); ap.add_argument("--duration", type=float, default=10)
    ap.add_argument("--runs", type=int, default=10); ap.add_argument("--rate", type=int); ap.add_argument("--shapes", type=int)
    ap.add_argument("--cpus", default=None); ap.add_argument("--outdir", default="/home/user/eval/results")
    ap.add_argument("--assets", default="/home/user/eval/assets"); ap.add_argument("--env", action="append", default=[])
    a = ap.parse_args(); os.makedirs(a.outdir, exist_ok=True)
    tag = f"{a.stack}-{a.scenario}" + (f"-r{a.rate}" if a.rate else "") + (f"-s{a.shapes}" if a.shapes else "") + (f"-cpu{a.cpus}" if a.cpus else "")
    clk = os.sysconf("SC_CLK_TCK"); runs = []
    for i in range(a.runs):
        out = f"{a.outdir}/{tag}.run{i}.json"
        if os.path.exists(out): os.remove(out)
        xd = f":{90+i%9}"
        xv = subprocess.Popen(["Xvfb", xd, "-screen", "0", "1280x720x24", "-nolisten", "tcp"], stderr=subprocess.DEVNULL)
        time.sleep(0.6)
        env = dict(os.environ, DISPLAY=xd, LIBGL_ALWAYS_SOFTWARE="1")
        for e in a.env: k, v = e.split("=", 1); env[k] = v
        cmd = a.cmd.split() + ["--scenario", a.scenario, "--duration", str(a.duration), "--out", out, "--assets", a.assets]
        if a.rate: cmd += ["--rate", str(a.rate)]
        if a.shapes: cmd += ["--shapes", str(a.shapes)]
        if a.cpus: cmd = ["taskset", "-c", a.cpus] + cmd
        t0 = time.perf_counter()
        p = subprocess.Popen(cmd, env=env, stdout=open(f"{a.outdir}/{tag}.run{i}.stdout","w"), stderr=subprocess.STDOUT)
        samples = []; last = {}; first_rss_t = None
        while p.poll() is None:
            if time.perf_counter() - t0 > a.duration + 120: p.kill(); break
            tot_t = 0; tot_r = 0
            for q in tree(p.pid):
                r = read_proc(q)
                if r: tot_t += r[0]; tot_r += r[1]; last[q] = r[0]
            samples.append((time.perf_counter() - t0, sum(last.values()) / clk, tot_r)); time.sleep(0.1)
        rc = p.wait(); wall = time.perf_counter() - t0
        xv.terminate(); xv.wait()
        res = {"run": i, "rc": rc, "wall_s": wall}
        if os.path.exists(out):
            try: d = json.load(open(out)); fm = d.get("frame_ms", [])
            except Exception as ex: d = {}; fm = []; res["json_error"] = str(ex)
            res.update(frames=len(fm), ops=d.get("ops"), checksum=d.get("checksum"))
            if fm:
                res.update(fps_mean=1000*len(fm)/sum(fm), frame_p50=pct(fm,50), frame_p95=pct(fm,95), frame_p99=pct(fm,99), frame_max=max(fm),
                           dropped_gt16_7=sum(1 for x in fm if x>16.7), dropped_gt8_3=sum(1 for x in fm if x>8.3),
                           dropped_ms_gt16_7=sum(x-16.7 for x in fm if x>16.7))
        if samples:
            rss = [s[2] for s in samples]; q = len(rss)//4
            cpu = samples[-1][1]-samples[0][1]; dt = samples[-1][0]-samples[0][0]
            # CPU moyen en % d'un cœur ; pic sur fenêtres de 1 s
            win = [ (samples[j][1]-samples[j-10][1])*100 for j in range(10,len(samples))]
            res.update(rss_kb_median=statistics.median(rss), rss_kb_peak=max(rss), rss_kb_first_quarter=statistics.median(rss[:max(q,1)]),
                       rss_kb_last_quarter=statistics.median(rss[-max(q,1):]), cpu_pct_core_mean=100*cpu/dt if dt>0 else None, cpu_pct_core_peak_1s=max(win) if win else None)
            res["samples"] = [[round(s[0],1), round(s[1],2), s[2]] for s in samples[::10]]
        runs.append(res); print(tag, "run", i, {k: (round(v,2) if isinstance(v,float) else v) for k,v in res.items() if k not in ("samples","ops")}, flush=True)
    def agg(k):
        v = [r[k] for r in runs if r.get(k) is not None]
        return {"median": statistics.median(v), "p95": pct(v,95), "min": min(v), "max": max(v)} if v else None
    keys = ["fps_mean","frame_p95","frame_p99","frame_max","dropped_gt16_7","dropped_gt8_3","rss_kb_median","rss_kb_peak","rss_kb_last_quarter","cpu_pct_core_mean","cpu_pct_core_peak_1s","wall_s"]
    summ = {"tag": tag, "stack": a.stack, "scenario": a.scenario, "runs": len(runs), "ok_runs": sum(1 for r in runs if r["rc"]==0 and r.get("frames",1)>=0 and "json_error" not in r),
            "agg": {k: agg(k) for k in keys}}
    json.dump({"summary": summ, "runs": runs}, open(f"{a.outdir}/{tag}.summary.json","w"), indent=1)
    print("SUMMARY", json.dumps(summ))
main()
