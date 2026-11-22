//! VIEW-05: transposing instruments play at concert pitch (`<transpose>`),
//! and a score shown at concert pitch plays exactly the same notes.
use cantor_score::{musicxml_concert_pitch, parse_musicxml};
use cantor_timeline::build_timemap;

const XML: &str = r#"<?xml version="1.0"?><score-partwise version="4.0">
<part-list><score-part id="P1"><part-name>Horn in F</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions>
<transpose><diatonic>-4</diatonic><chromatic>-7</chromatic></transpose></attributes>
<note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration></note>
<note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration></note></measure></part>
</score-partwise>"#;

#[test]
fn view_05_horn_in_f_sounds_a_fifth_lower() {
    let tm = build_timemap(&parse_musicxml(XML.as_bytes()).unwrap());
    let keys: Vec<u8> = tm.notes.iter().map(|n| n.pitch).collect();
    // Written C5 (72) sounds F4 (65); written G4 (67) sounds C4 (60).
    assert_eq!(keys, vec![65, 60]);
}

#[test]
fn view_05_concert_pitch_view_plays_the_same() {
    let a = build_timemap(&parse_musicxml(XML.as_bytes()).unwrap());
    let b =
        build_timemap(&parse_musicxml(musicxml_concert_pitch(XML).unwrap().as_bytes()).unwrap());
    let k = |t: &cantor_timeline::Timemap| {
        t.notes
            .iter()
            .map(|n| (n.pitch, n.tick_on))
            .collect::<Vec<_>>()
    };
    assert_eq!(k(&a), k(&b));
}
