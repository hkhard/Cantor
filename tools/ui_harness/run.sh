#!/usr/bin/env bash
# UI acceptance: the real Svelte UI in WebKit at iPad mini size (744x1133;
# W=/H= to change) with the Tauri backend mocked in memory.
#   harness.js  feature scenarios (library, setlists, reader, annotations ...)
#   a11y.js     NFR-08: axe-core WCAG 2.2 AA on every screen in all skins and
#               both languages, control names, keyboard, 200% text, reduced motion
# Needs a private score with PDF + measure map in scores/out (git-ignored).
#   tools/ui_harness/run.sh [harness|a11y|all]   screenshots in $OUT/shots
set -euo pipefail
export PATH="/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${OUT:-${TMPDIR:-/tmp}/cantor-ui}"
SCORE="${SCORE:-Glimmande Nymf}"
WHAT="${1:-all}"
mkdir -p "$OUT/shots"
cp "$ROOT"/tools/ui_harness/{harness,mock,a11y}.js "$OUT/"
cp "$ROOT/scores/out/$SCORE.musicxml" "$OUT/Glimmande Nymf.musicxml"
cp "$ROOT/scores/out/pdf/$SCORE.pdf" "$OUT/Glimmande Nymf.pdf"
cp "$ROOT/scores/out/pdf/$SCORE.measures.json" "$OUT/Glimmande Nymf.measures.json"
cd "$OUT"
if [ ! -d node_modules/playwright ] || [ ! -d node_modules/axe-core ] || [ ! -d node_modules/pixelmatch ]; then
  npm init -y >/dev/null
  npm i -s playwright@1 axe-core@4 pngjs@7 pixelmatch@5
  npx playwright install webkit
fi
cd "$ROOT/app/ui"
[ -d node_modules ] || npm ci
# A free port per run: CI and local runs share the machine.
PORT="${PORT:-$(python3 -c 'import socket; s=socket.socket(); s.bind(("",0)); print(s.getsockname()[1])')}"
export CANTOR_UI_URL="http://localhost:$PORT/"
npx vite --port $PORT --strictPort >"$OUT/vite.log" 2>&1 &
VITE=$!
trap 'kill $VITE 2>/dev/null' EXIT
for _ in $(seq 1 40); do curl -sf "http://localhost:$PORT/" >/dev/null && break; sleep 0.5; done
export CANTOR_REPO="$ROOT"
status=0
if [ "$WHAT" = all ] || [ "$WHAT" = harness ]; then node "$OUT/harness.js" || status=1; fi
if [ "$WHAT" = all ] || [ "$WHAT" = a11y ]; then node "$OUT/a11y.js" || status=1; fi
exit $status
