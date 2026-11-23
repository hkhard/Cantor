//! Stage 1 (homr `autocrop`, `resize`, `color_adjust`): crop the page to its
//! content, resize so staff lines have homr's expected size, and apply CLAHE.
use crate::Gray;

/// The image fed to the segmentation net, and how to map back to the page.
#[derive(Debug, Clone)]
pub struct Preprocessed {
    pub image: Gray,
    /// Top-left of the crop on the input page, in input pixels.
    pub crop_offset: (usize, usize),
    /// Input pixels per preprocessed pixel.
    pub scale: f64,
}

pub fn preprocess(page: &Gray) -> Preprocessed {
    let _ = page;
    todo!("port homr autocrop_with_offset, resize_image, apply_clahe")
}
