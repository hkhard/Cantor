#!/usr/bin/env bash
# Google Play graphics for ChoirBird, every slot and language, in one go:
#   1. tools/ui_harness/run.sh store with STORE_PLAY=1   raw Android screens
#   2. tools/store/compose_play.py                        headline + device,
#      feature graphic, icon
#   3. a check of every file's size and mode, and a contact sheet
#   bash tools/store/make_play.sh
# Result (git-ignored): store-out/play/{icon-512.png,feature-<lang>.png,
#   <en|sv>/<phone|tablet-7|tablet-10>/NN-name.png}
# Texts for the Play Console: docs/store/google-play.md.
set -euo pipefail
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="${OUT:-$ROOT/store-out}"
PY="$ROOT/tools/store/.venv/bin/python"
if [ ! -x "$PY" ]; then
  python3 -m venv "$ROOT/tools/store/.venv"
  "$ROOT/tools/store/.venv/bin/pip" install -q 'pillow>=11,<12'
fi
for L in en sv; do
  STORE_PLAY=1 LANG_UI=$L OUT="$OUT/raw-play" bash "$ROOT/tools/ui_harness/run.sh" store
done
rm -rf "$OUT/play"
"$PY" "$ROOT/tools/store/compose_play.py" "$OUT/raw-play/shots" "$OUT/play"
"$PY" - "$OUT/play" <<'EOF'
import glob, sys
from PIL import Image
out = sys.argv[1]
spec = {'phone': (1080, 2340), 'tablet-7': (1200, 1920), 'tablet-10': (1600, 2560)}
bad = []
files = sorted(glob.glob(f'{out}/*/*/*.png'))
for f in files:
    im = Image.open(f)
    if im.size != spec[f.split('/')[-2]] or im.mode != 'RGB':
        bad.append((f, im.size, im.mode))
for f in glob.glob(f'{out}/feature-*.png'):
    im = Image.open(f)
    if im.size != (1024, 500) or im.mode != 'RGB':
        bad.append((f, im.size, im.mode))
ic = Image.open(f'{out}/icon-512.png')
if ic.size != (512, 512):
    bad.append(('icon', ic.size, ic.mode))
for b in bad:
    print('BAD', *b)
print(f'{len(files)} screenshots, {len(bad)} bad')
for lang in ('en', 'sv'):
    ims = [Image.open(f) for f in sorted(glob.glob(f'{out}/{lang}/phone/*.png'))[:4]]
    w = 500; h = round(ims[0].height * w / ims[0].width); gap = 30
    sheet = Image.new('RGB', (4 * w + 5 * gap, h + 2 * gap), (13, 27, 62))
    for i, im in enumerate(ims):
        sheet.paste(im.resize((w, h), Image.LANCZOS), (gap + i * (w + gap), gap))
    sheet.save(f'{out}/showcase-{lang}.png')
sys.exit(1 if bad or len(files) != 36 else 0)
EOF
echo "Google Play graphics: $OUT/play"
