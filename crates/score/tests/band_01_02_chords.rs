//! BAND-01 chord symbols in the model; BAND-02 chord symbols transposed with
//! the score, spelled for the new key.

use cantor_score::{
    musicxml_concert_pitch, musicxml_transposed, parse_musicxml, write_musicxml, DegreeKind, Score,
    Step, PPQ,
};

const BLUES: &str = include_str!("../../../fixtures/musicxml/lead_sheet_blues.musicxml");

fn blues() -> Score {
    parse_musicxml(BLUES.as_bytes()).expect("fixture parses")
}

/// (measure number, tick offset, printed symbol) of every chord symbol.
fn chords(s: &Score) -> Vec<(String, u64, String)> {
    s.parts[0]
        .measures
        .iter()
        .flat_map(|m| {
            m.harmonies
                .iter()
                .map(move |h| (m.number.clone(), h.offset, h.symbol()))
        })
        .collect()
}

fn symbols(s: &Score) -> Vec<String> {
    chords(s).into_iter().map(|c| c.2).collect()
}

#[test]
fn band_01_lead_sheet_chords_with_their_timing() {
    let s = blues();
    let q = PPQ;
    assert_eq!(
        chords(&s),
        vec![
            ("1".into(), 0, "F7".into()),
            ("2".into(), 0, "Bb7".into()),
            ("2".into(), 2 * q, "F/A".into()),
            // Written after the first quarter note, plus <offset> of one quarter.
            ("3".into(), 2 * q, "C7(b9)".into()),
            ("4".into(), 0, "Gm7".into()),
            ("4".into(), 2 * q, "N.C.".into()),
        ]
    );
}

#[test]
fn band_01_harmony_fields_root_kind_bass_degrees() {
    let s = blues();
    let m = &s.parts[0].measures;
    let bb7 = &m[1].harmonies[0];
    assert_eq!(bb7.root, Some((Step::B, -1)));
    assert_eq!(bb7.kind, "dominant");
    assert_eq!(bb7.kind_text.as_deref(), Some("7"));
    assert_eq!(m[1].harmonies[1].bass, Some((Step::A, 0)));
    let c7b9 = &m[2].harmonies[0];
    assert_eq!(c7b9.degrees.len(), 1);
    assert_eq!(c7b9.degrees[0].value, 9);
    assert_eq!(c7b9.degrees[0].alter, -1);
    assert_eq!(c7b9.degrees[0].kind, DegreeKind::Add);
    assert_eq!(c7b9.staff, 1);
}

#[test]
fn band_01_chord_tones_from_kind_and_degrees() {
    let s = blues();
    let m = &s.parts[0].measures;
    // F7 = R 3 5 b7.
    assert_eq!(m[0].harmonies[0].intervals(), vec![0, 4, 7, 10]);
    // C7(b9) adds a minor ninth.
    assert_eq!(m[2].harmonies[0].intervals(), vec![0, 4, 7, 10, 13]);
    // Gm7.
    assert_eq!(m[3].harmonies[0].intervals(), vec![0, 3, 7, 10]);
    // N.C. sounds nothing.
    assert!(m[3].harmonies[1].intervals().is_empty());
}

#[test]
fn band_01_writer_keeps_chord_symbols() {
    let s = blues();
    let again = parse_musicxml(write_musicxml(&s).as_bytes()).expect("written file parses");
    assert_eq!(chords(&again), chords(&s));
    assert_eq!(again.parts[0].measures, s.parts[0].measures);
}

#[test]
fn band_02_chords_move_with_the_model_spelled_for_the_key() {
    let s = blues(); // F major, 1 flat
                     // Up 2: G major, sharps side.
    assert_eq!(
        symbols(&s.transposed(2)),
        ["G7", "C7", "G/B", "D7(b9)", "Am7", "N.C."]
    );
    // Down 2: Eb major, flats; never D#.
    assert_eq!(
        symbols(&s.transposed(-2)),
        ["Eb7", "Ab7", "Eb/G", "Bb7(b9)", "Fm7", "N.C."]
    );
    // Up 1: Gb (6 flats, tie goes up = F#)... F + 1 = F#/Gb tie -> sharps when up.
    assert_eq!(symbols(&s.transposed(1))[0], "F#7");
    // Up 5: Bb major, Bb not A#.
    assert_eq!(symbols(&s.transposed(5))[0], "Bb7");
    assert_eq!(symbols(&s.transposed(5))[1], "Eb7");
}

#[test]
fn band_02_c7_in_c_up_a_semitone_is_db7() {
    let xml = BLUES
        .replace("<fifths>-1</fifths>", "<fifths>0</fifths>")
        .replace(
            "<root-step>F</root-step></root>\n        <kind text=\"7\">",
            "<root-step>C</root-step></root>\n        <kind text=\"7\">",
        );
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    assert_eq!(symbols(&s)[0], "C7");
    assert_eq!(symbols(&s.transposed(1))[0], "Db7");
}

#[test]
fn band_02_transpose_there_and_back_keeps_the_chords() {
    let s = blues();
    for n in -11..=11 {
        assert_eq!(
            chords(&s.transposed(n).transposed(-n)),
            chords(&s),
            "n = {n}"
        );
    }
}

#[test]
fn band_02_transposed_musicxml_text_equals_transposed_model() {
    let s = blues();
    for n in [-5, -2, 1, 2, 7] {
        let text = musicxml_transposed(BLUES, n).expect("transposes");
        let parsed = parse_musicxml(text.as_bytes()).unwrap();
        assert_eq!(chords(&parsed), chords(&s.transposed(n)), "n = {n}");
        // Layout and other markup survive.
        assert!(text.contains("print-frame=\"no\""));
        assert!(text.contains("<offset>2</offset>"));
    }
}

#[test]
fn band_02_concert_pitch_moves_chords_of_transposing_parts() {
    // The same lead sheet written for a B-flat trumpet (sounds a tone lower).
    let xml = BLUES.replace(
        "<clef><sign>G</sign><line>2</line></clef>",
        "<clef><sign>G</sign><line>2</line></clef><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose>",
    );
    let concert = musicxml_concert_pitch(&xml).expect("concert pitch");
    let s = parse_musicxml(concert.as_bytes()).unwrap();
    assert_eq!(
        symbols(&s),
        ["Eb7", "Ab7", "Eb/G", "Bb7(b9)", "Fm7", "N.C."]
    );
}
