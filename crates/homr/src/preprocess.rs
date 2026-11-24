//! Stage 1 (homr `autocrop`, `resize`, `color_adjust`): crop the page to its
//! content, resize it to homr's width (1920 px) and apply CLAHE.
//!
//! A port of homr 0.7.1 `autocrop.autocrop_with_offset`,
//! `resize.resize_image` and `color_adjust.apply_clahe`. The float steps
//! follow numpy 2's types (float32 chromaticity, float64 means, NEP 50
//! comparisons in float32), so the crop matches homr's to the pixel.
use crate::boxes::contours::{find_contours, Retr};
use crate::boxes::geom::{bounding_rect, contour_area, convex_hull, fill_poly};
use crate::image_ops::{
    bgr_to_gray, blur_bgr, clahe, dilate, gray_to_bgr, morphology, pil_resize_bicubic_bgr,
    threshold, Bgr, Kernel, MorphOp, ThreshType,
};
use crate::{Error, Gray};

/// homr resizes every page to this width (`resize.calc_target_image_size`).
pub const TARGET_WIDTH: usize = 1920;

/// Light and shadow change the brightness of paper but hardly its colour
/// (homr `max_chromaticity_difference`, compared in float32).
const MAX_CHROMATICITY_DIFFERENCE: f32 = 0.07;

/// The image fed to the segmentation net, and how to map back to the page.
#[derive(Debug, Clone)]
pub struct Preprocessed {
    /// CLAHE-adjusted grey image, `TARGET_WIDTH` wide (homr `preprocessed`).
    pub image: Gray,
    /// Top-left of the crop on the input page, in input pixels.
    pub crop_offset: (usize, usize),
    /// Input pixels per preprocessed pixel, x and y (homr `undo_resize`).
    pub scale: (f64, f64),
}

impl Preprocessed {
    /// A point of the preprocessed image on the input page (homr
    /// `chain(undo_resize(...), undo_crop(...))`).
    pub fn to_page(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.scale.0 + self.crop_offset.0 as f64,
            y * self.scale.1 + self.crop_offset.1 as f64,
        )
    }
}

/// Stage 1 on a PNG page, read like `cv2.imread` (colour).
pub fn preprocess_png(png: &[u8]) -> Result<Preprocessed, Error> {
    Ok(preprocess_bgr(&Bgr::from_png(png)?))
}

/// Stage 1 on a grey page (same as a colour page with equal channels).
pub fn preprocess(page: &Gray) -> Preprocessed {
    preprocess_bgr(&gray_to_bgr(page))
}

/// Stage 1 (homr `load_and_preprocess_predictions` up to `apply_clahe`).
pub fn preprocess_bgr(page: &Bgr) -> Preprocessed {
    let (cropped, offset) = autocrop_with_offset(page);
    let resized = resize_image(&cropped);
    let image = apply_clahe(&resized);
    Preprocessed {
        scale: (
            cropped.width as f64 / image.width as f64,
            cropped.height as f64 / image.height as f64,
        ),
        image,
        crop_offset: offset,
    }
}

/// homr `color_adjust.apply_clahe`: BGR to grey, CLAHE (clip 1.0, 8x8 tiles).
pub fn apply_clahe(image: &Bgr) -> Gray {
    clahe(&bgr_to_gray(image), 1.0, 8, 8)
}

/// homr `resize.calc_target_image_size`.
pub fn target_size(width: usize, height: usize) -> (usize, usize) {
    if width == TARGET_WIDTH {
        return (width, height);
    }
    let ratio = TARGET_WIDTH as f64 / width as f64;
    // Python's round(): halves to even.
    (
        TARGET_WIDTH,
        (height as f64 * ratio).round_ties_even() as usize,
    )
}

/// homr `resize.resize_image`: Pillow bicubic to `TARGET_WIDTH` wide.
pub fn resize_image(image: &Bgr) -> Bgr {
    let (w, h) = target_size(image.width, image.height);
    if (w, h) == (image.width, image.height) {
        return image.clone();
    }
    pil_resize_bicubic_bgr(image, w, h)
}

/// numpy `_chromaticity`: every channel + 1, divided by the sum (float32).
fn chromaticity(p: [u8; 3]) -> [f32; 3] {
    let v = [p[0] as f32 + 1.0, p[1] as f32 + 1.0, p[2] as f32 + 1.0];
    let s = v[0] + v[1] + v[2];
    [v[0] / s, v[1] / s, v[2] / s]
}

/// `np.median` of float32 values (mean of the middle two in float32).
fn median_f32(v: &mut [f32]) -> f32 {
    let n = v.len();
    let (_, hi, _) = v.select_nth_unstable_by(n / 2, f32::total_cmp);
    let hi = *hi;
    if n % 2 == 1 {
        return hi;
    }
    let lo = v[..n / 2].iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (lo + hi) / 2.0
}

