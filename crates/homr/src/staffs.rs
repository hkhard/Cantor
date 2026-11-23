//! Stage 4 (homr `staff_detection`, `note_detection`, `bar_line_detection`,
//! `brace_dot_detection`, `system_repair`): find staffs and group them into
//! systems.
use crate::{Error, Gray, Predictions, System};

pub fn detect(pred: &Predictions, image: &Gray) -> Result<Vec<System>, Error> {
    let _ = (pred, image);
    todo!("port homr detect_staffs_in_image (after predictions) and repair_systems")
}
