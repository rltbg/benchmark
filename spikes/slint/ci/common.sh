# Fonctions communes aux scripts de CI (source).
# waitfor <fichier> <motif> <timeout_s> : attend qu'un motif apparaisse dans un journal.
waitfor() {
  local end=$((SECONDS + $3))
  until grep -q -- "$2" "$1" 2>/dev/null; do
    [ "$SECONDS" -ge "$end" ] && return 1
    sleep 0.2
  done
  return 0
}
now_ms() { python3 -c 'import time;print(int(time.time()*1000))'; }
# record <plateforme> <statut> <secondes> <sortie> [--artifact P]... [--note N]...
record() {
  local plat=$1 st=$2 sec=$3 out=$4; shift 4
  python3 "$GITHUB_WORKSPACE/ci/record.py" --stack slint --platform "$plat" --status "$st" --seconds "$sec" --out "$out" "$@"
}
