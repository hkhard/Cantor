//! LIB-06 (promoted): PDF -> MusicXML via OMR -> timemap -> MIDI.
//! Uses a fake engine so the pipeline is tested without Audiveris.

use cantor_omr::{convert, Input, OmrEngine, OmrError};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const TINY_XML: &str = r#"<?xml version="1.0"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration></note>
  </measure></part>
</score-partwise>"#;

/// Writes TINY_XML as the "recognised" score and records which PDFs it saw.
struct FakeEngine {
    seen: Mutex<Vec<Vec<u8>>>,
    fail: bool,
}

impl OmrEngine for FakeEngine {
    fn name(&self) -> &str {
        "fake"
    }
    fn recognise(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
        self.seen.lock().unwrap().push(std::fs::read(pdf).unwrap());
        if self.fail {
            return Err(OmrError::Engine {
                engine: "fake".into(),
                detail: "no staves found".into(),
            });
        }
        let out = out_dir.join("score.musicxml");
        std::fs::write(&out, TINY_XML).unwrap();
        Ok(out)
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-omr-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn lib_06_pdf_to_musicxml_and_midi() {
    let dir = tmp("pdf");
    let pdf = b"%PDF-1.4 fake".to_vec();
    let engine = FakeEngine {
        seen: Mutex::new(vec![]),
        fail: false,
    };
    let out = convert(
        &engine,
        Input::Pdf {
            title: "Fake".into(),
            bytes: pdf.clone(),
        },
        &dir,
    )
    .unwrap();

    assert_eq!(engine.seen.lock().unwrap().as_slice(), &[pdf]);
    assert_eq!(out.title, "Fake");
    assert!(out.musicxml.exists());
    assert_eq!(out.notes, 1);
    assert_eq!(out.parts, 1);
    let midi = std::fs::read(&out.midi).unwrap();
    assert!(midi.starts_with(b"MThd"), "standard MIDI file written");
    assert!(
        (out.duration_sec - 2.4).abs() < 1e-9,
        "4 beats at default 100 bpm"
    );
}

#[test]
fn lib_06_4sc_embedded_pdf_is_recognised() {
    // Reuse the forScore test helper shape: gzip(bplist{pdfData, "X.pdf|title"}).
    let sc = cantor_omr::testing::fake_4sc("Song.pdf", "Song", b"%PDF-1.4 inside 4sc");
    let dir = tmp("4sc");
    let engine = FakeEngine {
        seen: Mutex::new(vec![]),
        fail: false,
    };
    let inputs = Input::from_file_bytes("Song.4sc", &sc).unwrap();
    assert_eq!(inputs.len(), 1);
    let out = convert(&engine, inputs.into_iter().next().unwrap(), &dir).unwrap();
    assert_eq!(out.title, "Song");
    assert_eq!(engine.seen.lock().unwrap()[0], b"%PDF-1.4 inside 4sc");
}

#[test]
fn lib_06_engine_failure_is_reported_not_panicked() {
    let dir = tmp("fail");
    let engine = FakeEngine {
        seen: Mutex::new(vec![]),
        fail: true,
    };
    let err = convert(
        &engine,
        Input::Pdf {
            title: "X".into(),
            bytes: b"%PDF".to_vec(),
        },
        &dir,
    )
    .unwrap_err();
    assert!(err.to_string().contains("no staves found"), "{err}");
}

#[test]
fn lib_06_rejects_unknown_input() {
    assert!(Input::from_file_bytes("notes.txt", b"hello").is_err());
}

/// Fails on the raw PDF, succeeds on the cleaned image (like real scans).
struct NeedsCleanScan {
    calls: Mutex<Vec<String>>,
}

impl OmrEngine for NeedsCleanScan {
    fn name(&self) -> &str {
        "picky"
    }
    fn recognise(&self, input: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
        let ext = input.extension().unwrap().to_string_lossy().into_owned();
        self.calls.lock().unwrap().push(ext.clone());
        if ext == "pdf" {
            return Err(OmrError::Engine {
                engine: "picky".into(),
                detail: "too low interline".into(),
            });
        }
        let out = out_dir.join("score.musicxml");
        std::fs::write(&out, TINY_XML).unwrap();
        Ok(out)
    }
}

struct FakeCleaner;
impl cantor_omr::Preprocessor for FakeCleaner {
    fn clean(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
        let out = out_dir.join("clean.tif");
        std::fs::copy(pdf, &out)?;
        Ok(out)
    }
}

#[test]
fn lib_06_retries_with_cleaned_scan_when_raw_pdf_fails() {
    let engine = NeedsCleanScan {
        calls: Mutex::new(vec![]),
    };
    let out = cantor_omr::convert_with(
        &engine,
        Some(&FakeCleaner),
        Input::Pdf {
            title: "Scan".into(),
            bytes: b"%PDF scan".to_vec(),
        },
        &tmp("retry"),
    )
    .unwrap();
    assert_eq!(*engine.calls.lock().unwrap(), ["pdf", "tif"]);
    assert!(out.cleaned, "result records that cleanup was needed");
    assert_eq!(out.notes, 1);
}

#[test]
fn lib_06_no_cleanup_when_raw_pdf_works() {
    let engine = FakeEngine {
        seen: Mutex::new(vec![]),
        fail: false,
    };
    let out = cantor_omr::convert_with(
        &engine,
        Some(&FakeCleaner),
        Input::Pdf {
            title: "Clean".into(),
            bytes: b"%PDF".to_vec(),
        },
        &tmp("noretry"),
    )
    .unwrap();
    assert!(!out.cleaned);
    assert_eq!(engine.seen.lock().unwrap().len(), 1);
}

#[test]
fn lib_06_both_attempts_failing_reports_original_error() {
    let engine = FakeEngine {
        seen: Mutex::new(vec![]),
        fail: true,
    };
    let err = cantor_omr::convert_with(
        &engine,
        Some(&FakeCleaner),
        Input::Pdf {
            title: "Bad".into(),
            bytes: b"%PDF".to_vec(),
        },
        &tmp("bothfail"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("no staves found"));
    assert_eq!(engine.seen.lock().unwrap().len(), 2);
}

/// An OMR result with no notes at all (Audiveris found staves but read
/// nothing) counts as a failure, so cleanup is tried.
#[test]
fn lib_06_empty_result_triggers_cleanup() {
    struct EmptyThenGood(Mutex<u32>);
    impl OmrEngine for EmptyThenGood {
        fn name(&self) -> &str {
            "empty"
        }
        fn recognise(&self, _: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
            let mut n = self.0.lock().unwrap();
            *n += 1;
            let xml = if *n == 1 {
                TINY_XML.replace("<pitch><step>A</step><octave>4</octave></pitch>", "<rest/>")
            } else {
                TINY_XML.to_string()
            };
            let out = out_dir.join(format!("s{n}.musicxml"));
            std::fs::write(&out, xml).unwrap();
            Ok(out)
        }
    }
    let engine = EmptyThenGood(Mutex::new(0));
    let out = cantor_omr::convert_with(
        &engine,
        Some(&FakeCleaner),
        Input::Pdf {
            title: "E".into(),
            bytes: b"%PDF".to_vec(),
        },
        &tmp("empty"),
    )
    .unwrap();
    assert!(out.cleaned);
    assert_eq!(out.notes, 1);
}

/// Fewer than one note per measure on average means OMR mostly failed
/// (seen on a real scan: 13 measures, 6 notes). Treat it like an empty result.
#[test]
fn lib_06_sparse_result_triggers_cleanup() {
    let sparse = r#"<?xml version="1.0"?><score-partwise version="4.0">
      <part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list>
      <part id="P1">
        <measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
          <note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration></note></measure>
        <measure number="2"><note><rest/><duration>2</duration></note></measure>
        <measure number="3"><note><rest/><duration>2</duration></note></measure>
        <measure number="4"><note><rest/><duration>2</duration></note></measure>
      </part></score-partwise>"#;
    struct SparseThenGood(Mutex<u32>, &'static str);
    impl OmrEngine for SparseThenGood {
        fn name(&self) -> &str {
            "sparse"
        }
        fn recognise(&self, _: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
            let mut n = self.0.lock().unwrap();
            *n += 1;
            let out = out_dir.join(format!("s{n}.musicxml"));
            std::fs::write(&out, if *n == 1 { self.1 } else { TINY_XML }).unwrap();
            Ok(out)
        }
    }
    let engine = SparseThenGood(Mutex::new(0), sparse);
    let out = cantor_omr::convert_with(
        &engine,
        Some(&FakeCleaner),
        Input::Pdf {
            title: "S".into(),
            bytes: b"%PDF".to_vec(),
        },
        &tmp("sparse"),
    )
    .unwrap();
    assert!(out.cleaned);
}

/// Runs the real Audiveris on a real PDF. Opt-in: needs Audiveris and samples.
/// `CANTOR_AUDIVERIS=/path/to/Audiveris CANTOR_SAMPLES=dir cargo test -- --ignored`
#[test]
#[ignore]
fn lib_06_audiveris_recognises_real_sample() {
    let (Some(bin), Some(samples)) = (
        std::env::var_os("CANTOR_AUDIVERIS"),
        std::env::var_os("CANTOR_SAMPLES"),
    ) else {
        eprintln!("skipped: set CANTOR_AUDIVERIS and CANTOR_SAMPLES");
        return;
    };
    let pdf = std::fs::read(Path::new(&samples).join("En glad calypso om våren TTBB.pdf")).unwrap();
    let engine = cantor_omr::Audiveris::new(bin);
    let out = convert(
        &engine,
        Input::Pdf {
            title: "Calypso".into(),
            bytes: pdf,
        },
        &tmp("real"),
    )
    .unwrap();
    assert!(out.parts >= 2, "TTBB on two staves");
    assert!(out.notes > 100, "got {} notes", out.notes);
}
