//! PDF -> MusicXML via optical music recognition, then timemap and MIDI.
//!
//! forScore files (.4sc/.4ss) and plain PDFs carry no note data, so Cantor
//! reads the notes from the page images with an OMR engine (Audiveris by
//! default, PRD section 6) and continues from MusicXML like any other score.
//!
//! The engine is a trait so tests run without Audiveris and so a server-side
//! engine can be used on iPad/phone where a Java sidecar is not possible.

use cantor_library::forscore::{parse_4sc, parse_4ss, SetlistEntry};
use cantor_score::{parse_score_file, write_musicxml, EventKind};
use cantor_timeline::{build_timemap, smf::to_smf};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum OmrError {
    #[error("{engine}: {detail}")]
    Engine { engine: String, detail: String },
    #[error("unsupported input {0:?}: expected .pdf, .4sc or .4ss")]
    UnsupportedInput(String),
    #[error("forScore file: {0}")]
    Forscore(#[from] cantor_library::forscore::ForscoreError),
    #[error("OMR output is not readable MusicXML: {0}")]
    Score(#[from] cantor_score::ParseError),
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
}

/// An optical music recognition engine: one PDF in, one MusicXML/MXL file out.
pub trait OmrEngine {
    fn name(&self) -> &str;
    /// Recognise `pdf` and write the result under `out_dir`; return its path.
    fn recognise(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError>;
    /// Number of pages, if the engine can work page by page.
    fn page_count(&self, _pdf: &Path) -> Option<u32> {
        None
    }
    /// Recognise a single 1-based page. Used when the whole book fails.
    fn recognise_page(
        &self,
        _pdf: &Path,
        _page: u32,
        _out_dir: &Path,
    ) -> Result<PathBuf, OmrError> {
        Err(OmrError::Engine {
            engine: self.name().into(),
            detail: "page-by-page recognition not supported".into(),
        })
    }
}

/// Audiveris CLI (`-batch -transcribe -export`).
pub struct Audiveris {
    bin: PathBuf,
    /// Tesseract languages for lyrics and text, e.g. `swe+eng`.
    languages: String,
}

/// Default OCR languages: Swedish and English (PRD Q4: English at launch,
/// Swedish second; the first users' choir library is Swedish).
pub const DEFAULT_OCR_LANGUAGES: &str = "swe+eng";

impl Audiveris {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            bin: bin.into(),
            languages: std::env::var("CANTOR_OCR_LANGUAGES")
                .unwrap_or_else(|_| DEFAULT_OCR_LANGUAGES.to_string()),
        }
    }

    pub fn with_languages(mut self, languages: impl Into<String>) -> Self {
        self.languages = languages.into();
        self
    }

    /// Command-line arguments for one run (without the input path).
    pub fn args(&self, sheet: Option<u32>, out_dir: &Path) -> Vec<std::ffi::OsString> {
        let mut a: Vec<std::ffi::OsString> =
            vec!["-batch".into(), "-transcribe".into(), "-export".into()];
        if !self.languages.is_empty() {
            a.push("-option".into());
            a.push(
                format!(
                    "org.audiveris.omr.text.Language.defaultSpecification={}",
                    self.languages
                )
                .into(),
            );
        }
        if let Some(s) = sheet {
            a.push("-sheets".into());
            a.push(s.to_string().into());
        }
        a.push("-output".into());
        a.push(out_dir.into());
        a.push("--".into());
        a
    }

    /// `$CANTOR_AUDIVERIS`, else the usual install locations on macOS/Linux.
    pub fn locate() -> Option<Self> {
        if let Some(p) = std::env::var_os("CANTOR_AUDIVERIS") {
            return Some(Self::new(p));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        [
            home.join(".local/opt/Audiveris.app/Contents/MacOS/Audiveris"),
            PathBuf::from("/Applications/Audiveris.app/Contents/MacOS/Audiveris"),
            PathBuf::from("/opt/audiveris/bin/Audiveris"),
        ]
        .into_iter()
        .find(|p| p.exists())
        .map(Self::new)
    }
}

impl OmrEngine for Audiveris {
    fn name(&self) -> &str {
        "audiveris"
    }

    fn recognise(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
        self.run(pdf, None, out_dir)
    }

    fn page_count(&self, file: &Path) -> Option<u32> {
        let bytes = std::fs::read(file).ok()?;
        count_pages(&bytes)
    }

    fn recognise_page(&self, file: &Path, page: u32, out_dir: &Path) -> Result<PathBuf, OmrError> {
        self.run(file, Some(page), out_dir)
    }
}

/// Page count of a PDF (`/Type /Page` objects) or TIFF (IFD chain).
pub fn count_pages(bytes: &[u8]) -> Option<u32> {
    if bytes.starts_with(b"%PDF") {
        let pat = b"/Type";
        let mut n = 0u32;
        let mut i = 0;
        while let Some(pos) = bytes[i..].windows(pat.len()).position(|w| w == pat) {
            let rest = &bytes[i + pos + pat.len()..];
            let rest = &rest[rest.iter().take_while(|b| b.is_ascii_whitespace()).count()..];
            if rest.starts_with(b"/Page") && !rest.starts_with(b"/Pages") {
                n += 1;
            }
            i += pos + pat.len();
        }
        return (n > 0).then_some(n);
    }
    let le = bytes.starts_with(b"II*\0");
    if !le && !bytes.starts_with(b"MM\0*") {
        return None;
    }
    let u16_at = |o: usize| -> Option<usize> {
        let b = bytes.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        } as usize)
    };
    let u32_at = |o: usize| -> Option<usize> {
        let b: [u8; 4] = bytes.get(o..o + 4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        } as usize)
    };
    let mut off = u32_at(4)?;
    let mut n = 0u32;
    while off != 0 && n < 10_000 {
        n += 1;
        let entries = u16_at(off)?;
        off = u32_at(off + 2 + entries * 12)?;
    }
    Some(n)
}

