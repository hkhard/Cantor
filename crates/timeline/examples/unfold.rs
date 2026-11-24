//! Dev aid: the playback order of a score's measures (1-based written
//! numbers), to check repeats, voltas, D.S. and coda.
//!   cargo run -q -p cantor-timeline --example unfold -- score.mxl
fn main() {
    let path = std::env::args().nth(1).expect("score file");
    let score = cantor_score::parse_score_file(&std::fs::read(path).unwrap()).unwrap();
    let order = cantor_timeline::unfold(&score);
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for i in order {
        match runs.last_mut() {
            Some((_, e)) if *e + 1 == i => *e = i,
            _ => runs.push((i, i)),
        }
    }
    let s: Vec<String> = runs
        .iter()
        .map(|&(a, b)| {
            if a == b {
                format!("{}", a + 1)
            } else {
                format!("{}-{}", a + 1, b + 1)
            }
        })
        .collect();
    println!("{}", s.join(" "));
}
