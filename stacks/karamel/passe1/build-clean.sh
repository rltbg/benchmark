#!/bin/bash
# Pipeline complet Low* -> C -> .a/.so -> tests -> bench. Usage : ./build.sh   (logs dans logs/9x-*.log)
set -e
cd /home/user/eval/stacks/karamel; . ./env-clean.sh
true
mkdir -p krml-clean gen-clean lib-clean bin-clean logs
echo "== 1. F* verify+extract";  fstar.exe $FSTAR_FLAGS --codegen krml --extract 'Core_Low C.Loops' --odir krml-clean Core_Low.fst 2>&1 | tee logs/90-fstar.log | grep -E "Error|discharged"
echo "== 2. krml (F*->C)";       $KRML -skip-compilation -skip-makefiles -tmpdir gen-clean krml-clean/out.krml 2>&1 | tee logs/91-krml.log | grep -iE "warn|error|fatal" || true
INC="-I karamel-clean/out/include/krml -I karamel-clean/out/lib/krml/dist/minimal -I gen-clean -I /home/user/eval/core"
echo "== 3. gcc -O2 -> .o/.a/.so"
gcc -O2 -Wall -Wextra -Werror -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -c gen-clean/Core_Low.c -o lib-clean/Core_Low.o 2>&1 | tee logs/92-cc.log
gcc -O2 -fPIC -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -shared gen-clean/Core_Low.c -o lib-clean/libcorelow.so
ar rcs lib-clean/libcorelow.a lib-clean/Core_Low.o
gcc -O2 -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC -c tests/kr_shim.c -o lib-clean/kr_shim.o
ar rcs lib-clean/libcorelow_shim.a lib-clean/kr_shim.o lib-clean/Core_Low.o
echo "== 4. tests"
R=/home/user/eval/core/target/release/libbenchcore.a
gcc -O2 -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC tests/test_core.c lib-clean/libcorelow_shim.a $R -lpthread -ldl -lm -o bin-clean/test_core
gcc -O1 -g -fsanitize=address,undefined -fwrapv -std=c11 -D_DEFAULT_SOURCE $INC tests/test_core.c gen-clean/Core_Low.c tests/kr_shim.c $R -lpthread -ldl -lm -o bin-clean/test_core_asan
./bin-clean/test_core 2>&1 | tee logs/93-test.log
./bin-clean/test_core_asan 2>&1 | tee logs/94-test-asan-ubsan.log
echo "== 5. bench binaries"
sed 's/C->Rust C-ABI/Rust lien statique (re-mesure)/' /home/user/eval/core/callbench.c > tests/callbench_rust.c
sed 's/C->Rust C-ABI/Low*->C via shim (.a)/' tests/callbench_kr.c > tests/callbench_krshim.c
gcc -O2 -I /home/user/eval/core tests/callbench_rust.c $R -lpthread -ldl -lm -o bin-clean/cb_rust
gcc -O2 -I /home/user/eval/core -Dcore_validate=kr_validate -Dcore_parse=kr_parse -Dcore_hash=kr_hash tests/callbench_krshim.c lib-clean/libcorelow_shim.a -o bin-clean/cb_kr_shim
gcc -O2 $INC tests/callbench_direct.c lib-clean/libcorelow.a -o bin-clean/cb_kr_direct
# variante : shim + code généré dans la même unité de compilation (inlining possible)
cat > tests/kr_samettu.c <<'EOT'
#include "Core_Low.c"
#include "kr_shim.c"
EOT
gcc -O2 -fwrapv $INC -I tests -I /home/user/eval/core -Dcore_validate=kr_validate -Dcore_parse=kr_parse -Dcore_hash=kr_hash tests/callbench_krshim.c tests/kr_samettu.c -o bin-clean/cb_kr_tu
ls -la lib bin
