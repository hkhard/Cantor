//! Check exported measure maps against the MusicXML they belong to.
//!
//! Usage: check_pdf_maps <pdf-dir> <musicxml-dir>
//! Prints `name  map  musicxml  ok|MISMATCH|no MusicXML` per
//! `<name>.measures.json` in `<pdf-dir>`; exits 1 on any mismatch.
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1).map(PathBuf::from);
    let (Some(maps), Some(xml)) = (args.next(), args.next()) else {
        eprintln!("usage: check_pdf_maps <pdf-dir> <musicxml-dir>");
        return std::process::ExitCode::from(2);
    };
    let mut names: Vec<String> = std::fs::read_dir(&maps)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let f = e.file_name().to_string_lossy().into_owned();
            f.strip_suffix(".measures.json").map(str::to_string)
        })
        .collect();
    names.sort();
    let mut bad = false;
    println!("{:<40} {:>5} {:>8}", "score", "map", "musicxml");
    for name in names {
        let json = std::fs::read(maps.join(format!("{name}.measures.json"))).unwrap();
        let map: Vec<serde_json::Value> = serde_json::from_slice(&json).unwrap_or_default();
        let score = ["mxl", "musicxml"]
            .iter()
            .map(|e| xml.join(format!("{name}.{e}")))
            .find(|p| p.exists())
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| cantor_score::parse_score_file(&b).ok());
        let (count, verdict) = match score {
            Some(s) => {
                let n = s.measure_count();
                bad |= n != map.len();
                (
                    n.to_string(),
                    if n == map.len() { "ok" } else { "MISMATCH" },
                )
            }
            None => ("-".into(), "no MusicXML"),
        };
        println!("{name:<40} {:>5} {count:>8} {verdict}", map.len());
    }
    if bad {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}
