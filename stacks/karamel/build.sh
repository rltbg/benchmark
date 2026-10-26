#!/bin/bash
# Porte 2, passe 2 : F* v2026.10.25 (Pulse) -> Custard --codegen KrmlC -> KaRaMeL HEAD -> C -> tests/ASan/différentiel/bench.
# Low* (Core_Low.fst) ne se vérifie plus avec cette version de F* : voir docs/fstar/RAPPORT-KRML.md.
# Toutes les sorties vont dans $BUILD (hors dépôt). Usage : ./build.sh   [BENCH=1 ./build.sh pour le débit]
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=${BENCH_ROOT:-$(cd "$HERE/../.." && pwd)}
FSTAR_HOME=${FSTAR_HOME:-/opt/fstar-2026.10.25/fstar}
KRML_HOME=${KRML_HOME:-/opt/krml/karamel/out}          # make LOWSTAR=false local-install de karamel HEAD
RUST_LIB=${RUST_LIB:-/opt/krml/rust-target/release/libbenchcore.a}   # cœur Rust (cargo build --release, Rust 1.99.1)
BUILD=${BUILD:-/opt/krml/p2/build}
export PATH=$FSTAR_HOME/bin:$PATH
K=$KRML_HOME/bin/krml
mkdir -p "$BUILD"/{src,krml,gen,gen-direct,lib,bin}
cp "$HERE/Core_Pulse.fst" "$BUILD/src/"
cd "$BUILD/src"
F="--z3version 5.3.0 --cache_checked_modules --cache_dir $BUILD/cache"
E="--custard_entry Core_Pulse.validate --custard_entry Core_Pulse.parse --custard_entry Core_Pulse.hash"
fstar.exe --version | head -1; $K -version
echo "== 1. F* : vérification Pulse";       fstar.exe $F Core_Pulse.fst
echo "== 2. F* : extraction KrmlC";          fstar.exe $F --codegen KrmlC $E Core_Pulse.fst -o "$BUILD/krml/Core_Pulse.krml"
echo "== 3. krml HEAD -> C";                 $K -skip-compilation -skip-makefiles -no-prefix Custard -tmpdir "$BUILD/gen" "$BUILD/krml/Core_Pulse.krml"
echo "== 3b. (comparaison) Custard --codegen C direct"
fstar.exe $F --codegen C --custard_monomorphize_types true $E Core_Pulse.fst -o "$BUILD/gen-direct/Core_Pulse.c"
INC="-I $KRML_HOME/include/krml -I $KRML_HOME/lib/krml/dist/minimal -I $BUILD/gen -I $ROOT/core"
CF="-O2 -fwrapv -std=c11 -D_DEFAULT_SOURCE"
cd "$BUILD"
echo "== 4. gcc -O2 -Wall -Wextra -Werror"
gcc $CF -Wall -Wextra -Werror $INC -c gen/Custard.c -o lib/Custard.o
gcc $CF -Wall -Wextra -Werror -c gen-direct/Core_Pulse.c -o lib/Core_Pulse_direct.o
gcc $CF -fPIC $INC -shared gen/Custard.c -o lib/libcorepulse.so
ar rcs lib/libcorepulse.a lib/Custard.o
gcc $CF $INC -c "$HERE/tests/kr_shim_pulse.c" -o lib/kr_shim.o
ar rcs lib/libcorepulse_shim.a lib/kr_shim.o lib/Custard.o
size lib/Custard.o lib/Core_Pulse_direct.o
echo "== 5. tests (2634 vérifications attendues) + différentiel 2M vs Rust"
gcc $CF $INC "$HERE/tests/test_core_pulse.c" lib/libcorepulse_shim.a "$RUST_LIB" -lpthread -ldl -lm -o bin/test_core
gcc -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC \
    "$HERE/tests/test_core_pulse.c" gen/Custard.c "$HERE/tests/kr_shim_pulse.c" "$RUST_LIB" -lpthread -ldl -lm -o bin/test_core_asan
./bin/test_core
./bin/test_core_asan
echo "== 6. binaires de bench"
sed 's/C->Rust C-ABI/Rust lien statique/' "$ROOT/core/callbench.c" > src/cb_rust.c
sed 's/C->Rust C-ABI/Pulse->C via shim (.a)/' "$ROOT/core/callbench.c" > src/cb_shim.c
gcc -O2 -I "$ROOT/core" src/cb_rust.c "$RUST_LIB" -lpthread -ldl -lm -o bin/cb_rust
gcc -O2 -I "$ROOT/core" -Dcore_validate=kr_validate -Dcore_parse=kr_parse -Dcore_hash=kr_hash src/cb_shim.c lib/libcorepulse_shim.a -o bin/cb_kr_shim
gcc -O2 $INC "$HERE/tests/callbench_direct_pulse.c" lib/libcorepulse.a -o bin/cb_kr_direct
if [ "${BENCH:-0}" = 1 ]; then
  echo "loadavg: $(cat /proc/loadavg)"
  for r in 1 2 3 4 5; do for b in cb_rust cb_kr_shim cb_kr_direct; do taskset -c 2 ./bin/$b; done; done
  echo "loadavg: $(cat /proc/loadavg)"
fi
