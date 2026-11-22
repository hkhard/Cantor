//! PLAY-08 (core side): the follow-along cursor highlights the measure of the
//! sounding note. The app finds the playing measure from the timemap's
//! measure start times; this checks that, for every note, the measure found
//! at its onset time is the written measure the note comes from, also with
//! repeats, a volta, a pickup bar, a meter change and tempo changes, and
//! when looking it up in seconds (as the cursor does) rather than ticks.

use cantor_score::parse_musicxml;
use cantor_timeline::{build_timemap, Timemap};

const N: &str =
    r#"<note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration></note>"#;

fn xml() -> String {
    // Written measures (divisions = 2: quarter = 2, eighth = 1):
    // 0 pickup of 1 beat in 3/4, tempo 72
    // 1 3/4
    // 2 |: 3 beats :| played twice
    // 3 |: meter 2/4, tempo 133
    // 4 volta 1 (back to 3), 5 volta 2
    // 6 4/4, eighth notes, tempo 91
    let eighth =
        r#"<note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration></note>"#;
    let ms = [
        format!(
            r#"<measure number="0" implicit="yes"><attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="72"/></direction>{}</measure>"#,
            N.replace("<duration>1", "<duration>2")
        ),
        format!(
            r#"<measure number="1">{}</measure>"#,
            N.replace("<duration>1", "<duration>2").repeat(3)
        ),
        format!(
            r#"<measure number="2"><barline location="left"><repeat direction="forward"/></barline>{}<barline location="right"><repeat direction="backward"/></barline></measure>"#,
            N.replace("<duration>1", "<duration>2").repeat(3)
        ),
        format!(
            r#"<measure number="3"><attributes><time><beats>2</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="133"/></direction><barline location="left"><repeat direction="forward"/></barline>{}</measure>"#,
            N.replace("<duration>1", "<duration>2").repeat(2)
        ),
        format!(
            r#"<measure number="4"><barline location="left"><ending number="1" type="start"/></barline>{}<barline location="right"><ending number="1" type="stop"/><repeat direction="backward"/></barline></measure>"#,
            N.replace("<duration>1", "<duration>2").repeat(2)
        ),
        format!(
            r#"<measure number="5"><barline location="left"><ending number="2" type="start"/></barline>{}<barline location="right"><ending number="2" type="discontinue"/></barline></measure>"#,
            N.replace("<duration>1", "<duration>2").repeat(2)
        ),
        format!(
            r#"<measure number="6"><attributes><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="91"/></direction>{}</measure>"#,
            eighth.repeat(8)
        ),
    ];
    format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>S</part-name></score-part><score-part id="P2"><part-name>A</part-name></score-part></part-list>
        <part id="P1">{m}</part><part id="P2">{m}</part></score-partwise>"#,
        m = ms.concat()
    )
}

/// The cursor's lookup: index of the played measure sounding at `sec`.
fn visit_at(tm: &Timemap, sec: f64) -> usize {
    let starts: Vec<f64> = tm
        .measure_starts
        .iter()
        .map(|&t| tm.seconds_at(t))
        .collect();
    starts.partition_point(|&s| s <= sec + 1e-9) - 1
}

#[test]
fn play_08_every_note_onset_maps_to_the_measure_containing_it() {
    let s = parse_musicxml(xml().as_bytes()).unwrap();
    let tm = build_timemap(&s);
    // Unfolded: 0 1 2 2 3 4 3 5 6
    assert_eq!(tm.measure_order, [0, 1, 2, 2, 3, 4, 3, 5, 6]);
    assert_eq!(tm.measure_starts.len(), tm.measure_order.len());
    let per_measure = [1, 3, 3, 3, 2, 2, 2, 2, 8];
    assert_eq!(tm.notes.len(), 2 * per_measure.iter().sum::<usize>());
    let mut seen = vec![0; tm.measure_order.len()];
    for n in &tm.notes {
        let k = visit_at(&tm, n.sec_on);
        assert_eq!(
            tm.measure_order[k], n.measure,
            "note at {:.4} s (tick {}) highlighted in played measure {k}",
            n.sec_on, n.tick_on
        );
        // The note ends inside the same measure (no early hand-over).
        let end = tm
            .measure_starts
            .get(k + 1)
            .copied()
            .unwrap_or(tm.duration_ticks);
        assert!(n.tick_on >= tm.measure_starts[k] && n.tick_off <= end);
        seen[k] += 1;
    }
    let want: Vec<usize> = per_measure.iter().map(|c| c * 2).collect();
    assert_eq!(seen, want, "notes per played measure");
}

#[test]
fn play_08_cursor_moves_on_at_the_exact_barline_time() {
    let s = parse_musicxml(xml().as_bytes()).unwrap();
    let tm = build_timemap(&s);
    for (k, &t) in tm.measure_starts.iter().enumerate() {
        let sec = tm.seconds_at(t);
        assert_eq!(visit_at(&tm, sec), k, "on the barline");
        if k > 0 {
            assert_eq!(visit_at(&tm, sec - 1e-4), k - 1, "just before");
        }
    }
    // Measure start times strictly increase, so the lookup is well defined.
    assert!(tm.measure_starts.windows(2).all(|w| w[0] < w[1]));
}
