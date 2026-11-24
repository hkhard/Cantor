//! LIB-06 (homr port, stage 1): preprocessing the public-domain Lacrymosa
//! page (autocrop, Pillow bicubic resize to 1920 px wide, CLAHE) gives the
//! same image as homr 0.7.1's Python (`fixtures/omr/lacrymosa-p1/
//! preprocessed.png`, captured by `tools/omr/homr_port/capture_stages.py`).
//! No models needed.
use std::path::{Path, PathBuf};
use std::time::Instant;

use cantor_homr::{preprocess, Gray};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/omr/lacrymosa-p1")
}

#[test]
fn lib_06_homr_preprocess_matches_python() {
    let png = std::fs::read(fixtures().join("page.png")).unwrap();
    let want =
        Gray::from_png(&std::fs::read(fixtures().join("preprocessed.png")).unwrap()).unwrap();
    let t = Instant::now();
    let pre = preprocess::preprocess_png(&png).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(
        (pre.image.width, pre.image.height),
        (want.width, want.height)
    );
    let n = want.data.len();
    let (mut max, mut sum, mut differ) = (0u8, 0u64, 0usize);
    for (a, b) in pre.image.data.iter().zip(&want.data) {
        let d = a.abs_diff(*b);
        max = max.max(d);
        sum += d as u64;
        differ += (d > 0) as usize;
    }
    eprintln!(
        "preprocess {ms:.0} ms: max diff {max}, mean {:.5}, {differ} of {n} px differ",
        sum as f64 / n as f64
    );
    // Identical (was 1 grey level on 0.39% px before the CLAHE padding fix).
    assert_eq!(differ, 0, "{differ} px differ, max {max}");
    assert_eq!(pre.crop_offset, (0, 0));
    let (sx, sy) = pre.scale;
    assert!((sx - 2550.0 / 1920.0).abs() < 1e-12 && (sy - 3600.0 / 2711.0).abs() < 1e-12);
}
