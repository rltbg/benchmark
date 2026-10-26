# Environnement de build (source env.sh)
B=/home/user/eval/stacks/karamel
FS=/tmp/claude-0/-home-user/950b1927-9cd8-5fd1-8b28-a00f39a279fc/scratchpad/dl/fstar/fstar
export KRML_HOME=/tmp/claude-0/-home-user/950b1927-9cd8-5fd1-8b28-a00f39a279fc/scratchpad/karamel
export KRML=/home/user/eval/stacks/karamel/krml-prefix/bin/krml
export PATH=$FS/bin:$PATH
FSTAR_FLAGS="--include $KRML_HOME/krmllib --cache_checked_modules --cache_dir $B/cache"
