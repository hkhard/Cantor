//! The OpenCV (and Pillow) operations homr uses, in plain Rust (no OpenCV
//! dependency). Each one follows OpenCV 4.14's scalar code path (or
//! Pillow 12's, for [`pil_resize_bicubic`]) and is tested against their
//! output on small images (`fixtures/omr/cv2/`, made by
//! `tools/omr/homr_port/cv2_refs.py`).
//!
//! - colour: [`Bgr`], [`bgr_to_gray`], [`gray_to_bgr`]
//! - [`threshold`]
//! - morphology: [`structuring_element`], [`Kernel::ones`], [`erode`],
//!   [`dilate`], [`morphology`] (open, close), border ignored as in OpenCV
//! - resize: [`resize`] (nearest, linear incl. the 2x area-fast case),
//!   [`pil_resize_bicubic`] (homr's `resize_image` uses Pillow)
//! - [`blur`] (normalised box filter, `BORDER_REFLECT_101`)
//! - [`clahe`]
//!
//! Contours, rectangles and polygons live in `boxes::contours` and
//! `boxes::geom`.
use crate::Gray;

/// A colour image in OpenCV's channel order (B, G, R), interleaved, row-major.
#[derive(Clone, PartialEq)]
pub struct Bgr {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Bgr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bgr({}x{})", self.width, self.height)
    }
}

impl Bgr {
    pub fn new(width: usize, height: usize) -> Self {
        Bgr {
            width,
            height,
            data: vec![0; width * height * 3],
        }
    }
    pub fn get(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
    pub fn set(&mut self, x: usize, y: usize, v: [u8; 3]) {
        let i = (y * self.width + x) * 3;
        self.data[i..i + 3].copy_from_slice(&v);
    }
    /// Decode a PNG like `cv2.imread(path)` (colour, alpha dropped).
    pub fn from_png(bytes: &[u8]) -> Result<Self, crate::Error> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|e| crate::Error::Image(e.to_string()))?;
        let rgb = img.to_rgb8();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let mut data = rgb.into_raw();
        for p in data.as_chunks_mut::<3>().0 {
            p.swap(0, 2);
        }
        Ok(Bgr {
            width: w,
            height: h,
            data,
        })
    }
    /// The sub-image `x..x+w`, `y..y+h`.
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Bgr {
        let mut out = Bgr::new(w, h);
        for r in 0..h {
            let s = ((y + r) * self.width + x) * 3;
            out.data[r * w * 3..(r + 1) * w * 3].copy_from_slice(&self.data[s..s + w * 3]);
        }
        out
    }
    /// One channel (0 = B, 1 = G, 2 = R).
    pub fn channel(&self, c: usize) -> Gray {
        Gray {
            width: self.width,
            height: self.height,
            data: self.data.iter().skip(c).step_by(3).copied().collect(),
        }
    }
    /// Join three channels (B, G, R) of the same size.
    pub fn from_channels(ch: [&Gray; 3]) -> Bgr {
        let (w, h) = (ch[0].width, ch[0].height);
        let mut data = Vec::with_capacity(w * h * 3);
        for i in 0..w * h {
            data.extend_from_slice(&[ch[0].data[i], ch[1].data[i], ch[2].data[i]]);
        }
        Bgr {
            width: w,
            height: h,
            data,
        }
    }
}

impl Gray {
    /// The sub-image `x..x+w`, `y..y+h`.
    pub fn crop(&self, x: usize, y: usize, w: usize, h: usize) -> Gray {
        let mut out = Gray::new(w, h);
        for r in 0..h {
            let s = (y + r) * self.width + x;
            out.data[r * w..(r + 1) * w].copy_from_slice(&self.data[s..s + w]);
        }
        out
    }
}

// ---------------------------------------------------------------- colour

/// `cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)`: fixed point, 15 bits
/// (R 9798, G 19235, B 3735), rounded.
pub fn bgr_to_gray(src: &Bgr) -> Gray {
    const RY: u32 = 9798;
    const GY: u32 = 19235;
    const BY: u32 = 3735;
    let data = src
        .data
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| ((p[0] as u32 * BY + p[1] as u32 * GY + p[2] as u32 * RY + (1 << 14)) >> 15) as u8)
        .collect();
    Gray {
        width: src.width,
        height: src.height,
        data,
    }
}

/// `cv2.cvtColor(img, cv2.COLOR_GRAY2BGR)`.
pub fn gray_to_bgr(src: &Gray) -> Bgr {
    Bgr {
        width: src.width,
        height: src.height,
        data: src.data.iter().flat_map(|&v| [v, v, v]).collect(),
    }
}

// ---------------------------------------------------------------- threshold

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreshType {
    /// `THRESH_BINARY`: `src > thresh ? maxval : 0`.
    Binary,
    /// `THRESH_BINARY_INV`: `src > thresh ? 0 : maxval`.
    BinaryInv,
}

/// `cv2.threshold(src, thresh, maxval, kind)[1]` on 8-bit images
/// (the threshold is floored, as OpenCV does for `CV_8U`).
pub fn threshold(src: &Gray, thresh: f64, maxval: u8, kind: ThreshType) -> Gray {
    let t = thresh.floor();
    let (hi, lo) = match kind {
        ThreshType::Binary => (maxval, 0),
        ThreshType::BinaryInv => (0, maxval),
    };
    Gray {
        width: src.width,
        height: src.height,
        data: src
            .data
            .iter()
            .map(|&v| if v as f64 > t { hi } else { lo })
            .collect(),
    }
}

// ---------------------------------------------------------------- morphology

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect,
    Ellipse,
    Cross,
}

/// A structuring element, row-major, anchored at its centre (`w/2`, `h/2`)
/// like OpenCV's default anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kernel {
    pub width: usize,
    pub height: usize,
    pub data: Vec<bool>,
}

impl Kernel {
    /// `np.ones((rows, cols))`: note the argument order is width, height.
    pub fn ones(width: usize, height: usize) -> Kernel {
        Kernel {
            width,
            height,
            data: vec![true; width * height],
        }
    }
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.data[y * self.width + x]
    }
    fn anchor(&self) -> (usize, usize) {
        (self.width / 2, self.height / 2)
    }
    /// Contiguous runs per kernel row: (dy, x0, len).
    fn runs(&self) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                if self.get(x, y) {
                    let x0 = x;
                    while x < self.width && self.get(x, y) {
                        x += 1;
                    }
                    out.push((y, x0, x - x0));
                } else {
                    x += 1;
                }
            }
        }
        out
    }
}

