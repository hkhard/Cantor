//! LIB-06: the process wrappers around Audiveris and the scan cleaner, tested
//! with stand-in scripts (no Java, no Python needed): arguments passed, the
//! exported MusicXML found, failures summarised for the user, TIFF page
//! counts for cleaned scans.
#![cfg(unix)]

use cantor_omr::{count_pages, Audiveris, OmrEngine, Preprocessor, ScriptCleaner};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-omrproc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn lib_06_audiveris_wrapper_finds_the_exported_mxl() {
    let d = tmp("ok");
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    // Fake Audiveris: records its arguments, writes <stem>.mxl into -output.
    let fake = script(
        &d,
        "audiveris",
        r#"echo "$@" > "$(dirname "$0")/args.txt"
prev=""; out=""; for a in "$@"; do [ "$prev" = "-output" ] && out="$a"; prev="$a"; last="$a"; done
stem=$(basename "$last" .pdf); echo PK > "$out/$stem.mxl""#,
    );
    let a = Audiveris::new(&fake).with_languages("swe+eng");
    assert_eq!(a.name(), "audiveris");
    let pdf = d.join("Olaf.pdf");
    std::fs::write(
        &pdf,
        b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj\n2 0 obj << /Type /Page >> endobj",
    )
    .unwrap();
    assert_eq!(a.page_count(&pdf), Some(2));
    let got = a.recognise(&pdf, &out).unwrap();
    assert_eq!(got, out.join("Olaf.mxl"));
    let args = std::fs::read_to_string(d.join("args.txt")).unwrap();
    assert!(args.contains("-batch -transcribe -export"), "{args}");
    assert!(
        args.contains("Language.defaultSpecification=swe+eng"),
        "{args}"
    );
    // One page only.
    let got = a.recognise_page(&pdf, 2, &out).unwrap();
    assert!(got.ends_with("Olaf.mxl"));
    let args = std::fs::read_to_string(d.join("args.txt")).unwrap();
    assert!(args.contains("-sheets 2"), "{args}");
}

#[test]
fn lib_06_audiveris_failure_is_explained() {
    let d = tmp("fail");
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let pdf = d.join("Supa.pdf");
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    // Low-resolution scan: the user gets the 300 DPI advice.
    let low = script(&d, "low", "echo 'WARN too low interline value' >&2");
    let e = Audiveris::new(&low)
        .recognise(&pdf, &out)
        .unwrap_err()
        .to_string();
    assert!(e.contains("300 DPI"), "{e}");
    // Other failures: the last warning or error line.
    let other = script(
        &d,
        "other",
        "echo 'INFO start'; echo 'ERROR Sheet 1 has no staff'; echo done",
    );
    let e = Audiveris::new(&other)
        .recognise(&pdf, &out)
        .unwrap_err()
        .to_string();
    assert!(e.contains("no staff"), "{e}");
    // Silent failure.
    let silent = script(&d, "silent", "true");
    let e = Audiveris::new(&silent)
        .recognise(&pdf, &out)
        .unwrap_err()
        .to_string();
    assert!(e.contains("no MusicXML"), "{e}");
}

#[test]
fn lib_06_locate_uses_cantor_audiveris() {
    let d = tmp("loc");
    let fake = script(&d, "audiveris", "true");
    std::env::set_var("CANTOR_AUDIVERIS", &fake);
    assert!(Audiveris::locate().is_some());
    std::env::remove_var("CANTOR_AUDIVERIS");
}

#[test]
fn lib_06_scan_cleaner_wrapper() {
    let d = tmp("clean");
    let pdf = d.join("Scan.pdf");
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    // Fake cleaner writes the output TIFF given as the last argument.
    let ok = script(
        &d,
        "clean_ok",
        r#"for a in "$@"; do last="$a"; done; printf 'II*\0' > "$last""#,
    );
    std::env::set_var("CANTOR_CLEAN_SCAN", format!("{} --dpi 300", ok.display()));
    let c = ScriptCleaner::locate(&d).expect("located from env");
    let tif = c.clean(&pdf, &d).unwrap();
    assert_eq!(tif, d.join("Scan.tif"));
    let bad = script(&d, "clean_bad", "echo 'boom: no pages' >&2; exit 3");
    std::env::set_var("CANTOR_CLEAN_SCAN", bad.display().to_string());
    let e = ScriptCleaner::locate(&d)
        .unwrap()
        .clean(&pdf, &d)
        .unwrap_err()
        .to_string();
    assert!(e.contains("boom"), "{e}");
    std::env::remove_var("CANTOR_CLEAN_SCAN");
    // Without env and without the venv: not available.
    assert!(ScriptCleaner::locate(&d).is_none());
}

#[test]
fn lib_06_tiff_page_count_both_byte_orders() {
    // Two IFDs, no entries: header, IFD1 at 8 -> IFD2 at 14 -> end.
    let le: Vec<u8> = [
        &b"II*\0"[..],
        &8u32.to_le_bytes(),
        &0u16.to_le_bytes(),
        &14u32.to_le_bytes(),
        &0u16.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(count_pages(&le), Some(2));
    let be: Vec<u8> = [
        &b"MM\0*"[..],
        &8u32.to_be_bytes(),
        &0u16.to_be_bytes(),
        &0u32.to_be_bytes(),
    ]
    .concat();
    assert_eq!(count_pages(&be), Some(1));
    assert_eq!(count_pages(b"GIF89a"), None);
    assert_eq!(count_pages(b"%PDF-1.4 no pages"), None);
}

#[test]
fn lib_06_cli_converts_and_reports() {
    // The `cantor-omr` command with a fake Audiveris that "exports" a small
    // valid MXL, plus one missing input file.
    let d = tmp("cli");
    let xml = r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list><score-part id="P1"><part-name>S</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>"#
        .to_string()
        + &r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>"#.repeat(40)
        + "</measure></part></score-partwise>";
    std::fs::write(d.join("score.xml"), xml).unwrap();
    let mxl = d.join("prebuilt.mxl");
    {
        use std::io::Write;
        let f = std::fs::File::create(&mxl).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("META-INF/container.xml", o).unwrap();
        z.write_all(br#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="score.xml"/></rootfiles></container>"#).unwrap();
        z.start_file("score.xml", o).unwrap();
        z.write_all(&std::fs::read(d.join("score.xml")).unwrap())
            .unwrap();
        z.finish().unwrap();
    }
    let fake = script(
        &d,
        "audiveris",
        &format!(
            r#"prev=""; out=""; for a in "$@"; do [ "$prev" = "-output" ] && out="$a"; prev="$a"; last="$a"; done
stem=$(basename "$last"); stem="${{stem%.*}}"; cp "{}" "$out/$stem.mxl""#,
            mxl.display()
        ),
    );
    let pdf = d.join("Test piece.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n1 0 obj << /Type /Page >> endobj").unwrap();
    let out = d.join("out");
    let bin = env!("CARGO_BIN_EXE_cantor-omr");
    let run = std::process::Command::new(bin)
        .arg(&out)
        .arg(&pdf)
        .arg(d.join("missing.pdf"))
        .env("CANTOR_AUDIVERIS", &fake)
        .env("CANTOR_CLEAN_SCAN", "/nonexistent-cleaner")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("ok   Test piece"),
        "{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(stdout.contains("1 converted, 1 failed"), "{stdout}");
    assert!(
        !run.status.success(),
        "a failed file makes the exit code non-zero"
    );
    assert!(std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "mid")));
    // Usage error.
    let usage = std::process::Command::new(bin).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
}