/// `np.median(_chromaticity(pixels), axis=0)`.
fn median_chromaticity(pixels: &[[u8; 3]]) -> [f32; 3] {
    let chroma: Vec<[f32; 3]> = pixels.iter().map(|&p| chromaticity(p)).collect();
    let mut out = [0.0; 3];
    let mut col = Vec::with_capacity(chroma.len());
    for (c, o) in out.iter_mut().enumerate() {
        col.clear();
        col.extend(chroma.iter().map(|v| v[c]));
        *o = median_f32(&mut col);
    }
    out
}

/// `np.median(pixels, axis=0)` of uint8 colours (float64).
fn median_color(pixels: &[[u8; 3]]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let mut hist = [0usize; 256];
        for p in pixels {
            hist[p[c] as usize] += 1;
        }
        let n = pixels.len();
        let nth = |k: usize| {
            let mut acc = 0;
            for (v, &h) in hist.iter().enumerate() {
                acc += h;
                if acc > k {
                    return v as f64;
                }
            }
            255.0
        };
        *o = if n % 2 == 1 {
            nth(n / 2)
        } else {
            (nth(n / 2 - 1) + nth(n / 2)) / 2.0
        };
    }
    out
}

fn chroma_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// homr `_is_paper` on the pixels of `img[y0..y1, x0..x1]`.
fn is_paper(img: &Bgr, x0: usize, y0: usize, x1: usize, y1: usize, paper: [f32; 3]) -> bool {
    if x1 <= x0 || y1 <= y0 {
        return true;
    }
    let mut px = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for y in y0..y1 {
        for x in x0..x1 {
            px.push(img.get(x, y));
        }
    }
    chroma_distance(median_chromaticity(&px), paper) <= MAX_CHROMATICITY_DIFFERENCE
}

/// homr `_keep_sides_with_paper`: extend the box to the image border on
/// every side where the strip beside it is paper.
fn keep_sides_with_paper(
    img: &Bgr,
    (x, y, w, h): (usize, usize, usize, usize),
    paper: [f32; 3],
) -> (usize, usize, usize, usize) {
    let (width, height) = (img.width, img.height);
    let keep_left = is_paper(img, 0, y, x, y + h, paper);
    let keep_right = is_paper(img, x + w, y, width, y + h, paper);
    let keep_top = is_paper(img, x, 0, x + w, y, paper);
    let keep_bottom = is_paper(img, x, y + h, x + w, height, paper);
    let left = if keep_left { 0 } else { x };
    let top = if keep_top { 0 } else { y };
    let right = if keep_right { width } else { x + w };
    let bottom = if keep_bottom { height } else { y + h };
    (left, top, right - left, bottom - top)
}