impl Audiveris {
    fn run(&self, input: &Path, sheet: Option<u32>, out_dir: &Path) -> Result<PathBuf, OmrError> {
        let mut cmd = Command::new(&self.bin);
        cmd.args(self.args(sheet, out_dir));
        let out = cmd.arg(input).output()?;
        let log = String::from_utf8_lossy(&out.stdout).into_owned()
            + &String::from_utf8_lossy(&out.stderr);
        // Audiveris names the export after the input stem; multi-movement
        // books add .mvtN. Take the first .mxl found.
        let stem = input
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut found: Vec<PathBuf> = std::fs::read_dir(out_dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "mxl"))
            .filter(|p| {
                p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().starts_with(&stem))
            })
            .collect();
        found.sort();
        found.into_iter().next().ok_or_else(|| OmrError::Engine {
            engine: "audiveris".into(),
            detail: summarise_failure(&log),
        })
    }
}

/// Pull the most useful line out of an Audiveris log for the user.
fn summarise_failure(log: &str) -> String {
    if log.contains("too low interline value") {
        return "no readable staves: the scan resolution is too low (Audiveris needs about 300 DPI)".into();
    }
    log.lines()
        .rev()
        .find(|l| l.contains("WARN") || l.contains("ERROR") || l.contains("Exception"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "no MusicXML produced".into())
}

/// Image cleanup applied to scans when OMR on the raw PDF fails.
pub trait Preprocessor {
    /// Produce a cleaned image file (e.g. multi-page TIFF) from `pdf` in `out_dir`.
    fn clean(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError>;
}

/// Runs `tools/omr/clean_scan.py` (or `$CANTOR_CLEAN_SCAN`): 300 DPI render,
/// border crop, halftone blur, Otsu threshold, speck and edge-bar removal.
pub struct ScriptCleaner {
    program: PathBuf,
    args: Vec<String>,
}

impl ScriptCleaner {
    pub fn locate(repo_root: &Path) -> Option<Self> {
        if let Some(cmd) = std::env::var_os("CANTOR_CLEAN_SCAN") {
            let cmd = cmd.to_string_lossy().into_owned();
            let mut parts = cmd.split_whitespace().map(str::to_string);
            let program = PathBuf::from(parts.next()?);
            return Some(Self {
                program,
                args: parts.collect(),
            });
        }
        let py = repo_root.join("tools/omr/.venv/bin/python");
        let script = repo_root.join("tools/omr/clean_scan.py");
        (py.exists() && script.exists()).then(|| Self {
            program: py,
            args: vec![script.to_string_lossy().into_owned()],
        })
    }
}

impl Preprocessor for ScriptCleaner {
    fn clean(&self, pdf: &Path, out_dir: &Path) -> Result<PathBuf, OmrError> {
        let stem = pdf
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let out = out_dir.join(format!("{stem}.tif"));
        let res = Command::new(&self.program)
            .args(&self.args)
            .arg(pdf)
            .arg(&out)
            .output()?;
        if !res.status.success() || !out.exists() {
            return Err(OmrError::Engine {
                engine: "clean_scan".into(),
                detail: String::from_utf8_lossy(&res.stderr)
                    .lines()
                    .last()
                    .unwrap_or("failed")
                    .to_string(),
            });
        }
        Ok(out)
    }
}

/// One PDF to recognise, with the title to show the user.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Pdf { title: String, bytes: Vec<u8> },
}

impl Input {
    /// Expand a file into PDFs: a .pdf is one, a .4sc is its embedded PDF,
    /// a .4ss is every score in setlist order (placeholders skipped).
    pub fn from_file_bytes(name: &str, bytes: &[u8]) -> Result<Vec<Input>, OmrError> {
        let lower = name.to_lowercase();
        let stem = Path::new(name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if lower.ends_with(".pdf") || (bytes.starts_with(b"%PDF-") && !lower.ends_with(".4sc")) {
            return Ok(vec![Input::Pdf {
                title: stem,
                bytes: bytes.to_vec(),
            }]);
        }
        if lower.ends_with(".4sc") {
            let s = parse_4sc(bytes)?;
            return Ok(vec![Input::Pdf {
                title: s.title.unwrap_or(stem),
                bytes: s.pdf,
            }]);
        }
        if lower.ends_with(".4ss") {
            return Ok(parse_4ss(bytes)?
                .items
                .into_iter()
                .filter_map(|it| match it.entry {
                    SetlistEntry::Score(s) => Some(Input::Pdf {
                        title: it.title,
                        bytes: s.pdf,
                    }),
                    SetlistEntry::Placeholder => None,
                })
                .collect());
        }
        Err(OmrError::UnsupportedInput(name.to_string()))
    }

    pub fn title(&self) -> &str {
        match self {
            Input::Pdf { title, .. } => title,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub title: String,
    pub musicxml: PathBuf,
    pub midi: PathBuf,
    pub parts: usize,
    pub notes: usize,
    pub measures: usize,
    pub duration_sec: f64,
    /// True when the raw PDF failed and the cleaned scan was used.
    pub cleaned: bool,
    /// 1-based pages that could not be recognised (page-by-page fallback).
    pub skipped_pages: Vec<u32>,
}

/// File-system-safe version of a title.
fn safe_name(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "score".into()
    } else {
        s
    }
}

/// Recognise one input and write `<title>.mxl|.musicxml` and `<title>.mid` in `out_dir`.
pub fn convert(
    engine: &dyn OmrEngine,
    input: Input,
    out_dir: &Path,
) -> Result<Converted, OmrError> {
    convert_with(engine, None, input, out_dir)
}

fn note_count(score: &cantor_score::Score) -> usize {
    score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .filter(|e| matches!(e.kind, EventKind::Note(_)))
        .count()
}

/// Run the engine on `file` and parse the result. A result with fewer notes
/// than measures (all parts counted) means recognition mostly failed.
fn attempt(
    engine: &dyn OmrEngine,
    file: &Path,
    work: &Path,
) -> Result<(PathBuf, cantor_score::Score), OmrError> {
    let recognised = engine.recognise(file, work)?;
    let score = parse_score_file(&std::fs::read(&recognised)?)?;
    check_enough_notes(engine, &score)?;
    Ok((recognised, score))
}

fn check_enough_notes(engine: &dyn OmrEngine, score: &cantor_score::Score) -> Result<(), OmrError> {
    let notes = note_count(score);
    let measures = score.measure_count();
    if notes == 0 || notes < measures {
        return Err(OmrError::Engine {
            engine: engine.name().into(),
            detail: format!("recognised only {notes} notes in {measures} measures"),
        });
    }
    Ok(())
}

/// Recognise each page on its own and join the pages that worked. Returns the
/// joined score and the pages skipped, or an error if no page worked.
fn attempt_by_page(
    engine: &dyn OmrEngine,
    file: &Path,
    work: &Path,
) -> Result<(cantor_score::Score, Vec<u32>), OmrError> {
    let Some(pages) = engine.page_count(file).filter(|&n| n > 1) else {
        return Err(OmrError::Engine {
            engine: engine.name().into(),
            detail: "no page-by-page fallback".into(),
        });
    };
    let mut joined: Option<cantor_score::Score> = None;
    let mut skipped = Vec::new();
    for page in 1..=pages {
        let dir = work.join(format!("page{page}"));
        std::fs::create_dir_all(&dir)?;
        let score = engine
            .recognise_page(file, page, &dir)
            .and_then(|p| Ok(parse_score_file(&std::fs::read(p)?)?));
        match score {
            Ok(s) if note_count(&s) > 0 => match &mut joined {
                Some(j) => j.append(s),
                None => joined = Some(s),
            },
            _ => skipped.push(page),
        }
    }
    let joined = joined.ok_or_else(|| OmrError::Engine {
        engine: engine.name().into(),
        detail: "no page could be recognised".into(),
    })?;
    check_enough_notes(engine, &joined)?;
    Ok((joined, skipped))
}

/// Like [`convert`], retrying once on a cleaned scan when the raw PDF fails
/// or yields no notes. The first error is reported if both attempts fail.
pub fn convert_with(
    engine: &dyn OmrEngine,
    cleaner: Option<&dyn Preprocessor>,
    input: Input,
    out_dir: &Path,
) -> Result<Converted, OmrError> {
    let Input::Pdf { title, bytes } = input;
    let name = safe_name(&title);
    let work = out_dir.join(format!(".{name}.omr"));
    std::fs::create_dir_all(&work)?;
    let pdf = work.join(format!("{name}.pdf"));
    std::fs::write(&pdf, &bytes)?;

    // Strategy ladder: whole book, cleaned book, raw page by page, cleaned page
    // by page. The first error is reported if everything fails.
    let mut cleaned = false;
    let mut skipped_pages = Vec::new();
    let (recognised, score) = match attempt(engine, &pdf, &work) {
        Ok((r, s)) => (Some(r), s),
        Err(first_err) => {
            let cleaned_img = match cleaner {
                Some(c) => {
                    let dir = work.join("cleaned");
                    std::fs::create_dir_all(&dir)?;
                    c.clean(&pdf, &dir).ok()
                }
                None => None,
            };
            let cleaned_dir = work.join("cleaned");
            if let Some((r, s)) = cleaned_img
                .as_deref()
                .and_then(|img| attempt(engine, img, &cleaned_dir).ok())
            {
                cleaned = true;
                (Some(r), s)
            } else if let Ok((s, skipped)) = attempt_by_page(engine, &pdf, &work.join("pages")) {
                skipped_pages = skipped;
                (None, s)
            } else if let Some((s, skipped)) = cleaned_img
                .as_deref()
                .and_then(|img| attempt_by_page(engine, img, &cleaned_dir.join("pages")).ok())
            {
                cleaned = true;
                skipped_pages = skipped;
                (None, s)
            } else {
                return Err(first_err);
            }
        }
    };
    let tm = build_timemap(&score);

    // Keep the engine's own file when there is one (it holds layout and
    // lyrics we do not model yet); otherwise write the joined pages.
    let musicxml = match &recognised {
        Some(r) => {
            let ext = r
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or("musicxml".into());
            let dest = out_dir.join(format!("{name}.{ext}"));
            std::fs::copy(r, &dest)?;
            dest
        }
        None => {
            let dest = out_dir.join(format!("{name}.musicxml"));
            std::fs::write(&dest, write_musicxml(&score))?;
            dest
        }
    };
    let midi = out_dir.join(format!("{name}.mid"));
    std::fs::write(&midi, to_smf(&score, &tm))?;

    Ok(Converted {
        title,
        musicxml,
        midi,
        parts: score.parts.len(),
        notes: note_count(&score),
        measures: tm.measure_starts.len(),
        duration_sec: tm.duration_sec,
        cleaned,
        skipped_pages,
    })
}

/// Test helpers shared with integration tests.
#[doc(hidden)]
pub mod testing {
    /// Build a minimal `.4sc`: gzip(binary plist {pdfData, "<file>|title"}).
    pub fn fake_4sc(file: &str, title: &str, pdf: &[u8]) -> Vec<u8> {
        cantor_library::forscore::testing::fake_4sc(file, title, pdf)
    }
}