/// `cv2.getStructuringElement(shape, (width, height))`.
pub fn structuring_element(shape: Shape, width: usize, height: usize) -> Kernel {
    let shape = if width == 1 && height == 1 {
        Shape::Rect
    } else {
        shape
    };
    let (ax, ay) = (width / 2, height / 2);
    let r = (height / 2) as i64;
    let c = (width / 2) as i64;
    let inv_r2 = if r != 0 { 1.0 / (r * r) as f64 } else { 0.0 };
    let mut data = vec![false; width * height];
    for i in 0..height {
        let (j1, j2) = match shape {
            Shape::Rect => (0, width),
            Shape::Cross if i == ay => (0, width),
            Shape::Cross => (ax, ax + 1),
            Shape::Ellipse => {
                let dy = i as i64 - r;
                if dy.abs() <= r {
                    let dx = (c as f64 * (((r * r - dy * dy) as f64) * inv_r2).sqrt())
                        .round_ties_even() as i64;
                    ((c - dx).max(0) as usize, ((c + dx + 1) as usize).min(width))
                } else {
                    (0, 0)
                }
            }
        };
        for j in j1..j2 {
            data[i * width + j] = true;
        }
    }
    Kernel {
        width,
        height,
        data,
    }
}

/// Sliding max (`is_max`) or min over a window `[x + off, x + off + len)`,
/// positions outside the row ignored. van Herk / Gil-Werman, O(n).
fn sliding(row: &[u8], off: isize, len: usize, is_max: bool, out: &mut [u8]) {
    let n = row.len();
    let id = if is_max { 0u8 } else { 255u8 };
    let pick = |a: u8, b: u8| if is_max { a.max(b) } else { a.min(b) };
    // Padded row p[i] = row[i - pad]; window for x is p[x + off + pad ..][..len].
    let pad = len + off.unsigned_abs();
    let total = n + 2 * pad;
    let blocks = total.div_ceil(len);
    let plen = blocks * len;
    let mut p = vec![id; plen];
    p[pad..pad + n].copy_from_slice(row);
    let mut g = vec![id; plen];
    let mut h = vec![id; plen];
    for b in 0..blocks {
        let s = b * len;
        g[s] = p[s];
        for i in s + 1..s + len {
            g[i] = pick(g[i - 1], p[i]);
        }
        h[s + len - 1] = p[s + len - 1];
        for i in (s..s + len - 1).rev() {
            h[i] = pick(h[i + 1], p[i]);
        }
    }
    for (x, o) in out.iter_mut().enumerate().take(n) {
        let start = (x as isize + off + pad as isize) as usize;
        let end = start + len - 1;
        *o = pick(h[start], g[end]);
    }
}

fn morph1(src: &Gray, k: &Kernel, is_max: bool) -> Gray {
    let (w, h) = (src.width, src.height);
    let (ax, ay) = k.anchor();
    let id = if is_max { 0u8 } else { 255u8 };
    let runs = k.runs();
    let mut out = Gray {
        width: w,
        height: h,
        data: vec![id; w * h],
    };
    if runs.is_empty() || w == 0 || h == 0 {
        return out;
    }
    // Horizontal pass per distinct (x0, len), then combine over kernel rows.
    let mut distinct: Vec<(usize, usize)> = runs.iter().map(|&(_, x0, l)| (x0, l)).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let mut planes: Vec<Vec<u8>> = Vec::with_capacity(distinct.len());
    for &(x0, len) in &distinct {
        let mut plane = vec![0u8; w * h];
        let off = x0 as isize - ax as isize;
        for y in 0..h {
            sliding(
                &src.data[y * w..(y + 1) * w],
                off,
                len,
                is_max,
                &mut plane[y * w..(y + 1) * w],
            );
        }
        planes.push(plane);
    }
    for &(dy, x0, len) in &runs {
        let pi = distinct.binary_search(&(x0, len)).unwrap();
        let plane = &planes[pi];
        let oy = dy as isize - ay as isize;
        for y in 0..h {
            let sy = y as isize + oy;
            if sy < 0 || sy >= h as isize {
                continue;
            }
            let s = &plane[sy as usize * w..(sy as usize + 1) * w];
            let d = &mut out.data[y * w..(y + 1) * w];
            if is_max {
                for (a, &b) in d.iter_mut().zip(s) {
                    *a = (*a).max(b);
                }
            } else {
                for (a, &b) in d.iter_mut().zip(s) {
                    *a = (*a).min(b);
                }
            }
        }
    }
    out
}

/// `cv2.erode(src, kernel, iterations=n)` with the default border (pixels
/// outside the image are ignored).
pub fn erode(src: &Gray, k: &Kernel, iterations: usize) -> Gray {
    let mut img = src.clone();
    for _ in 0..iterations.max(1) {
        img = morph1(&img, k, false);
    }
    img
}

/// `cv2.dilate(src, kernel, iterations=n)` with the default border.
pub fn dilate(src: &Gray, k: &Kernel, iterations: usize) -> Gray {
    let mut img = src.clone();
    for _ in 0..iterations.max(1) {
        img = morph1(&img, k, true);
    }
    img
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphOp {
    Erode,
    Dilate,
    Open,
    Close,
}

/// `cv2.morphologyEx(src, op, kernel, iterations=n)`.
pub fn morphology(src: &Gray, op: MorphOp, k: &Kernel, iterations: usize) -> Gray {
    match op {
        MorphOp::Erode => erode(src, k, iterations),
        MorphOp::Dilate => dilate(src, k, iterations),
        MorphOp::Open => dilate(&erode(src, k, iterations), k, iterations),
        MorphOp::Close => erode(&dilate(src, k, iterations), k, iterations),
    }
}

// ---------------------------------------------------------------- resize

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    /// `INTER_NEAREST`
    Nearest,
    /// `INTER_LINEAR` (the `cv2.resize` default)
    Linear,
}

const COEF_BITS: i32 = 11;
const COEF_SCALE: f32 = (1 << COEF_BITS) as f32;

