//! On-device text recognition for printed scores (LIB-03 metadata from the
//! page, lyrics text later) and a title-page reader.
//!
//! Recognition uses Apple Vision (`VNRecognizeTextRequest`), which runs on
//! the iPad and the Mac with no network and reads Swedish (`sv-SE`) since
//! iOS/macOS 26. Other platforms return [`OcrError::Unsupported`].

use serde::Serialize;

mod meta;
pub use meta::{guess_metadata, TitlePage};

/// One recognised line of text. The box is in page fractions (0..1) with
/// the origin at the top left, like the PDF view.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub confidence: f32,
}

#[derive(Debug)]
pub enum OcrError {
    /// No recogniser on this platform.
    Unsupported,
    /// The image could not be read or recognition failed.
    Failed(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::Unsupported => {
                write!(f, "text recognition is not available on this platform")
            }
            OcrError::Failed(e) => write!(f, "text recognition failed: {e}"),
        }
    }
}

impl std::error::Error for OcrError {}

/// Recognise the text lines in an encoded image (PNG/JPEG), in reading
/// order (top to bottom, then left to right). `languages` are BCP-47 codes
/// in priority order, e.g. `["sv-SE", "en-US"]`.
pub fn recognize(image: &[u8], languages: &[&str]) -> Result<Vec<Line>, OcrError> {
    let mut lines = imp::recognize(image, languages)?;
    lines.sort_by(|a, b| {
        // Same line when the vertical centres are within half a line height.
        let same = ((a.y + a.h / 2.0) - (b.y + b.h / 2.0)).abs() < a.h.min(b.h) / 2.0;
        if same {
            a.x.total_cmp(&b.x)
        } else {
            a.y.total_cmp(&b.y)
        }
    });
    Ok(lines)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod imp {
    use super::{Line, OcrError};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::AnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    pub fn recognize(image: &[u8], languages: &[&str]) -> Result<Vec<Line>, OcrError> {
        objc2::rc::autoreleasepool(|_| {
            let data = NSData::with_bytes(image);
            let opts: Retained<NSDictionary<VNImageOption, AnyObject>> = NSDictionary::new();
            let handler = VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &data,
                &opts,
            );
            let req = unsafe { VNRecognizeTextRequest::init(VNRecognizeTextRequest::alloc()) };
            req.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            // Language correction helps titles and credits; the score's
            // own syllables ("Vår-vin-dar") are not dictionary words anyway.
            req.setUsesLanguageCorrection(true);
            let langs: Vec<Retained<NSString>> =
                languages.iter().map(|l| NSString::from_str(l)).collect();
            req.setRecognitionLanguages(&NSArray::from_retained_slice(&langs));
            let base: Retained<VNRequest> = Retained::into_super(Retained::into_super(req.clone()));
            handler
                .performRequests_error(&NSArray::from_retained_slice(&[base]))
                .map_err(|e| OcrError::Failed(e.localizedDescription().to_string()))?;
            let mut out = Vec::new();
            for obs in req.results().map(|r| r.to_vec()).unwrap_or_default() {
                let Some(best) = obs.topCandidates(1).firstObject() else {
                    continue;
                };
                let text = best.string().to_string();
                if text.trim().is_empty() {
                    continue;
                }
                // Vision boxes are normalised with the origin at the bottom left.
                let b = unsafe { obs.boundingBox() };
                out.push(Line {
                    text: text.trim().to_string(),
                    x: b.origin.x as f32,
                    y: (1.0 - b.origin.y - b.size.height) as f32,
                    w: b.size.width as f32,
                    h: b.size.height as f32,
                    confidence: best.confidence(),
                });
            }
            Ok(out)
        })
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod imp {
    use super::{Line, OcrError};
    pub fn recognize(_image: &[u8], _languages: &[&str]) -> Result<Vec<Line>, OcrError> {
        Err(OcrError::Unsupported)
    }
}
