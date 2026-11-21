//! Joining scores page by page (OMR fallback) and writing MusicXML.

use cantor_score::{parse_musicxml, write_musicxml, EventKind};

const PAGE: &str = r#"<?xml version="1.0"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Tenor</part-name></score-part>
    <score-part id="P2"><part-name>Bass</part-name></score-part>
  </part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
    <direction><sound tempo="72"/></direction>
    <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice></note>
    <note><rest/><duration>2</duration><voice>1</voice></note>
    <barline location="right"><repeat direction="backward" times="3"/></barline>
  </measure></part>
  <part id="P2"><measure number="1">
    <attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>3</octave></pitch><duration>6</duration><voice>1</voice></note>
    <backup><duration>6</duration></backup>
    <note><pitch><step>G</step><octave>2</octave></pitch><duration>6</duration><voice>2</voice></note>
    <note><chord/><pitch><step>D</step><octave>3</octave></pitch><duration>6</duration><voice>2</voice></note>
  </measure></part>
</score-partwise>"#;

#[test]
fn lib_06_musicxml_write_roundtrip_keeps_model() {
    let a = parse_musicxml(PAGE.as_bytes()).unwrap();
    let xml = write_musicxml(&a);
    let b = parse_musicxml(xml.as_bytes()).expect("written MusicXML parses");
    assert_eq!(a, b);
}

/// Regression (Glimmande Nymf): two voices with a note at the same onset and
/// length must not be written as one `<chord/>` across voices; MuseScore
/// rejects that file.
#[test]
fn lib_06_writer_never_chords_across_voices() {
    let xml = r#"<?xml version="1.0"?><score-partwise><part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list>
      <part id="P1"><measure number="1">
        <attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
        <note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
        <note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note>
        <backup><duration>1</duration></backup>
        <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice></note>
      </measure></part></score-partwise>"#;
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let out = write_musicxml(&s);
    assert!(!out.contains("<chord/>"), "{out}");
    assert_eq!(parse_musicxml(out.as_bytes()).unwrap(), s);
}

/// Regression (I bröllopsgården): a padded empty measure of length 4/4 next
/// to a part whose measure holds only 2 beats must keep its 4/4 length after
/// a write/read round trip, else the parts drift apart.
#[test]
fn lib_06_padded_empty_measures_keep_length_through_writer() {
    let mut a = parse_musicxml(PAGE.as_bytes()).unwrap();
    let one = PAGE
        .replace(
            r#"<score-part id="P2"><part-name>Bass</part-name></score-part>"#,
            "",
        )
        .split(r#"<part id="P2">"#)
        .next()
        .unwrap()
        .to_string()
        + "</score-partwise>";
    // First page has only a tenor; the bass appears on the next page.
    let mut first = parse_musicxml(one.as_bytes()).unwrap();
    first.append(std::mem::take(&mut a));
    let back = parse_musicxml(write_musicxml(&first).as_bytes()).unwrap();
    assert_eq!(
        back.parts[1].measures[0].duration,
        first.parts[0].measures[0].duration
    );
    assert_eq!(back, first);
}

/// OMR pages often lack a time signature. A padded empty measure then has a
/// duration but no time signature; the writer must keep that length (a
/// measure rest) so the parts stay aligned in other readers.
#[test]
fn lib_06_empty_measure_without_time_signature_keeps_length() {
    let mut s = parse_musicxml(PAGE.as_bytes()).unwrap();
    for p in &mut s.parts {
        for m in &mut p.measures {
            m.time_signature = None;
        }
    }
    s.parts[1].measures[0].events.clear();
    s.parts[1].measures[0].content_duration = 0;
    let xml = write_musicxml(&s);
    let back = parse_musicxml(xml.as_bytes()).unwrap();
    assert_eq!(
        back.parts[1].measures[0].duration, s.parts[1].measures[0].duration,
        "{xml}"
    );
}

#[test]
fn lib_06_append_joins_parts_by_position() {
    let mut a = parse_musicxml(PAGE.as_bytes()).unwrap();
    let b = parse_musicxml(PAGE.as_bytes()).unwrap();
    a.append(b);
    assert_eq!(a.parts.len(), 2);
    assert_eq!(a.parts[0].measures.len(), 2);
    assert_eq!(a.parts[1].measures.len(), 2);
    assert_eq!(a.parts[1].measures[1].events.len(), 3);
    assert!(matches!(
        a.parts[0].measures[1].events[0].kind,
        EventKind::Note(_)
    ));
}

#[test]
fn lib_06_append_pads_missing_parts_with_empty_measures() {
    let mut a = parse_musicxml(PAGE.as_bytes()).unwrap();
    // Page with only one part (OMR merged staves differently).
    let one = PAGE
        .replace(
            r#"<score-part id="P2"><part-name>Bass</part-name></score-part>"#,
            "",
        )
        .split(r#"<part id="P2">"#)
        .next()
        .unwrap()
        .to_string()
        + "</score-partwise>";
    let b = parse_musicxml(one.as_bytes()).unwrap();
    a.append(b);
    assert_eq!(
        a.parts[1].measures.len(),
        2,
        "bass padded so parts stay aligned"
    );
    assert!(a.parts[1].measures[1].events.is_empty());
    assert_eq!(
        a.parts[1].measures[1].duration,
        a.parts[0].measures[1].duration
    );
}
