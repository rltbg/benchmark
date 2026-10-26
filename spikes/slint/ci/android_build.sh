#!/usr/bin/env bash
# Android : voie officielle (xbuild, `x build`), repli cargo-ndk + aapt2/apksigner. usage: android_build.sh <OUT>
# écrit $OUT/slintspike.apk et $OUT/build-route.txt
set -uo pipefail
OUT=$(realpath -m "$1"); mkdir -p "$OUT"
cd "$GITHUB_WORKSPACE/spikes/slint"; source ci/common.sh
export ANDROID_HOME=${ANDROID_HOME:-/usr/local/lib/android/sdk}
NDK=$(ls -d "$ANDROID_HOME"/ndk/27.3.* 2>/dev/null | head -1); export ANDROID_NDK_HOME=$NDK ANDROID_NDK_ROOT=$NDK
BT=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)
{ rustc -Vv; java -version; echo "NDK=$NDK"; echo "BUILD_TOOLS=$BT"; ls "$ANDROID_HOME/platforms"; } > "$OUT/versions.txt" 2>&1
rustup target add x86_64-linux-android aarch64-linux-android >> "$OUT/versions.txt" 2>&1
echo none > "$OUT/build-route.txt"
# --- voie A : xbuild depuis git (la doc Slint déconseille la version crates.io) ---
{
  cargo install --git https://github.com/rust-mobile/xbuild.git --locked xbuild 2>&1 | tail -5
} > "$OUT/xbuild-install.log" 2>&1
if command -v x >/dev/null; then
  x doctor > "$OUT/xbuild-doctor.log" 2>&1
  x build --release --platform android --arch x64 --format apk > "$OUT/build-xbuild.log" 2>&1
  echo "x build exit=$?" >> "$OUT/build-xbuild.log"
  A=$(find target/x -name '*.apk' 2>/dev/null | head -1)
  if [ -n "$A" ]; then cp "$A" "$OUT/slintspike.apk"; echo xbuild > "$OUT/build-route.txt"; fi
fi
# --- voie B : cargo-ndk + assemblage manuel de l'APK ---
if [ ! -f "$OUT/slintspike.apk" ]; then
  echo "=== voie xbuild en échec : repli cargo-ndk ===" | tee -a "$OUT/build-xbuild.log"
  cargo install cargo-ndk --locked --version 4.1.2 > "$OUT/cargo-ndk-install.log" 2>&1
  cargo ndk -t x86_64 -o /tmp/jniLibs build --release --locked --lib 2>&1 | tee "$OUT/build-cargo-ndk.log"
  if [ -f /tmp/jniLibs/x86_64/libslintspike.so ]; then
    AJAR=$(ls -d "$ANDROID_HOME"/platforms/android-* | sort -V | tail -1)/android.jar
    rm -rf /tmp/apk && mkdir -p /tmp/apk/lib/x86_64 && cp /tmp/jniLibs/x86_64/libslintspike.so /tmp/apk/lib/x86_64/
    "$BT/aapt2" link -I "$AJAR" --manifest android/AndroidManifest.xml --min-sdk-version 26 --target-sdk-version 35 -o /tmp/apk/base.apk 2>&1 | tee -a "$OUT/build-cargo-ndk.log"
    (cd /tmp/apk && zip -q -r -X base.apk lib)
    "$BT/zipalign" -f -p 4 /tmp/apk/base.apk /tmp/apk/aligned.apk
    [ -f /tmp/debug.keystore ] || keytool -genkeypair -keystore /tmp/debug.keystore -storepass android -keypass android -alias debug -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=debug" >/dev/null 2>&1
    "$BT/apksigner" sign --ks /tmp/debug.keystore --ks-pass pass:android --key-pass pass:android --out "$OUT/slintspike.apk" /tmp/apk/aligned.apk 2>&1 | tee -a "$OUT/build-cargo-ndk.log"
    [ -f "$OUT/slintspike.apk" ] && echo cargo-ndk+aapt2 > "$OUT/build-route.txt"
  fi
fi
# --- informatif : taille de la .so arm64 (appareils réels) ---
cargo ndk -t arm64-v8a -o /tmp/jniLibs build --release --locked --lib > "$OUT/build-arm64.log" 2>&1
[ -f /tmp/jniLibs/arm64-v8a/libslintspike.so ] && cp /tmp/jniLibs/arm64-v8a/libslintspike.so "$OUT/libslintspike-arm64-v8a.so"
echo "build_route=$(cat "$OUT/build-route.txt")"
[ -f "$OUT/slintspike.apk" ]
