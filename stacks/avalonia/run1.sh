#!/bin/bash
# usage: run1.sh scenario duration [extra args]  (smoke test, Xvfb :71 must run)
O=${BIN:-/home/user/eval/stacks/avalonia/out/aot}
sc=$1; d=$2; shift 2
cd $O && DISPLAY=:71 LIBGL_ALWAYS_SOFTWARE=1 ./avalonia-bench --scenario $sc --duration $d --out /tmp/claude-0/$sc.json --assets /home/user/eval/assets "$@" 2>&1 | tail -15
python3 - $sc <<'PY'
import json,sys;d=json.load(open('/tmp/claude-0/%s.json'%sys.argv[1]));f=sorted(d['frame_ms']);n=len(f)
if n: print('frames',n,'fps',1000*n/sum(f),'p50',f[n//2],'p95',f[int(n*.95)],'max',f[-1])
print(d['ops'])
PY
