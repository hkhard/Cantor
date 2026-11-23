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
use cantor_omr::measure_map::map_for_work;
use std::path::PathBuf;

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
        let Some((boxes, source)) = map_for_work(&work, &name, measures) else {
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
