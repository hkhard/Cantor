//! LIB-01 / LIB-02 / LIB-07: importing files into the on-device library.
//!
//! A score in the library is a stem with any of: `<stem>.mxl` or
//! `<stem>.musicxml` (plays), `<stem>.pdf` (shown), `<stem>.measures.json`
//! (cursor on the PDF). Import never runs OMR (that needs Audiveris on a
//! desktop); a PDF-only score is readable but has no sound until converted.

use cantor_library::import::{import_file, list_library, Imported, LibraryEntry};
use std::path::Path;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-lib-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const XML: &str = r#"<?xml version="1.0"?><score-partwise version="4.0"><work><work-title>Que?</work-title></work>
<part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration></note></measure></part></score-partwise>"#;

#[test]
fn lib_01_musicxml_imported_and_listed_as_playable() {
    let d = tmp("xml");
    let got = import_file(&d, "Que medley.musicxml", XML.as_bytes()).unwrap();
    assert_eq!(
        got,
        vec![Imported {
            stem: "Que medley".into(),
            playable: true,
            pdf: false
        }]
    );
    let lib = list_library(&[d.as_path()]);
    assert_eq!(
        lib,
        vec![LibraryEntry {
            title: "Que medley".into(),
            stem: "Que medley".into(),
            dir: d.clone(),
            score_file: Some("Que medley.musicxml".into()),
            pdf_file: None,
        }]
    );
}

#[test]
fn lib_01_invalid_musicxml_is_rejected_with_a_message() {
    let d = tmp("bad");
    let err = import_file(&d, "broken.musicxml", b"<score-partwise><part").unwrap_err();
    assert!(!err.is_empty());
    assert!(
        std::fs::read_dir(&d).unwrap().next().is_none(),
        "nothing written"
    );
}

#[test]
fn lib_02_pdf_imported_as_readable_score() {
    let d = tmp("pdf");
    let got = import_file(&d, "Psalm.pdf", b"%PDF-1.4\n%fake\n").unwrap();
    assert!(got[0].pdf);
    assert!(!got[0].playable);
    let lib = list_library(&[d.as_path()]);
    assert_eq!(lib[0].pdf_file.as_deref(), Some("Psalm.pdf"));
    assert_eq!(lib[0].score_file, None);
}

#[test]
fn lib_07_4sc_imports_its_pdf_under_its_title() {
    let d = tmp("4sc");
    let bytes =
        cantor_library::forscore::testing::fake_4sc("Holst.pdf", "In the bleak", b"%PDF-1.4\n");
    let got = import_file(&d, "x.4sc", &bytes).unwrap();
    assert_eq!(got[0].stem, "In the bleak");
    assert!(d.join("In the bleak.pdf").exists());
}

#[test]
fn lib_01_same_name_does_not_overwrite() {
    let d = tmp("dup");
    import_file(&d, "Que.musicxml", XML.as_bytes()).unwrap();
    let second = import_file(&d, "Que.musicxml", XML.as_bytes()).unwrap();
    assert_eq!(second[0].stem, "Que (2)");
    assert_eq!(list_library(&[d.as_path()]).len(), 2);
}

#[test]
fn lib_01_names_are_made_safe() {
    let d = tmp("safe");
    let got = import_file(&d, "../../etc/a:b?.musicxml", XML.as_bytes()).unwrap();
    assert_eq!(got[0].stem, "a_b_");
    assert!(Path::new(&d).join("a_b_.musicxml").exists());
}

#[test]
fn lib_01_library_merges_dirs_and_pairs_pdf_with_score() {
    let bundled = tmp("bundled");
    let user = tmp("user");
    std::fs::write(bundled.join("Olaf.mxl"), b"PK").unwrap();
    std::fs::write(bundled.join("Olaf.pdf"), b"%PDF").unwrap();
    std::fs::write(bundled.join("Olaf.measures.json"), b"[]").unwrap();
    std::fs::write(user.join("Mine.pdf"), b"%PDF").unwrap();
    let lib = list_library(&[bundled.as_path(), user.as_path()]);
    let titles: Vec<_> = lib.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Mine", "Olaf"]);
    assert_eq!(lib[1].score_file.as_deref(), Some("Olaf.mxl"));
    assert_eq!(lib[1].pdf_file.as_deref(), Some("Olaf.pdf"));
}
