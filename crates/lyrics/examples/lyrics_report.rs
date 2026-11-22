//! Print the sung text of every part and verse of a score, phrase by phrase.
//! Usage: cargo run -p cantor-lyrics --example lyrics_report -- <score.mxl|.musicxml> [max-phrases]
use cantor_lyrics::{extract, NoteKind};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("usage: lyrics_report <score> [max-phrases]");
    let max: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3);
    let score = cantor_score::parse_score_file(&std::fs::read(path).expect("read")).expect("parse");
    let tm = cantor_timeline::build_timemap(&score);
    for (pi, part) in score.parts.iter().enumerate() {
        for verse in part.verses() {
            let phrases = extract(&score, &tm, pi, None, verse);
            let notes: usize = phrases.iter().map(|p| p.notes.len()).sum();
            let syl = phrases
                .iter()
                .flat_map(|p| &p.notes)
                .filter(|n| n.kind == NoteKind::Syllable)
                .count();
            let mel = phrases
                .iter()
                .flat_map(|p| &p.notes)
                .filter(|n| n.kind == NoteKind::Melisma)
                .count();
            let none = notes - syl - mel;
            println!(
                "{} [{}] verse {verse}: {} phrases, {notes} sung notes ({syl} syllables, {mel} melisma, {none} without lyric)",
                part.id, part.name, phrases.len()
            );
            for p in phrases.iter().take(max) {
                println!("    {:6.1}s  {}", p.start_sec, p.words().join(" "));
            }
        }
    }
}