fn saturate_i16(v: f32) -> i16 {
    v.round_ties_even().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

/// Linear coefficients for one axis, as OpenCV's `resizeGeneric` setup:
/// (source index, alpha0, alpha1, both-taps-valid).
fn linear_tab(src: usize, dst: usize, scale: f64) -> Vec<(usize, i16, i16, bool)> {
    (0..dst)
        .map(|d| {
            let mut f = ((d as f64 + 0.5) * scale - 0.5) as f32;
            let mut s = f.floor() as i64;
            f -= s as f32;
            let mut both = true;
            if s < 0 {
                f = 0.0;
                s = 0;
            }
            if s + 1 >= src as i64 {
                both = false;
                if s >= src as i64 - 1 {
                    f = 0.0;
                    s = src as i64 - 1;
                }
            }
            (
                s as usize,
                saturate_i16((1.0 - f) * COEF_SCALE),
                saturate_i16(f * COEF_SCALE),
                both,
            )
        })
        .collect()
}

/// `cv2.resize(src, (width, height), interpolation)` for 8-bit grey images,
/// bit-exact with OpenCV 4.14 on arm64 (KleidiCV and carotene HALs, then
/// OpenCV's fixed-point code with 11-bit coefficients).
pub fn resize(src: &Gray, width: usize, height: usize, interp: Interp) -> Gray {
    resize_cn(&src.data, src.width, src.height, 1, width, height, interp).map_or_else(
        || src.clone(),
        |data| Gray {
            width,
            height,
            data,
        },
    )
}

/// None: same size (OpenCV copies).
fn resize_cn(
    src: &[u8],
    sw: usize,
    sh: usize,
    cn: usize,
    dw: usize,
    dh: usize,
    interp: Interp,
) -> Option<Vec<u8>> {
    assert!(sw > 0 && sh > 0 && dw > 0 && dh > 0, "empty image");
    if sw == dw && sh == dh {
        return None;
    }
    let inv_x = dw as f64 / sw as f64;
    let inv_y = dh as f64 / sh as f64;
    let mut out = vec![0u8; dw * dh * cn];
    match interp {
        Interp::Nearest => {
            let (ifx, ify) = (1.0 / inv_x, 1.0 / inv_y);
            let xs: Vec<usize> = (0..dw)
                .map(|x| ((x as f64 * ifx).floor() as usize).min(sw - 1))
                .collect();
            for y in 0..dh {
                let sy = ((y as f64 * ify).floor() as usize).min(sh - 1);
                let srow = &src[sy * sw * cn..];
                for (x, &sx) in xs.iter().enumerate() {
                    for c in 0..cn {
                        out[(y * dw + x) * cn + c] = srow[sx * cn + c];
                    }
                }
            }
        }
        Interp::Linear => {
            // OpenCV for arm64 (pip wheels, iOS) tries the KleidiCV HAL
            // first, then carotene, then its own code; each rounds
            // differently. Follow the same order for 1-channel images.
            let wr = (1.0 / inv_x) as f32;
            let hr = (1.0 / inv_y) as f32;
            if cn == 1 {
                if let Some(out) = kleidicv_linear(src, sw, sh, dw, dh) {
                    return Some(out);
                }
            }
            if cn == 1 && sw >= 8 && dw >= 8 && dh >= 8 {
                return Some(if wr <= 1.0 && hr <= 1.0 {
                    carotene_linear(src, sw, sh, dw, dh, wr, hr)
                } else if wr <= 2.0 && hr <= 2.0 && sw >= 16 {
                    carotene_downsample(src, sw, sh, dw, dh, wr, hr)
                } else {
                    carotene_linear(src, sw, sh, dw, dh, wr, hr)
                });
            }
            let (sx, sy) = (1.0 / inv_x, 1.0 / inv_y);
            let area_fast = (sx - 2.0).abs() < f64::EPSILON && (sy - 2.0).abs() < f64::EPSILON;
            if area_fast {
                for y in 0..dh {
                    let r0 = &src[2 * y * sw * cn..];
                    let r1 = &src[(2 * y + 1) * sw * cn..];
                    for x in 0..dw {
                        for c in 0..cn {
                            let i = 2 * x * cn + c;
                            let s =
                                r0[i] as u32 + r0[i + cn] as u32 + r1[i] as u32 + r1[i + cn] as u32;
                            out[(y * dw + x) * cn + c] = ((s + 2) >> 2) as u8;
                        }
                    }
                }
                return Some(out);
            }
            let xt = linear_tab(sw, dw, sx);
            // Vertical taps (rows clamped to the image), beta from the unclamped fraction.
            let yt: Vec<(usize, usize, i32, i32)> = (0..dh)
                .map(|d| {
                    let mut f = ((d as f64 + 0.5) * sy - 0.5) as f32;
                    let s = f.floor() as i64;
                    f -= s as f32;
                    let r0 = s.clamp(0, sh as i64 - 1) as usize;
                    let r1 = (s + 1).clamp(0, sh as i64 - 1) as usize;
                    (
                        r0,
                        r1,
                        saturate_i16((1.0 - f) * COEF_SCALE) as i32,
                        saturate_i16(f * COEF_SCALE) as i32,
                    )
                })
                .collect();
            let hrow = |r: usize, buf: &mut [i32]| {
                let row = &src[r * sw * cn..(r + 1) * sw * cn];
                for (d, &(s, a0, a1, both)) in xt.iter().enumerate() {
                    for c in 0..cn {
                        let i = s * cn + c;
                        buf[d * cn + c] = if both {
                            row[i] as i32 * a0 as i32 + row[i + cn] as i32 * a1 as i32
                        } else {
                            row[i] as i32 * (1 << COEF_BITS)
                        };
                    }
                }
            };
            let n = dw * cn;
            let mut b0 = vec![0i32; n];
            let mut b1 = vec![0i32; n];
            let (mut have0, mut have1) = (usize::MAX, usize::MAX);
            for (y, &(r0, r1, be0, be1)) in yt.iter().enumerate() {
                if have0 != r0 {
                    if have1 == r0 {
                        std::mem::swap(&mut b0, &mut b1);
                        std::mem::swap(&mut have0, &mut have1);
                    } else {
                        hrow(r0, &mut b0);
                        have0 = r0;
                    }
                }
                if have1 != r1 {
                    hrow(r1, &mut b1);
                    have1 = r1;
                }
                let d = &mut out[y * n..(y + 1) * n];
                for x in 0..n {
                    let v = ((be0 * (b0[x] >> 4)) >> 16) + ((be1 * (b1[x] >> 4)) >> 16) + 2;
                    d[x] = (v >> 2) as u8;
                }
            }
        }
    }
    Some(out)
}

/// KleidiCV 26.03's `resize_linear_u8` for one channel (Apache-2.0, Copyright
/// Arm Limited), as OpenCV 4.14's arm64 HAL calls it. None where KleidiCV
/// does not implement the size (OpenCV then tries carotene).
fn kleidicv_linear(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; dw * dh];
    if sw == 2 * dw && sh == 2 * dh {
        for y in 0..dh {
            let (r0, r1) = (&src[2 * y * sw..], &src[(2 * y + 1) * sw..]);
            for x in 0..dw {
                let s = r0[2 * x] as u32
                    + r0[2 * x + 1] as u32
                    + r1[2 * x] as u32
                    + r1[2 * x + 1] as u32;
                out[y * dw + x] = ((s + 2) >> 2) as u8;
            }
        }
        return Some(out);
    }
    if dw == 2 * sw && dh == 2 * sh {
        return Some(kleidicv_up(src, sw, sh, 2));
    }
    if dw == 4 * sw && dh == 4 * sh {
        return Some(kleidicv_up(src, sw, sh, 4));
    }
    if !(dw * 3 >= sw && dw < sw && dh < sh) || dw < 8 {
        return None;
    }
    let ratio = if dw * 2 >= sw {
        if sw < 16 {
            return None;
        }
        2
    } else {
        if sw < 32 {
            return None;
        }
        3
    };
    const BITS: u64 = 16;
    const HALF: u64 = 1 << (BITS - 1);
    const STEP: usize = 16;
    const HSTEP: usize = 8;
    let rdiv = |n: u64, d: u64| (n + d / 2) / d;
    let aligned =
        |x: u64, n: u64, d: u64| rdiv(((x << BITS) + HALF) * n, d) - HALF + (1 << (BITS - 9));
    let (swu, dwu) = (sw as u64, dw as u64);
    let scale_x = |j: u64| rdiv((j * swu) << BITS, dwu) as u32;
    // Per output column: source column and 8-bit fraction.
    let mut cols = vec![(0usize, 0u32); dw];
    let mut put = |dx: usize, sx_base: u64, sx_fixp: u64, n: usize| {
        let frac0 = sx_fixp.wrapping_sub(sx_base << BITS) as u32;
        for j in 0..n {
            let delta = scale_x(j as u64).wrapping_add(frac0);
            cols[dx + j] = (
                sx_base as usize + ((delta >> 16) & 0xFF) as usize,
                (delta >> 8) & 0xFF,
            );
        }
    };
    let two_x = if sw >= STEP * ratio {
        dw / (2 * STEP)
    } else {
        0
    };
    let max_sx_2x = sw.saturating_sub(STEP * ratio) as u64;
    let vstep = rdiv((swu * STEP as u64) << BITS, dwu);
    let (mut dx, mut sx_fixp) = (0usize, 0u64);
    for i in 0..two_x {
        if i & 0xFF == 0 {
            sx_fixp = aligned(dx as u64, swu, dwu);
        }
        for _ in 0..2 {
            put(dx, (sx_fixp >> BITS).min(max_sx_2x), sx_fixp, STEP);
            sx_fixp += vstep;
            dx += STEP;
        }
    }
    sx_fixp = aligned(dx as u64, swu, dwu);
    let one = rdiv(swu << BITS, dwu);
    let max_sx_half = (sw - STEP * (ratio - 1)) as u64;
    let max_dx_half = dw - HSTEP;
    let hstep = rdiv((swu * HSTEP as u64) << BITS, dwu);
    let half = (dw - two_x * 2 * STEP).div_ceil(HSTEP);
    for _ in 0..half {
        let pulled = dx.min(max_dx_half);
        sx_fixp -= (dx - pulled) as u64 * one;
        dx = pulled;
        put(dx, (sx_fixp >> BITS).min(max_sx_half), sx_fixp, HSTEP);
        dx += HSTEP;
        sx_fixp += hstep;
    }
    let lerp = |a: u8, b: u8, f: u32| -> u8 {
        (((a as i32) << 8) + (b as i32 - a as i32) * f as i32 + 128).wrapping_shr(8) as u8
    };
    for dy in 0..dh {
        let sy_fixp = aligned(dy as u64, sh as u64, dh as u64);
        let sy = (sy_fixp >> BITS) as usize;
        let yf = ((sy_fixp - ((sy as u64) << BITS)) >> (BITS - 8)) as u32;
        let (top, bot) = (&src[sy * sw..], &src[(sy + 1) * sw..]);
        let o = &mut out[dy * dw..(dy + 1) * dw];
        for (x, &(sx, xf)) in cols.iter().enumerate() {
            let left = lerp(top[sx], bot[sx], yf);
            let right = lerp(top[sx + 1], bot[sx + 1], yf);
            o[x] = lerp(left, right, xf);
        }
    }
    Some(out)
}

