import subprocess, time, statistics, os, sys, resource
AOT='/home/user/fstar-aot/aot-out/coreapp'
JIT=['dotnet','/home/user/fstar-aot/fsproj/app/bin/Release/net10.0/linux-x64/coreapp.dll']
def drop():
    try:
        os.sync(); open('/proc/sys/vm/drop_caches','w').write('3'); return True
    except Exception: return False
def p95(v):
    s=sorted(v); k=max(0,min(len(s)-1,int(round(0.95*len(s)+0.5))-1)); return s[k]
def startup(cmd,n=10,drop_cache=False):
    ts=[]
    for _ in range(n):
        if drop_cache: drop()
        t=time.perf_counter(); r=subprocess.run(cmd+['once'],capture_output=True); ts.append((time.perf_counter()-t)*1000)
        assert r.returncode==0 and r.stdout.strip()==b'4F9F2CAB', r
    return ts
def rss(cmd,n=10):
    out=[]
    for _ in range(n):
        p=subprocess.Popen(cmd+['idle'],stdout=subprocess.PIPE)
        p.stdout.readline(); time.sleep(1.0)
        d=dict(l.split(':',1) for l in open(f'/proc/{p.pid}/status').read().splitlines() if ':' in l)
        out.append((int(d['VmRSS'].split()[0]),int(d['VmHWM'].split()[0])))
        p.kill(); p.wait()
    return out
can_drop=drop(); print("drop_caches possible:",can_drop)
for name,cmd in (('AOT',[AOT]),('JIT (dotnet coreapp.dll)',JIT)):
    ts=startup(cmd,10,can_drop)
    print(f"{name} startup ms (10 runs, caches {'dropped' if can_drop else 'NOT dropped'}): min={min(ts):.1f} median={statistics.median(ts):.1f} p95={p95(ts):.1f} max={max(ts):.1f}")
    print("  runs:",[round(t,1) for t in ts])
    r=rss(cmd,10)
    print(f"  idle RSS kB: median={statistics.median([a for a,b in r]):.0f} min={min(a for a,b in r)} max={max(a for a,b in r)}; peak(VmHWM) median={statistics.median([b for a,b in r]):.0f}")
