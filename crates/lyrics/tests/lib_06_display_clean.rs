//! LIB-06: OMR output shown in the notation view is cleaned of OCR noise:
//! lyric texts that are not sung (dynamics, credits, URLs) and chord symbols
//! made from misread text. Files from notation programs are left alone.

use cantor_lyrics::clean_omr_musicxml;

const OMR: &str = r#"<?xml version="1.0"?><score-partwise version="4.0">
<identification><encoding><software>Audiveris 5.11.0</software></encoding></identification>
<part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
<part id="P1"><measure number="1">
<harmony><root><root-step>A</root-step></root><kind>major</kind></harmony>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
  <lyric number="1"><syllabic>single</syllabic><text>Bre</text></lyric>
  <lyric number="2"><syllabic>single</syllabic><text>mf</text></lyric></note>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
  <lyric number="1"><text>www.studentersangforeningen.no</text></lyric></note>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
  <lyric number="1"><syllabic>begin</syllabic><text>1.3.want</text></lyric></note>
</measure></part></score-partwise>"#;

#[test]
fn lib_06_omr_lyric_noise_and_chord_symbols_removed() {
    let out = clean_omr_musicxml(OMR);
    assert!(out.contains("<text>Bre</text>"));
    assert!(!out.contains("<text>mf</text>"));
    assert!(!out.contains("studentersangforeningen"));
    assert!(out.contains("<text>want</text>"), "verse number stripped");
    assert!(!out.contains("<harmony"), "OCR chord symbols removed");
    // Still valid for the parser, same notes.
    let s = cantor_score::parse_musicxml(out.as_bytes()).unwrap();
    assert_eq!(s.parts[0].measures[0].events.len(), 3);
}

#[test]
fn lib_06_notation_program_files_untouched() {
    let musescore = OMR.replace("Audiveris 5.11.0", "MuseScore 4.4.4");
    assert_eq!(clean_omr_musicxml(&musescore), musescore);
}
