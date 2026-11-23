//! LIB-06 (ROADMAP phase 1): the director's one command, `cantor-omr convert`,
//! and `cantor-omr doctor`, run as the real binary with a stand-in Audiveris
//! (a shell script that "exports" a small MXL and `.omr` book). Checks the
//! files per piece (title from the score header), the `.cantorset` and that
//! it imports, plain-language errors in Swedish and English.
#![cfg(unix)]

use cantor_library::bundle::import_bundle;
use cantor_library::store::Store;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-omrcli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn zip_file(path: &Path, files: &[(&str, &[u8])]) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let o = zip::write::SimpleFileOptions::default();
    for (n, b) in files {
        z.start_file(*n, o).unwrap();
        z.write_all(b).unwrap();
    }
    z.finish().unwrap();
}

/// A fake Audiveris: copies `prebuilt.mxl` and `prebuilt.omr` (when present)
/// to `<stem>.mxl|.omr` in `-output`.
fn fake_audiveris(d: &Path, title: &str) -> PathBuf {
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><work><work-title>{title}</work-title></work><part-list><score-part id="P1"><part-name>S</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>{}</measure><measure number="2">{}</measure></part></score-partwise>"#,
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>"#
            .repeat(4),
        r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration></note>"#
            .repeat(4),
    );
    zip_file(
        &d.join("prebuilt.mxl"),
        &[
            ("META-INF/container.xml", br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="score.xml"/></rootfiles></container>"#),
            ("score.xml", xml.as_bytes()),
        ],
    );
    // One sheet, one system, two measures.
    let sheet = r#"<?xml version="1.0" ?><sheet><picture width="2000" height="3000"/><page id="1"><system id="1"><stack id="1" left="200" right="1000"/><stack id="2" left="1000" right="1800"/><barline staff="1" id="1"><bounds x="0" y="500" w="3" h="40"/></barline><barline staff="2" id="2"><bounds x="0" y="900" w="3" h="40"/></barline></system></page></sheet>"#;
    zip_file(
        &d.join("prebuilt.omr"),
        &[
            (
                "book.xml",
                br#"<?xml version="1.0" ?><book><sheet number="1"><page id="1"/></sheet></book>"#,
            ),
            ("sheet#1/sheet#1.xml", sheet.as_bytes()),
        ],
    );
    let p = d.join("audiveris");
    std::fs::write(
        &p,
        format!(
            "#!/bin/sh\nprev=\"\"; out=\"\"; for a in \"$@\"; do [ \"$prev\" = \"-output\" ] && out=\"$a\"; prev=\"$a\"; last=\"$a\"; done\nstem=$(basename \"$last\"); stem=\"${{stem%.*}}\"; cp \"{0}/prebuilt.mxl\" \"$out/$stem.mxl\"; cp \"{0}/prebuilt.omr\" \"$out/$stem.omr\"\n",
            d.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn cli(audiveris: &Path, lang: &str, args: &[&str]) -> (Output, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_cantor-omr"))
        .args(args)
        .env("CANTOR_AUDIVERIS", audiveris)
        .env("CANTOR_CLEAN_SCAN", "/nonexistent-cleaner")
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    (out, s)
}

#[test]
fn lib_06_convert_writes_titled_files_and_an_importable_set() {
    let d = tmp("convert");
    let fake = fake_audiveris(&d, "Ave verum");
    let pdf = d.join("scan_0042.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj").unwrap();
    let out = d.join("out");
    let (run, stdout) = cli(
        &fake,
        "en_GB.UTF-8",
        &[
            "convert",
            pdf.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--set",
            "Spring concert",
        ],
    );
    assert!(run.status.success(), "{stdout}");
    assert!(
        stdout.contains("✓ Ave verum: 1 parts, 2 measures"),
        "{stdout}"
    );
    assert!(stdout.contains("cursor follows the PDF"), "{stdout}");
    assert!(stdout.contains("AirDrop"), "{stdout}");
    for f in [
        "Ave verum.mxl",
        "Ave verum.pdf",
        "Ave verum.measures.json",
        "Ave verum.mid",
    ] {
        assert!(out.join(f).exists(), "{f} missing: {stdout}");
    }
    let map: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(out.join("Ave verum.measures.json")).unwrap())
            .unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(
        std::fs::read(out.join("Ave verum.pdf")).unwrap(),
        std::fs::read(&pdf).unwrap()
    );

    // The set imports into a fresh library with all three Cantor files.
    let bundle = std::fs::read(out.join("Spring concert.cantorset")).unwrap();
    let lib = d.join("lib");
    let mut store = Store::open(&lib);
    let imported = import_bundle(&lib, &mut store, &lib.join("ann"), &bundle).unwrap();
    assert_eq!(imported.score_ids.len(), 1);
    let set = &store.setlists()[0];
    assert_eq!(set.name, "Spring concert");
    let names: Vec<String> = std::fs::read_dir(&lib)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    for ext in [".mxl", ".pdf", ".measures.json"] {
        assert!(names.iter().any(|n| n.ends_with(ext)), "{ext}: {names:?}");
    }
}

#[test]
fn lib_06_convert_keeps_forscore_titles() {
    let d = tmp("forscore");
    let fake = fake_audiveris(&d, "Header title");
    let pdf = b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj";
    let sc = cantor_omr::testing::fake_4sc("a.pdf", "Mitt val", pdf);
    let f = d.join("a.4sc");
    std::fs::write(&f, sc).unwrap();
    let out = d.join("out");
    let (run, stdout) = cli(
        &fake,
        "sv_SE.UTF-8",
        &["convert", f.to_str().unwrap(), "-o", out.to_str().unwrap()],
    );
    assert!(run.status.success(), "{stdout}");
    assert!(stdout.contains("Skriver till"), "{stdout}");
    assert!(
        stdout.contains("✓ Mitt val: 1 stämmor, 2 takter"),
        "{stdout}"
    );
    assert!(stdout.contains("1 klara, 0 misslyckades"), "{stdout}");
    assert!(out.join("Mitt val.mxl").exists());
    assert!(!out.join("Header title.mxl").exists());
}

#[test]
fn lib_06_convert_explains_failures_and_missing_audiveris() {
    let d = tmp("fail");
    // An Audiveris that exports nothing.
    let bad = d.join("audiveris");
    std::fs::write(&bad, "#!/bin/sh\necho 'WARN no staves found'\n").unwrap();
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pdf = d.join("Dålig.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj").unwrap();
    let doc = d.join("notes.doc");
    std::fs::write(&doc, b"hello").unwrap();
    let (run, stdout) = cli(
        &bad,
        "sv_SE.UTF-8",
        &[
            "convert",
            pdf.to_str().unwrap(),
            doc.to_str().unwrap(),
            d.join("borta.pdf").to_str().unwrap(),
            "--set",
            "Tom",
        ],
    );
    assert_eq!(run.status.code(), Some(1), "{stdout}");
    assert!(
        stdout.contains("✗ Dålig: kunde inte läsa noterna"),
        "{stdout}"
    );
    assert!(stdout.contains("bara .pdf, .4sc och .4ss"), "{stdout}");
    assert!(stdout.contains("filen finns inte"), "{stdout}");
    assert!(stdout.contains("0 klara, 3 misslyckades"), "{stdout}");
    // Default output folder: "Cantor" next to the first file; no empty set.
    assert!(d.join("Cantor").is_dir());
    assert!(!d.join("Cantor/Tom.cantorset").exists());

    let (run, stdout) = cli(
        &d.join("no-such-audiveris"),
        "en_US.UTF-8",
        &["convert", pdf.to_str().unwrap()],
    );
    assert_eq!(run.status.code(), Some(2));
    assert!(stdout.contains("Audiveris was not found"), "{stdout}");
    assert!(
        stdout.contains("github.com/Audiveris/audiveris/releases"),
        "{stdout}"
    );
    // Old form still works and unknown commands print usage.
    let (run, _) = cli(
        &bad,
        "C",
        &[d.join("o").to_str().unwrap(), pdf.to_str().unwrap()],
    );
    assert_eq!(run.status.code(), Some(1));
    let (run, stdout) = cli(&bad, "C", &["--nope"]);
    assert_eq!(run.status.code(), Some(2));
    assert!(stdout.contains("Usage"), "{stdout}");
}

#[test]
fn lib_06_doctor_lists_what_is_missing() {
    let d = tmp("doctor");
    let td = d.join("tessdata");
    std::fs::create_dir_all(&td).unwrap();
    std::fs::write(td.join("eng.traineddata"), b"x").unwrap();
    let fake = fake_audiveris(&d, "x");
    let doctor = |bin: &Path, lang: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_cantor-omr"))
            .arg("doctor")
            .env("CANTOR_AUDIVERIS", bin)
            .env("CANTOR_TESSDATA", &td)
            .env("CANTOR_OCR_LANGUAGES", "swe+eng")
            .env("CANTOR_CLEAN_SCAN", "/bin/echo")
            .env("LANG", lang)
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .output()
            .unwrap();
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    };
    let (code, s) = doctor(&fake, "en_US.UTF-8");
    assert_eq!(code, Some(0), "{s}");
    assert!(s.contains("✓ Audiveris"), "{s}");
    assert!(s.contains("✓ Text language eng"), "{s}");
    assert!(s.contains("! Text language swe missing"), "{s}");
    assert!(s.contains("swe.traineddata"), "{s}");
    assert!(s.contains("✓ Scan cleanup"), "{s}");
    assert!(s.contains("Ready"), "{s}");
    let (code, s) = doctor(&d.join("gone"), "sv_SE.UTF-8");
    assert_eq!(code, Some(2), "{s}");
    assert!(s.contains("✗ Audiveris"), "{s}");
    assert!(s.contains("Ladda ner"), "{s}");
    assert!(s.contains("kör doctor igen"), "{s}");
}

/// Opt-in end-to-end check of a set made by a real Audiveris run:
/// `CANTOR_E2E_SET=<file.cantorset> cargo test -p cantor-omr --test lib_06_cli -- --ignored`.
#[test]
#[ignore]
fn lib_06_real_cantorset_imports() {
    let Some(p) = std::env::var_os("CANTOR_E2E_SET") else {
        return;
    };
    let lib = tmp("e2e");
    let mut store = Store::open(&lib);
    let got = import_bundle(
        &lib,
        &mut store,
        &lib.join("ann"),
        &std::fs::read(p).unwrap(),
    )
    .unwrap();
    assert_eq!(got.score_ids.len(), 1);
    for ext in [".mxl", ".pdf", ".measures.json"] {
        assert!(
            std::fs::read_dir(&lib)
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(ext)),
            "{ext}"
        );
    }
}
