//! LIB-06 (homr port, stage 7): from the symbols homr 0.7.1 decoded for
//! each strip of the Lacrymosa page (`tokens_<i>.json`) and the page's staff
//! layout (`staffs.json`), the Rust port writes the MusicXML homr wrote
//! (`page.musicxml`, captured by `tools/omr/homr_port/capture_stages.py`).
//!
//! homr adds `<!-- imgpos: x, y -->` comments from the transformer's
//! attention, which stages 5 and 6 provide. They are stripped from the
//! reference; everything else must match byte for byte.
use std::path::{Path, PathBuf};

use cantor_homr::{musicxml, Staff, System};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/omr/lacrymosa-p1")
}

/// Stage 7 needs only the grand-staff flags (a grand staff's grid has ten
/// line ys, which `StaffPoint` does not hold, so the grid is not read).
#[derive(serde::Deserialize)]
struct Layout {
    multi: Vec<usize>,
    staffs: Vec<Flags>,
}

#[derive(serde::Deserialize)]
struct Flags {
    grandstaff: bool,
}

fn systems() -> Vec<System> {
    let layout: Layout =
        serde_json::from_slice(&std::fs::read(fixtures().join("staffs.json")).unwrap()).unwrap();
    let mut staffs = layout.staffs.into_iter();
    layout
        .multi
        .iter()
        .map(|&n| System {
            staffs: staffs
                .by_ref()
                .take(n)
                .map(|f| Staff {
                    grandstaff: f.grandstaff,
                    min_x: 0.0,
                    max_x: 0.0,
                    min_y: 0.0,
                    max_y: 0.0,
                    grid: vec![],
                })
                .collect(),
        })
        .collect()
}

fn tokens() -> Vec<Vec<String>> {
    (0..10)
        .map(|i| {
            let path = fixtures().join(format!("tokens_{i}.json"));
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        })
        .collect()
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
fn lib_06_homr_musicxml_matches_python() {
    let systems = systems();
    // homr reads voice by voice: voice k of every system, then voice k+1.
    let order = musicxml::strip_order(&systems);
    assert_eq!(order.len(), 10);
    assert_eq!(&order[..4], &[(0, 0), (1, 0), (0, 1), (1, 1)]);
    let ours = musicxml::generate(&systems, &tokens());
    let python = std::fs::read_to_string(fixtures().join("page.musicxml")).unwrap();
    let python = without_comments(&python);
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
}

#[test]
fn lib_06_homr_musicxml_reads_in_our_parser() {
    let xml = musicxml::generate(&systems(), &tokens());
    let score = cantor_score::parse_score_file(xml.as_bytes()).expect("parses");
    assert_eq!(score.parts.len(), 5);
}
