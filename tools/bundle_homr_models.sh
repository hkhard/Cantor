#!/usr/bin/env bash
# LIB-10: copy homr's models (models/homr, see tools/omr/homr_port/
# fetch_models.sh) into app/src-tauri/resources/homr for builds with the
# `homr` feature. Both folders are git-ignored: the models are AGPL and
# 123 MB. Builds without the feature must not carry them: run with --clear.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/app/src-tauri/resources/homr"
ASSETS="$ROOT/app/src-tauri/gen/apple/assets/resources/homr"
if [ "${1:-}" = "--clear" ]; then
  rm -rf "$ROOT/app/src-tauri/resources/homr" "$ROOT/app/src-tauri/gen/apple/assets/resources/homr"
  echo "homr models removed from the app bundle"
  exit 0
fi
bash "$ROOT/tools/omr/homr_port/fetch_models.sh"
mkdir -p "$OUT"
cp "$ROOT"/models/homr/*.onnx "$ROOT"/models/homr/tokenizer_*.json "$OUT/"
[ -d "$ASSETS" ] && rm -rf "$ROOT/app/src-tauri/gen/apple/assets/resources/homr"
du -sh "$OUT"
