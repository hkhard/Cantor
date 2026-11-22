#!/usr/bin/env bash
# UI acceptance harness: the real Svelte UI in WebKit at iPad mini size
# (744x1133; W=/H= to change) with the Tauri backend mocked in memory.
# Needs a private score with PDF + measure map in scores/out (git-ignored).
#   tools/ui_harness/run.sh            screenshots in $OUT (default /tmp/cantor-ui)
set -euo pipefail
export PATH="/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${OUT:-${TMPDIR:-/tmp}/cantor-ui}"
SCORE="${SCORE:-Glimmande Nymf}"
mkdir -p "$OUT"
cp "$ROOT/tools/ui_harness/harness.js" "$OUT/"
cp "$ROOT/scores/out/$SCORE.musicxml" "$OUT/Glimmande Nymf.musicxml"
cp "$ROOT/scores/out/pdf/$SCORE.pdf" "$OUT/Glimmande Nymf.pdf"
cp "$ROOT/scores/out/pdf/$SCORE.measures.json" "$OUT/Glimmande Nymf.measures.json"
cd "$OUT"
[ -d node_modules/playwright ] || { npm init -y >/dev/null; npm i -s playwright@1; npx playwright install webkit; }
cd "$ROOT/app/ui"
npx vite --port 5173 --strictPort >"$OUT/vite.log" 2>&1 &
VITE=$!
trap 'kill $VITE' EXIT
sleep 3
node "$OUT/harness.js"
