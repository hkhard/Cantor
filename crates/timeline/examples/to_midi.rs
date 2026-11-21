//! Convert MusicXML/MXL to a Standard MIDI File.
//! Usage: cargo run -p cantor-timeline --example to_midi -- <in.mxl|in.musicxml> <out.mid>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, input, output] = a.as_slice() else {
        eprintln!("usage: to_midi <in> <out.mid>");
        std::process::exit(2)
    };
    let score = cantor_score::parse_score_file(&std::fs::read(input).expect("read input"))
        .unwrap_or_else(|e| {
            eprintln!("{input}: {e}");
            std::process::exit(1)
        });
    let tm = cantor_timeline::build_timemap(&score);
    std::fs::write(output, cantor_timeline::smf::to_smf(&score, &tm)).expect("write output");
    println!(
        "{output}: {} notes, {:.1} s",
        tm.notes.len(),
        tm.duration_sec
    );
}
