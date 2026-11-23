#!/usr/bin/env bash
# Fetch homr 0.7.1's ONNX models (AGPL-3.0, from homr's GitHub release) into
# models/homr/ (git-ignored) and check their SHA-256. Used by crates/homr
# tests and bundled into app builds by tools/bundle_homr_models.sh.
# The app itself never downloads anything (no network calls in the client).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
DEST="$ROOT/models/homr"
BASE="https://github.com/liebharc/homr/releases/download/onnx_checkpoints"
mkdir -p "$DEST"
# name  sha256 (checked against the files homr 0.7.1 downloaded, 2026-11-23)
MODELS="segnet_308-3296ccd40960f90ca6ab9c035cca945675d30a0f_fp16 60f495496cb41473c0521d0811d8f44b9d5cff892d287974a8aebb3eaee2fa83
encoder_pytorch_model_509-2293bba14bfea47d9d28411339362af15fce4c31 ee5767286c60dc3d567fe72b6560e7285261ede1535b4215153c199ab3057559
decoder_pytorch_model_509-2293bba14bfea47d9d28411339362af15fce4c31 99a6ac39770616bc1a20d6c409bb85fbbe21ba47f972984f9ef834fb5741b96f"
# A local copy (e.g. from a homr venv) can be used instead of the network.
LOCAL="${HOMR_MODELS_FROM:-}"
echo "$MODELS" | while read -r name sum; do
  f="$DEST/$name.onnx"
  if [ ! -f "$f" ]; then
    src=""
    [ -n "$LOCAL" ] && src="$(find "$LOCAL" -name "$name.onnx" 2>/dev/null | head -1)"
    if [ -n "$src" ]; then
      cp "$src" "$f"
    else
      tmp="$(mktemp -d)"
      curl -fsSL "$BASE/$name.zip" -o "$tmp/m.zip"
      unzip -q -o "$tmp/m.zip" -d "$tmp"
      cp "$(find "$tmp" -name "$name.onnx" | head -1)" "$f"
      rm -r "$tmp"
    fi
  fi
  got="$(shasum -a 256 "$f" | cut -d' ' -f1)"
  if [ "$got" != "$sum" ]; then
    echo "checksum mismatch for $name.onnx: $got" >&2
    exit 1
  fi
done
cp "$ROOT"/crates/homr/src/tokenizer_*.json "$DEST/"
echo "homr models ready in $DEST"
