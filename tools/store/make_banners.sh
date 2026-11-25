#!/usr/bin/env bash
# App Store creative assets (product page header + search results) for
# ChoirBird in English and Swedish:
#   store-out/banners/universal-<en|sv>.png  5244 x 2950, PNG, RGB (the one uploaded)
#   store-out/banners/header-<en|sv>.png     3840 x 1646 (header-only variant)
#   *-safearea.png                           the same with Apple's art safe area outlined
# Uses the raw app screens from tools/store/make_screenshots.sh (run it first).
#   bash tools/store/make_banners.sh            render and check
#   bash tools/store/make_banners.sh --upload   ... and upload to App Store Connect
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
export LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/store-out"
PY="$ROOT/tools/store/.venv/bin/python"
[ -d "$OUT/raw/shots" ] || { echo "run tools/store/make_screenshots.sh first" >&2; exit 1; }
PW_DIR="$OUT/raw" node "$ROOT/tools/store/banner.js" "$OUT/raw/shots" "$OUT/banners"
# App Store Connect refuses alpha: flatten and check every file.
"$PY" - "$OUT/banners" <<'PYEOF'
import glob, sys
from PIL import Image
spec = {'universal': (5244, 2950), 'header': (3840, 1646)}
bad = 0
for f in sorted(glob.glob(sys.argv[1] + '/*.png')):
    im = Image.open(f)
    if im.mode != 'RGB':
        im.convert('RGB').save(f, 'PNG', optimize=True)
        im = Image.open(f)
    ok = im.size == spec[f.split('/')[-1].split('-')[0]] and im.mode == 'RGB'
    bad += not ok
    print('ok ' if ok else 'BAD', f.split('/')[-1], im.size, im.mode)
sys.exit(1 if bad else 0)
PYEOF
if [ "${1:-}" = "--upload" ]; then
  # fastlane's own Ruby and gems (for Spaceship's API token).
  FL="$(readlink -f "$(command -v fastlane)")"
  GEMS="$(dirname "$FL")/../libexec"
  RUBY_BIN="$(head -1 "$GEMS/bin/fastlane" | sed 's/^#!//')"
  GEM_HOME="$GEMS" GEM_PATH="$GEMS" "$RUBY_BIN" -e 'gem "fastlane"; require "spaceship"; load ARGV[0]' \
    "$ROOT/tools/store/upload_creative.rb" "$OUT/banners"
fi
