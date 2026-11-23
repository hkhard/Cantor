//! LIB-05: a MusicXML and a PDF of the same piece that arrived separately
//! (different file names) are suggested as one score and can be combined,
//! so the play cursor runs on the PDF.

use cantor_library::import::{combine, pair_suggestions, Candidate};

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-comb-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cand(stem: &str, title: &str, playable: bool, pdf: bool, text: &str) -> Candidate {
    Candidate {
        stem: stem.into(),
        title: title.into(),
        playable,
        pdf,
        pdf_text: text.into(),
    }
}

#[test]
fn lib_05_suggests_pairs_by_title_or_first_page_text() {
    let c = vec![
        // MusicXML "Lacrymosa" + a PDF named after a download id whose page 1 says Lacrymosa.
        cand("mozart-lacrymosa", "Lacrymosa", true, false, ""),
        cand(
            "score-b503fb",
            "score-b503fb",
            false,
            true,
            "Wolfgang Amadeus Mozart Lacrymosa From Requiem in D Minor",
        ),
        // Same title, different file names.
        cand("anchor_mxl", "The Anchor Song", true, false, ""),
        cand("Anchor song print", "The Anchor Song", false, true, ""),
        // A PDF that matches nothing, and a complete score.
        cand("Unrelated", "Unrelated", false, true, "Kung Liljekonvalje"),
        cand("Island", "Island", true, true, ""),
    ];
    let p = pair_suggestions(&c);
    assert_eq!(
        p,
        vec![
            ("anchor_mxl".to_string(), "Anchor song print".to_string()),
            ("mozart-lacrymosa".to_string(), "score-b503fb".to_string()),
        ]
    );
    // Short titles do not match by substring ("Ave" is in many texts).
    let c = vec![
        cand("ave", "Ave", true, false, ""),
        cand("x", "x", false, true, "Ave verum corpus"),
    ];
    assert!(pair_suggestions(&c).is_empty());
    // One PDF is never offered to two scores.
    let c = vec![
        cand("a", "Gloria", true, false, ""),
        cand("b", "Gloria", true, false, ""),
        cand("p", "Gloria", false, true, ""),
    ];
    assert_eq!(pair_suggestions(&c).len(), 1);
}

#[test]
fn lib_05_combine_moves_the_pdf_and_its_files_to_the_score() {
    let d = tmp("move");
    std::fs::write(d.join("lacrymosa.mxl"), b"PK").unwrap();
    std::fs::write(d.join("score-b5.pdf"), b"%PDF-1.4").unwrap();
    std::fs::write(d.join("score-b5.measures.json"), b"[]").unwrap();
    combine(&d, "lacrymosa", "score-b5").unwrap();
    assert!(d.join("lacrymosa.pdf").exists());
    assert!(d.join("lacrymosa.measures.json").exists());
    assert!(!d.join("score-b5.pdf").exists());
    // Refuses to overwrite: the score already has a PDF now.
    std::fs::write(d.join("other.pdf"), b"%PDF-1.4").unwrap();
    assert!(combine(&d, "lacrymosa", "other").is_err());
    assert!(d.join("other.pdf").exists());
    // Refuses when the target has no score file.
    assert!(combine(&d, "nothing", "other").is_err());
}

#[test]
fn lib_05_mscz_is_kept_with_its_score_not_played() {
    use cantor_library::import::{import_file, list_library};
    let d = tmp("mscz");
    let read = |p: &str| {
        std::fs::read(format!(
            "{}/../../fixtures/scores/the-anchor-song/{p}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };
    // Alone it cannot be used: the message says what to do.
    let err = import_file(&d, "the-anchor-song.mscz", &read("the-anchor-song.mscz")).unwrap_err();
    assert!(err.contains("MusicXML"), "{err}");
    // With its MusicXML it is stored beside it; the library still lists one score.
    import_file(&d, "the-anchor-song.mxl", &read("the-anchor-song.mxl")).unwrap();
    import_file(&d, "the-anchor-song.pdf", &read("the-anchor-song.pdf")).unwrap();
    let got = import_file(&d, "the-anchor-song.mscz", &read("the-anchor-song.mscz")).unwrap();
    assert!(got[0].playable && got[0].pdf);
    assert!(d.join("the-anchor-song.mscz").exists());
    let lib = list_library(&[d.as_path()]);
    assert_eq!(lib.len(), 1);
    assert_eq!(lib[0].title, "The Anchor Song");
    // Not a zip: rejected.
    assert!(import_file(&d, "x.mscz", b"nope").is_err());
}
