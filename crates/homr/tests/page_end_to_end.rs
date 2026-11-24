//! LIB-10 (homr port, all stages): `recognize_page` turns the Lacrymosa page
//! image into the MusicXML homr 0.7.1 wrote for it (`page.musicxml`, apart
//! from homr's `imgpos` comments), and our own parser reads the result.
//!
//! Needs ONNX Runtime and the models (`tools/omr/homr_port/fetch_models.sh`).
//! Without the models it skips. Run in release: debug takes minutes.
#![cfg(feature = "download")]

use std::path::{Path, PathBuf};
use std::time::Instant;

use cantor_homr::Models;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn without_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("-->").expect("closed comment") + start + 3;
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn lib_10_homr_page_to_musicxml_matches_python() {
    let dir = root().join("models/homr");
    if !dir.join(cantor_homr::models::DECODER).exists() {
        eprintln!(
            "skipped: no models in {} (run fetch_models.sh)",
            dir.display()
        );
        return;
    }
    let cache = std::env::temp_dir().join("cantor-homr-coreml");
    let mut models = Models::load(&dir, false, &cache).expect("load models");
    let fx = root().join("fixtures/omr/lacrymosa-p1");
    let png = std::fs::read(fx.join("page.png")).unwrap();
    let mut stages: Vec<(String, f32)> = Vec::new();
    let t = Instant::now();
    let ours = cantor_homr::recognize_page(&mut models, &png, &mut |s, f| {
        stages.push((s.to_string(), f))
    })
    .expect("recognises the page");
    eprintln!("whole page: {:.1} s", t.elapsed().as_secs_f32());

    // Progress only moves forward and ends at 1.
    assert!(stages.windows(2).all(|w| w[0].1 <= w[1].1), "{stages:?}");
    assert_eq!(stages.last().map(|s| s.1), Some(1.0));

    let python = without_comments(&std::fs::read_to_string(fx.join("page.musicxml")).unwrap());
    let ours = without_comments(&ours);
    if ours != python {
        let at = ours
            .bytes()
            .zip(python.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(ours.len().min(python.len()));
        let from = at.saturating_sub(300);
        panic!(
            "differs at byte {at} (ours {} bytes, homr {} bytes)\n ours: {}\n homr: {}",
            ours.len(),
            python.len(),
            &ours[from..(at + 300).min(ours.len())],
            &python[from..(at + 300).min(python.len())],
        );
    }
    let score = cantor_score::parse_score_file(ours.as_bytes()).expect("parses");
    assert_eq!(score.parts.len(), 5);
}
