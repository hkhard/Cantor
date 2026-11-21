"""OMR preprocessing for scanned PDFs: 300 DPI grayscale, crop scanner border,
blur away halftone/screen patterns, Otsu threshold, remove specks. Multi-page TIFF out."""
import sys, numpy as np, pypdfium2 as pdfium
from PIL import Image, ImageFilter
from scipy import ndimage
def otsu(a):
    h = np.bincount(a.ravel(), minlength=256).astype(float); p = h / h.sum()
    w = np.cumsum(p); m = np.cumsum(p * np.arange(256)); mt = m[-1]
    return int(np.argmax((mt * w - m) ** 2 / (w * (1 - w) + 1e-12)))
pages = []
for page in pdfium.PdfDocument(sys.argv[1]):
    img = page.render(scale=300/72, grayscale=True).to_pil().convert("L")
    a = np.asarray(img)
    rows = np.where(np.median(a, axis=1) > 150)[0]; cols = np.where(np.median(a, axis=0) > 150)[0]
    if len(rows) and len(cols): a = a[rows[0]:rows[-1]+1, cols[0]:cols[-1]+1]
    a = np.asarray(Image.fromarray(a).filter(ImageFilter.GaussianBlur(1.2)))
    ink = a < otsu(a)
    lab, n = ndimage.label(ink); sizes = ndimage.sum(ink, lab, range(1, n + 1))
    ink[np.isin(lab, np.flatnonzero(sizes < 12) + 1)] = False
    # Drop solid bars along the edges (scanner shadow / book gutter): columns or
    # rows within 6% of an edge that are more than 60% ink.
    h, w = ink.shape
    for c in list(range(int(w * .06))) + list(range(int(w * .94), w)):
        if ink[:, c].mean() > .6: ink[:, c] = False
    for r in list(range(int(h * .06))) + list(range(int(h * .94), h)):
        if ink[r, :].mean() > .6: ink[r, :] = False
    lab, n = ndimage.label(ink); sizes = ndimage.sum(ink, lab, range(1, n + 1))
    tall = [i + 1 for i, s in enumerate(ndimage.find_objects(lab)) if (s[0].stop - s[0].start) > .5 * h and (s[1].stop - s[1].start) < .03 * w and (s[1].start < .06 * w or s[1].stop > .94 * w)]
    ink[np.isin(lab, tall)] = False
    pages.append(Image.fromarray(np.where(ink, 0, 255).astype(np.uint8)).convert("1"))
    print(f"page {len(pages)}: ink {ink.mean():.3f}, specks removed {(sizes < 12).sum()}", file=sys.stderr)
pages[0].save(sys.argv[2], save_all=True, append_images=pages[1:], compression="group4", dpi=(300, 300))
