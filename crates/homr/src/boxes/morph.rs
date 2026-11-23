//! Binary morphology with OpenCV semantics (anchor at the kernel centre,
//! pixels outside the image ignored), for the masks of stages 3 and 4.
//! Private helper until `crate::image_ops` has erode/dilate.
use crate::Gray;

/// A kernel as (dx, dy) offsets from the anchor.
pub struct Kernel(Vec<(isize, isize)>);

/// `cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (w, h))`.
pub fn ellipse(w: usize, h: usize) -> Kernel {
    if w == 1 && h == 1 {
        return rect(1, 1);
    }
    let r = (h / 2) as i64;
    let c = (w / 2) as i64;
    let inv_r2 = if r != 0 { 1.0 / (r * r) as f64 } else { 0.0 };
    let mut out = Vec::new();
    for i in 0..h as i64 {
        let dy = i - r;
        if dy.abs() <= r {
            let dx =
                (c as f64 * (((r * r - dy * dy) as f64) * inv_r2).sqrt()).round_ties_even() as i64;
            let j1 = (c - dx).max(0);
            let j2 = (c + dx + 1).min(w as i64);
            for j in j1..j2 {
                out.push(((j - c) as isize, (i - r) as isize));
            }
        }
    }
    Kernel(out)
}

/// `np.ones((h, w))` as a kernel.
pub fn rect(w: usize, h: usize) -> Kernel {
    let (ax, ay) = ((w / 2) as isize, (h / 2) as isize);
    let mut out = Vec::new();
    for i in 0..h as isize {
        for j in 0..w as isize {
            out.push((j - ax, i - ay));
        }
    }
    Kernel(out)
}

fn morph(src: &Gray, k: &Kernel, dilate: bool) -> Gray {
    let (w, h) = (src.width as isize, src.height as isize);
    let mut out = Gray::new(src.width, src.height);
    for y in 0..h {
        for x in 0..w {
            let mut acc: u8 = if dilate { 0 } else { 255 };
            for &(dx, dy) in &k.0 {
                let (xx, yy) = (x + dx, y + dy);
                if xx < 0 || yy < 0 || xx >= w || yy >= h {
                    continue;
                }
                let v = src.data[(yy * w + xx) as usize];
                acc = if dilate { acc.max(v) } else { acc.min(v) };
            }
            out.data[(y * w + x) as usize] = acc;
        }
    }
    out
}

/// `cv2.dilate(src, kernel)`.
pub fn dilate(src: &Gray, k: &Kernel) -> Gray {
    morph(src, k, true)
}

/// `cv2.erode(src, kernel)`.
pub fn erode(src: &Gray, k: &Kernel) -> Gray {
    morph(src, k, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipse_kernels_match_opencv() {
        // cv2.getStructuringElement(MORPH_ELLIPSE, (1, 2)): a 1x2 column, anchor (0, 1)
        assert_eq!(ellipse(1, 2).0, vec![(0, -1), (0, 0)]);
        // (5, 3): rows [0,0,1,0,0], [1,1,1,1,1], [0,0,1,0,0]
        assert_eq!(ellipse(5, 3).0.len(), 7);
    }

    #[test]
    fn dilate_and_erode_a_dot() {
        let mut g = Gray::new(5, 5);
        g.set(2, 2, 1);
        let d = dilate(&g, &rect(3, 3));
        assert_eq!(d.data.iter().filter(|&&v| v == 1).count(), 9);
        assert_eq!(erode(&d, &rect(3, 3)), g);
    }
}
