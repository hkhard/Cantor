//! Prepare a part for SoulX-Singer: score -> phrases -> English phonemes ->
//! target metadata JSON, plus the render settings and a word report.
//!
//! Usage: sing_prepare <score.mxl|.musicxml> <part-index> <verse> <out.json> [voice]
//! `voice` is a MusicXML voice number; omitted = top line.
use cantor_vocal::english::EnglishPhonemizer;
use cantor_vocal::soulx::{to_json, to_target_metadata_lenient, Language, RenderSettings};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: sing_prepare <score> <part-index> <verse> <out.json> [voice]");
        std::process::exit(2);
    }
    let score = cantor_score::parse_score_file(&std::fs::read(&a[1]).expect("read score"))
        .expect("parse score");
    let part: usize = a[2].parse().expect("part index");
    let verse: u32 = a[3].parse().expect("verse");
    let voice: Option<u32> = a.get(5).map(|v| v.parse().expect("voice"));
    let tm = cantor_timeline::build_timemap(&score);
    let phrases = cantor_lyrics::extract(&score, &tm, part, voice, verse);
    let g2p = EnglishPhonemizer::bundled();
    let (segs, unknown) =
        to_target_metadata_lenient(&phrases, Language::English, &g2p).expect("English only");
    std::fs::write(&a[4], to_json(&segs)).expect("write json");
    let settings = RenderSettings::default();
    std::fs::write(
        format!("{}.settings.json", a[4].trim_end_matches(".json")),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .expect("write settings");
    let notes: usize = phrases.iter().map(|p| p.notes.len()).sum();
    println!(
        "{}: part {} '{}' verse {verse}: {} phrases, {notes} notes, {:.1} s; {} unknown words sung on 'la': {:?}",
        a[4],
        part,
        score.parts[part].name,
        segs.len(),
        phrases.last().map_or(0.0, |p| p.end_sec),
        unknown.len(),
        unknown
    );
}
