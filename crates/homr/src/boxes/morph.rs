//! Thin wrappers over [`crate::image_ops`] morphology for stages 3 and 4,
//! named after the homr calls they replace.
use crate::image_ops::{self, Kernel, Shape};
use crate::Gray;

/// `cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (w, h))`.
pub fn ellipse(w: usize, h: usize) -> Kernel {
    image_ops::structuring_element(Shape::Ellipse, w, h)
}

/// `np.ones((h, w))` as a kernel.
pub fn rect(w: usize, h: usize) -> Kernel {
    Kernel::ones(w, h)
}

/// `cv2.dilate(src, kernel)`.
pub fn dilate(src: &Gray, k: &Kernel) -> Gray {
    image_ops::dilate(src, k, 1)
}

/// `cv2.erode(src, kernel)`.
pub fn erode(src: &Gray, k: &Kernel) -> Gray {
    image_ops::erode(src, k, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dilate_and_erode_a_dot() {
        let mut g = Gray::new(5, 5);
        g.set(2, 2, 1);
        let d = dilate(&g, &rect(3, 3));
        assert_eq!(d.data.iter().filter(|&&v| v == 1).count(), 9);
        assert_eq!(erode(&d, &rect(3, 3)), g);
        // homr make_lines_stronger kernel (1, 2): a column of two, anchor at the bottom
        assert_eq!(ellipse(1, 2).data, vec![true, true]);
    }
}
