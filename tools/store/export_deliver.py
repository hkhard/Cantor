#!/usr/bin/env python3
"""Lay out the App Store listing for `fastlane deliver` (or for copying by
hand): docs/store/app-store.md -> metadata/<locale>/*.txt, and the finished
screenshots -> screenshots/<locale>/.

  python3 tools/store/export_deliver.py SCREENSHOT_FINAL_DIR OUT_DIR

Then, with an App Store Connect API key (Users and Access > Integrations):
  fastlane deliver --api_key_path key.json --app_identifier se.nikielhard.choirbird \
    --metadata_path OUT_DIR/metadata --screenshots_path OUT_DIR/screenshots \
    --skip_binary_upload --overwrite_screenshots --force
fastlane uses the image size to pick the device slot.
"""

import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCALES = {"English": "en-GB", "Swedish": "sv"}
SHOT_LANG = {"en-GB": "en", "sv": "sv"}
FIELDS = {
    "subtitle": "subtitle.txt",
    "promotional text": "promotional_text.txt",
    "keywords": "keywords.txt",
    "description": "description.txt",
    "what's new in this version": "release_notes.txt",
    "nyheter i den här versionen": "release_notes.txt",
}
URLS = {
    "en-GB": ("https://nikielhard.se/en/choirbird", "https://nikielhard.se/en/choirbird/privacy"),
    "sv": ("https://nikielhard.se/se/choirbird", "https://nikielhard.se/se/choirbird/privacy"),
}


def main():
    shots, out = Path(sys.argv[1]), Path(sys.argv[2])
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
    rv = out / "metadata" / "review_information"
    rv.mkdir(parents=True, exist_ok=True)
    (rv / "notes.txt").write_text(review + "\n", encoding="utf-8")
    (rv / "email_address.txt").write_text("choirbird-support@nikielhard.se\n", encoding="utf-8")
    (out / "metadata" / "copyright.txt").write_text("2026 FILL-IN-COPYRIGHT-HOLDER\n", encoding="utf-8")
    (out / "metadata" / "primary_category.txt").write_text("MUSIC\n", encoding="utf-8")
    (out / "metadata" / "secondary_category.txt").write_text("EDUCATION\n", encoding="utf-8")
    print(f"metadata for {', '.join(LOCALES.values())}; {n_files} screenshots in {out}")
    sys.exit(0 if n_files == 36 else 1)


if __name__ == "__main__":
    main()