/// KleidiCV's exact 2x and 4x upscales (fixed weights 3:1 and 7:5:3:1).
fn kleidicv_up(src: &[u8], sw: usize, sh: usize, k: usize) -> Vec<u8> {
    let (dw, dh) = (sw * k, sh * k);
    // Weights in quarters (2x) or eighths (4x).
    let den = if k == 2 { 4 } else { 8 };
    // Source pair and weight of the first, for one axis.
    let axis = |n: usize, d: usize| -> (usize, usize, u32) {
        let lead = k / 2; // output pixels before the first interior pair
        if d < lead {
            return (0, 0, den);
        }
        if d >= n * k - lead {
            return (n - 1, n - 1, den);
        }
        let e = d - lead;
        let (s, p) = (e / k, e % k);
        // Phases of the left half weight the left pixel more.
        (s, s + 1, if k == 2 { [3, 1][p] } else { [7, 5, 3, 1][p] })
    };
    let xs: Vec<_> = (0..dw).map(|d| axis(sw, d)).collect();
    let mut out = vec![0u8; dw * dh];
    for dy in 0..dh {
        let (r0, r1, wy) = axis(sh, dy);
        let edge_y = r0 == r1;
        for (dx, &(c0, c1, wx)) in xs.iter().enumerate() {
            let a = src[r0 * sw + c0] as u32;
            let b = src[r0 * sw + c1] as u32;
            let c = src[r1 * sw + c0] as u32;
            let d = src[r1 * sw + c1] as u32;
            let v = match (edge_y, c0 == c1) {
                (true, true) => a,
                (true, false) => (wx * a + (den - wx) * b + den / 2) / den,
                (false, true) => (wy * a + (den - wy) * c + den / 2) / den,
                (false, false) => {
                    let (wx2, wy2) = (den - wx, den - wy);
                    (wx * wy * a + wx2 * wy * b + wx * wy2 * c + wx2 * wy2 * d + den * den / 2)
                        / (den * den)
                }
            };
            out[dy * dw + dx] = v as u8;
        }
    }
    out
}

