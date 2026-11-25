#!/usr/bin/env bash
# App Store screenshots for ChoirBird, every slot and language, in one go:
#   1. tools/ui_harness/run.sh store   raw screens (real UI, public-domain titles)
#   2. tools/store/compose.py          headline + device frame, exact store sizes
#   3. a check of every file's size and colour mode, and a contact sheet
#   4. tools/store/export_deliver.py   the upload package (fastlane deliver layout)
#   bash tools/store/make_screenshots.sh
# Result (git-ignored, inside the repo; OUT=/some/dir to put it elsewhere):
#   store-out/final/<en|sv>/<iphone-69|iphone-63|ipad-13>/NN-name.png
#   store-out/deliver/{metadata,screenshots}/    ready to upload
# Headlines: tools/store/headlines.json. Text for App Store Connect:
# docs/store/app-store.md.
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
  LANG_UI=$L OUT="$OUT/raw" bash "$ROOT/tools/ui_harness/run.sh" store
done
rm -rf "$OUT/final"
"$PY" "$ROOT/tools/store/compose.py" "$OUT/raw/shots" "$OUT/final"
"$PY" - "$OUT" <<'EOF'
import glob, sys
from PIL import Image
out = sys.argv[1]
spec = {'iphone-69': (1320, 2868), 'iphone-63': (1206, 2622), 'ipad-13': (2064, 2752)}
files = sorted(glob.glob(f'{out}/final/*/*/*.png'))
bad = [f for f in files if Image.open(f).size != spec[f.split('/')[-2]] or Image.open(f).mode != 'RGB']
for f in bad:
    print('BAD', f, Image.open(f).size, Image.open(f).mode)
per = {}
for f in files:
    k = tuple(f.split('/')[-3:-1])
    per[k] = per.get(k, 0) + 1
print(f'{len(files)} screenshots, {len(bad)} bad;', ', '.join(f'{l}/{s}: {n}' for (l, s), n in sorted(per.items())))
# Showcase: the first three iPhone screenshots in each language, side by side.
for lang in ('en', 'sv'):
    ims = [Image.open(f) for f in sorted(glob.glob(f'{out}/final/{lang}/iphone-69/*.png'))[:3]]
    w = 600; h = round(ims[0].height * w / ims[0].width); gap = 40
    sheet = Image.new('RGB', (3 * w + 4 * gap, h + 2 * gap), (13, 27, 62))
    for i, im in enumerate(ims):
        sheet.paste(im.resize((w, h), Image.LANCZOS), (gap + i * (w + gap), gap))
    sheet.save(f'{out}/final/showcase-{lang}.png')
sys.exit(1 if bad or len(files) != 36 else 0)
EOF
"$PY" "$ROOT/tools/store/export_deliver.py" "$OUT/final" "$OUT/deliver"
echo "App Store screenshots: $OUT/final"
echo "Upload package:        $OUT/deliver"
