//! Print a summary of a MusicXML/MXL file: parts, measures, notes, tempo.
//! Usage: cargo run -p cantor-score --example inspect -- <file.mxl|file.musicxml>
use cantor_score::{parse_score_file, EventKind};

fn main() {
    let path = std::env::args().nth(1).expect("usage: inspect <file>");
    let bytes = std::fs::read(&path).expect("read file");
    let s = match parse_score_file(&bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };
    println!("{path}: {} parts", s.parts.len());
    for p in &s.parts {
        let notes = p
            .measures
            .iter()
            .flat_map(|m| &m.events)
            .filter(|e| matches!(e.kind, EventKind::Note(_)))
            .count();
        let voices: std::collections::BTreeSet<_> = p
            .measures
            .iter()
            .flat_map(|m| &m.events)
            .map(|e| (e.staff, e.voice))
            .collect();
        let pitches: Vec<i32> = p
            .measures
            .iter()
            .flat_map(|m| &m.events)
            .filter_map(|e| e.kind.midi())
            .collect();
        let tempos: Vec<_> = p
            .measures
            .iter()
            .flat_map(|m| &m.tempos)
            .map(|t| t.1)
            .collect();
        println!(
            "  {:4} {:24} measures={:3} notes={:4} staff/voices={:?} range={:?}..{:?} tempos={:?} time={:?} program={:?}",
            p.id, p.name, p.measures.len(), notes, voices, pitches.iter().min(), pitches.iter().max(), tempos,
            p.measures.first().and_then(|m| m.time_signature), p.midi_program
        );
    }
}
