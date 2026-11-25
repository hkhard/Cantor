#!/usr/bin/env python3
"""Lay out the App Store listing for `fastlane deliver` (or for copying by
hand): docs/store/app-store.md -> metadata/<locale>/*.txt, and the finished
screenshots -> screenshots/<locale>/.

  python3 tools/store/export_deliver.py [SCREENSHOT_FINAL_DIR OUT_DIR]

Without arguments: store-out/final -> store-out/deliver (what
tools/store/make_screenshots.sh writes; it also runs this script).

Then upload text and screenshots (settings in tools/store/Deliverfile):
  cd tools/store && fastlane deliver
fastlane uses the image size to pick the device slot.
"""

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCALES = {"English": "en-US", "Swedish": "sv"}
SHOT_LANG = {"en-US": "en", "sv": "sv"}
FIELDS = {
    "subtitle": "subtitle.txt",
    "promotional text": "promotional_text.txt",
    "keywords": "keywords.txt",
    "description": "description.txt",
    "what's new in this version": "release_notes.txt",
    "nyheter i den här versionen": "release_notes.txt",
}
URLS = {
    "en-US": ("https://nikielhard.se/en/choirbird", "https://nikielhard.se/en/choirbird/privacy"),
    "sv": ("https://nikielhard.se/se/choirbird", "https://nikielhard.se/se/choirbird/privacy"),
}


def main():
    args = sys.argv[1:]
    shots = Path(args[0]) if args else ROOT / "store-out/final"
    out = Path(args[1]) if len(args) > 1 else ROOT / "store-out/deliver"
    text = (ROOT / "docs/store/app-store.md").read_text(encoding="utf-8")
    review = re.search(r"\*\*Notes for the reviewer:\*\*\s*\n\n```\n(.*?)\n```", text, re.S).group(1)
    n_files = 0
    for block in re.split(r"^### ", text, flags=re.M)[1:]:
        head = block.splitlines()[0]
        locale = next((v for k, v in LOCALES.items() if head.startswith(k)), None)
        if not locale:
            continue
        md = out / "metadata" / locale
        md.mkdir(parents=True, exist_ok=True)
        (md / "name.txt").write_text("ChoirBird\n", encoding="utf-8")
        support, privacy = URLS[locale]
        (md / "support_url.txt").write_text(support + "\n", encoding="utf-8")
        (md / "marketing_url.txt").write_text(support + "\n", encoding="utf-8")
        (md / "privacy_url.txt").write_text(privacy + "\n", encoding="utf-8")
        for name, body in re.findall(r"\*\*([^*]+)\*\*[^`]*?\n\n```\n(.*?)\n```", block, flags=re.S):
            f = FIELDS.get(name.strip().lower())
            if f:
                (md / f).write_text(body + "\n", encoding="utf-8")
        sd = out / "screenshots" / locale
        if sd.exists():
            shutil.rmtree(sd)
        sd.mkdir(parents=True)
        for slot in ("iphone-69", "iphone-63", "ipad-13"):
            for p in sorted((shots / SHOT_LANG[locale] / slot).glob("*.png")):
                shutil.copy(p, sd / f"{slot}_{p.name}")
                n_files += 1
    # App Review contact: App Store Connect requires first name, last name
    # and phone (+country code) together with the notes. Fill them in
    # docs/store/app-store.md ("Contact: First Last, +46 ..."); until then the
    # review information is left out and entered by hand.
    rv = out / "metadata" / "review_information"
    if rv.exists():
        shutil.rmtree(rv)
    cm = re.search(r"^- \*\*Contact:\*\* (\S.*?), (\+[\d ]{8,})$", text, re.M)
    if cm:
        first, _, last = cm.group(1).partition(" ")
        rv.mkdir(parents=True)
        for f, v in {"notes": review, "email_address": "choirbird-support@nikielhard.se",
                     "first_name": first, "last_name": last, "phone_number": cm.group(2).strip()}.items():
            (rv / f"{f}.txt").write_text(v + "\n", encoding="utf-8")
    # Copyright only once the owner has filled in the holder's name (the
    # first line of the App information table in docs/store/app-store.md).
    m = re.search(r"^\| Copyright \| (.+?) \|$", text, re.M)
    cp = out / "metadata" / "copyright.txt"
    if m and "fill in" not in m.group(1).lower():
        cp.write_text(m.group(1) + "\n", encoding="utf-8")
    elif cp.exists():
        cp.unlink()
    (out / "metadata" / "primary_category.txt").write_text("MUSIC\n", encoding="utf-8")
    (out / "metadata" / "secondary_category.txt").write_text("EDUCATION\n", encoding="utf-8")
    print(f"metadata for {', '.join(LOCALES.values())}; {n_files} screenshots in {out}")
    sys.exit(0 if n_files == 36 else 1)


if __name__ == "__main__":
    main()
