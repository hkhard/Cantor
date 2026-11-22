//! VIEW-05: transpose display and playback by semitones.

use cantor_score::{
    musicxml_transposed, parse_musicxml, transposed_fifths, write_musicxml, EventKind, Pitch,
    Score, Step,
};

/// D major (2 sharps) with naturals, a flat and a sharp, chords, lyrics and
/// layout markup that must survive the text-level transpose.
const SONG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1" width="320">
      <print new-system="yes"/>
      <attributes><divisions>1</divisions><key><fifths>2</fifths><mode>major</mode></key><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
      <note default-x="80"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>begin</syllabic><text>Hal</text></lyric></note>
      <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>end</syllabic><text>lo</text></lyric></note>
      <note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><accidental>natural</accidental></note>
      <note><chord/><pitch><step>E</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice></note>
      <note><pitch><step>B</step><alter>-1</alter><octave>3</octave></pitch><duration>1</duration><voice>1</voice><accidental>flat</accidental></note>
    </measure>
    <measure number="2">
      <attributes><key><fifths>-1</fifths></key></attributes>
      <!-- a comment -->
      <note><pitch><step>B</step><alter>-1</alter><octave>4</octave></pitch><duration>2</duration><voice>1</voice></note>
      <note><pitch><step>G</step><alter>1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
      <note><rest/><duration>1</duration><voice>1</voice></note>
    </measure>
  </part>
</score-partwise>"#;

fn song() -> Score {
    parse_musicxml(SONG.as_bytes()).expect("parses")
}

fn pitches(s: &Score) -> Vec<Pitch> {
    s.parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .filter_map(|e| match &e.kind {
            EventKind::Note(p) => Some(*p),
            EventKind::Rest => None,
        })
        .collect()
}

fn midis(s: &Score) -> Vec<i32> {
    pitches(s).iter().map(Pitch::midi).collect()
}

fn keys(s: &Score) -> Vec<Option<i32>> {
    s.parts[0].measures.iter().map(|m| m.key_fifths).collect()
}

fn p(step: Step, alter: i32, octave: i32) -> Pitch {
    Pitch {
        step,
        alter,
        octave,
    }
}

#[test]
fn view_05_key_fifths_parsed_and_carried() {
    assert_eq!(keys(&song()), vec![Some(2), Some(-1)]);
}

#[test]
fn view_05_up_then_down_is_identity() {
    let s = song();
    for n in [-11, -7, -2, -1, 1, 2, 3, 5, 6, 7, 12] {
        assert_eq!(s.transposed(n).transposed(-n), s, "n = {n}");
    }
}

#[test]
fn view_05_midi_shifts_exactly() {
    let s = song();
    for n in -14..=14 {
        let expected: Vec<i32> = midis(&s).iter().map(|m| m + n).collect();
        assert_eq!(midis(&s.transposed(n)), expected, "n = {n}");
    }
}

#[test]
fn view_05_octave_shifts_octave_only() {
    let s = song();
    let up = s.transposed(12);
    assert_eq!(keys(&up), keys(&s));
    for (a, b) in pitches(&s).iter().zip(pitches(&up)) {
        assert_eq!((b.step, b.alter, b.octave), (a.step, a.alter, a.octave + 1));
    }
}

#[test]
fn view_05_spelling_follows_new_key() {
    // D major up a whole tone: E major (4 sharps).
    let up = song().transposed(2);
    assert_eq!(keys(&up), vec![Some(4), Some(1)]);
    assert_eq!(
        pitches(&up),
        vec![
            p(Step::E, 0, 4),
            p(Step::G, 1, 4),
            p(Step::D, 0, 5), // C natural in D -> D natural in E
            p(Step::F, 1, 5),
            p(Step::C, 0, 4), // Bb in D -> C natural in E
            p(Step::C, 0, 5), // F major -> G major
            p(Step::A, 1, 4),
        ]
    );
    // Down a semitone from D major: Db major (5 flats), flats not sharps.
    let down = song().transposed(-1);
    assert_eq!(keys(&down)[0], Some(-5));
    assert_eq!(pitches(&down)[0], p(Step::D, -1, 4));
}

#[test]
fn view_05_fifths_wrap_into_range() {
    assert_eq!(transposed_fifths(0, 2), 2);
    assert_eq!(transposed_fifths(0, -2), -2);
    assert_eq!(transposed_fifths(6, 2), -4);
    assert_eq!(transposed_fifths(-6, -2), 4);
    assert_eq!(transposed_fifths(3, 12), 3);
    for f in -7..=7 {
        for n in -24..=24 {
            assert!((-7..=7).contains(&transposed_fifths(f, n)));
        }
    }
}

#[test]
fn view_05_write_round_trip() {
    let t = song().transposed(3);
    let back = parse_musicxml(write_musicxml(&t).as_bytes()).expect("parses");
    assert_eq!(back, t);
}

