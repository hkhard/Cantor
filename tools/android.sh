#!/usr/bin/env bash
# Build ChoirBird for Android.
#
#   tools/android.sh apk               debug APK with note recognition, installed
#                                      on the connected device (ADB_SERIAL=...)
#   tools/android.sh release           signed release bundle (AAB) of the tagged
#                                      commit, for Google Play (not uploaded)
#   tools/android.sh install-release   install that AAB on the connected device
#                                      the way Play would (bundletool)
#
# Release: HEAD must be a clean commit tagged android-vX.Y.Z (Android has its
# own tags so it can ship on its own schedule: git tag android-v1.0.0). The
# version is the tag, the version code the commit count (unique, rising, as
# the iOS build number), and About shows the tag and links its source
# (AGPL 6(d)). Upload key: app/src-tauri/gen/android/keystore.properties
# (git-ignored; see docs/DEVELOPING.md).
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/app/src-tauri"
export ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}"
export JAVA_HOME="${JAVA_HOME:-/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-android}"
ADB="$ANDROID_HOME/platform-tools/adb"
AAB="$APP/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab"
BUNDLETOOL="${BUNDLETOOL:-$ROOT/vendor/bundletool-all-1.18.1.jar}"

# Demo scores: only those in tools/demo-scores.txt (Lacrymosa).
bash "$ROOT/tools/bundle_demo_scores.sh"

# LIB-10 note recognition: the models go in the Play asset pack, ONNX
# Runtime (Microsoft, MIT) into jniLibs; both git-ignored.
homr() {
  bash "$ROOT/tools/bundle_homr_models.sh" --clear >/dev/null
  bash "$ROOT/tools/bundle_homr_models.sh" --android
  bash "$ROOT/tools/omr/homr_port/fetch_ort_android.sh"
  mkdir -p "$APP/gen/android/app/src/main/jniLibs/arm64-v8a"
  cp "$ROOT/vendor/onnxruntime-android/arm64-v8a/libonnxruntime.so" "$APP/gen/android/app/src/main/jniLibs/arm64-v8a/"
  # Tauri copies resources/ into the base assets; the models must only be in
  # the pack (Play rejects the same file in two modules).
  rm -rf "$APP/gen/android/app/src/main/assets/resources/homr"
  export ORT_LIB_PATH="$ROOT/vendor/onnxruntime-android/arm64-v8a" ORT_PREFER_DYNAMIC_LINK=1
}

bundletool() {
  if [ ! -f "$BUNDLETOOL" ]; then
    mkdir -p "$(dirname "$BUNDLETOOL")"
    curl -fsSL -o "$BUNDLETOOL" https://github.com/google/bundletool/releases/download/1.18.1/bundletool-all-1.18.1.jar
  fi
  "$JAVA_HOME/bin/java" -jar "$BUNDLETOOL" "$@"
}

case "${1:-}" in
  apk)
    homr
    cd "$APP"
    cargo tauri android build --debug --apk --target aarch64 --features homr
    "$ADB" ${ADB_SERIAL:+-s "$ADB_SERIAL"} install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
    ;;
  release)
    cd "$ROOT"
    TAG="$(git describe --tags --exact-match --match 'android-v[0-9]*' 2>/dev/null || true)"
    if [ -z "$TAG" ] || [ -n "$(git status --porcelain --untracked-files=no -- . ':!app/src-tauri/Cargo.lock' ':!app/src-tauri/gen/apple/cantor-app_iOS/Info.plist')" ]; then
      echo "release: HEAD must be a clean commit tagged android-vX.Y.Z (git tag android-v1.0.0)" >&2
      exit 1
    fi
    [ -f "$APP/gen/android/keystore.properties" ] || { echo "release: no upload key (gen/android/keystore.properties)" >&2; exit 1; }
    VERSION="${TAG#android-v}"
    CODE="$(git rev-list --count HEAD)"
    homr
    cd "$APP"
    CANTOR_VERSION="$TAG" cargo tauri android build --aab --target aarch64 --features homr \
      --config "{\"version\":\"$VERSION\",\"bundle\":{\"android\":{\"versionCode\":$CODE}}}"
    echo "ChoirBird $TAG (version code $CODE): $AAB"
    ;;
  install-release)
    PW="$(security find-generic-password -s choirbird-android-upload -w)"
    T="$(mktemp -d)"
    bundletool build-apks --bundle="$AAB" --output="$T/cb.apks" --connected-device \
      ${ADB_SERIAL:+--device-id="$ADB_SERIAL"} --adb="$ADB" \
      --ks="$HOME/.android-keys/choirbird-upload.jks" --ks-key-alias=upload \
      --ks-pass="pass:$PW" --key-pass="pass:$PW"
    bundletool install-apks --apks="$T/cb.apks" ${ADB_SERIAL:+--device-id="$ADB_SERIAL"} --adb="$ADB"
    rm -r "$T"
    ;;
  *)
    echo "usage: $0 apk|release|install-release" >&2
    exit 2
    ;;
esac
