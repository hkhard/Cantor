//! PLAY-10 (P1, playback style): swing.
//!
//! Product decision 2026-11-22: swing notation is followed. The encoding in
//! the file wins: `<sound><swing><first>5</first><second>3</second>
//! <swing-type>eighth</swing-type>` plays each pair of eighths 5:3. When the
//! score only says "Swing" (words, or `<swing>` without a ratio), the pair is
//! played triplet based, 2:1. `<straight/>` or the word "Straight" ends it.
//! `<swing-type>16th</swing-type>` swings sixteenths instead of eighths.

use cantor_score::{parse_musicxml, PPQ};
use cantor_timeline::build_timemap;

fn score(directions_m1: &str, body_m1: &str, body_m2: &str) -> cantor_score::Score {
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
        <part id="P1">
        <measure number="1"><attributes><divisions>4</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
          <direction><sound tempo="60"/></direction>{directions_m1}{body_m1}</measure>
        <measure number="2">{body_m2}</measure>
        </part></score-partwise>"#
    );
    parse_musicxml(xml.as_bytes()).unwrap()
}

fn n(step: &str, dur: u32, ty: &str) -> String {
    format!(
        r#"<note><pitch><step>{step}</step><octave>4</octave></pitch><duration>{dur}</duration><type>{ty}</type></note>"#
    )
}

fn eighths() -> String {
    ["C", "D", "E", "F"]
        .iter()
        .map(|s| n(s, 2, "eighth"))
        .collect()
}

fn sixteenths() -> String {
    ["C", "D", "E", "F", "G", "A", "B", "C"]
        .iter()
        .map(|s| n(s, 1, "16th"))
        .collect()
}

fn swing(first: u32, second: u32, ty: &str) -> String {
    format!(
        r#"<direction><direction-type><words>Swing</words></direction-type>
<sound><swing><first>{first}</first><second>{second}</second><swing-type>{ty}</swing-type></swing></sound></direction>"#
    )
}

const STRAIGHT: &str = r#"<direction><direction-type><words>Straight</words></direction-type>
<sound><swing><straight/></swing></sound></direction>"#;

fn ons(s: &cantor_score::Score) -> Vec<u64> {
    build_timemap(s).notes.iter().map(|n| n.tick_on).collect()
}

#[test]
fn play_10_encoded_ratio_is_kept() {
    // MuseScore writes 5:3 (its "60%" swing): first eighth 5/8 of the beat.
    let s = score(&swing(5, 3, "eighth"), &eighths(), "");
    let tm = build_timemap(&s);
    let on: Vec<u64> = tm.notes.iter().map(|n| n.tick_on).collect();
    let long = PPQ * 5 / 8;
    assert_eq!(on, [0, long, PPQ, PPQ + long]);
    assert_eq!(tm.notes[0].tick_off, long, "first of the pair is long");
    assert_eq!(tm.notes[1].tick_off, PPQ, "second ends on the beat");
    assert!((tm.notes[1].sec_on - 0.625).abs() < 1e-6, "60 bpm");
}

#[test]
fn play_10_encoded_triplet_ratio() {
    let s = score(&swing(2, 1, "eighth"), &eighths(), "");
    assert_eq!(ons(&s), [0, 2 * PPQ / 3, PPQ, PPQ + 2 * PPQ / 3]);
}

#[test]
fn play_10_swing_word_without_ratio_is_triplet_based() {
    let words = r#"<direction><direction-type><words>Swing</words></direction-type></direction>"#;
    let s = score(words, &eighths(), "");
    assert_eq!(ons(&s), [0, 2 * PPQ / 3, PPQ, PPQ + 2 * PPQ / 3]);
    let bare = r#"<direction><sound><swing/></sound></direction>"#;
    let s = score(bare, &eighths(), "");
    assert_eq!(ons(&s)[1], 2 * PPQ / 3);
}

#[test]
fn play_10_straight_ends_swing() {
    let s = score(
        &swing(2, 1, "eighth"),
        &eighths(),
        &format!("{STRAIGHT}{}", eighths()),
    );
    let m2 = 2 * PPQ;
    assert_eq!(ons(&s)[4..], [m2, m2 + PPQ / 2, m2 + PPQ, m2 + 3 * PPQ / 2]);
}

#[test]
fn play_10_no_swing_mark_plays_straight() {
    let s = score("", &eighths(), "");
    assert_eq!(ons(&s), [0, PPQ / 2, PPQ, 3 * PPQ / 2]);
}

#[test]
fn play_10_swing_carries_over_barlines() {
    let s = score(&swing(2, 1, "eighth"), &eighths(), &eighths());
    assert_eq!(ons(&s)[5], 2 * PPQ + 2 * PPQ / 3, "measure 2 still swings");
}

#[test]
fn play_10_beats_and_other_subdivisions_untouched() {
    // Quarters stay on the beat; a sixteenth inside a swung-eighth score
    // that starts on an eighth off-beat moves with the off-beat, others not.
    let body = format!("{}{}", n("C", 4, "quarter"), n("D", 4, "quarter"));
    let s = score(&swing(2, 1, "eighth"), &body, "");
    assert_eq!(ons(&s), [0, PPQ]);
}

#[test]
fn play_10_sixteenth_swing_type() {
    let s = score(&swing(2, 1, "16th"), &sixteenths(), "");
    let half = PPQ / 2;
    let long = half * 2 / 3;
    assert_eq!(
        ons(&s)[..4],
        [0, long, half, half + long],
        "pairs of sixteenths in each eighth"
    );
}
