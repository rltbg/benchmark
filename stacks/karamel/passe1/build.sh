#!/bin/bash
# Pipeline complet Low* -> C -> .a/.so -> tests -> bench. Usage : ./build.sh   (logs dans logs/5x-*.log)
set -e
cd /home/user/eval/stacks/karamel; . ./env.sh
export OCAMLPATH=/home/user/eval/stacks/karamel/sysroot/usr/lib/ocaml:$(ocamlfind printconf path | tr '\n' ':')
mkdir -p krml gen lib bin logs
echo "== 1. F* verify+extract";  fstar.exe $FSTAR_FLAGS --codegen krml --extract 'Core_Low C.Loops' --odir krml Core_Low.fst 2>&1 | tee logs/50-fstar.log | grep -E "Error|discharged"
echo "== 2. krml (F*->C)";       $KRML -skip-compilation -skip-makefiles -tmpdir gen krml/out.krml 2>&1 | tee logs/51-krml.log | grep -iE "warn|error|fatal" || true
INC="-I krml-prefix/include/krml -I krml-prefix/lib/krml/dist/minimal -I gen -I /home/user/eval/core"
echo "== 3. gcc -O2 -> .o/.a/.so"
gcc -O2 -Wall -Wextra -Werror -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -c gen/Core_Low.c -o lib/Core_Low.o 2>&1 | tee logs/52-cc.log
gcc -O2 -fPIC -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -shared gen/Core_Low.c -o lib/libcorelow.so
ar rcs lib/libcorelow.a lib/Core_Low.o
gcc -O2 -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -c tests/kr_shim.c -o lib/kr_shim.o
ar rcs lib/libcorelow_shim.a lib/kr_shim.o lib/Core_Low.o
echo "== 4. tests"
R=/home/user/eval/core/target/release/libbenchcore.a
gcc -O2 -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC tests/test_core.c lib/libcorelow_shim.a $R -lpthread -ldl -lm -o bin/test_core
gcc -O1 -g -fsanitize=address,undefined -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC tests/test_core.c gen/Core_Low.c tests/kr_shim.c $R -lpthread -ldl -lm -o bin/test_core_asan
./bin/test_core 2>&1 | tee logs/53-test.log
./bin/test_core_asan 2>&1 | tee logs/54-test-asan-ubsan.log
echo "== 5. bench binaries"
sed 's/C->Rust C-ABI/Rust lien statique (re-mesure)/' /home/user/eval/core/callbench.c > tests/callbench_rust.c
sed 's/C->Rust C-ABI/Low*->C via shim (.a)/' tests/callbench_kr.c > tests/callbench_krshim.c
gcc -O2 -I /home/user/eval/core tests/callbench_rust.c $R -lpthread -ldl -lm -o bin/cb_rust
gcc -O2 -I /home/user/eval/core -Dcore_validate=kr_validate -Dcore_parse=kr_parse -Dcore_hash=kr_hash tests/callbench_krshim.c lib/libcorelow_shim.a -o bin/cb_kr_shim
gcc -O2 $INC tests/callbench_direct.c lib/libcorelow.a -o bin/cb_kr_direct
# variante : shim + code généré dans la même unité de compilation (inlining possible)
cat > tests/kr_samettu.c <<'EOT'
#include "Core_Low.c"
#include "kr_shim.c"
EOT
gcc -O2 -fwrapv $INC -I tests -I /home/user/eval/core -Dcore_validate=kr_validate -Dcore_parse=kr_parse -Dcore_hash=kr_hash tests/callbench_krshim.c tests/kr_samettu.c -o bin/cb_kr_tu
ls -la lib bin
