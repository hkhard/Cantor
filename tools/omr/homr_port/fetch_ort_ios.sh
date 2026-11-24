#!/usr/bin/env bash
# LIB-10: fetch ONNX Runtime 1.31.1 for iOS (Microsoft's NuGet package
# Microsoft.ML.OnnxRuntime, MIT) into vendor/onnxruntime.xcframework
# (git-ignored) and check the package's SHA-512 against the value from the
# nuget.org catalog. Build machines only: the app makes no network calls.
#
#   ORT_NUPKG_FROM=path/to/ort.nupkg  use a local copy instead of the network
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DEST="$ROOT/vendor"
VER=1.31.1
SHA512=42f13cf89ca111c71f38bececf59be7125d919877840aeff06db1da026a7d1e685daaaa039254357f1fb84de43e492467ac279bc83420cb0d57472e73b4c6327
[ -d "$DEST/onnxruntime.xcframework/ios-arm64" ] && { echo "ready: $DEST/onnxruntime.xcframework"; exit 0; }
mkdir -p "$DEST"
tmp="$(mktemp -d)"
pkg="${ORT_NUPKG_FROM:-}"
if [ -z "$pkg" ]; then
  pkg="$tmp/ort.nupkg"
  curl -fsSL "https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime/$VER" -o "$pkg"
fi
got="$(shasum -a 512 "$pkg" | cut -d' ' -f1)"
[ "$got" = "$SHA512" ] || { echo "SHA-512 mismatch: $got" >&2; exit 1; }
unzip -q -o "$pkg" 'runtimes/ios/native/*' -d "$tmp"
unzip -q -o "$tmp/runtimes/ios/native/onnxruntime.xcframework.zip" -d "$DEST"
rm -r "$tmp"
echo "ready: $DEST/onnxruntime.xcframework"
