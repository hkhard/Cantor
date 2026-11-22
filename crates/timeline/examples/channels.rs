//! Print the mixer channel plan (role, timbre, pan) for scores.
use cantor_timeline::{
    build_timemap,
    channels::{choir_from_title, plan_channels_for},
};
fn main() {
    for path in std::env::args().skip(1) {
        let s = cantor_score::parse_score_file(&std::fs::read(&path).unwrap()).unwrap();
        let tm = build_timemap(&s);
        let title = std::path::Path::new(&path)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let pl = plan_channels_for(&s, &tm, choir_from_title(&title));
        println!("{path}");
        for c in &pl.channels {
            println!(
                "  {:10} prog {:3} pan {:+.2} part {} median {:.0} notes {}",
                c.label, c.program, c.pan, c.part, c.median_pitch, c.note_count
            );
        }
    }
}
