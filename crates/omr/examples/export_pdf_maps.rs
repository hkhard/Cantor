//! Export the original PDF and a measure map (JSON) next to each converted
//! score, for the PDF reader view.
//!
//! Usage: export_pdf_maps <out-dir> <omr-work-dir>...
//! where each work dir is a `scores/out/.<name>.omr` folder made by cantor-omr.
//! Prints, per score, the measure count of the map against the MusicXML.
//!
//! Books recognised from preprocessed images (`cleaned/`, `split/` made by
//! `tools/omr/split_spreads.py`) carry a `<name>.pages.json` crop sidecar;
//! their boxes are mapped back to the PDF pages through it.
use cantor_omr::measure_map::{
    export_movement, measure_map_movements, parse_crops, uncrop, MeasureBox,
};
use std::path::{Path, PathBuf};

/// Map boxes on preprocessed images back to the PDF when `dir` has a crop
/// sidecar. `None` if the sidecar does not cover the boxes.
fn to_pdf(dir: &Path, name: &str, mut boxes: Vec<MeasureBox>) -> Option<Vec<MeasureBox>> {
    match std::fs::read(dir.join(format!("{name}.pages.json"))) {
        Ok(json) => {
            let crops = parse_crops(&json).ok()?;
            uncrop(&mut boxes, &crops).ok()?;
            Some(boxes)
        }
        Err(_) => Some(boxes),
    }
}

/// Candidate maps in the converter's ladder order: whole book, cleaned book,
/// spread-split book, raw page books, cleaned page books; every movement of
/// a whole book is a candidate (movement 1 first). The caller picks the one
/// whose measure count matches the MusicXML that was kept.
fn candidates(work: &Path, name: &str) -> Vec<(Vec<MeasureBox>, String)> {
    let movements = |p: &Path| {
        std::fs::read(p)
            .ok()
            .and_then(|b| measure_map_movements(&b).ok())
    };
    let mut out = Vec::new();
    for (sub, label) in [("", "book"), ("cleaned", "cleaned"), ("split", "split")] {
        let dir = work.join(sub);
        let Some(mv) = movements(&dir.join(format!("{name}.omr"))) else {
            continue;
        };
        for (k, b) in mv.into_iter().enumerate() {
            if b.is_empty() {
                continue;
            }
            if let Some(b) = to_pdf(&dir, name, b) {
                let label = if k == 0 {
                    label.to_string()
                } else {
                    format!("{label} movement {}", k + 1)
                };
                out.push((b, label));
            }
        }
    }
    // Page books: the converter joined, in page order, the one export each
    // page run left behind; take that export's movement. Pages without an
    // export were skipped by the converter and have no measures.
    let pages = |root: &Path| -> Option<Vec<MeasureBox>> {
        let dir = root.join("pages");
        let mut out = Vec::new();
        let mut n = 1;
        while dir.join(format!("page{n}")).exists() {
            let page = dir.join(format!("page{n}"));
            let export = std::fs::read_dir(&page)
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|f| f.ends_with(".mxl"));
            if let (Some(export), Some(mv)) = (export, movements(&page.join(format!("{name}.omr"))))
            {
                if let Some(k) = export_movement(&export, mv.len()) {
                    let mut b = mv[k].clone();
                    for m in &mut b {
                        m.page = n; // page books hold one sheet each
                    }
                    out.extend(b);
                }
            }
            n += 1;
        }
        (!out.is_empty()).then_some(out)
    };
    if let Some(b) = pages(work) {
        out.push((b, "pages".into()));
    }
    let cleaned = work.join("cleaned");
    if let Some(b) = pages(&cleaned).and_then(|b| to_pdf(&cleaned, name, b)) {
        out.push((b, "cleaned pages".into()));
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: export_pdf_maps <out> <work>..."));
    std::fs::create_dir_all(&out).unwrap();
    for work in args.map(PathBuf::from) {
        let dir = work.file_name().unwrap().to_string_lossy().to_string();
        let name = dir
            .trim_start_matches('.')
            .trim_end_matches(".omr")
            .to_string();
        let pdf = work.join(format!("{name}.pdf"));
        let parent = work.parent().unwrap();
        let score = ["mxl", "musicxml"]
            .iter()
            .map(|e| parent.join(format!("{name}.{e}")))
            .find(|p| p.exists())
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| cantor_score::parse_score_file(&b).ok());
        let Some(measures) = score.as_ref().map(|s| s.measure_count()) else {
            println!("{name}: no MusicXML kept, no map written");
            continue;
        };
        let cands = candidates(&work, &name);
        let Some((boxes, source)) = cands
            .iter()
            .find(|(b, _)| b.len() == measures)
            .or(cands.first())
            .cloned()
        else {
            println!("{name}: no usable .omr book");
            continue;
        };
        std::fs::copy(&pdf, out.join(format!("{name}.pdf"))).unwrap();
        std::fs::write(
            out.join(format!("{name}.measures.json")),
            serde_json::to_string(&boxes).unwrap(),
        )
        .unwrap();
        let ok = if boxes.len() == measures {
            "ok"
        } else {
            "MISMATCH"
        };
        println!(
            "{name}: {} boxes from {source}, MusicXML {measures} measures: {ok}",
            boxes.len()
        );
    }
}
