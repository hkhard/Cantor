//! LIB-06 (homr port, stage 6): the transformer reads each staff strip of
//! the Lacrymosa page to exactly the symbols homr 0.7.1's Python produced
//! (`fixtures/omr/lacrymosa-p1/tokens_<i>.json`, captured by
//! `tools/omr/homr_port/capture_stages.py` from the same `staff_<i>.png`).
//!
//! Needs ONNX Runtime and the models: run with
//! `cargo test -p cantor-homr --features download` after
//! `tools/omr/homr_port/fetch_models.sh`. Without the models it skips.
#![cfg(feature = "download")]

use std::path::{Path, PathBuf};
use std::time::Instant;

use cantor_homr::{transformer, Gray, Models};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_models() -> Option<Models> {
    let dir = root().join("models/homr");
    if !dir.join(cantor_homr::models::DECODER).exists() {
        eprintln!(
            "skipped: no models in {} (run fetch_models.sh)",
            dir.display()
        );
        return None;
    }
    let cache = std::env::temp_dir().join("cantor-homr-coreml");
    Some(Models::load(&dir, false, &cache).expect("load models"))
}

#[test]
fn lib_06_homr_transformer_reads_lacrymosa_staffs_like_python() {
    let Some(mut models) = load_models() else {
        return;
    };
    let fixtures = root().join("fixtures/omr/lacrymosa-p1");
    let mut total = 0.0;
    let mut tokens = 0;
    let mut n = 0;
    while fixtures.join(format!("staff_{n}.png")).exists() {
        let strip =
            Gray::from_png(&std::fs::read(fixtures.join(format!("staff_{n}.png"))).unwrap())
                .unwrap();
        let want: Vec<String> = serde_json::from_str(
            &std::fs::read_to_string(fixtures.join(format!("tokens_{n}.json"))).unwrap(),
        )
        .unwrap();
        let t = Instant::now();
        let got = transformer::read_staff(&mut models, &strip).unwrap();
        let s = t.elapsed().as_secs_f64();
        total += s;
        tokens += got.len();
        eprintln!("staff {n}: {} symbols in {s:.2} s", got.len());
        assert_eq!(got, want, "staff {n}");
        n += 1;
    }
    assert_eq!(n, 10, "expected 10 staff strips");
    eprintln!(
        "{n} staffs, {tokens} symbols: {total:.2} s, {:.2} s per staff (CPU)",
        total / n as f64
    );
}

#[test]
fn lib_06_homr_transformer_rejects_a_strip_of_the_wrong_size() {
    let Some(mut models) = load_models() else {
        return;
    };
    let err = transformer::read_staff(&mut models, &Gray::new(100, 50)).unwrap_err();
    assert!(err.to_string().contains("1280x256"), "{err}");
}
