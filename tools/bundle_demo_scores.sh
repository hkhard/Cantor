#!/usr/bin/env bash
# Fill app/src-tauri/resources/scores with exactly the scores listed in
# tools/demo-scores.txt: the score file, its PDF, and a measure map computed
# from the MusicXML layout when the score carries one (LIB-05).
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/app/src-tauri/resources/scores"
mkdir -p "$OUT"
find "$OUT" -mindepth 1 -maxdepth 1 -type f -delete
# The iOS build copies resources into gen/apple/assets; clear that copy too.
ASSETS="$ROOT/app/src-tauri/gen/apple/assets/resources/scores"
[ -d "$ASSETS" ] && find "$ASSETS" -mindepth 1 -maxdepth 1 -type f -delete
# Same for the Android APK assets.
DROID="$ROOT/app/src-tauri/gen/android/app/src/main/assets/resources/scores"
[ -d "$DROID" ] && find "$DROID" -mindepth 1 -maxdepth 1 -type f -delete
while IFS= read -r line; do
  case "$line" in ''|'#'*) continue ;; esac
  src="$ROOT/$line"
  [ -f "$src" ] || { echo "missing: $line" >&2; exit 1; }
  dir="$(dirname "$src")"; base="$(basename "$src")"; stem="${base%.*}"
  cp "$src" "$OUT/"
  [ -f "$dir/$stem.pdf" ] && cp "$dir/$stem.pdf" "$OUT/"
  if [ -f "$dir/$stem.measures.json" ]; then
    cp "$dir/$stem.measures.json" "$OUT/"
  elif [ -f "$dir/$stem.pdf" ]; then
    (cd "$ROOT" && cargo run -q -p cantor-score --example layout_boxes -- "$src" > "$OUT/$stem.measures.json") || rm -f "$OUT/$stem.measures.json"
  fi
done < "$ROOT/tools/demo-scores.txt"
ls "$OUT"
