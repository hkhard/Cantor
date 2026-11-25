#!/usr/bin/env python3
"""App Store screenshots for ChoirBird: a headline over a framed device.

Takes the raw screenshots from tools/ui_harness/store.js (exact store sizes)
and the headlines in tools/store/headlines.json, and writes App Store
Connect-ready images: RGB, no alpha, exact pixel sizes.

  iphone-69  1320 x 2868  iPhone with Dynamic Island (large display, 6.9")
  iphone-63  1206 x 2622  iPhone with Dynamic Island (medium display, 6.3", required)
  ipad-13    2064 x 2752  iPad 13" (required for iPad apps)

Layout after the aso-appstore-screenshots skill (MIT, Adam Lyttle): a solid
brand colour, a two-line headline (a big action verb, a smaller benefit) in
the top part, and the device high on the canvas, bleeding off the bottom.
The device frame is drawn here (no Apple artwork is used).

  tools/store/.venv/bin/python tools/store/compose.py RAW_DIR OUT_DIR
"""

import json
import os
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))

# ChoirBird brand (app/ui/src/lib/skins.ts): the icon's navy sky and the
# bird's coral. Navy is the background: the app's pages are cream/white, so
# they pop on it; coral marks the verb.
NAVY = (0x1E, 0x3A, 0x7B)
NAVY_DEEP = (0x0D, 0x1B, 0x3E)
CORAL = (0xFF, 0x7A, 0x5C)
CREAM = (0xFF, 0xF6, 0xEC)

SIZES = {
    "iphone-69": (1320, 2868, "iphone"),
    "iphone-63": (1206, 2622, "iphone"),
    "ipad-13": (2064, 2752, "ipad"),
}

