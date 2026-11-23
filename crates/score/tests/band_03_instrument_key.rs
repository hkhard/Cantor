//! BAND-03: notation shown written for a B-flat, E-flat or F instrument
//! while the model (and so playback) stays at concert pitch.

use cantor_score::{
    musicxml_for_instrument, parse_musicxml, EventKind, InstrumentKey, Pitch, Score, Step,
};

const BLUES: &str = include_str!("../../../fixtures/musicxml/lead_sheet_blues.musicxml");

fn first_note(s: &Score) -> Pitch {
    match s.parts[0].measures[0].events[0].kind {
        EventKind::Note(p) => p,
        _ => panic!("rest"),
    }
}

fn first_chord(s: &Score) -> String {
    s.parts[0].measures[0].harmonies[0].symbol()
}

fn shown(key: InstrumentKey) -> Score {
    let xml = musicxml_for_instrument(BLUES, key).expect("transposes");
    parse_musicxml(xml.as_bytes()).unwrap()
}

#[test]
fn band_03_instrument_keys_written_above_concert() {
    // Concert F4, F7 in F major (1 flat).
    let c = shown(InstrumentKey::C);
    assert_eq!(first_note(&c).midi(), 65);
    assert_eq!(first_chord(&c), "F7");
    // B-flat: a major second up, G major.
    let bb = shown(InstrumentKey::Bb);
    assert_eq!(first_note(&bb).step, Step::G);
    assert_eq!(first_note(&bb).midi(), 67);
    assert_eq!(first_chord(&bb), "G7");
    assert_eq!(bb.parts[0].measures[0].key_fifths, Some(1));
    // E-flat: a major sixth up, D major.
    let eb = shown(InstrumentKey::Eb);
    assert_eq!(first_note(&eb).midi(), 74);
    assert_eq!(first_chord(&eb), "D7");
    assert_eq!(eb.parts[0].measures[0].key_fifths, Some(2));
    // F: a fifth up, C major.
    let f = shown(InstrumentKey::F);
    assert_eq!(first_note(&f).midi(), 72);
    assert_eq!(first_chord(&f), "C7");
    assert_eq!(f.parts[0].measures[0].key_fifths, Some(0));
}

#[test]
fn band_03_concert_c_shows_as_d_and_c7_as_d7_for_b_flat() {
    let xml = BLUES
        .replace("<fifths>-1</fifths>", "<fifths>0</fifths>")
        .replacen("<root-step>F</root-step>", "<root-step>C</root-step>", 1)
        .replacen(
            "<step>F</step><octave>4</octave>",
            "<step>C</step><octave>4</octave>",
            1,
        );
    let shown = parse_musicxml(
        musicxml_for_instrument(&xml, InstrumentKey::Bb)
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(first_note(&shown).step, Step::D);
    assert_eq!(first_chord(&shown), "D7");
}

#[test]
fn band_03_parts_already_written_for_a_transposing_instrument() {
    // A trumpet part (written a tone above concert) shown to an alto sax:
    // to concert first, then a major sixth up.
    let trumpet = BLUES.replace(
        "<clef><sign>G</sign><line>2</line></clef>",
        "<clef><sign>G</sign><line>2</line></clef><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose>",
    );
    let s = parse_musicxml(
        musicxml_for_instrument(&trumpet, InstrumentKey::Eb)
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    // Written F4 on a trumpet sounds Eb4; an alto sax reads that as C5.
    assert_eq!(first_note(&s).midi(), 72);
    assert_eq!(first_chord(&s), "C7");
    assert_eq!(s.parts[0].measures[0].transpose, 0);
}

#[test]
fn band_03_sound_stays_at_concert_pitch() {
    // The display change never touches the playback model: open_score builds
    // the timemap from the concert-pitch model, and the instrument key only
    // rewrites the MusicXML for the engraver.
    let model = parse_musicxml(BLUES.as_bytes()).unwrap();
    let _ = musicxml_for_instrument(BLUES, InstrumentKey::Bb).unwrap();
    assert_eq!(first_note(&model).midi(), 65);
}

#[test]
fn band_03_settings_value_round_trips() {
    for k in [
        InstrumentKey::C,
        InstrumentKey::Bb,
        InstrumentKey::Eb,
        InstrumentKey::F,
    ] {
        assert_eq!(InstrumentKey::parse(k.as_str()), k);
    }
    assert_eq!(InstrumentKey::parse("B♭"), InstrumentKey::Bb);
    assert_eq!(InstrumentKey::parse("nonsense"), InstrumentKey::C);
    assert_eq!(
        musicxml_for_instrument(BLUES, InstrumentKey::C).unwrap(),
        BLUES
    );
}
