//! VOX-01: lyrics per note, verse, syllabic and extender are parsed into the
//! model and survive the MusicXML writer.

use cantor_score::{parse_musicxml, write_musicxml, Syllabic};

const XML: &str = r#"<?xml version="1.0"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Tenor</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
      <lyric number="1"><syllabic>begin</syllabic><text>Glo</text></lyric>
      <lyric number="2"><syllabic>single</syllabic><text>Är</text></lyric></note>
    <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration>
      <lyric number="1"><syllabic>end</syllabic><text>ri</text><extend type="start"/></lyric></note>
    <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration>
      <lyric number="1"><extend type="stop"/></lyric></note>
    <note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration>
      <lyric number="1"><syllabic>single</syllabic><text>a &amp; b</text></lyric>
      <lyric name="chorus"><text>la</text></lyric></note>
  </measure></part>
</score-partwise>"#;

#[test]
fn vox_01_lyrics_parsed_per_note_and_verse() {
    let s = parse_musicxml(XML.as_bytes()).unwrap();
    let ev = &s.parts[0].measures[0].events;

    let l = &ev[0].lyrics;
    assert_eq!(l.len(), 2);
    assert_eq!(
        (l[0].verse, l[0].text.as_str(), l[0].syllabic),
        (1, "Glo", Syllabic::Begin)
    );
    assert_eq!(
        (l[1].verse, l[1].text.as_str(), l[1].syllabic),
        (2, "Är", Syllabic::Single)
    );

    let ri = &ev[1].lyrics[0];
    assert_eq!(
        (ri.text.as_str(), ri.syllabic, ri.extend),
        ("ri", Syllabic::End, true)
    );

    // Extender-only lyric: no new syllable, continues the melisma.
    let cont = &ev[2].lyrics[0];
    assert!(cont.text.is_empty());
    assert!(cont.extend);

    assert_eq!(ev[3].lyrics[0].text, "a & b");
    // Unnumbered lyric gets the next free verse number.
    assert_eq!(ev[3].lyrics[1].verse, 2);
}

#[test]
fn vox_01_score_reports_verses() {
    let s = parse_musicxml(XML.as_bytes()).unwrap();
    assert_eq!(s.parts[0].verses(), vec![1, 2]);
}

#[test]
fn vox_01_lyrics_survive_writer_roundtrip() {
    let s = parse_musicxml(XML.as_bytes()).unwrap();
    let back = parse_musicxml(write_musicxml(&s).as_bytes()).unwrap();
    assert_eq!(back, s);
}