# Safe areas the capture leaves out, in points, and the capture scale.
BARS = {"iphone": (62, 34, 3), "ipad": (24, 20, 2)}


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def with_bars(raw, kind):
    """The full screen: a status bar (9:41, signal, Wi-Fi, battery) above the
    app in the colour of its header, and a home indicator below."""
    top_pt, bot_pt, s = BARS[kind]
    raw = raw.convert("RGB")
    w = raw.width
    top, bot = top_pt * s, bot_pt * s
    full = Image.new("RGB", (w, raw.height + top + bot))
    c_top = raw.getpixel((w // 2, 2))
    c_bot = raw.getpixel((w // 2, raw.height - 2))
    full.paste(c_top, (0, 0, w, top))
    full.paste(raw, (0, top))
    full.paste(c_bot, (0, top + raw.height, w, full.height))
    d = ImageDraw.Draw(full)
    ink = (0, 0, 0) if lum(c_top) > 140 else (255, 255, 255)
    # Time on the left, as iOS shows it.
    tf = font(round(17 * s), "Semibold")
    if kind == "iphone":
        tx, cy = round(w * 0.135), round(top * 0.55)
    else:
        tx, cy = round(20 * s), round(top * 0.5)
    tb = d.textbbox((0, 0), "9:41", font=tf)
    d.text((tx - tb[0], cy - (tb[3] - tb[1]) / 2 - tb[1]), "9:41", font=tf, fill=ink)
    # Right: signal bars (iPhone), Wi-Fi, battery.
    rx = w - (round(w * 0.12) if kind == "iphone" else round(20 * s))
    bw, bh = round(25 * s), round(12 * s)
    x0 = rx - bw
    d.rounded_rectangle([x0, cy - bh / 2, rx, cy + bh / 2], radius=round(3.5 * s), outline=ink, width=max(2, round(s)))
    d.rounded_rectangle([x0 + 2 * s, cy - bh / 2 + 2 * s, rx - 2 * s, cy + bh / 2 - 2 * s], radius=round(2 * s), fill=ink)
    d.rounded_rectangle([rx + s, cy - 2 * s, rx + 2.5 * s, cy + 2 * s], radius=s, fill=ink)
    # Wi-Fi: three arcs.
    wx = x0 - round(14 * s)
    for i, r in enumerate((11, 7.5, 4)):
        rr = r * s
        d.pieslice([wx - rr, cy + 4 * s - rr, wx + rr, cy + 4 * s + rr], 225, 315, fill=ink if i == 2 else None)
        if i < 2:
            d.arc([wx - rr, cy + 4 * s - rr, wx + rr, cy + 4 * s + rr], 225, 315, fill=ink, width=round(2.2 * s))
    if kind == "iphone":
        sx = wx - round(34 * s)
        for i in range(4):
            hh = (4 + 2.6 * i) * s
            d.rounded_rectangle([sx + i * 5 * s, cy + 5.5 * s - hh, sx + i * 5 * s + 3.2 * s, cy + 5.5 * s], radius=s, fill=ink)
    # Home indicator.
    hi = (0, 0, 0) if lum(c_bot) > 140 else (255, 255, 255)
    hw, hh = round((134 if kind == "iphone" else 315) * s), round(5 * s)
    y = full.height - round(bot * 0.38)
    d.rounded_rectangle([(w - hw) // 2, y - hh, (w + hw) // 2, y], radius=hh // 2, fill=hi)
    return full


def font(size, weight="Black"):
    """SF Pro (system variable font) at a named weight."""
    for path in ("/Library/Fonts/SF-Pro-Display-Black.otf",):
        if weight == "Black" and os.path.exists(path):
            return ImageFont.truetype(path, size)
    f = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", size)
    try:
        f.set_variation_by_name(weight)
    except Exception:
        pass
    return f


def wrap(draw, text, f, max_w):
    """Fewest lines that fit, then the split with the most even widths (no
    lone word on the last line)."""
    words, lines, cur = text.split(), [], ""
    for w in words:
        t = f"{cur} {w}".strip()
        if draw.textlength(t, font=f) <= max_w or not cur:
            cur = t
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    n = len(lines)
    if n < 2:
        return lines
    best, best_w = lines, max(draw.textlength(x, font=f) for x in lines)

    def splits(ws, k):
        if k == 1:
            yield [" ".join(ws)]
            return
        for i in range(1, len(ws) - k + 2):
            for rest in splits(ws[i:], k - 1):
                yield [" ".join(ws[:i])] + rest

    for cand in splits(words, n):
        widths = [draw.textlength(x, font=f) for x in cand]
        if max(widths) <= max_w and max(widths) < best_w:
            best, best_w = cand, max(widths)
    return best


def fit(draw, text, max_w, start, minimum, weight="Black"):
    s = start
    while s > minimum and draw.textlength(text, font=font(s, weight)) > max_w:
        s -= 4
    return font(s, weight)


def rounded_mask(size, r):
    m = Image.new("L", size, 0)
    ImageDraw.Draw(m).rounded_rectangle([0, 0, size[0] - 1, size[1] - 1], radius=r, fill=255)
    return m


def device(screen, kind, width):
    """The screenshot in a drawn device body, `width` px wide (body)."""
    if kind == "iphone":
        bezel = round(width * 0.035)
        body_r = round(width * 0.17)
    else:
        bezel = round(width * 0.028)
        body_r = round(width * 0.06)
    sw = width - 2 * bezel
    sh = round(screen.height * sw / screen.width)
    scr = screen.convert("RGB").resize((sw, sh), Image.LANCZOS)
    w, h = width, sh + 2 * bezel
    body = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(body)
    # Outer rim (titanium edge), then the black glass border.
    d.rounded_rectangle([0, 0, w - 1, h - 1], radius=body_r, fill=(58, 62, 74, 255))
    rim = max(3, round(width * 0.006))
    d.rounded_rectangle([rim, rim, w - 1 - rim, h - 1 - rim], radius=body_r - rim, fill=(12, 12, 14, 255))
    scr_r = max(8, body_r - bezel)
    body.paste(scr, (bezel, bezel), rounded_mask((sw, sh), scr_r))
    if kind == "iphone":
        # Dynamic Island.
        di_w, di_h = round(sw * 0.29), round(sw * 0.085)
        x = (w - di_w) // 2
        y = bezel + round(sw * 0.025)
        d.rounded_rectangle([x, y, x + di_w, y + di_h], radius=di_h // 2, fill=(0, 0, 0, 255))
        # Side buttons (action, volume up, volume down) on the left edge.
        bw = max(4, round(width * 0.008))
        for y0, y1 in ((0.10, 0.13), (0.16, 0.21), (0.23, 0.28)):
            d.rounded_rectangle([0, round(w * 2.17 * y0), bw, round(w * 2.17 * y1)], radius=bw // 2, fill=(58, 62, 74, 255))
    else:
        # Front camera on the long edge (landscape top in portrait use: top centre).
        cr = max(5, round(width * 0.004))
        d.ellipse([w // 2 - cr, bezel // 2 - cr, w // 2 + cr, bezel // 2 + cr], fill=(30, 30, 34, 255))
    return body


def shadow(img, radius, alpha=110, offset=(0, 0)):
    a = img.getchannel("A").point(lambda v: alpha if v else 0)
    pad = radius * 3
    s = Image.new("RGBA", (img.width + 2 * pad, img.height + 2 * pad), (0, 0, 0, 0))
    black = Image.new("RGBA", img.size, (0, 0, 0, 255))
    black.putalpha(a)
    s.paste(black, (pad + offset[0], pad + offset[1]), black)
    return s.filter(ImageFilter.GaussianBlur(radius)), pad


def compose(raw, kind, size, verb, rest, out_path, popout=None):
    W, H = size
    canvas = Image.new("RGB", (W, H), NAVY)
    # A quiet depth: the bottom of the canvas a little darker (one flat
    # tone per half keeps the set solid, as the skill asks; no glows).
    grad = Image.new("RGB", (1, 256))
    for i in range(256):
        t = i / 255
        grad.putpixel((0, i), tuple(round(NAVY[c] * (1 - t * 0.35) + NAVY_DEEP[c] * t * 0.35) for c in range(3)))
    canvas.paste(grad.resize((W, H)), (0, 0))
    draw = ImageDraw.Draw(canvas)

    # Headline: verb (coral, biggest), benefit (cream) below.
    margin = round(W * 0.08)
    max_w = W - 2 * margin
    top = round(H * (0.045 if kind == "iphone" else 0.035))
    vsize = round(W * (0.155 if kind == "iphone" else 0.095))
    vf = fit(draw, verb, max_w, vsize, round(vsize * 0.6))
    vb = draw.textbbox((0, 0), verb, font=vf)
    draw.text(((W - (vb[2] - vb[0])) / 2 - vb[0], top - vb[1]), verb, font=vf, fill=CORAL)
    y = top + (vb[3] - vb[1]) + round(H * 0.012)
    dsize = round(W * (0.068 if kind == "iphone" else 0.046))
    df = font(dsize, "Heavy")
    for line in wrap(draw, rest, df, max_w):
        lb = draw.textbbox((0, 0), line, font=df)
        draw.text(((W - (lb[2] - lb[0])) / 2 - lb[0], y - lb[1]), line, font=df, fill=CREAM)
        y += (lb[3] - lb[1]) + round(dsize * 0.28)

    # Device: high on the canvas, bleeding off the bottom.
    dev_w = round(W * (0.84 if kind == "iphone" else 0.84))
    body = device(raw, kind, dev_w)
    dy = y + round(H * (0.03 if kind == "iphone" else 0.03))
    dx = (W - body.width) // 2
    sh, pad = shadow(body, round(W * 0.025), 120, (0, round(W * 0.01)))
    canvas.paste(sh, (dx - pad, dy - pad), sh)
    canvas.paste(body, (dx, dy), body)

    # Pop-out (the skill's "breakout"): one real panel of the app, copied
    # pixel for pixel from the screenshot, scaled up in front of the device
    # at the same height, wider than the device, with a soft shadow.
    # On iPad the mixer is a side column: a pop-out there would cover the
    # music, so it is used on iPhone only.
    if popout and kind == "iphone":
        x, y0, w0, h0 = popout
        top_px = BARS[kind][0] * BARS[kind][2]
        crop = raw.crop((round(x), round(y0) + top_px, round(x + w0), round(y0 + h0) + top_px))
        scale_dev = (dev_w - 2 * (round(dev_w * (0.035 if kind == "iphone" else 0.028)))) / raw.width
        cx = dx + round(dev_w * (0.035 if kind == "iphone" else 0.028)) + (x + w0 / 2) * scale_dev
        cy = dy + round(dev_w * (0.035 if kind == "iphone" else 0.028)) + (y0 + top_px + h0 / 2) * scale_dev
        target_w = round(W * (0.94 if kind == "iphone" else 0.62))
        k = target_w / crop.width
        big = crop.resize((target_w, round(crop.height * k)), Image.LANCZOS)
        r = round(big.height * 0.16)
        card = Image.new("RGBA", big.size, (0, 0, 0, 0))
        card.paste(big, (0, 0), rounded_mask(big.size, r))
        px, py = round(cx - big.width / 2), round(cy - big.height / 2)
        px = max(round(W * 0.03), min(px, W - big.width - round(W * 0.03)))
        shd, pad2 = shadow(card, round(W * 0.02), 150, (0, round(W * 0.012)))
        canvas.paste(shd, (px - pad2, py - pad2), shd)
        canvas.paste(card, (px, py), card)

    canvas = canvas.convert("RGB")
    assert canvas.size == (W, H)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    canvas.save(out_path, "PNG", optimize=True)


def main():
    raw_dir, out_dir = sys.argv[1], sys.argv[2]
    spec = json.load(open(os.path.join(HERE, "headlines.json"), encoding="utf-8"))
    n = 0
    for lang, shots in spec.items():
        for i, s in enumerate(shots, 1):
            for slot, (w, h, kind) in SIZES.items():
                raw_path = os.path.join(raw_dir, f"store-{lang}-{kind}-{s['shot']}.png")
                if not os.path.exists(raw_path):
                    print("missing", raw_path)
                    continue
                raw = with_bars(Image.open(raw_path), kind)
                meta_path = raw_path[:-4] + ".json"
                popout = json.load(open(meta_path))["popout"] if os.path.exists(meta_path) else None
                out = os.path.join(out_dir, lang, slot, f"{i:02d}-{s['shot']}.png")
                compose(raw, kind, (w, h), s["verb"], s["rest"], out, popout)
                n += 1
    print(f"{n} images in {out_dir}")


if __name__ == "__main__":
    main()
