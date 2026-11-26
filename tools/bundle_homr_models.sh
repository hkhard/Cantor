#!/usr/bin/env bash
# LIB-10: copy homr's models (models/homr, see tools/omr/homr_port/
# fetch_models.sh) into the app for builds with the `homr` feature. All
# target folders are git-ignored: the models are AGPL and 123 MB. Builds
# without the feature must not carry them: run with --clear.
#
#   bundle_homr_models.sh            iOS/desktop: app/src-tauri/resources/homr
#                                    (bundled by tauri.homr.conf.json)
#   bundle_homr_models.sh --android  Android: the Play install-time asset pack
#                                    gen/android/homr_models (resources/homr)
#   bundle_homr_models.sh --clear    remove every copy
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/app/src-tauri/resources/homr"
ASSETS="$ROOT/app/src-tauri/gen/apple/assets/resources/homr"
PACK="$ROOT/app/src-tauri/gen/android/homr_models/src/main/assets/resources/homr"
if [ "${1:-}" = "--clear" ]; then
  rm -rf "$ROOT/app/src-tauri/resources/homr" "$ROOT/app/src-tauri/gen/apple/assets/resources/homr" \
    "$ROOT/app/src-tauri/gen/android/homr_models/src/main/assets/resources/homr"
  echo "homr models removed from the app bundle"
  exit 0
fi
bash "$ROOT/tools/omr/homr_port/fetch_models.sh"
if [ "${1:-}" = "--android" ]; then
  OUT="$PACK"
fi
mkdir -p "$OUT"
cp "$ROOT"/models/homr/*.onnx "$ROOT"/models/homr/tokenizer_*.json "$OUT/"
[ -d "$ASSETS" ] && rm -rf "$ROOT/app/src-tauri/gen/apple/assets/resources/homr"
du -sh "$OUT"
