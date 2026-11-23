//! On-device note recognition: a Rust port of homr 0.7.1
//! (<https://github.com/liebharc/homr>, AGPL-3.0, Copyright the homr
//! contributors). See README.md for the licence terms of this crate (AGPL
//! only, without the app store permission) and the port status per stage.
//!
//! Pipeline, one module per stage, each tested against homr's Python output
//! on `fixtures/omr/lacrymosa-p1/`:
//!
//! 1. [`preprocess`]: autocrop, resize, CLAHE (`autocrop`, `resize`, `color_adjust`)
//! 2. [`segnet`]: the segmentation net, five masks (`segmentation/inference_segnet`)
//! 3. [`boxes`]: mask filtering and bounding boxes (`noise_filtering`, `bounding_boxes`)
//! 4. [`staffs`]: staffs, notes on staffs, braces, systems (`staff_detection`, ...)
//! 5. [`staff_images`]: dewarped staff strips for the transformer (`staff_dewarping`, `staff_parsing`)
//! 6. [`transformer`]: encoder + greedy decoder to symbols (`transformer/*`)
//! 7. [`musicxml`]: MusicXML from the symbols (`music_xml_generator`)
//!
//! [`recognize_page`] runs them all; [`Models`] holds the ONNX sessions.

pub mod boxes;
pub mod image_ops;
pub mod models;
pub mod musicxml;
pub mod preprocess;
pub mod segnet;
pub mod staff_images;
pub mod staffs;
pub mod transformer;

pub use models::Models;

/// A grey image, row-major, one byte per pixel.
#[derive(Clone, PartialEq)]
pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Gray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gray({}x{})", self.width, self.height)
    }
}

impl Gray {
    pub fn new(width: usize, height: usize) -> Self {
        Gray {
            width,
            height,
            data: vec![0; width * height],
        }
    }
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }
    pub fn set(&mut self, x: usize, y: usize, v: u8) {
        self.data[y * self.width + x] = v;
    }
    /// Decode a PNG (any colour type) to grey, like `cv2.imread(..., GRAYSCALE)`.
    pub fn from_png(bytes: &[u8]) -> Result<Self, Error> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .map_err(|e| Error::Image(e.to_string()))?
            .to_luma8();
        Ok(Gray {
            width: img.width() as usize,
            height: img.height() as usize,
            data: img.into_raw(),
        })
    }
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        image::GrayImage::from_raw(self.width as u32, self.height as u32, self.data.clone())
            .expect("size matches")
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .expect("png encode");
        out
    }
}

/// The five segmentation masks, at the preprocessed image's size (0 or 1).
#[derive(Debug, Clone)]
pub struct Predictions {
    pub notehead: Gray,
    pub symbols: Gray,
    pub staff: Gray,
    pub clefs_keys: Gray,
    pub stems_rest: Gray,
}

/// One point of a staff's grid: x and the y of its five lines (homr `StaffPoint`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StaffPoint {
    pub x: f64,
    pub y: [f64; 5],
    pub angle: f64,
}

/// A detected staff (homr `Staff`, the parts later stages need).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Staff {
    pub grandstaff: bool,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub grid: Vec<StaffPoint>,
}

/// A system: staffs joined by a brace or bracket (homr `MultiStaff`).
#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub staffs: Vec<Staff>,
}

/// One decoded symbol, in homr's text form, e.g. `"note_4 C4 _ _ _ upper"`.
pub type Symbol = String;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("image: {0}")]
    Image(String),
    #[error("model: {0}")]
    Model(String),
    #[error("no staffs found on the page")]
    NoStaffs,
    #[error("{0}")]
    Other(String),
}

/// Progress of a page: stage name and fraction done (0..1), for the app's
/// progress bar. Called from the worker thread.
pub type Progress<'a> = &'a mut dyn FnMut(&str, f32);

/// Recognise one page image (PNG bytes, any size) to MusicXML.
pub fn recognize_page(
    models: &mut Models,
    png: &[u8],
    progress: Progress,
) -> Result<String, Error> {
    progress("preprocess", 0.0);
    let page = Gray::from_png(png)?;
    let pre = preprocess::preprocess(&page);
    progress("segmentation", 0.1);
    let pred = segnet::segment(models, &pre.image)?;
    progress("staffs", 0.4);
    let pred = boxes::filter_predictions(pred);
    let systems = staffs::detect(&pred, &pre.image)?;
    progress("symbols", 0.5);
    let strips = staff_images::cut(&pre.image, &systems);
    let mut tokens = Vec::with_capacity(strips.len());
    for (i, s) in strips.iter().enumerate() {
        progress(
            "symbols",
            0.5 + 0.45 * i as f32 / strips.len().max(1) as f32,
        );
        tokens.push(transformer::read_staff(models, s)?);
    }
    progress("musicxml", 0.95);
    let xml = musicxml::generate(&systems, &tokens);
    progress("done", 1.0);
    Ok(xml)
}
