//! Join per-page OMR results (MXL/MusicXML, in argument order) into one
//! MusicXML file and a MIDI file. Useful to re-export after writer fixes
//! without re-running OMR.
//! Usage: join_pages <out-stem> <page1.mxl> <page2.mxl> ...
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (stem, pages) = args
        .split_first()
        .expect("usage: join_pages <out-stem> <pages...>");
    let mut joined: Option<cantor_score::Score> = None;
    for p in pages {
        let s = cantor_score::parse_score_file(&std::fs::read(p).expect("read page"))
            .expect("parse page");
        match &mut joined {
            Some(j) => j.append(s),
            None => joined = Some(s),
        }
    }
    let score = joined.expect("at least one page");
    let tm = cantor_timeline::build_timemap(&score);
    std::fs::write(
        format!("{stem}.musicxml"),
        cantor_score::write_musicxml(&score),
    )
    .unwrap();
    std::fs::write(
        format!("{stem}.mid"),
        cantor_timeline::smf::to_smf(&score, &tm),
    )
    .unwrap();
    println!(
        "{stem}: {} measures, {} notes",
        tm.measure_order.len(),
        tm.notes.len()
    );
}
