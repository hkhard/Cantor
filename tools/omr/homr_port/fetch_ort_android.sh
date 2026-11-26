#!/usr/bin/env bash
# LIB-10 on Android: fetch ONNX Runtime 1.31.1 for Android (Microsoft's AAR
# com.microsoft.onnxruntime:onnxruntime-android, MIT) from Maven Central,
# check its SHA-512, and put libonnxruntime.so for arm64 into
# vendor/onnxruntime-android/arm64-v8a (git-ignored). Same version as iOS
# (fetch_ort_ios.sh). Build machines only: the app makes no network calls.
#
#   ORT_AAR_FROM=path/to/onnxruntime-android-1.31.1.aar  use a local copy
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DEST="$ROOT/vendor/onnxruntime-android"
VER=1.31.1
SHA512=ba41c94765d2162d9f60504defd5ee4c92ac189a0cb771de4af61b2ad91322549dc01795130fef55e2973474b7cd6ad6a0b49fc096d36660c79429d2519d51a1
[ -f "$DEST/arm64-v8a/libonnxruntime.so" ] && { echo "ready: $DEST"; exit 0; }
mkdir -p "$DEST"
tmp="$(mktemp -d)"
aar="${ORT_AAR_FROM:-}"
if [ -z "$aar" ]; then
  aar="$tmp/ort.aar"
  curl -fsSL "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/$VER/onnxruntime-android-$VER.aar" -o "$aar"
fi
got="$(shasum -a 512 "$aar" | cut -d' ' -f1)"
[ "$got" = "$SHA512" ] || { echo "SHA-512 mismatch: $got" >&2; exit 1; }
unzip -q -o "$aar" 'jni/arm64-v8a/libonnxruntime.so' -d "$tmp"
mkdir -p "$DEST/arm64-v8a"
mv "$tmp/jni/arm64-v8a/libonnxruntime.so" "$DEST/arm64-v8a/"
rm -r "$tmp"
echo "ready: $DEST"