/// carotene `resizeLinearOpenCVchan<1>` (OpenCV 4.14 `hal/carotene/src/
/// resize.cpp`, BSD-3, Copyright NVIDIA): 7-bit weights, vertical pass first.
fn carotene_linear(
    src: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    wr: f32,
    hr: f32,
) -> Vec<u8> {
    let xoff = 0.5 * wr - 0.5;
    // Per output column: low and high source column, weight of the low one.
    let cols: Vec<(usize, usize, u32)> = (0..dw)
        .map(|d| {
            // clang contracts vmlaq_f32 to a fused multiply-add.
            let w = wr.mul_add(d as f32, xoff);
            let wi = w as i32; // truncation, like vcvtq_s32_f32
            let hi = if (wi as f32) < w { wi + 1 } else { wi };
            let lo = hi - 1;
            let weight = (((hi as f32 - w) * 128.0) as u32) & 0xFF;
            let max = sw as i32 - 1;
            (lo.clamp(0, max) as usize, hi.min(max) as usize, weight)
        })
        .collect();
    let yoff = 0.5 * hr - 0.5;
    let mut out = vec![0u8; dw * dh];
    let mut buf = vec![0u8; sw];
    for r in 0..dh {
        let w = (r as f32).mul_add(hr, yoff);
        let r0 = w.floor() as i64;
        let r1 = r0 + 1;
        let rw = ((r1 as f32 - w) * 128.0) as u32 & 0xFF;
        let r0 = r0.max(0) as usize;
        let r1 = r1.min(sh as i64 - 1) as usize;
        let (a, b) = (&src[r0 * sw..(r0 + 1) * sw], &src[r1 * sw..(r1 + 1) * sw]);
        for x in 0..sw {
            buf[x] = ((a[x] as u32 * rw + b[x] as u32 * (128 - rw) + 64) >> 7) as u8;
        }
        let o = &mut out[r * dw..(r + 1) * dw];
        for (d, &(lo, hi, cw)) in cols.iter().enumerate() {
            o[d] = ((buf[lo] as u32 * cw + buf[hi] as u32 * (128 - cw) + 64) >> 7) as u8;
        }
    }
    out
}

/// carotene `downsample_bilinear_8uc1` (scale factors in (1, 2]): 11-bit
/// weights and the `vqdmulh` rounding of OpenCV's old SSE code.
fn carotene_downsample(
    src: &[u8],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    wr: f32,
    hr: f32,
) -> Vec<u8> {
    const SHIFT: i32 = 11;
    let xoff = 0.5 * wr - 0.5;
    let yoff = 0.5 * hr - 0.5;
    let cols: Vec<(usize, usize, i32)> = (0..dw)
        .map(|d| {
            let c = (d as f32).mul_add(wr, xoff);
            let c1 = c as i64;
            let c2 = c1 + 1;
            let mut cw = ((c2 as f32 - c).mul_add((1 << SHIFT) as f32, 0.5)).floor() as i16 as i32;
            if c < 0.0 {
                cw = 1 << SHIFT;
            }
            (c1.max(0) as usize, c2.min(sw as i64 - 1) as usize, cw)
        })
        .collect();
    let qdmulh = |a: i32, b: i32| -> i32 {
        let v = (2 * a as i64 * b as i64) >> 16;
        v.clamp(i16::MIN as i64, i16::MAX as i64) as i32
    };
    let mut out = vec![0u8; dw * dh];
    for row in 0..dh {
        let r = (row as f32).mul_add(hr, yoff);
        let r0 = r.floor() as i64;
        let r1 = r0 + 1;
        let rw = ((r1 as f32 - r).mul_add((1 << SHIFT) as f32, 0.5)).floor() as i32 as i16 as i32;
        let rw2 = ((1 << SHIFT) - rw) as i16 as i32;
        let r0 = r0.max(0) as usize;
        let r1 = r1.min(sh as i64 - 1) as usize;
        let (a, b) = (&src[r0 * sw..], &src[r1 * sw..]);
        let o = &mut out[row * dw..(row + 1) * dw];
        for (d, &(lo, hi, cw)) in cols.iter().enumerate() {
            let h1 = ((a[hi] as i32) << SHIFT) + (a[lo] as i32 - a[hi] as i32) * cw;
            let h2 = ((b[hi] as i32) << SHIFT) + (b[lo] as i32 - b[hi] as i32) * cw;
            let s1 = qdmulh((h1 >> 4) as i16 as i32, rw);
            let s2 = qdmulh((h2 >> 4) as i16 as i32, rw2);
            let sum = ((s1 >> 1) + (s2 >> 1)) as i16 as i32;
            o[d] = ((sum + 2) >> 2).clamp(0, 255) as u8;
        }
    }
    out
}