#[test]
fn view_05_text_transpose_matches_model() {
    for n in [-5, -1, 2, 7, 12] {
        let x = musicxml_transposed(SONG, n).expect("transposes");
        let parsed = parse_musicxml(x.as_bytes()).expect("parses");
        assert_eq!(parsed, song().transposed(n), "n = {n}");
    }
}

/// Blanks the contents of `open..close` elements.
fn blank(x: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = x;
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i + open.len()]);
        rest = &rest[i + open.len()..];
        rest = &rest[rest.find(close).expect("closed")..];
    }
    out.push_str(rest);
    out
}

/// The document with every element the transpose may edit blanked.
fn skeleton(x: &str) -> String {
    let x = blank(x, "<pitch>", "</pitch>");
    let x = blank(&x, "<fifths>", "</fifths>");
    blank(&x, "<accidental>", "</accidental>")
}

#[test]
fn view_05_text_transpose_preserves_other_markup() {
    let x = musicxml_transposed(SONG, 2).expect("transposes");
    assert_ne!(x, SONG);
    assert_eq!(skeleton(&x), skeleton(SONG));
    assert!(x.contains("<fifths>4</fifths><mode>major</mode>"));
    assert!(x.contains("<pitch><step>G</step><alter>1</alter><octave>4</octave></pitch>"));
    // Bb -> C natural: the written accidental follows.
    assert!(x.contains("<pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><accidental>natural</accidental>"));
    assert!(x.contains("<!-- a comment -->") && x.contains("<text>Hal</text>"));
}

#[test]
fn view_05_text_transpose_rejects_garbage() {
    assert!(musicxml_transposed("<nope", 1).is_err());
}

/// A B-flat clarinet part: written D5 sounds C5 (`<transpose>` chromatic -2),
/// and a tenor line written an octave up (octave-change -1).
const CLARINET: &str = r#"<?xml version="1.0"?><score-partwise version="4.0">
<part-list><score-part id="P1"><part-name>Clarinet in B-flat</part-name></score-part>
<score-part id="P2"><part-name>Tenor</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions>
<transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes>
<note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration></note></measure>
<measure number="2"><note><pitch><step>E</step><octave>5</octave></pitch><duration>1</duration></note></measure></part>
<part id="P2"><measure number="1"><attributes><divisions>1</divisions>
<transpose><diatonic>0</diatonic><chromatic>0</chromatic><octave-change>-1</octave-change></transpose></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note></measure>
<measure number="2"><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note></measure></part>
</score-partwise>"#;

#[test]
fn view_05_transposing_instrument_sounds_at_concert_pitch() {
    let s = parse_musicxml(CLARINET.as_bytes()).unwrap();
    let sounding: Vec<Vec<i32>> = s
        .parts
        .iter()
        .map(|p| {
            p.measures
                .iter()
                .flat_map(|m| {
                    m.events
                        .iter()
                        .filter_map(move |e| e.kind.midi().map(|k| k + m.transpose))
                })
                .collect()
        })
        .collect();
    // Written D5 (74) sounds C5 (72); E5 (76) sounds D5 (74), carried over.
    assert_eq!(sounding[0], vec![72, 74]);
    // Written C4 (60) sounds C3 (48) in both bars.
    assert_eq!(sounding[1], vec![48, 48]);
}

#[test]
fn view_05_concert_pitch_view_rewrites_written_pitches() {
    // Concert pitch display: the written notes become the sounding ones and
    // the <transpose> element goes, so the notation and the sound agree.
    let concert = cantor_score::musicxml_concert_pitch(CLARINET).unwrap();
    assert!(!concert.contains("<transpose>"), "{concert}");
    let s = parse_musicxml(concert.as_bytes()).unwrap();
    let written: Vec<i32> = s.parts[0]
        .measures
        .iter()
        .flat_map(|m| m.events.iter().filter_map(|e| e.kind.midi()))
        .collect();
    assert_eq!(written, vec![72, 74]);
    assert!(s.parts[0].measures.iter().all(|m| m.transpose == 0));
    // Without any <transpose> the text is returned unchanged.
    assert_eq!(cantor_score::musicxml_concert_pitch(SONG).unwrap(), SONG);
}

#[test]
fn view_05_transpose_survives_write_and_parse() {
    let s = parse_musicxml(CLARINET.as_bytes()).unwrap();
    let back = parse_musicxml(write_musicxml(&s).as_bytes()).unwrap();
    let t = |s: &Score| {
        s.parts
            .iter()
            .map(|p| p.measures.iter().map(|m| m.transpose).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    assert_eq!(t(&back), t(&s));
    assert_eq!(t(&s), vec![vec![-2, -2], vec![-12, -12]]);
}
