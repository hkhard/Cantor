//! Export the original PDF and a measure map (JSON) next to each converted
//! score, for the PDF reader view.
//!
//! Usage: export_pdf_maps <out-dir> <omr-work-dir>...
//! where each work dir is a `scores/out/.<name>.omr` folder made by cantor-omr.
//! Prints, per score, the measure count of the map against the MusicXML.
use cantor_omr::measure_map::{measure_map, MeasureBox};
use std::path::{Path, PathBuf};

/// Candidate maps in the converter's ladder order: whole book, cleaned book,
/// raw page books, cleaned page books. The caller picks the one whose
/// measure count matches the MusicXML that was kept.
fn candidates(work: &Path, name: &str) -> Vec<(Vec<MeasureBox>, &'static str)> {
    let whole = work.join(format!("{name}.omr"));
    let cleaned = work.join("cleaned").join(format!("{name}.omr"));
    let try_book = |p: &Path| std::fs::read(p).ok().and_then(|b| measure_map(&b).ok());
    let pages = |dir: PathBuf| -> Option<Vec<MeasureBox>> {
        let mut out = Vec::new();
        let mut n = 1;
        loop {
            let p = dir.join(format!("page{n}")).join(format!("{name}.omr"));
            if !p.exists() {
                break;
            }
            if let Some(mut b) = try_book(&p) {
                for m in &mut b {
                    m.page = n; // page books hold one sheet each
                }
                out.extend(b);
            }
            n += 1;
        }
        (!out.is_empty()).then_some(out)
    };
    let mut out = Vec::new();
    if let Some(b) = try_book(&whole).filter(|b| !b.is_empty()) {
        out.push((b, "book"));
    }
    if let Some(b) = try_book(&cleaned).filter(|b| !b.is_empty()) {
        out.push((b, "cleaned"));
    }
    if let Some(b) = pages(work.join("pages")) {
        out.push((b, "pages"));
    }
    if let Some(b) = pages(work.join("cleaned").join("pages")) {
        out.push((b, "cleaned pages"));
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
        let measures = score.as_ref().map_or(0, |s| s.measure_count());
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
