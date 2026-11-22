//! LIB-06: correct OMR mistakes in the app and re-save. A note is addressed
//! by part index, written measure index and note index within the measure
//! (document order, chords included), the same order Verovio draws them.
//! Edits change only that `<note>` (pitch or rest); everything else in the
//! file stays byte-identical.
use cantor_score::{edit_note, parse_musicxml, NoteEdit, Pitch, Step};

const XML: &str = r#"<?xml version="1.0"?><score-partwise version="4.0">
<part-list><score-part id="P1"><part-name>S</part-name></score-part><score-part id="P2"><part-name>B</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>-1</fifths></key></attributes>
<note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric><text>Glo</text></lyric></note>
<note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
<note><chord/><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice></note>
<note><rest/><duration>2</duration><voice>1</voice></note>
</measure><measure number="2">
<note><pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch><duration>4</duration><voice>1</voice></note>
</measure></part>
<part id="P2"><measure number="1"><attributes><divisions>1</divisions></attributes>
<note><pitch><step>F</step><octave>3</octave></pitch><duration>4</duration></note></measure>
<measure number="2"><note><pitch><step>F</step><octave>3</octave></pitch><duration>4</duration></note></measure></part>
</score-partwise>"#;

fn pitches(xml: &str, part: usize, measure: usize) -> Vec<Option<i32>> {
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    s.parts[part].measures[measure]
        .events
        .iter()
        .map(|e| e.kind.midi())
        .collect()
}

#[test]
fn lib_06_step_a_note_up_in_the_key() {
    // A4 up one step in F major is B-flat 4 (the key's flat), keeping the lyric elsewhere.
    let out = edit_note(XML, 0, 0, 1, NoteEdit::Step(1)).unwrap();
    assert_eq!(
        pitches(&out, 0, 0),
        vec![Some(65), Some(70), Some(72), None]
    );
    // Only that note changed.
    let want = XML.replacen(
        "<pitch><step>A</step><octave>4</octave></pitch>",
        "<pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch>",
        1,
    );
    assert_eq!(out, want);
    assert!(out.contains("<lyric><text>Glo</text></lyric>"));
}

#[test]
fn lib_06_step_down_over_the_octave_and_set_pitch() {
    // F4 down a step -> E4; C5 (chord note) down a step -> B-flat 4.
    let out = edit_note(XML, 0, 0, 0, NoteEdit::Step(-1)).unwrap();
    assert_eq!(pitches(&out, 0, 0)[0], Some(64));
    let out = edit_note(&out, 0, 0, 2, NoteEdit::Step(-1)).unwrap();
    assert_eq!(pitches(&out, 0, 0)[2], Some(70));
    // Exact pitch.
    let p = Pitch {
        step: Step::G,
        alter: 1,
        octave: 3,
    };
    let out = edit_note(XML, 1, 1, 0, NoteEdit::Set(p)).unwrap();
    assert_eq!(pitches(&out, 1, 1), vec![Some(56)]);
}

#[test]
fn lib_06_note_to_rest_and_rest_to_note() {
    let out = edit_note(XML, 0, 1, 0, NoteEdit::Rest).unwrap();
    assert_eq!(pitches(&out, 0, 1), vec![None]);
    // A rest becomes a note.
    let p = Pitch {
        step: Step::D,
        alter: 0,
        octave: 5,
    };
    let out = edit_note(XML, 0, 0, 3, NoteEdit::Set(p)).unwrap();
    assert_eq!(pitches(&out, 0, 0)[3], Some(74));
    // Durations unchanged.
    let a = parse_musicxml(XML.as_bytes()).unwrap();
    let b = parse_musicxml(out.as_bytes()).unwrap();
    assert_eq!(
        a.parts[0].measures[0].duration,
        b.parts[0].measures[0].duration
    );
}

#[test]
fn lib_06_bad_address_is_an_error() {
    assert!(edit_note(XML, 0, 0, 9, NoteEdit::Step(1)).is_err());
    assert!(edit_note(XML, 5, 0, 0, NoteEdit::Step(1)).is_err());
    assert!(edit_note(XML, 0, 7, 0, NoteEdit::Step(1)).is_err());
}
