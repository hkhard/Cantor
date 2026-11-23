//! Stage 5 (homr `staff_dewarping`, `staff_parsing`): cut a straightened
//! strip per staff (grand staffs: both) as the transformer's input.
use crate::{Gray, System};

/// One strip per staff (or grand staff), in reading order.
pub fn cut(image: &Gray, systems: &[System]) -> Vec<Gray> {
    let _ = (image, systems);
    todo!("port homr staff_parsing (prepare_staff_image, dewarp)")
}
