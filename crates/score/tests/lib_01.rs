//! LIB-01: Import MusicXML (.musicxml, .xml) and compressed MXL.

use cantor_score::{parse_musicxml, EventKind, ParseError, Pitch, Step, PPQ};

fn fixture(name: &str) -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/musicxml/");
    std::fs::read(format!("{path}{name}")).expect("fixture exists")
}

#[test]
fn lib_01_parses_hello_world_musicxml() {
    let score = parse_musicxml(&fixture("hello_world.musicxml")).expect("parses");

    assert_eq!(score.parts.len(), 1);
    let part = &score.parts[0];
    assert_eq!(part.id, "P1");
    assert_eq!(part.name, "Music");
    assert_eq!(part.measures.len(), 1);

    let m = &part.measures[0];
    assert_eq!(m.number, "1");
    assert_eq!(m.events.len(), 1);

    let e = &m.events[0];
    assert_eq!(e.offset, 0);
    assert_eq!(
        e.duration,
        4 * PPQ,
        "whole note in 4/4 normalised to 960 ppq"
    );
    assert_eq!(e.voice, 1);
    assert_eq!(e.staff, 1);
    assert_eq!(
        e.kind,
        EventKind::Note(Pitch {
            step: Step::C,
            alter: 0,
            octave: 4
        })
    );
    assert_eq!(e.kind.midi(), Some(60));
}

#[test]
fn lib_01_invalid_xml_returns_error_not_panic() {
    for bad in [
        &b""[..],
        b"not xml at all",
        b"<score-partwise><part-list>",
        b"<?xml version=\"1.0\"?><html/>",
        b"<score-partwise><part id=\"P1\"><measure number=\"1\"><note><duration>x</duration></note></measure></part></score-partwise>",
    ] {
        let r = parse_musicxml(bad);
        assert!(r.is_err(), "expected error for {:?}", String::from_utf8_lossy(bad));
        let msg = r.unwrap_err().to_string();
        assert!(!msg.is_empty(), "error must be readable");
    }
}

#[test]
fn lib_01_rejects_unsupported_root_with_readable_error() {
    let err = parse_musicxml(b"<?xml version=\"1.0\"?><html/>").unwrap_err();
    assert!(matches!(err, ParseError::UnsupportedRoot(ref r) if r == "html"));
}
