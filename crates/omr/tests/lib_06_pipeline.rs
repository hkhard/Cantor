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

/// Audiveris aborts a whole book when one page has no staves (a page of
/// verses). The pipeline then recognises page by page, skips failed pages and
/// joins the rest in page order.
#[test]
fn lib_06_page_by_page_when_book_fails() {
    struct PagedEngine {
        calls: Mutex<Vec<Option<u32>>>,
    }
    impl OmrEngine for PagedEngine {
        fn name(&self) -> &str {
            "paged"
        }
        fn recognise(&self, _: &Path, _: &Path) -> Result<PathBuf, OmrError> {
            self.calls.lock().unwrap().push(None);
            Err(OmrError::Engine {
                engine: "paged".into(),
                detail: "page 3: no staves".into(),
            })
        }
        fn page_count(&self, _: &Path) -> Option<u32> {
            Some(3)
        }
        fn recognise_page(&self, _: &Path, page: u32, out_dir: &Path) -> Result<PathBuf, OmrError> {
            self.calls.lock().unwrap().push(Some(page));
            if page == 3 {
                return Err(OmrError::Engine {
                    engine: "paged".into(),
                    detail: "no staves".into(),
                });
            }
            let step = if page == 1 { "C" } else { "D" };
            let xml = TINY_XML.replace("<step>A</step>", &format!("<step>{step}</step>"));
            let out = out_dir.join(format!("p{page}.musicxml"));
            std::fs::write(&out, xml).unwrap();
            Ok(out)
        }
    }
    let engine = PagedEngine {
        calls: Mutex::new(vec![]),
    };
    let dir = tmp("paged");
    let out = cantor_omr::convert_with(
        &engine,
        None,
        Input::Pdf {
            title: "Nymf".into(),
            bytes: b"%PDF".to_vec(),
        },
        &dir,
    )
    .unwrap();
    assert_eq!(
        *engine.calls.lock().unwrap(),
        [None, Some(1), Some(2), Some(3)]
    );
    assert_eq!(out.measures, 2, "one measure from each good page");
    assert_eq!(out.skipped_pages, vec![3]);
    let s = cantor_score::parse_score_file(&std::fs::read(&out.musicxml).unwrap()).unwrap();
    let pitches: Vec<_> = s.parts[0]
        .measures
        .iter()
        .map(|m| m.events[0].kind.midi())
        .collect();
    assert_eq!(pitches, [Some(60), Some(62)], "pages joined in order");
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

#[test]
fn lib_06_count_pages_pdf_and_tiff() {
    assert_eq!(
        cantor_omr::count_pages(b"%PDF-1.4 1 0 obj <</Type /Pages /Count 2>> 2 0 obj <</Type /Page>> 3 0 obj <</Type/Page>>"),
        Some(2)
    );
    // Little-endian TIFF with two IFDs (0 entries each).
    let mut t = b"II*\0".to_vec();
    t.extend(8u32.to_le_bytes()); // first IFD at 8
    t.extend(0u16.to_le_bytes()); // 0 entries
    t.extend(14u32.to_le_bytes()); // next IFD at 14
    t.extend(0u16.to_le_bytes());
    t.extend(0u32.to_le_bytes()); // end
    assert_eq!(cantor_omr::count_pages(&t), Some(2));
    assert_eq!(cantor_omr::count_pages(b"garbage"), None);
}

/// VOX-01 needs lyrics; Audiveris reads them with Tesseract. Swedish and
/// English are requested by default so å/ä/ö survive (seen on "Är jag född").
#[test]
fn lib_06_audiveris_requests_swedish_and_english_ocr() {
    let a =
        cantor_omr::Audiveris::new("/bin/false").with_languages(cantor_omr::DEFAULT_OCR_LANGUAGES);
    let args: Vec<String> = a
        .args(Some(2), Path::new("/out"))
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        [
            "-batch",
            "-transcribe",
            "-export",
            "-option",
            "org.audiveris.omr.text.Language.defaultSpecification=swe+eng",
            "-sheets",
            "2",
            "-output",
            "/out",
            "--"
        ]
    );
}