/// Pillow's `Image.resize((width, height))` (bicubic, its default) on 8-bit
/// data with `cn` interleaved channels. homr's `resize_image` uses it.
fn pil_bicubic_cn(src: &[u8], sw: usize, sh: usize, cn: usize, dw: usize, dh: usize) -> Vec<u8> {
    const PREC: u32 = 22;
    fn filter(x: f64) -> f64 {
        let a = -0.5;
        let x = x.abs();
        if x < 1.0 {
            ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0
        } else if x < 2.0 {
            (((x - 5.0) * x + 8.0) * x - 4.0) * a
        } else {
            0.0
        }
    }
    // (xmin, coefficients) per output position.
    fn coeffs(in_size: usize, out_size: usize) -> Vec<(usize, Vec<i32>)> {
        let scale = (in_size as f32 as f64) / out_size as f64;
        let filterscale = scale.max(1.0);
        let support = 2.0 * filterscale;
        (0..out_size)
            .map(|xx| {
                let center = (xx as f64 + 0.5) * scale;
                let ss = 1.0 / filterscale;
                let xmin = ((center - support + 0.5) as i64).max(0) as usize;
                let xmax = ((center + support + 0.5) as i64).min(in_size as i64) as usize;
                let n = xmax.saturating_sub(xmin);
                let mut k: Vec<f64> = (0..n)
                    .map(|x| filter((x as f64 + xmin as f64 - center + 0.5) * ss))
                    .collect();
                let ww: f64 = k.iter().sum();
                if ww != 0.0 {
                    for v in &mut k {
                        *v /= ww;
                    }
                }
                let ik = k
                    .iter()
                    .map(|&v| {
                        if v < 0.0 {
                            (-0.5 + v * (1u32 << PREC) as f64) as i32
                        } else {
                            (0.5 + v * (1u32 << PREC) as f64) as i32
                        }
                    })
                    .collect();
                (xmin, ik)
            })
            .collect()
    }
    fn clip8(v: i32) -> u8 {
        (v >> PREC).clamp(0, 255) as u8
    }
    let mut cur = src.to_vec();
    let mut cw = sw;
    if dw != sw {
        let tab = coeffs(sw, dw);
        let mut out = vec![0u8; dw * sh * cn];
        for y in 0..sh {
            let row = &cur[y * sw * cn..];
            for (xx, (xmin, k)) in tab.iter().enumerate() {
                for c in 0..cn {
                    let mut s: i32 = 1 << (PREC - 1);
                    for (i, &kv) in k.iter().enumerate() {
                        s = s.wrapping_add(row[(xmin + i) * cn + c] as i32 * kv);
                    }
                    out[(y * dw + xx) * cn + c] = clip8(s);
                }
            }
        }
        cur = out;
        cw = dw;
    }
    if dh != sh {
        let tab = coeffs(sh, dh);
        let n = cw * cn;
        let mut out = vec![0u8; n * dh];
        for (yy, (ymin, k)) in tab.iter().enumerate() {
            for x in 0..n {
                let mut s: i32 = 1 << (PREC - 1);
                for (i, &kv) in k.iter().enumerate() {
                    s = s.wrapping_add(cur[(ymin + i) * n + x] as i32 * kv);
                }
                out[yy * n + x] = clip8(s);
            }
        }
        cur = out;
    }
    cur
}

/// `np.array(PIL.Image.fromarray(src).resize((width, height)))`.
pub fn pil_resize_bicubic(src: &Gray, width: usize, height: usize) -> Gray {
    Gray {
        width,
        height,
        data: pil_bicubic_cn(&src.data, src.width, src.height, 1, width, height),
    }
}

/// [`pil_resize_bicubic`] for a colour image (Pillow treats it as RGB; the
/// channel order does not matter).
pub fn pil_resize_bicubic_bgr(src: &Bgr, width: usize, height: usize) -> Bgr {
    Bgr {
        width,
        height,
        data: pil_bicubic_cn(&src.data, src.width, src.height, 3, width, height),
    }
}

// ---------------------------------------------------------------- blur

/// `BORDER_REFLECT_101` index.
fn reflect101(i: isize, n: usize) -> usize {
    if n == 1 {
        return 0;
    }
    let n = n as isize;
    let mut i = i;
    while i < 0 || i >= n {
        if i < 0 {
            i = -i;
        }
        if i >= n {
            i = 2 * (n - 1) - i;
        }
    }
    i as usize
}

