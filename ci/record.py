#!/usr/bin/env python3
"""Écrit result.json pour une plateforme : statut, tailles (brut, gzip, brotli), durées, notes.
usage: record.py --stack S --platform P --status ok|fail|skipped [--artifact PATH]... [--seconds N] [--note TXT] --out DIR
Les journaux de build doivent être copiés par le workflow dans DIR (build.log, run.log, captures PNG...)."""
import argparse, gzip, json, os, subprocess, sys, time
ap = argparse.ArgumentParser()
ap.add_argument("--stack", required=True); ap.add_argument("--platform", required=True)
ap.add_argument("--status", required=True, choices=["ok", "fail", "skipped"])
ap.add_argument("--artifact", action="append", default=[]); ap.add_argument("--seconds", type=float)
ap.add_argument("--note", action="append", default=[]); ap.add_argument("--out", required=True)
a = ap.parse_args(); os.makedirs(a.out, exist_ok=True)
def size_of(p):
    if os.path.isdir(p):
        return sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(p) for f in fs)
    return os.path.getsize(p)
def compressed(p, kind):
    try:
        if os.path.isdir(p): return None
        data = open(p, "rb").read()
        if kind == "gzip": return len(gzip.compress(data, 9))
        import brotli; return len(brotli.compress(data, quality=11))
    except Exception: return None
arts = []
for p in a.artifact:
    if os.path.exists(p):
        arts.append({"path": p, "bytes": size_of(p), "gzip": compressed(p, "gzip"), "brotli": compressed(p, "brotli")})
    else:
        arts.append({"path": p, "missing": True})
res = {"stack": a.stack, "platform": a.platform, "status": a.status, "build_seconds": a.seconds,
       "artifacts": arts, "notes": a.note, "runner": os.environ.get("RUNNER_OS", "") + "/" + os.environ.get("RUNNER_ARCH", ""),
       "sha": os.environ.get("GITHUB_SHA", ""), "date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
json.dump(res, open(os.path.join(a.out, "result.json"), "w"), indent=1)
print(json.dumps(res))
