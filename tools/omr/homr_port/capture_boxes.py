"""Capture homr's stage-3 boxes on the Lacrymosa fixture masks, as references
for crates/homr/tests/boxes_golden.rs.

Run with homr's Python (0.7.1):
  python capture_boxes.py fixtures/omr/lacrymosa-p1

Reads pred_*.png and writes boxes.json: per list, [cx, cy, w, h, angle] of
every box homr's predict_symbols (and the later stage-4 steps) makes.
"""

import json
import os
import sys

import cv2
import numpy as np

from homr import main as homr_main
from homr.bar_line_detection import detect_bar_lines
from homr.bounding_boxes import create_rotated_bounding_boxes
from homr.brace_dot_detection import prepare_brace_dot_image
from homr.debug import Debug
from homr.model import InputPredictions
from homr.note_detection import combine_noteheads_with_stems
from homr.staff_detection import break_wide_fragments

d = sys.argv[1]


def load(name):  # type: ignore[no-untyped-def]
    return (cv2.imread(os.path.join(d, f"pred_{name}.png"), 0) > 0).astype(np.uint8)


pre = cv2.imread(os.path.join(d, "preprocessed.png"), 0)
pred = InputPredictions(
    pre, pre, load("notehead"), load("symbols"), load("staff"), load("clefs_keys"), load("stems_rest")
)
sym = homr_main.predict_symbols(Debug(pre, "x.png", False), pred)


def boxes(bs):  # type: ignore[no-untyped-def]
    return [[round(float(v), 4) for v in (b.box[0][0], b.box[0][1], b.box[1][0], b.box[1][1], b.box[2])] for b in bs]


broken = break_wide_fragments(sym.staff_fragments)
notes = combine_noteheads_with_stems(sym.noteheads, sym.stems_rest)
unit = float(np.median([n.notehead.size[1] for n in notes]))
heads = [n.notehead for n in notes]
stems = [n.stem for n in notes if n.stem is not None]
bars = detect_bar_lines(
    [b for b in sym.bar_lines if not b.is_overlapping_with_any(heads) and not b.is_overlapping_with_any(stems)],
    unit,
)
brace = create_rotated_bounding_boxes(
    prepare_brace_dot_image(pred.symbols, pred.staff), skip_merging=True, max_size=(100, -1)
)
out = {
    "noteheads": boxes(sym.noteheads),
    "staff_fragments": boxes(sym.staff_fragments),
    "clefs_keys": boxes(sym.clefs_keys),
    "stems_rest": boxes(sym.stems_rest),
    "bar_lines": boxes(sym.bar_lines),
    "broken": boxes(broken),
    "bar_line_boxes": boxes(bars),
    "brace_dot": boxes(brace),
    "avg_notehead_height": unit,
}
json.dump(out, open(os.path.join(d, "boxes.json"), "w"), separators=(",", ":"))