/// homr `_remove_surroundings`: paint what is beside the sheet (outside the
/// convex hull of its contour) and not paper-coloured in the paper colour.
fn remove_surroundings(crop: &Bgr, sheet: &[(i32, i32)], paper: &[[u8; 3]]) -> Bgr {
    let (w, h) = (crop.width, crop.height);
    let mut beside = Gray {
        width: w,
        height: h,
        data: vec![1; w * h],
    };
    fill_poly(&mut beside, &[convex_hull(sheet)], 0);
    let paper_color = median_color(paper);
    let paper_chroma = median_chromaticity(paper);
    let smooth = blur_bgr(crop, 7, 7);
    let paper_mean = (paper_color[0] + paper_color[1] + paper_color[2]) / 3.0;
    let bright_limit = 0.25 * paper_mean;
    let mut background = Gray::new(w, h);
    for i in 0..w * h {
        let p = [
            smooth.data[3 * i],
            smooth.data[3 * i + 1],
            smooth.data[3 * i + 2],
        ];
        let mean = (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0;
        let bright = mean > bright_limit;
        let diff = chroma_distance(chromaticity(p), paper_chroma);
        background.data[i] =
            (beside.data[i] != 0 && bright && diff > MAX_CHROMATICITY_DIFFERENCE) as u8;
    }
    let background = dilate(&background, &Kernel::ones(5, 5), 1);
    let fill = [
        paper_color[0] as u8,
        paper_color[1] as u8,
        paper_color[2] as u8,
    ];
    let mut out = crop.clone();
    for i in 0..w * h {
        if background.data[i] != 0 && beside.data[i] != 0 {
            out.data[3 * i..3 * i + 3].copy_from_slice(&fill);
        }
    }
    out
}

/// homr `autocrop_with_offset`: find the sheet of paper (the largest
/// contour of the bright area), crop to it with a margin, and paint the
/// desk beside a rotated sheet in the paper colour. Returns the image
/// unchanged with offset (0, 0) for a full-page scan.
pub fn autocrop_with_offset(img: &Bgr) -> (Bgr, (usize, usize)) {
    let (page_width, page_height) = (img.width, img.height);
    let gray = bgr_to_gray(img);
    // cv2.calcHist([img], [0], ...): the histogram of the blue channel.
    let mut hist = [0usize; 256];
    for p in img.data.iter().step_by(3) {
        hist[*p as usize] += 1;
    }
    let mut dominant = 0;
    for v in 1..256 {
        if hist[v] > hist[dominant] {
            dominant = v;
        }
    }
    let thresh = threshold(&gray, dominant as f64 - 30.0, 255, ThreshType::Binary);
    let morph = morphology(&thresh, MorphOp::Close, &Kernel::ones(7, 7), 1);
    let morph = morphology(&morph, MorphOp::Erode, &Kernel::ones(9, 9), 1);

    let mut best_area = 0.0;
    let mut big: Option<Vec<(i32, i32)>> = None;
    for c in find_contours(&morph, Retr::External) {
        let area = contour_area(&c);
        if area > best_area {
            best_area = area;
            big = Some(c);
        }
    }
    let Some(big) = big else {
        return (img.clone(), (0, 0));
    };
    let (x, y, w, h) = bounding_rect(&big);
    let (x, y, w, h) = (x as usize, y as usize, w as usize, h as usize);
    if (w as f64) < page_width as f64 * 0.25 || (h as f64) < page_height as f64 * 0.25 {
        return (img.clone(), (0, 0));
    }
    // A sample of the paper: every 10th bright pixel of the box, row-major.
    let mut paper = Vec::new();
    let mut k = 0usize;
    for yy in y..y + h {
        for xx in x..x + w {
            if thresh.get(xx, yy) > 0 {
                if k.is_multiple_of(10) {
                    paper.push(img.get(xx, yy));
                }
                k += 1;
            }
        }
    }
    if paper.is_empty() {
        return (img.clone(), (0, 0));
    }
    let paper_chroma = median_chromaticity(&paper);
    // The contour was eroded: keep a margin so staffs at the paper's edge stay.
    let margin = 5.max(page_width.max(page_height) / 100);
    let (x0, y0) = (x.saturating_sub(margin), y.saturating_sub(margin));
    let x1 = page_width.min(x + w + margin);
    let y1 = page_height.min(y + h + margin);
    let (x, y, w, h) = keep_sides_with_paper(img, (x0, y0, x1 - x0, y1 - y0), paper_chroma);
    if (x, y, w, h) == (0, 0, page_width, page_height) {
        return (img.clone(), (0, 0));
    }
    let crop = img.crop(x, y, w, h);
    let sheet: Vec<(i32, i32)> = big
        .iter()
        .map(|&(px, py)| (px - x as i32, py - y as i32))
        .collect();
    (remove_surroundings(&crop, &sheet, &paper), (x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/omr/cv2")
    }
    fn bgr(name: &str) -> Bgr {
        Bgr::from_png(&std::fs::read(dir().join(format!("{name}.png"))).unwrap()).unwrap()
    }

    #[test]
    fn lib_06_homr_autocrop_matches_python_on_photos() {
        let offsets: std::collections::HashMap<String, (usize, usize)> =
            serde_json::from_slice(&std::fs::read(dir().join("photo_offsets.json")).unwrap())
                .unwrap();
        for name in ["photo_rotated", "photo_left_edge", "photo_dark_desk"] {
            let (crop, off) = autocrop_with_offset(&bgr(name));
            let want = bgr(&format!("{name}_crop"));
            assert_eq!(off, offsets[name], "{name}: offset");
            assert_eq!(
                (crop.width, crop.height),
                (want.width, want.height),
                "{name}: size"
            );
            let diff = (0..crop.data.len())
                .filter(|&i| crop.data[i] != want.data[i])
                .count();
            assert_eq!(diff, 0, "{name}: {diff} channel values differ");
        }
    }

    #[test]
    fn lib_06_homr_target_size_rounds_like_python() {
        assert_eq!(target_size(1920, 7), (1920, 7));
        assert_eq!(target_size(2550, 3600), (1920, 2711));
        // 960 * 2 = 1920 exactly; 5 * 2 = 10.
        assert_eq!(target_size(960, 5), (1920, 10));
        // 3840 -> half: 5 / 2 = 2.5 rounds to even (2).
        assert_eq!(target_size(3840, 5), (1920, 2));
    }

    #[test]
    fn lib_06_homr_median_like_numpy() {
        assert_eq!(median_f32(&mut [3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_f32(&mut [4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median_color(&[[1, 2, 3], [4, 6, 8]]), [2.5, 4.0, 5.5]);
        assert_eq!(median_color(&[[9, 0, 0], [1, 0, 0], [5, 0, 0]])[0], 5.0);
    }

    #[test]
    fn lib_06_homr_to_page_undoes_resize_and_crop() {
        let p = Preprocessed {
            image: Gray::new(1920, 10),
            crop_offset: (10, 20),
            scale: (2.0, 0.5),
        };
        assert_eq!(p.to_page(3.0, 4.0), (16.0, 22.0));
    }
}
