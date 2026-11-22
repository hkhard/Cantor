//! Per part: note count and median pitch per (staff, voice).
use cantor_score::parse_score_file;
use std::collections::BTreeMap;
fn main() {
    for path in std::env::args().skip(1) {
        let s = parse_score_file(&std::fs::read(&path).unwrap()).unwrap();
        println!("{path}");
        for p in &s.parts {
            let mut m: BTreeMap<(u32, u32), Vec<i32>> = BTreeMap::new();
            for e in p.measures.iter().flat_map(|m| &m.events) {
                if let Some(x) = e.kind.midi() {
                    m.entry((e.staff, e.voice)).or_default().push(x);
                }
            }
            let parts: Vec<String> = m
                .iter_mut()
                .map(|(k, v)| {
                    v.sort();
                    format!("{}/{}:{}@{}", k.0, k.1, v.len(), v[v.len() / 2])
                })
                .collect();
            println!(
                "  {:12} prog={:?} {}",
                p.name,
                p.midi_program,
                parts.join(" ")
            );
        }
    }
}
