# OMR helpers

`clean_scan.py <in.pdf> <out.tif>`: cleans scanned PDFs before Audiveris when a
raw run fails. It renders at 300 DPI, crops the scanner border, blurs away
halftone patterns, applies an Otsu threshold, removes specks and edge bars, and
writes a group-4 TIFF.

Setup (once):

```
uv venv tools/omr/.venv && uv pip install --python tools/omr/.venv/bin/python -r tools/omr/requirements.txt
```

`cantor-omr` finds it through `$CANTOR_CLEAN_SCAN` (a command), or uses
`tools/omr/.venv/bin/python tools/omr/clean_scan.py` when that exists.
To be ported to Rust (WP-6.x) so mobile and server builds have no Python.
