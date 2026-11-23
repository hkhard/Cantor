//! Stage 2 (homr `segmentation/inference_segnet`): run the segmentation net
//! over 320x320 tiles (step 320) and stitch the five class masks.
use crate::{Error, Gray, Models, Predictions};

pub fn segment(models: &mut Models, image: &Gray) -> Result<Predictions, Error> {
    let _ = (models, image);
    todo!("port homr extract()")
}
