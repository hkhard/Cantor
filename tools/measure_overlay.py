#!/usr/bin/env python3
"""Draw a measure map (JSON boxes) over the pages of a PDF, for checking by
eye that the play cursor lands on the right bars.

    python tools/measure_overlay.py score.pdf boxes.json out-prefix
writes out-prefix-p1.png, -p2.png, ... (needs PyMuPDF)."""
import json
import sys

import pymupdf

pdf, boxes, out = sys.argv[1:4]
doc = pymupdf.open(pdf)
items = json.load(open(boxes))
for i, page in enumerate(doc):
    r = page.rect
    for k, b in enumerate(items):
        if b["page"] != i + 1:
            continue
        rect = pymupdf.Rect(
            r.x0 + b["x"] * r.width,
            r.y0 + b["y"] * r.height,
            r.x0 + (b["x"] + b["w"]) * r.width,
            r.y0 + (b["y"] + b["h"]) * r.height,
        )
        page.draw_rect(rect, color=(1, 0, 0) if k % 2 else (0, 0, 1), width=1.2)
        page.insert_text((rect.x0 + 2, rect.y0 + 8), str(k + 1), fontsize=7, color=(1, 0, 0))
    page.get_pixmap(dpi=70).save(f"{out}-p{i + 1}.png")
print(len(doc), "pages", len(items), "boxes")