fn blur_cn(src: &[u8], w: usize, h: usize, cn: usize, kw: usize, kh: usize) -> Vec<u8> {
    let (ax, ay) = ((kw / 2) as isize, (kh / 2) as isize);
    // Row sums.
    let mut rows = vec![0i32; w * h * cn];
    for y in 0..h {
        for x in 0..w {
            for c in 0..cn {
                let mut s = 0;
                for j in 0..kw as isize {
                    let sx = reflect101(x as isize + j - ax, w);
                    s += src[(y * w + sx) * cn + c] as i32;
                }
                rows[(y * w + x) * cn + c] = s;
            }
        }
    }
    let scale = 1.0 / (kw * kh) as f64;
    let mut out = vec![0u8; w * h * cn];
    for y in 0..h {
        for i in 0..w * cn {
            let mut s = 0;
            for k in 0..kh as isize {
                let sy = reflect101(y as isize + k - ay, h);
                s += rows[sy * w * cn + i];
            }
            out[y * w * cn + i] = (s as f64 * scale).round_ties_even().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// `cv2.blur(src, (kw, kh))`: normalised box filter, `BORDER_REFLECT_101`.
pub fn blur(src: &Gray, kw: usize, kh: usize) -> Gray {
    Gray {
        width: src.width,
        height: src.height,
        data: blur_cn(&src.data, src.width, src.height, 1, kw, kh),
    }
}

/// [`blur`] for a colour image.
pub fn blur_bgr(src: &Bgr, kw: usize, kh: usize) -> Bgr {
    Bgr {
        width: src.width,
        height: src.height,
        data: blur_cn(&src.data, src.width, src.height, 3, kw, kh),
    }
}

// ---------------------------------------------------------------- CLAHE

/// `cv2.createCLAHE(clipLimit, (tiles_x, tiles_y)).apply(src)` for 8-bit
/// images (OpenCV's CPU code: histogram clip and redistribution per tile,
/// bilinear blend of the tile lookup tables in `f32`).
pub fn clahe(src: &Gray, clip_limit: f64, tiles_x: usize, tiles_y: usize) -> Gray {
    const HIST: usize = 256;
    let (w, h) = (src.width, src.height);
    // Pad right and bottom with BORDER_REFLECT_101 unless both sizes divide.
    // OpenCV 4.14 pads by `tiles - size % tiles` on both axes, so a side
    // that already divides still gets a full tile row or column of padding.
    let (right, bottom) = if w.is_multiple_of(tiles_x) && h.is_multiple_of(tiles_y) {
        (0, 0)
    } else {
        (tiles_x - w % tiles_x, tiles_y - h % tiles_y)
    };
    let (ew, eh) = (w + right, h + bottom);
    let ext = |x: usize, y: usize| -> u8 {
        if x < w && y < h {
            src.data[y * w + x]
        } else {
            src.data[reflect101(y as isize, h) * w + reflect101(x as isize, w)]
        }
    };
    let (tw, th) = (ew / tiles_x, eh / tiles_y);
    let total = tw * th;
    let lut_scale = (HIST - 1) as f32 / total as f32;
    let clip = if clip_limit > 0.0 {
        ((clip_limit * total as f64 / HIST as f64) as i32).max(1)
    } else {
        0
    };
    let mut lut = vec![0u8; tiles_x * tiles_y * HIST];
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let mut hist = [0i32; HIST];
            for y in ty * th..(ty + 1) * th {
                for x in tx * tw..(tx + 1) * tw {
                    hist[ext(x, y) as usize] += 1;
                }
            }
            if clip > 0 {
                let mut clipped = 0;
                for v in hist.iter_mut() {
                    if *v > clip {
                        clipped += *v - clip;
                        *v = clip;
                    }
                }
                let batch = clipped / HIST as i32;
                let mut residual = clipped - batch * HIST as i32;
                for v in hist.iter_mut() {
                    *v += batch;
                }
                if residual != 0 {
                    let step = (HIST as i32 / residual).max(1) as usize;
                    let mut i = 0;
                    while i < HIST && residual > 0 {
                        hist[i] += 1;
                        i += step;
                        residual -= 1;
                    }
                }
            }
            let l = &mut lut[(ty * tiles_x + tx) * HIST..][..HIST];
            let mut sum = 0i32;
            for i in 0..HIST {
                sum += hist[i];
                l[i] = (sum as f32 * lut_scale).round_ties_even().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let inv_tw = 1.0f32 / tw as f32;
    let inv_th = 1.0f32 / th as f32;
    let xs: Vec<(usize, usize, f32, f32)> = (0..w)
        .map(|x| {
            // OpenCV is built with clang, which contracts `a * b + c` to
            // a fused multiply-add on arm64: do the same to match bit for bit.
            let txf = (x as f32).mul_add(inv_tw, -0.5);
            let t1 = txf.floor() as i64;
            let xa = txf - t1 as f32;
            (
                t1.max(0) as usize,
                ((t1 + 1).min(tiles_x as i64 - 1)) as usize,
                xa,
                1.0 - xa,
            )
        })
        .collect();
    let mut out = Gray::new(w, h);
    for y in 0..h {
        let tyf = (y as f32).mul_add(inv_th, -0.5);
        let t1 = tyf.floor() as i64;
        let ya = tyf - t1 as f32;
        let ya1 = 1.0 - ya;
        let ty1 = t1.max(0) as usize;
        let ty2 = ((t1 + 1).min(tiles_y as i64 - 1)) as usize;
        let p1 = &lut[ty1 * tiles_x * HIST..];
        let p2 = &lut[ty2 * tiles_x * HIST..];
        for (x, &(tx1, tx2, xa, xa1)) in xs.iter().enumerate() {
            let v = src.data[y * w + x] as usize;
            let (i1, i2) = (tx1 * HIST + v, tx2 * HIST + v);
            let l1 = (p1[i1] as f32).mul_add(xa1, p1[i2] as f32 * xa);
            let l2 = (p2[i1] as f32).mul_add(xa1, p2[i2] as f32 * xa);
            let res = l1.mul_add(ya1, l2 * ya);
            out.data[y * w + x] = res.round_ties_even().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/omr/cv2")
    }
    fn gray(name: &str) -> Gray {
        let p = dir().join(format!("{name}.png"));
        Gray::from_png(&std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
            .unwrap()
    }
    fn bgr(name: &str) -> Bgr {
        Bgr::from_png(&std::fs::read(dir().join(format!("{name}.png"))).unwrap()).unwrap()
    }
    #[track_caller]
    fn same(name: &str, got: &Gray, want: &Gray) {
        assert_eq!(
            (got.width, got.height),
            (want.width, want.height),
            "{name}: size"
        );
        let diff: Vec<usize> = (0..got.data.len())
            .filter(|&i| got.data[i] != want.data[i])
            .collect();
        if let Some(&i) = diff.first() {
            panic!(
                "{name}: {} of {} px differ, first at ({}, {}): got {} want {}",
                diff.len(),
                got.data.len(),
                i % got.width,
                i / got.width,
                got.data[i],
                want.data[i]
            );
        }
    }
    fn kernel_img(k: &Kernel) -> Gray {
        Gray {
            width: k.width,
            height: k.height,
            data: k.data.iter().map(|&b| if b { 255 } else { 0 }).collect(),
        }
    }

    #[test]
    fn lib_06_cv_bgr_to_gray() {
        same(
            "gray",
            &bgr_to_gray(&bgr("bgr_23x17")),
            &gray("gray_bgr_23x17"),
        );
    }

    #[test]
    fn lib_06_cv_threshold() {
        let g = gray("noise_37x29");
        same(
            "bin",
            &threshold(&g, 127.0, 255, ThreshType::Binary),
            &gray("thresh_bin_127"),
        );
        same(
            "inv",
            &threshold(&g, 100.5, 1, ThreshType::BinaryInv),
            &gray("thresh_inv_100.5"),
        );
    }

    #[test]
    fn lib_06_cv_structuring_elements() {
        for (w, h) in [
            (1, 5),
            (5, 35),
            (5, 3),
            (9, 3),
            (3, 3),
            (4, 6),
            (7, 7),
            (2, 2),
            (6, 4),
            (8, 11),
        ] {
            same(
                &format!("ellipse {w}x{h}"),
                &kernel_img(&structuring_element(Shape::Ellipse, w, h)),
                &gray(&format!("kernel_ellipse_{w}x{h}")),
            );
            same(
                &format!("cross {w}x{h}"),
                &kernel_img(&structuring_element(Shape::Cross, w, h)),
                &gray(&format!("kernel_cross_{w}x{h}")),
            );
        }
    }

    #[test]
    fn lib_06_cv_erode_dilate_open_close() {
        for src in ["sparse_41x31", "noise_37x29"] {
            let g = gray(src);
            for (w, h) in [(5, 3), (9, 3), (1, 5), (5, 35), (4, 6)] {
                let k = structuring_element(Shape::Ellipse, w, h);
                same(
                    &format!("dilate {w}x{h} {src}"),
                    &dilate(&g, &k, 1),
                    &gray(&format!("dilate_ellipse_{w}x{h}_{src}")),
                );
                same(
                    &format!("erode {w}x{h} {src}"),
                    &erode(&g, &k, 1),
                    &gray(&format!("erode_ellipse_{w}x{h}_{src}")),
                );
            }
            same(
                "dilate ones 5x3",
                &dilate(&g, &Kernel::ones(3, 5), 1),
                &gray(&format!("dilate_ones_5x3_{src}")),
            );
            same(
                "erode ones 9x9",
                &morphology(&g, MorphOp::Erode, &Kernel::ones(9, 9), 1),
                &gray(&format!("erode_ones_9x9_{src}")),
            );
            same(
                "close ones 7x7",
                &morphology(&g, MorphOp::Close, &Kernel::ones(7, 7), 1),
                &gray(&format!("close_ones_7x7_{src}")),
            );
            let k = structuring_element(Shape::Rect, 10, 1);
            same(
                "erode rect 10x1 it2",
                &morphology(&g, MorphOp::Erode, &k, 2),
                &gray(&format!("erode_rect_10x1_it2_{src}")),
            );
            same(
                "close rect 10x1 it2",
                &morphology(&g, MorphOp::Close, &k, 2),
                &gray(&format!("close_rect_10x1_it2_{src}")),
            );
            let k = structuring_element(Shape::Ellipse, 5, 3);
            same(
                "open ellipse 5x3 it2",
                &morphology(&g, MorphOp::Open, &k, 2),
                &gray(&format!("open_ellipse_5x3_it2_{src}")),
            );
        }
    }

    #[test]
    fn lib_06_cv_resize_linear_and_nearest() {
        let g = gray("noise_37x29");
        for (w, h) in [
            (50, 40),
            (20, 15),
            (18, 14),
            (74, 58),
            (37, 60),
            (11, 29),
            (3, 2),
            (100, 7),
        ] {
            same(
                &format!("linear {w}x{h}"),
                &resize(&g, w, h, Interp::Linear),
                &gray(&format!("resize_linear_{w}x{h}")),
            );
            same(
                &format!("nearest {w}x{h}"),
                &resize(&g, w, h, Interp::Nearest),
                &gray(&format!("resize_nearest_{w}x{h}")),
            );
        }
        same(
            "linear half",
            &resize(&gray("noise_36x28"), 18, 14, Interp::Linear),
            &gray("resize_linear_half_36x28"),
        );
        assert_eq!(resize(&g, 37, 29, Interp::Linear), g);
        let b = gray("smooth_400x304_b");
        for (w, h) in [
            (293, 223),
            (150, 114),
            (520, 395),
            (95, 70),
            (400, 150),
            (612, 304),
            (37, 29),
        ] {
            same(
                &format!("linear big {w}x{h}"),
                &resize(&b, w, h, Interp::Linear),
                &gray(&format!("resize_linear_big_{w}x{h}")),
            );
        }
        let s = gray("smooth_133x101");
        for (w, h) in [(266, 202), (532, 404)] {
            same(
                &format!("linear up {w}x{h}"),
                &resize(&s, w, h, Interp::Linear),
                &gray(&format!("resize_linear_up_{w}x{h}")),
            );
        }
    }

    #[test]
    fn lib_06_pil_resize_bicubic() {
        let g = gray("noise_37x29");
        for (w, h) in [(28, 22), (50, 40), (37, 50), (20, 29)] {
            same(
                &format!("bicubic {w}x{h}"),
                &pil_resize_bicubic(&g, w, h),
                &gray(&format!("pil_bicubic_{w}x{h}")),
            );
        }
        let got = pil_resize_bicubic_bgr(&bgr("bgr_23x17"), 17, 13);
        let want = bgr("pil_bicubic_bgr_17x13");
        assert_eq!(got, want);
    }

    #[test]
    fn lib_06_cv_blur() {
        same(
            "blur 7x7",
            &blur(&gray("noise_37x29"), 7, 7),
            &gray("blur_7x7_noise_37x29"),
        );
        same(
            "blur 3x5",
            &blur(&gray("smooth_133x101"), 3, 5),
            &gray("blur_3x5_smooth_133x101"),
        );
        assert_eq!(blur_bgr(&bgr("bgr_23x17"), 7, 7), bgr("blur_7x7_bgr_23x17"));
    }

    #[test]
    fn lib_06_cv_clahe() {
        same(
            "clahe 133x101",
            &clahe(&gray("smooth_133x101"), 1.0, 8, 8),
            &gray("clahe_smooth_133x101"),
        );
        same(
            "clahe noise",
            &clahe(&gray("noise_37x29"), 1.0, 8, 8),
            &gray("clahe_noise_37x29"),
        );
        same(
            "clahe 400x304",
            &clahe(&gray("smooth_400x304"), 1.0, 8, 8),
            &gray("clahe_smooth_400x304"),
        );
        same(
            "clahe 40 4x3",
            &clahe(&gray("smooth_133x101"), 40.0, 4, 3),
            &gray("clahe40_smooth_133x101"),
        );
        same(
            "clahe 160x101 (width divides)",
            &clahe(&gray("smooth_160x101"), 1.0, 8, 8),
            &gray("clahe_smooth_160x101"),
        );
    }

    #[test]
    fn lib_06_cv_crop_and_channels() {
        let b = bgr("bgr_23x17");
        let c = b.crop(2, 3, 5, 4);
        assert_eq!(c.get(0, 0), b.get(2, 3));
        let joined = Bgr::from_channels([&b.channel(0), &b.channel(1), &b.channel(2)]);
        assert_eq!(joined, b);
        assert_eq!(gray_to_bgr(&b.channel(1)).get(4, 4)[2], b.get(4, 4)[1]);
    }
}
