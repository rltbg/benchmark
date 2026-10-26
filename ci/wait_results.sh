#!/usr/bin/env bash
# Attend les résultats d'une exécution CI (poussée par le workflow dans la branche ci-results).
# usage: ci/wait_results.sh <stack> [sha=HEAD] [timeout_s=2400]   -> extrait dans /tmp/ci/<stack>/<sha>/ et affiche le résumé
set -u
stack="$1"; sha="${2:-$(git rev-parse HEAD)}"; timeout="${3:-2400}"; t0=$(date +%s)
while :; do
  git fetch -q origin ci-results:refs/remotes/origin/ci-results 2>/dev/null
  if git cat-file -e "origin/ci-results:${stack}/${sha}/DONE" 2>/dev/null; then break; fi
  if [ $(( $(date +%s) - t0 )) -gt "$timeout" ]; then echo "TIMEOUT après ${timeout}s : pas de résultat pour ${stack}/${sha}"; exit 2; fi
  sleep 30
done
dest="/tmp/ci/${stack}/${sha}"; rm -rf "$dest"; mkdir -p "$dest"; git archive "origin/ci-results" "${stack}/${sha}" | tar -x -C /tmp/ci
echo "résultats dans $dest ($(( $(date +%s) - t0 )) s d'attente)"
for f in "$dest"/res-*/result.json; do [ -f "$f" ] && python3 -c "
import json,sys; r=json.load(open('$f')); a=r['artifacts'][0] if r['artifacts'] else {}
print(f\"{r['platform']:16s} {r['status']:8s} build={r.get('build_seconds')}s artefact={a.get('bytes')} octets  notes={r['notes'][:2]}\")"; done
