//! Stage 3 (homr `noise_filtering`, `bounding_boxes`): remove noise from the
//! masks and turn blobs into rotated boxes and ellipses.
use crate::Predictions;

pub fn filter_predictions(pred: Predictions) -> Predictions {
    let _ = pred;
    todo!("port homr filter_predictions and make_lines_stronger")
}
