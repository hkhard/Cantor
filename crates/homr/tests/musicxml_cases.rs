//! LIB-06 (homr port, stage 7): on synthetic token cases which use what
//! the Lacrymosa page does not (marked and unmarked triplets, repeats,
//! voltas, dynamics, grace notes, ties, two voices on a staff, a clef change
//! in a chord, multi-measure and measure rests, no time signature), the
//! clean-up and MusicXML writer give byte for byte what homr 0.7.1 gives.
//! The references come from `tools/omr/homr_port/musicxml_cases.py`.
use std::path::Path;

use cantor_homr::musicxml::{clean_up_voice, generate_from_parts, Sym};

#[derive(serde::Deserialize)]
struct Case {
    voices: Vec<Vec<String>>,
    musicxml: String,
}

#[test]
fn lib_06_homr_musicxml_cases_match_python() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/omr/homr_musicxml_cases.json");
    let cases: std::collections::BTreeMap<String, Case> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(cases.len() >= 6);
    let mut failed = Vec::new();
    for (name, case) in &cases {
        let parts: Vec<Vec<Sym>> = case
            .voices
            .iter()
            .map(|v| clean_up_voice(&v.iter().map(|t| Sym::parse(t)).collect::<Vec<_>>()))
            .collect();
        let ours = generate_from_parts(&parts);
        if ours != case.musicxml {
            let at = ours
                .bytes()
                .zip(case.musicxml.bytes())
                .position(|(a, b)| a != b)
                .unwrap_or(ours.len().min(case.musicxml.len()));
            let from = at.saturating_sub(200);
            eprintln!(
                "{name}: differs at byte {at}\n ours: {}\n homr: {}",
                &ours[from..(at + 200).min(ours.len())],
                &case.musicxml[from..(at + 200).min(case.musicxml.len())]
            );
            failed.push(name.clone());
        }
        cantor_score::parse_score_file(ours.as_bytes())
            .unwrap_or_else(|e| panic!("{name}: our parser rejects it: {e}"));
    }
    assert!(failed.is_empty(), "differ from homr: {failed:?}");
}
