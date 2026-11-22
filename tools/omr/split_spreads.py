"""OMR preprocessing for screenshots of 2-up score spreads (e.g. an iPad
screenshot of a publisher preview): crop the dark screen border, split each
spread at the gutter into single pages, upscale to ~300 DPI, and threshold so
large grey watermarks ("PREVIEW") vanish: divide by a median-filtered
background (watermark strokes are much wider than notation strokes, so they
end up in the background) and keep pixels clearly darker than it.
Also writes out.pages.json: per output image, the source PDF page (1-based)
and the crop rectangle on it as fractions of the page, so measure boxes found
on the images can be mapped back to the PDF (cantor_omr::measure_map::uncrop).
Usage: split_spreads.py in.pdf out.tif"""
import sys, json, os, numpy as np, pypdfium2 as pdfium
from PIL import Image
from scipy import ndimage

pages = []
crops = []  # per kept image: {page, x, y, w, h} as fractions of the PDF page
seen = []  # small thumbnails of kept pages, to drop repeated spreads
for pno, page in enumerate(pdfium.PdfDocument(sys.argv[1]), 1):
    a = np.asarray(page.render(scale=2, grayscale=True).to_pil().convert("L")).astype(np.uint8)
    H, W = a.shape
    # Paper = bright; keep the bounding box of bright rows/columns.
    bright = a > 200
    rows = np.where(bright.mean(1) > .3)[0]; cols = np.where(bright.mean(0) > .3)[0]
    a = a[rows[0]:rows[-1] + 1, cols[0]:cols[-1] + 1]
    h, w = a.shape
    halves = [a]
    x0s = [cols[0]]
    if w > 1.15 * h:  # landscape spread: split at the emptiest column near the middle
        mid = slice(int(w * .4), int(w * .6))
        ink = (a[:, mid] < 128).mean(0)
        cut = int(w * .4) + int(np.argmin(ndimage.uniform_filter1d(ink, 15)))
        halves = [a[:, :cut], a[:, cut:]]
        x0s = [cols[0], cols[0] + cut]
    for half, x0 in zip(halves, x0s):
        img = Image.fromarray(half)
        scale = 3300 / half.shape[0]  # ~300 DPI for a letter-height page
        img = img.resize((int(half.shape[1] * scale), 3300), Image.LANCZOS)
        b = np.asarray(img)
        # Background at 1/8 scale (fast), upsampled. Watermark letters are much
        # wider than any notation stroke, so a grey closing at that scale keeps
        # them as "background" while erasing notes, stems and staff lines.
        small = np.asarray(img.resize((img.width // 8, img.height // 8), Image.BILINEAR))
        small = ndimage.grey_closing(small, size=(5, 5))
        bg = np.asarray(Image.fromarray(small).resize(img.size, Image.BILINEAR)).astype(float)
        ink = (b.astype(float) / np.maximum(bg, 1)) < 0.78
        lab, n = ndimage.label(ink)
        sizes = ndimage.sum(ink, lab, range(1, n + 1))
        ink[np.isin(lab, np.flatnonzero(sizes < 20) + 1)] = False
        bw = np.where(ink, 0, 255).astype(np.uint8)
        thumb = np.asarray(Image.fromarray(bw).resize((120, 160)), dtype=float) / 255
        if any(np.abs(thumb - t).mean() < 0.01 for t in seen):
            print("duplicate page skipped", file=sys.stderr)
            continue
        seen.append(thumb)
        crops.append({"page": pno, "x": x0 / W, "y": rows[0] / H,
                      "w": half.shape[1] / W, "h": half.shape[0] / H})
        pages.append(Image.fromarray(bw).convert("1"))
        print(f"page {len(pages)}: ink {ink.mean():.3f}", file=sys.stderr)
pages[0].save(sys.argv[2], save_all=True, append_images=pages[1:], compression="group4", dpi=(300, 300))
with open(os.path.splitext(sys.argv[2])[0] + ".pages.json", "w") as f:
    json.dump(crops, f)
