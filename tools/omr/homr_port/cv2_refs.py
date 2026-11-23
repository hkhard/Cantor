"""Reference outputs of the OpenCV (and Pillow) operations homr uses, on tiny
synthetic images, for the unit tests of crates/homr/src/image_ops.rs.

Run with homr's Python (opencv-python 4.14, Pillow 12):
  python cv2_refs.py fixtures/omr/cv2

Every input is drawn here from a fixed pseudo-random generator (our own
work, no third-party images). Writes <case>_in.png and <case>_out.png.
"""

import os
import sys

import cv2
import numpy as np
from PIL import Image

out = sys.argv[1]
os.makedirs(out, exist_ok=True)


def lcg(n: int, seed: int) -> np.ndarray:
    """Same generator as `lcg` in image_ops.rs tests (not used there, but documents the data)."""
    x = seed
    v = np.empty(n, np.uint8)
    for i in range(n):
        x = (x * 1103515245 + 12345) & 0x7FFFFFFF
        v[i] = (x >> 16) & 255
    return v


def save(name: str, img: np.ndarray) -> None:
    cv2.imwrite(os.path.join(out, name + ".png"), img)


def noise(w: int, h: int, seed: int) -> np.ndarray:
    return lcg(w * h, seed).reshape(h, w)


def smooth(w: int, h: int, seed: int) -> np.ndarray:
    """A page-like image: light paper, darker strokes, a gradient."""
    n = noise(w, h, seed).astype(np.float32)
    yy, xx = np.mgrid[0:h, 0:w]
    base = 200 + 40 * np.sin(xx / 13.0) * np.cos(yy / 17.0)
    img = base - (n > 230) * 150 + (n.astype(np.float32) - 128) * 0.1
    return np.clip(img, 0, 255).astype(np.uint8)


def sparse(w: int, h: int, seed: int) -> np.ndarray:
    return ((noise(w, h, seed) > 225) * 255).astype(np.uint8)


g = noise(37, 29, 1)
save("noise_37x29", g)
s = smooth(133, 101, 2)
save("smooth_133x101", s)
b = sparse(41, 31, 3)
save("sparse_41x31", b)
bgr = np.stack([noise(23, 17, 4), noise(23, 17, 5), noise(23, 17, 6)], axis=2)
save("bgr_23x17", bgr)

# Colour
save("gray_bgr_23x17", cv2.cvtColor(bgr, cv2.COLOR_BGR2GRAY))

# Threshold
save("thresh_bin_127", cv2.threshold(g, 127, 255, cv2.THRESH_BINARY)[1])
save("thresh_inv_100.5", cv2.threshold(g, 100.5, 1, cv2.THRESH_BINARY_INV)[1])

# Structuring elements
for w, h in [(1, 5), (5, 35), (5, 3), (9, 3), (3, 3), (4, 6), (7, 7), (2, 2), (6, 4), (8, 11)]:
    save(f"kernel_ellipse_{w}x{h}", cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (w, h)) * 255)
    save(f"kernel_cross_{w}x{h}", cv2.getStructuringElement(cv2.MORPH_CROSS, (w, h)) * 255)

# Morphology
for src_name, src in [("sparse_41x31", b), ("noise_37x29", g)]:
    for w, h in [(5, 3), (9, 3), (1, 5), (5, 35), (4, 6)]:
        k = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (w, h))
        save(f"dilate_ellipse_{w}x{h}_{src_name}", cv2.dilate(src, k))
        save(f"erode_ellipse_{w}x{h}_{src_name}", cv2.erode(src, k))
    save(f"dilate_ones_5x3_{src_name}", cv2.dilate(src, np.ones((5, 3), np.uint8)))
    save(f"erode_ones_9x9_{src_name}", cv2.morphologyEx(src, cv2.MORPH_ERODE, np.ones((9, 9), np.uint8)))
    save(f"close_ones_7x7_{src_name}", cv2.morphologyEx(src, cv2.MORPH_CLOSE, np.ones((7, 7), np.uint8)))
    k = cv2.getStructuringElement(cv2.MORPH_RECT, (10, 1))
    save(f"erode_rect_10x1_it2_{src_name}", cv2.morphologyEx(src, cv2.MORPH_ERODE, k, iterations=2))
    save(f"close_rect_10x1_it2_{src_name}", cv2.morphologyEx(src, cv2.MORPH_CLOSE, k, iterations=2))
    k = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 3))
    save(f"open_ellipse_5x3_it2_{src_name}", cv2.morphologyEx(src, cv2.MORPH_OPEN, k, iterations=2))

# Resize (cv2)
for w, h in [(50, 40), (20, 15), (18, 14), (74, 58), (37, 60), (11, 29), (3, 2), (100, 7)]:
    save(f"resize_linear_{w}x{h}", cv2.resize(g, (w, h)))
    save(f"resize_nearest_{w}x{h}", cv2.resize(g, (w, h), interpolation=cv2.INTER_NEAREST))
even = noise(36, 28, 7)
save("noise_36x28", even)
save("resize_linear_half_36x28", cv2.resize(even, (18, 14)))
# Larger cases: every HAL branch (KleidiCV ratio 2 and 3, 2x and 4x up;
# carotene up and down; generic fallback for > 3x down).
big_in = smooth(400, 304, 9)
save("smooth_400x304_b", big_in)
for w, h in [(293, 223), (150, 114), (520, 395), (95, 70), (400, 150), (612, 304), (37, 29)]:
    save(f"resize_linear_big_{w}x{h}", cv2.resize(big_in, (w, h)))
for w, h in [(266, 202), (532, 404)]:
    save(f"resize_linear_up_{w}x{h}", cv2.resize(s, (w, h)))

# Resize (Pillow bicubic, homr resize.py)
for w, h in [(28, 22), (50, 40), (37, 50), (20, 29)]:
    save(f"pil_bicubic_{w}x{h}", np.array(Image.fromarray(g).resize((w, h))))
save("pil_bicubic_bgr_17x13", np.array(Image.fromarray(bgr).resize((17, 13))))

# Box blur
save("blur_7x7_noise_37x29", cv2.blur(g, (7, 7)))
save("blur_7x7_bgr_23x17", cv2.blur(bgr, (7, 7)))
save("blur_3x5_smooth_133x101", cv2.blur(s, (3, 5)))

# CLAHE (homr color_adjust.py: clipLimit 1.0, 8x8 tiles)
clahe = cv2.createCLAHE(clipLimit=1.0, tileGridSize=(8, 8))
save("clahe_smooth_133x101", clahe.apply(s))
save("clahe_noise_37x29", clahe.apply(g))
big = smooth(400, 304, 8)
save("smooth_400x304", big)
save("clahe_smooth_400x304", clahe.apply(big))
save("clahe40_smooth_133x101", cv2.createCLAHE(clipLimit=40.0, tileGridSize=(4, 3)).apply(s))

print("written to", out)
