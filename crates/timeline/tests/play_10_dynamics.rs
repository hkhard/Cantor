//! PLAY-10 (P1): dynamics and articulations shape playback. Dynamic marks
//! (pp..ff, and `<sound dynamics>`) set note velocity per staff until the
//! next mark; staccato plays half length, staccatissimo a quarter, accents
//! louder. pp < mf < ff in velocity.

use cantor_score::{parse_musicxml, PPQ};
use cantor_timeline::build_timemap;

fn score(body: &str) -> cantor_score::Score {
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
        <part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>8</beats><beat-type>4</beat-type></time></attributes>
        {body}</measure></part></score-partwise>"#
    );
    parse_musicxml(xml.as_bytes()).unwrap()
}

fn dyn_mark(m: &str) -> String {
    format!(
        r#"<direction><direction-type><dynamics><{m}/></dynamics></direction-type></direction>"#
    )
}

fn note(extra: &str) -> String {
    format!(
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>{extra}</note>"#
    )
}

#[test]
fn play_10_pp_lt_mf_lt_ff_velocity() {
    let body = [
        dyn_mark("pp"),
        note(""),
        dyn_mark("mf"),
        note(""),
        dyn_mark("ff"),
        note(""),
        note(""),
    ]
    .concat();
    let tm = build_timemap(&score(&body));
    let v: Vec<u8> = tm.notes.iter().map(|n| n.velocity).collect();
    assert!(v[0] < v[1] && v[1] < v[2], "{v:?}");
    assert_eq!(v[2], v[3], "a dynamic lasts until the next one");
}

#[test]
fn play_10_no_dynamics_plays_mf() {
    let tm = build_timemap(&score(&note("")));
    let mf = build_timemap(&score(&[dyn_mark("mf"), note("")].concat()));
    assert_eq!(tm.notes[0].velocity, mf.notes[0].velocity);
}

#[test]
fn play_10_sound_dynamics_attribute_wins() {
    // <sound dynamics="X">: X is percent of forte's 90 (MusicXML: 100 = f).
    let body = format!(
        r#"<direction><direction-type><dynamics><p/></dynamics></direction-type><sound dynamics="100"/></direction>{}"#,
        note("")
    );
    let tm = build_timemap(&score(&body));
    assert_eq!(tm.notes[0].velocity, 90);
}

#[test]
fn play_10_staccato_halves_length() {
    let body = [
        note("<notations><articulations><staccato/></articulations></notations>"),
        note("<notations><articulations><staccatissimo/></articulations></notations>"),
        note(""),
    ]
    .concat();
    let tm = build_timemap(&score(&body));
    let len: Vec<u64> = tm.notes.iter().map(|n| n.tick_off - n.tick_on).collect();
    assert_eq!(len, [PPQ / 2, PPQ / 4, PPQ]);
}

#[test]
fn play_10_accent_is_louder() {
    let body = [
        dyn_mark("mf"),
        note(""),
        note("<notations><articulations><accent/></articulations></notations>"),
    ]
    .concat();
    let tm = build_timemap(&score(&body));
    assert!(tm.notes[1].velocity > tm.notes[0].velocity);
}
