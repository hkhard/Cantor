//! LIB-01 / LIB-03: the score's own header (title, composer, arranger,
//! lyricist) is read from MusicXML, so the library shows "Que?" by
//! "Fogerty, …", not the file name "que-a-status-quo-medley-for-ttt-ttbb".

use cantor_score::{parse_musicxml, read_header, write_musicxml, Header};

const MUSESCORE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <work>
    <work-number>Op. 12</work-number>
    <work-title>Que?</work-title>
  </work>
  <movement-number>2</movement-number>
  <movement-title>Medley</movement-title>
  <identification>
    <creator type="arranger">HKH</creator>
    <creator type="composer">Fogerty, Parfitt/Brown, Rossi/Frost</creator>
    <creator type="lyricist">J. Fogerty</creator>
    <rights>© 2025 Test</rights>
    <encoding><software>MuseScore 4.4.4</software></encoding>
  </identification>
  <credit page="1"><credit-type>title</credit-type><credit-words>Que? (credit)</credit-words></credit>
  <part-list><score-part id="P1"><part-name>Tenor 1</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note>
  </measure></part>
</score-partwise>"#;

fn body() -> &'static str {
    r#"<part-list><score-part id="P1"><part-name>S</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note></measure></part>"#
}

#[test]
fn lib_01_header_from_work_and_identification() {
    let s = parse_musicxml(MUSESCORE.as_bytes()).unwrap();
    let h = &s.header;
    assert_eq!(h.work_title, "Que?");
    assert_eq!(h.work_number, "Op. 12");
    assert_eq!(h.movement_title, "Medley");
    assert_eq!(h.movement_number, "2");
    assert_eq!(h.composer, "Fogerty, Parfitt/Brown, Rossi/Frost");
    assert_eq!(h.arranger, "HKH");
    assert_eq!(h.lyricist, "J. Fogerty");
    assert_eq!(h.rights, "© 2025 Test");
    // Work title wins over the movement title for display.
    assert_eq!(h.title(), Some("Que?".to_string()));
}

#[test]
fn lib_01_title_fallbacks_movement_then_credits() {
    // Finale and Sibelius often write only <movement-title>.
    let x = format!(
        r#"<score-partwise version="3.1"><movement-title>Ave verum corpus</movement-title>{}</score-partwise>"#,
        body()
    );
    assert_eq!(
        parse_musicxml(x.as_bytes()).unwrap().header.title(),
        Some("Ave verum corpus".into())
    );
    // Dorico and some exports put the title and composer only in <credit>.
    let x = format!(
        r#"<score-partwise version="4.0">
        <credit page="1"><credit-type>title</credit-type><credit-words>Vårvindar friska</credit-words></credit>
        <credit page="1"><credit-type>composer</credit-type><credit-words>Svensk folkmelodi</credit-words></credit>
        <credit page="1"><credit-type>arranger</credit-type><credit-words>arr. Hugo Alfvén</credit-words></credit>
        {}</score-partwise>"#,
        body()
    );
    let h = parse_musicxml(x.as_bytes()).unwrap().header;
    assert_eq!(h.title(), Some("Vårvindar friska".into()));
    assert_eq!(h.composer, "Svensk folkmelodi");
    assert_eq!(h.arranger, "Hugo Alfvén");
    // Untyped credits: the biggest font on page 1 is the title.
    let x = format!(
        r#"<score-partwise version="3.0">
        <credit page="1"><credit-words font-size="12" justify="right">Music: J. S. Bach</credit-words></credit>
        <credit page="1"><credit-words font-size="24" justify="center">Jesu, meine Freude</credit-words></credit>
        {}</score-partwise>"#,
        body()
    );
    assert_eq!(
        parse_musicxml(x.as_bytes()).unwrap().header.title(),
        Some("Jesu, meine Freude".into())
    );
    // No header at all, or a placeholder title from a notation program.
    for t in ["", "Untitled score", "Title", "  "] {
        let x = format!(
            r#"<score-partwise version="4.0"><work><work-title>{t}</work-title></work>{}</score-partwise>"#,
            body()
        );
        assert_eq!(
            parse_musicxml(x.as_bytes()).unwrap().header.title(),
            None,
            "{t:?}"
        );
    }
    // A poet counts as lyricist; "words" credit type too.
    let x = format!(
        r#"<score-partwise version="4.0"><identification><creator type="poet">Gustaf Fröding</creator></identification>{}</score-partwise>"#,
        body()
    );
    assert_eq!(
        parse_musicxml(x.as_bytes()).unwrap().header.lyricist,
        "Gustaf Fröding"
    );
}

#[test]
fn lib_01_header_survives_writing() {
    // Saving a corrected score or an OMR result must not drop the header.
    let s = parse_musicxml(MUSESCORE.as_bytes()).unwrap();
    let back = parse_musicxml(write_musicxml(&s).as_bytes()).unwrap();
    assert_eq!(back.header, s.header);
    assert_eq!(back, s);
    let empty = Header::default();
    assert_eq!(empty.title(), None);
}

#[test]
fn lib_01_read_header_alone_matches_full_parse() {
    let full = parse_musicxml(MUSESCORE.as_bytes()).unwrap().header;
    assert_eq!(read_header(MUSESCORE.as_bytes()).unwrap(), full);
    // The arrangement fixture (MuseScore MXL, compressed; private, not on
    // GitHub): checked when present.
    if let Ok(mxl) = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/scores/the-anchor-song/the-anchor-song.mxl"
    )) {
        let h = read_header(&mxl).unwrap();
        assert_eq!(h.title(), Some("The Anchor Song".into()));
        assert_eq!(h.composer, "B. Gudmundsdottir");
        assert_eq!(h.arranger, "HKH");
        assert_eq!(cantor_score::parse_score_file(&mxl).unwrap().header, h);
    }
    assert!(read_header(b"<html>not a score</html>").is_err());
}

#[test]
fn lib_01_header_survives_a_note_correction() {
    let out = cantor_score::edit_note(MUSESCORE, 0, 0, 0, cantor_score::NoteEdit::Step(1)).unwrap();
    assert_eq!(
        parse_musicxml(out.as_bytes()).unwrap().header,
        parse_musicxml(MUSESCORE.as_bytes()).unwrap().header
    );
}

#[test]
fn lib_01_header_from_untyped_credits_lacrymosa() {
    // MuseScore 3 export with empty <work> metadata: title, subtitle and
    // composer exist only as page-1 credit text.
    let mxl = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/scores/mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.mxl"
    ))
    .unwrap();
    let h = read_header(&mxl).unwrap();
    assert_eq!(h.title(), Some("Lacrymosa".into()));
    assert_eq!(h.composer, "Wolfgang Amadeus Mozart");
    let s = cantor_score::parse_score_file(&mxl).unwrap();
    assert_eq!(s.header, h);
    assert_eq!(s.parts.len(), 5);
}
