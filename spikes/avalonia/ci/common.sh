# Fonctions communes (sourcees). Ne rien executer ici.
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1 AVALONIA_TELEMETRY_OPTOUT=1
PY=$(command -v python3 || command -v python)
# first_errors <log> : premieres lignes d'erreur exactes (pour les notes)
first_errors() { grep -E "error [A-Z]+[0-9]+|error:|Error:|: error " "$1" 2>/dev/null | sed 's/ \[\/.*//' | sort -u | head -${2:-6}; }
# timed <var> cmd... : lance cmd, met la duree (s) dans $var, retourne son code
timed() { local __v="$1"; shift; local s; s=$(date +%s); "$@"; local rc=$?; printf -v "$__v" '%s' "$(( $(date +%s) - s ))"; return $rc; }
