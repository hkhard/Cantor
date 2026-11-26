#!/usr/bin/env python3
"""Google Play graphics for ChoirBird: screenshots, feature graphic, icon.

Raw screens come from tools/ui_harness/store.js with STORE_PLAY=1 (an
Android user agent, so the app shows its Android wording), at Play's
recommended sizes. The layout follows the App Store set (compose.py): navy
canvas, coral verb, cream benefit line, the device high and bleeding off the
bottom; here the status bar is Android's (time left, no notch) with a
gesture pill, and the body is a generic phone or tablet (no Apple shapes).

  phone      1080 x 2340  (9:19.5)
  tablet-7   1200 x 1920  (made from the tablet capture)
  tablet-10  1600 x 2560
  feature    1024 x 500   (no alpha)
  icon       512 x 512

  tools/store/.venv/bin/python tools/store/compose_play.py RAW_DIR OUT_DIR
"""

import json
import os
import sys

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from compose import CORAL, CREAM, NAVY, NAVY_DEEP, fit, font, lum, rounded_mask, shadow, wrap  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(HERE))
SIZES = {
    "phone": (1080, 2340, "aphone"),
    "tablet-7": (1200, 1920, "atab"),
    "tablet-10": (1600, 2560, "atab"),
}
# Status bar and gesture bar the capture leaves out (dp) and its scale.
BARS = {"aphone": (24, 24, 2.625), "atab": (24, 24, 2)}


