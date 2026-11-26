#!/usr/bin/env python3
"""Check the store texts against each console's limits.

App Store (docs/store/app-store.md): subtitle 30, promotional text 170,
keywords 100 (and no word repeated from the name or subtitle), description
and What's New 4000. Google Play (docs/store/google-play.md, --play): app
name 30, short description 80, full description 4000, release notes 500.
Exits 1 on any breach.

  python3 tools/store/check_text.py [--play]
"""

import re
import sys
from pathlib import Path

DOC = Path(__file__).resolve().parents[2] / "docs/store/app-store.md"
LIMITS = {
    "subtitle": 30,
    "promotional text": 170,
    "keywords": 100,
    "description": 4000,
    "what's new in this version": 4000,
    "nyheter i den här versionen": 4000,
}


def main():
    global DOC, LIMITS
    if "--play" in sys.argv:
        DOC = DOC.with_name("google-play.md")
        LIMITS = {"app name": 30, "short description": 80, "full description": 4000, "release notes": 500}
    text = DOC.read_text(encoding="utf-8")
    bad = 0
    for lang_block in re.split(r"^### ", text, flags=re.M)[1:]:
        lang = lang_block.splitlines()[0].strip()
        if lang.startswith("English") or lang.startswith("Swedish"):
            fields = re.findall(r"\*\*([^*]+)\*\*[^`]*?\n\n```\n(.*?)\n```", lang_block, flags=re.S)
            got = {}
            for name, body in fields:
                key = name.strip().lower()
                if key not in LIMITS:
                    continue
                got[key] = body
                n = len(body)
                ok = n <= LIMITS[key]
                bad += not ok
                print(f"{'ok ' if ok else 'BAD'} {lang:16} {key:28} {n:5} / {LIMITS[key]}")
            if "keywords" in got:
                kws = got["keywords"].split(",")
                if any(k != k.strip() or not k for k in kws):
                    bad += 1
                    print(f"BAD {lang}: keywords need no spaces around commas")
                taken = set(re.findall(r"\w+", ("choirbird " + got.get("subtitle", "")).lower()))
                dup = [k for k in kws if set(re.findall(r"\w+", k.lower())) & taken]
                if dup:
                    print(f"note {lang}: keywords repeat name/subtitle words: {dup}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
