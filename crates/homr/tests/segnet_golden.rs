//! LIB-06 (homr port, stage 2): segnet on the preprocessed Lacrymosa page
//! gives the same merged class map as homr 0.7.1's Python on the ONNX
//! Runtime CPU provider (`fixtures/omr/lacrymosa-p1/pred_raw_classes.png`,
//! before `filter_predictions`; captured by
//! `tools/omr/homr_port/capture_stages.py`, which also notes in
//! `segnet_raw.json` how far homr's CoreML default differs from it).
//!
//! Needs ONNX Runtime and the models (`tools/omr/homr_port/fetch_models.sh`);
//! without the models it skips.
use std::path::{Path, PathBuf};
use std::time::Instant;

use cantor_homr::{segnet, Gray, Models};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gray(name: &str) -> Gray {
    let p = root().join("fixtures/omr/lacrymosa-p1").join(name);
    Gray::from_png(&std::fs::read(p).unwrap()).unwrap()
}

#[test]
fn lib_06_homr_segnet_masks_match_python() {
    let dir = root().join("models/homr");
    if !dir.join(cantor_homr::models::SEGNET).exists() {
        eprintln!(
            "skipped: no models in {} (run fetch_models.sh)",
            dir.display()
        );
        return;
    }
    let cache = std::env::temp_dir().join("cantor-homr-coreml");
    let t = Instant::now();
    let mut models = Models::load(&dir, false, &cache).expect("load models");
    let load_s = t.elapsed().as_secs_f64();
    let pre = gray("preprocessed.png");
    let want = gray("pred_raw_classes.png");

    let t = Instant::now();
    let got = segnet::classify(&mut models, &pre).unwrap();
    let run_s = t.elapsed().as_secs_f64();
    assert_eq!((got.width, got.height), (want.width, want.height));

    let n = want.data.len();
    let differ = (0..n).filter(|&i| got.data[i] != want.data[i]).count();
    // Per mask, for the report.
    let names = [
        "background",
        "stems_rest",
        "notehead",
        "clefs_keys",
        "staff",
        "symbols",
    ];
    let mut per = String::new();
    for (c, name) in names.iter().enumerate().skip(1) {
        let c = c as u8;
        let (g, w) = (
            got.data.iter().filter(|&&v| v == c).count(),
            want.data.iter().filter(|&&v| v == c).count(),
        );
        let d = (0..n)
            .filter(|&i| (got.data[i] == c) != (want.data[i] == c))
            .count();
        per += &format!(" {name} {g}/{w} px ({d} differ);");
    }
    eprintln!(
        "segnet (CPU) load {load_s:.2} s, run {run_s:.2} s, {} tiles: {differ} of {n} px differ;{per}",
        segnet::tile_origins(pre.width, pre.height).len()
    );
    // Same model, same provider, same fp16 input: the same argmax, exactly.
    assert_eq!(differ, 0, "{differ} px differ");

    let p = segnet::segment(&mut models, &pre).unwrap();
    for (mask, c) in [
        (&p.stems_rest, 1u8),
        (&p.notehead, 2),
        (&p.clefs_keys, 3),
        (&p.staff, 4),
        (&p.symbols, 5),
    ] {
        assert!(mask
            .data
            .iter()
            .zip(&got.data)
            .all(|(&m, &g)| m == (g == c) as u8));
    }
}