def with_android_bars(raw, kind):
    top_dp, bot_dp, s = BARS[kind]
    raw = raw.convert("RGB")
    w = raw.width
    top, bot = round(top_dp * s), round(bot_dp * s)
    full = Image.new("RGB", (w, raw.height + top + bot))
    c_top = raw.getpixel((w // 2, 2))
    c_bot = raw.getpixel((w // 2, raw.height - 2))
    full.paste(c_top, (0, 0, w, top))
    full.paste(raw, (0, top))
    full.paste(c_bot, (0, top + raw.height, w, full.height))
    d = ImageDraw.Draw(full)
    ink = (0, 0, 0) if lum(c_top) > 140 else (255, 255, 255)
    cy = top / 2
    # Time on the left (Android style), then Wi-Fi, signal and battery right.
    tf = font(round(14 * s), "Medium")
    tb = d.textbbox((0, 0), "12:30", font=tf)
    d.text((round(16 * s) - tb[0], cy - (tb[3] - tb[1]) / 2 - tb[1]), "12:30", font=tf, fill=ink)
    rx = w - round(16 * s)
    bw, bh = round(8 * s), round(13 * s)
    d.rounded_rectangle([rx - bw, cy - bh / 2, rx, cy + bh / 2], radius=round(1.5 * s), fill=ink)
    d.rectangle([rx - bw * 0.65, cy - bh / 2 - 1.5 * s, rx - bw * 0.35, cy - bh / 2], fill=ink)
    sx = rx - bw - round(8 * s)
    tri = round(12 * s)
    d.polygon([(sx, cy + tri / 2), (sx, cy - tri / 2), (sx - tri, cy + tri / 2)], fill=ink)
    wx = sx - tri - round(10 * s)
    r = round(8 * s)
    d.pieslice([wx - r, cy - r * 0.4, wx + r, cy + r * 1.6], 225, 315, fill=ink)
    # Gesture navigation pill.
    hi = (0, 0, 0) if lum(c_bot) > 140 else (255, 255, 255)
    pw, ph = round(108 * s), round(4 * s)
    y = full.height - bot / 2
    d.rounded_rectangle([(w - pw) / 2, y - ph / 2, (w + pw) / 2, y + ph / 2], radius=ph / 2, fill=hi)
    return full


def body(screen, kind, width):
    """A generic Android device: even black bezel, rounded corners, a punch-
    hole camera on the phone."""
    bezel = round(width * (0.028 if kind == "aphone" else 0.03))
    r_body = round(width * (0.11 if kind == "aphone" else 0.05))
    sw = width - 2 * bezel
    sh = round(screen.height * sw / screen.width)
    scr = screen.convert("RGB").resize((sw, sh), Image.LANCZOS)
    w, h = width, sh + 2 * bezel
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([0, 0, w - 1, h - 1], radius=r_body, fill=(44, 46, 54, 255))
    rim = max(3, round(width * 0.005))
    d.rounded_rectangle([rim, rim, w - 1 - rim, h - 1 - rim], radius=r_body - rim, fill=(10, 10, 12, 255))
    im.paste(scr, (bezel, bezel), rounded_mask((sw, sh), max(8, r_body - bezel)))
    if kind == "aphone":
        cr = round(sw * 0.018)
        cx, cy = w // 2, bezel + round(sw * 0.03)
        d.ellipse([cx - cr, cy - cr, cx + cr, cy + cr], fill=(0, 0, 0, 255))
    return im


def canvas_bg(W, H):
    c = Image.new("RGB", (W, H), NAVY)
    grad = Image.new("RGB", (1, 256))
    for i in range(256):
        t = i / 255 * 0.35
        grad.putpixel((0, i), tuple(round(NAVY[k] * (1 - t) + NAVY_DEEP[k] * t) for k in range(3)))
    c.paste(grad.resize((W, H)), (0, 0))
    return c


def screenshot(raw, kind, size, verb, rest, out_path):
    W, H = size
    canvas = canvas_bg(W, H)
    draw = ImageDraw.Draw(canvas)
    margin = round(W * 0.08)
    max_w = W - 2 * margin
    phone = kind == "aphone"
    top = round(H * (0.045 if phone else 0.035))
    vsize = round(W * (0.155 if phone else 0.1))
    vf = fit(draw, verb, max_w, vsize, round(vsize * 0.6))
    vb = draw.textbbox((0, 0), verb, font=vf)
    draw.text(((W - (vb[2] - vb[0])) / 2 - vb[0], top - vb[1]), verb, font=vf, fill=CORAL)
    y = top + (vb[3] - vb[1]) + round(H * 0.012)
    dsize = round(W * (0.068 if phone else 0.05))
    df = font(dsize, "Heavy")
    for line in wrap(draw, rest, df, max_w):
        lb = draw.textbbox((0, 0), line, font=df)
        draw.text(((W - (lb[2] - lb[0])) / 2 - lb[0], y - lb[1]), line, font=df, fill=CREAM)
        y += (lb[3] - lb[1]) + round(dsize * 0.28)
    dev = body(raw, kind, round(W * 0.84))
    dy = y + round(H * 0.03)
    dx = (W - dev.width) // 2
    sh, pad = shadow(dev, round(W * 0.025), 120, (0, round(W * 0.01)))
    canvas.paste(sh, (dx - pad, dy - pad), sh)
    canvas.paste(dev, (dx, dy), dev)
    canvas = canvas.convert("RGB")
    assert canvas.size == (W, H)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    canvas.save(out_path, "PNG", optimize=True)


def icon(out_path):
    """512 x 512 from the ChoirBird app icon (opaque, square; Play rounds it)."""
    src = Image.open(os.path.join(ROOT, "app/src-tauri/gen/apple/Assets.xcassets/ChoirBird.appiconset/AppIcon-1024.png"))
    im = src.convert("RGB").resize((512, 512), Image.LANCZOS)
    im.save(out_path, "PNG", optimize=True)


def feature(lang, raw_dir, out_path):
    """1024 x 500: the mark and name with one line on the left, the real
    phone score screen on the right. No price, rank or call to action."""
    W, H = 1024, 500
    c = canvas_bg(W, H)
    d = ImageDraw.Draw(c)
    # Faint staff lines, as on the App Store banner.
    for k in range(5):
        y = 150 + k * 22
        d.line([(0, y), (W, y)], fill=(255, 255, 255, 0) if False else (52, 78, 140), width=2)
    mark = Image.open(os.path.join(ROOT, "app/src-tauri/gen/apple/Assets.xcassets/ChoirBird.appiconset/AppIcon-1024.png")).convert("RGB")
    m = 132
    mk = mark.resize((m, m), Image.LANCZOS)
    c.paste(mk, (60, 70), rounded_mask((m, m), round(m * 0.22)))
    nf = font(84, "Black")
    d.text((60, 222), "ChoirBird", font=nf, fill=CREAM)
    line = {"en": "Hear your part. Sing it better.", "sv": "Hör din stämma. Sjung säkrare."}[lang]
    lf = fit(d, line, 520, 40, 26, "Heavy")
    d.text((62, 330), line, font=lf, fill=CORAL)
    raw_path = os.path.join(raw_dir, f"store-{lang}-aphone-1-score.png")
    if os.path.exists(raw_path):
        dev = body(with_android_bars(Image.open(raw_path), "aphone"), "aphone", 300)
        sh, pad = shadow(dev, 14, 130, (0, 6))
        c.paste(sh, (650 - pad, 40 - pad), sh)
        c.paste(dev, (650, 40), dev)
    c = c.convert("RGB")
    assert c.size == (W, H)
    c.save(out_path, "PNG", optimize=True)


def main():
    raw_dir, out_dir = sys.argv[1], sys.argv[2]
    spec = json.load(open(os.path.join(HERE, "headlines.json"), encoding="utf-8"))
    os.makedirs(out_dir, exist_ok=True)
    icon(os.path.join(out_dir, "icon-512.png"))
    n = 0
    for lang, shots in spec.items():
        feature(lang, raw_dir, os.path.join(out_dir, f"feature-{lang}.png"))
        for i, s in enumerate(shots, 1):
            for slot, (w, h, kind) in SIZES.items():
                raw_path = os.path.join(raw_dir, f"store-{lang}-{kind}-{s['shot']}.png")
                if not os.path.exists(raw_path):
                    print("missing", raw_path)
                    continue
                raw = with_android_bars(Image.open(raw_path), kind)
                out = os.path.join(out_dir, lang, slot, f"{i:02d}-{s['shot']}.png")
                screenshot(raw, kind, (w, h), s["verb"], s["rest"], out)
                n += 1
    print(f"{n} screenshots, 2 feature graphics, 1 icon in {out_dir}")


if __name__ == "__main__":
    main()
