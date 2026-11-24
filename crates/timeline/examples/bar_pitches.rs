//! Dev aid: sounding pitches of each written measure (all parts), one line
//! per measure, to match against another program's MIDI.
fn main() {
    let path = std::env::args().nth(1).expect("score file");
    let score = cantor_score::parse_score_file(&std::fs::read(path).unwrap()).unwrap();
    let n = score
        .parts
        .iter()
        .map(|p| p.measures.len())
        .max()
        .unwrap_or(0);
    for i in 0..n {
        let mut v: Vec<i32> = Vec::new();
        for p in &score.parts {
            if let Some(m) = p.measures.get(i) {
                v.extend(
                    m.events
                        .iter()
                        .filter_map(|e| e.kind.midi())
                        .map(|x| x + m.transpose),
                );
            }
        }
        v.sort();
        println!(
            "{}",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}
