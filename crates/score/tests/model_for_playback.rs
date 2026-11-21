//! Model features needed to turn notes into MIDI: MXL container, ties, tempo,
//! measure length, MIDI program. (LIB-01, PLAY-01, PLAY-09)

use cantor_score::{parse_musicxml, parse_score_file, EventKind, PPQ};
use std::io::Write;

const TWO_PARTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Tenor</part-name>
      <score-instrument id="P1-I1"><instrument-name>Voice</instrument-name></score-instrument>
      <midi-instrument id="P1-I1"><midi-channel>1</midi-channel><midi-program>53</midi-program></midi-instrument>
    </score-part>
    <score-part id="P2"><part-name>Bass</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="0" implicit="yes">
      <attributes><divisions>2</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
      <direction><sound tempo="90"/></direction>
      <note><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration><voice>1</voice></note>
    </measure>
    <measure number="1">
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>6</duration><tie type="start"/><voice>1</voice></note>
    </measure>
    <measure number="2">
      <direction><sound tempo="60"/></direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>1</voice></note>
      <note><rest/><duration>4</duration><voice>1</voice></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="0" implicit="yes">
      <note><rest/><duration>2</duration></note>
    </measure>
    <measure number="1"><note><rest measure="yes"/><duration>6</duration></note></measure>
    <measure number="2"><note><rest measure="yes"/><duration>6</duration></note></measure>
  </part>
</score-partwise>"#;

#[test]
fn play_09_midi_program_and_channel_from_part_list() {
    let s = parse_musicxml(TWO_PARTS.as_bytes()).unwrap();
    assert_eq!(
        s.parts[0].midi_program,
        Some(52),
        "MusicXML is 1-based, model is 0-based GM"
    );
    assert_eq!(s.parts[0].midi_channel, Some(0));
    assert_eq!(s.parts[1].midi_program, None);
}

#[test]
fn play_01_measure_length_time_signature_and_pickup() {
    let s = parse_musicxml(TWO_PARTS.as_bytes()).unwrap();
    let m = &s.parts[0].measures;
    assert_eq!(m[0].duration, PPQ, "implicit pickup = content length");
    assert_eq!(m[1].duration, 3 * PPQ, "3/4");
    assert_eq!(m[1].time_signature, Some((3, 4)));
}

#[test]
fn play_01_tempo_and_ties_parsed() {
    let s = parse_musicxml(TWO_PARTS.as_bytes()).unwrap();
    let m = &s.parts[0].measures;
    assert_eq!(m[0].tempos, vec![(0, 90.0)]);
    assert_eq!(m[2].tempos, vec![(0, 60.0)]);
    let first = &m[1].events[0];
    assert!(first.tie_start && !first.tie_stop);
    let second = &m[2].events[0];
    assert!(second.tie_stop && !second.tie_start);
    assert_eq!(m[2].events[1].kind, EventKind::Rest);
}

fn make_mxl(rootfile: &str, xml: &str, with_container: bool) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default();
        z.start_file("mimetype", opt).unwrap();
        z.write_all(b"application/vnd.recordare.musicxml").unwrap();
        if with_container {
            z.start_file("META-INF/container.xml", opt).unwrap();
            write!(
                z,
                r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="{rootfile}" media-type="application/vnd.recordare.musicxml+xml"/></rootfiles></container>"#
            )
            .unwrap();
        }
        z.start_file(rootfile, opt).unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn lib_01_parses_mxl_container() {
    let mxl = make_mxl("score/Tenor Bass.xml", TWO_PARTS, true);
    let s = parse_score_file(&mxl).expect("mxl parses");
    assert_eq!(s.parts.len(), 2);
    assert_eq!(s.parts[1].name, "Bass");
}

#[test]
fn lib_01_mxl_without_container_falls_back_to_single_xml() {
    let mxl = make_mxl("piece.musicxml", TWO_PARTS, false);
    assert_eq!(parse_score_file(&mxl).unwrap().parts.len(), 2);
}

#[test]
fn lib_01_score_file_sniffs_plain_xml_and_rejects_garbage() {
    assert_eq!(
        parse_score_file(TWO_PARTS.as_bytes()).unwrap().parts.len(),
        2
    );
    assert!(parse_score_file(b"PK\x03\x04broken").is_err());
    assert!(parse_score_file(b"").is_err());
}
