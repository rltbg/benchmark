#!/usr/bin/env bash
# Relevé de ce qu'offre un runner GitHub Actions (aucune écriture, aucune installation).
os="$1"; echo "== runner: $os  $(date -u +%FT%TZ)"
echo "== system"; uname -a; echo "arch: $(uname -m)"
echo "cpus: $(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null)"
(free -m 2>/dev/null | sed -n 2p) || true; (sysctl -n hw.memsize 2>/dev/null) || true
df -h . 2>/dev/null | tail -1
echo "== env"; env | grep -E "^(RUNNER_|ImageOS|ImageVersion|ANDROID|JAVA_HOME|DOTNET|ANDROID_NDK|EMSDK|XCODE|PLAYWRIGHT)" | sort
t() { printf "%-14s " "$1"; if command -v "$2" >/dev/null 2>&1; then ( "$2" ${3:---version} 2>&1 | head -${4:-1} ); else echo "absent"; fi; }
echo "== outils"
t git git; t python3 python3; t node node; t java java -version; t mvn mvn; t gradle gradle; t cmake cmake; t ninja ninja
t clang clang; t gcc gcc; t cargo cargo; t rustc rustc; t rustup rustup; t dotnet dotnet --version; t flutter flutter
t pwsh pwsh; t msbuild msbuild "-version" ; t cl cl "" ; t xcodebuild xcodebuild -version 2; t emcc emcc; t wasm-pack wasm-pack
echo "== dotnet"; command -v dotnet >/dev/null && { dotnet --list-sdks; dotnet --list-runtimes | head -5; dotnet workload list 2>&1 | head -8; }
echo "== rust targets"; command -v rustup >/dev/null && rustup target list --installed 2>&1 | head -20
echo "== xcode"; ls -d /Applications/Xcode* 2>/dev/null; command -v xcrun >/dev/null && { xcrun --show-sdk-version 2>&1 | head -1; xcrun simctl list runtimes 2>&1 | head -8; xcrun simctl list devices available 2>&1 | head -12; }
echo "== android"; echo "ANDROID_HOME=${ANDROID_HOME:-}"; ls "${ANDROID_HOME:-/nonexistent}/platforms" 2>/dev/null | tr '\n' ' '; echo; ls "${ANDROID_HOME:-/nonexistent}/ndk" 2>/dev/null | tr '\n' ' '; echo; ls "${ANDROID_HOME:-/nonexistent}/build-tools" 2>/dev/null | tr '\n' ' '; echo; ls "${ANDROID_HOME:-/nonexistent}/system-images" 2>/dev/null | head -5
echo "== kvm"; ls -l /dev/kvm 2>&1 | head -1
echo "== navigateurs"; for b in google-chrome chromium chromium-browser msedge firefox safaridriver chromedriver; do t "$b" "$b" --version; done
echo "== qt/emsdk"; ls -d /opt/Qt* /usr/local/Qt* "$HOME"/Qt* 2>/dev/null; ls -d /usr/lib/emscripten /opt/emsdk 2>/dev/null
