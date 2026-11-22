//! Print note onsets (beats) in a measure range of part 0, to check swing.
//! Usage: swing_probe <score> <first-measure-1based> <last>
use cantor_timeline::build_timemap;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let s = cantor_score::parse_score_file(&std::fs::read(&a[1]).unwrap()).unwrap();
    let (lo, hi): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let tm = build_timemap(&s);
    for (visit, &mi) in tm.measure_order.iter().enumerate() {
        if mi + 1 < lo || mi + 1 > hi {
            continue;
        }
        let start = tm.measure_starts[visit];
        let on: Vec<String> = tm
            .notes
            .iter()
            .filter(|n| n.part == 1 && n.measure == mi && n.tick_on >= start)
            .map(|n| {
                format!(
                    "{:.3}",
                    (n.tick_on - start) as f64 / cantor_score::PPQ as f64
                )
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        println!("m{} visit {visit}: beats {}", mi + 1, on.join(" "));
    }
}
