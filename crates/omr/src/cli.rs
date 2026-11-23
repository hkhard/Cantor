//! The `cantor-omr` command (LIB-06, ROADMAP phase 1): a choir director turns
//! PDFs into Cantor scores with one command.
//!
//! ```text
//! cantor-omr convert <file.pdf|.4sc|.4ss>... [--out DIR] [--set NAME]
//! cantor-omr doctor
//! cantor-omr <out-dir> <file>...        (old form, still accepted)
//! ```
//!
//! Per piece `convert` writes `<title>.mxl` (or `.musicxml`), the original
//! `<title>.pdf`, `<title>.measures.json` (the play cursor on the PDF) and
//! `<title>.mid`; `--set` also writes `<NAME>.cantorset` for AirDrop. Messages
//! are in Swedish when `LANG`/`LC_ALL`/`LC_MESSAGES` starts with `sv`.

use crate::measure_map::map_for_work;
use crate::{convert_with, safe_name, Audiveris, Input, OmrError, Preprocessor, ScriptCleaner};
use cantor_library::bundle::{export_bundle, BundleScore, BUNDLE_EXT};
use cantor_library::store::{SetItem, Setlist};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Where to download Audiveris.
pub const AUDIVERIS_URL: &str = "https://github.com/Audiveris/audiveris/releases";

/// Message language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Sv,
    En,
}

impl Lang {
    /// Swedish when the first set of `LC_ALL`, `LC_MESSAGES`, `LANG` starts with `sv`.
    pub fn from_env() -> Self {
        let v = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        if v.to_lowercase().starts_with("sv") {
            Lang::Sv
        } else {
            Lang::En
        }
    }

    fn t<'a>(self, sv: &'a str, en: &'a str) -> &'a str {
        match self {
            Lang::Sv => sv,
            Lang::En => en,
        }
    }
}

fn usage(l: Lang) -> String {
    l.t(
        "Användning:\n  cantor-omr convert <noter.pdf|.4sc|.4ss>... [--out MAPP] [--set NAMN]\n  cantor-omr doctor\n\n  convert  gör om PDF-noter till Cantor-filer (MusicXML, PDF, takt-karta)\n  --out    mapp att skriva till (standard: \"Cantor\" bredvid första filen)\n  --set    gör också NAMN.cantorset att skicka med AirDrop\n  doctor   kontrollerar att Audiveris och språkfilerna finns",
        "Usage:\n  cantor-omr convert <score.pdf|.4sc|.4ss>... [--out DIR] [--set NAME]\n  cantor-omr doctor\n\n  convert  turns PDF scores into Cantor files (MusicXML, PDF, measure map)\n  --out    folder to write to (default: \"Cantor\" next to the first file)\n  --set    also make NAME.cantorset to send by AirDrop\n  doctor   checks that Audiveris and the language files are installed",
    )
    .to_string()
}

fn audiveris_missing(l: Lang) -> String {
    match l {
        Lang::Sv => format!("Audiveris hittades inte. Det läser noterna i PDF:en.\n  Ladda ner det från {AUDIVERIS_URL} (macOS: .dmg, dra Audiveris till Program),\n  eller ange sökvägen i CANTOR_AUDIVERIS. Kör sedan: cantor-omr doctor"),
        Lang::En => format!("Audiveris was not found. It reads the notes in the PDF.\n  Download it from {AUDIVERIS_URL} (macOS: the .dmg, drag Audiveris to Applications),\n  or set CANTOR_AUDIVERIS to its program. Then run: cantor-omr doctor"),
    }
}

/// Plain-language explanation of a conversion error, with what to try.
pub fn explain(e: &OmrError, l: Lang) -> String {
    match e {
        OmrError::Engine { detail, .. } => match l {
            Lang::Sv => format!("kunde inte läsa noterna ({detail}). Prova en skarpare skanning (300 DPI, svartvit) eller dela upp PDF:en per stycke."),
            Lang::En => format!("could not read the notes ({detail}). Try a sharper scan (300 DPI, black and white) or split the PDF into one piece per file."),
        },
        OmrError::UnsupportedInput(name) => match l {
            Lang::Sv => format!("{name}: bara .pdf, .4sc och .4ss kan läsas"),
            Lang::En => format!("{name}: only .pdf, .4sc and .4ss can be read"),
        },
        OmrError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            l.t("filen finns inte", "the file does not exist").to_string()
        }
        other => other.to_string(),
    }
}

/// Run the command. `args` excludes the program name. Returns the exit code
/// (0 ok, 1 some piece failed, 2 usage or setup problem).
pub fn run(args: &[String], l: Lang, out: &mut dyn Write) -> i32 {
    match args.first().map(String::as_str) {
        Some("doctor") => doctor(l, out),
        Some("convert") => match parse_convert(&args[1..]) {
            Some(o) => convert_cmd(&o, l, out),
            None => {
                let _ = writeln!(out, "{}", usage(l));
                2
            }
        },
        Some("-h" | "--help" | "help") => {
            let _ = writeln!(out, "{}", usage(l));
            0
        }
        // Old form: <out-dir> <files>...
        Some(_) if args.len() >= 2 && !args[0].starts_with('-') => {
            let o = ConvertOpts {
                inputs: args[1..].iter().map(PathBuf::from).collect(),
                out: Some(PathBuf::from(&args[0])),
                set: None,
            };
            convert_cmd(&o, l, out)
        }
        _ => {
            let _ = writeln!(out, "{}", usage(l));
            2
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct ConvertOpts {
    inputs: Vec<PathBuf>,
    out: Option<PathBuf>,
    set: Option<String>,
}

fn parse_convert(args: &[String]) -> Option<ConvertOpts> {
    let mut o = ConvertOpts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" | "-o" => o.out = Some(PathBuf::from(it.next()?)),
            "--set" => o.set = Some(it.next()?.clone()).filter(|s| !s.trim().is_empty()),
            s if s.starts_with('-') => return None,
            s => o.inputs.push(PathBuf::from(s)),
        }
    }
    (!o.inputs.is_empty()).then_some(o)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One finished piece, for the summary and the bundle.
struct Piece {
    title: String,
    files: Vec<PathBuf>,
}

fn convert_cmd(o: &ConvertOpts, l: Lang, out: &mut dyn Write) -> i32 {
    let Some(engine) = Audiveris::locate().filter(|a| a.bin.exists()) else {
        let _ = writeln!(out, "{}", audiveris_missing(l));
        return 2;
    };
    let cleaner = ScriptCleaner::locate(&repo_root());
    let dir = o.out.clone().unwrap_or_else(|| {
        o.inputs[0]
            .parent()
            .unwrap_or(Path::new("."))
            .join("Cantor")
    });
    if let Err(e) = std::fs::create_dir_all(&dir) {
        let _ = writeln!(out, "{}: {e}", dir.display());
        return 2;
    }
    let _ = writeln!(
        out,
        "{} {}",
        l.t("Skriver till", "Writing to"),
        dir.display()
    );

    let mut pieces = Vec::new();
    let mut failed = 0;
    for file in &o.inputs {
        let name = file.to_string_lossy().into_owned();
        let inputs = match std::fs::read(file)
            .map_err(OmrError::from)
            .and_then(|b| Input::from_file_bytes(&name, &b))
        {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                let _ = writeln!(out, "✗ {name}: {}", explain(&e, l));
                continue;
            }
        };
        let is_pdf = name.to_lowercase().ends_with(".pdf");
        let total = inputs.len();
        for (k, input) in inputs.into_iter().enumerate() {
            let label = input.title().to_string();
            let _ = writeln!(
                out,
                "→ {label}{}: {}",
                if total > 1 {
                    format!(" ({}/{total})", k + 1)
                } else {
                    String::new()
                },
                l.t(
                    "läser noterna, det tar någon minut per sida...",
                    "reading the notes, about a minute per page..."
                )
            );
            let _ = out.flush();
            match convert_one(
                &engine,
                cleaner.as_ref().map(|c| c as &dyn Preprocessor),
                input,
                is_pdf,
                &dir,
            ) {
                Ok((piece, report)) => {
                    let _ = writeln!(out, "✓ {}: {}", piece.title, report.text(l));
                    pieces.push(piece);
                }
                Err(e) => {
                    failed += 1;
                    let _ = writeln!(out, "✗ {label}: {}", explain(&e, l));
                }
            }
        }
    }

    if let (Some(set), false) = (&o.set, pieces.is_empty()) {
        match write_set(set, &pieces, &dir) {
            Ok(p) => {
                let _ = writeln!(
                    out,
                    "{} {}\n  {}",
                    l.t("Set klart:", "Set ready:"),
                    p.display(),
                    l.t(
                        "Skicka filen med AirDrop till iPaden och öppna den i Cantor.",
                        "Send the file to the iPad by AirDrop and open it in Cantor."
                    )
                );
            }
            Err(e) => {
                failed += 1;
                let _ = writeln!(out, "✗ {set}: {e}");
            }
        }
    }
    let _ = writeln!(
        out,
        "{} {}, {} {}",
        pieces.len(),
        l.t("klara", "converted"),
        failed,
        l.t("misslyckades", "failed")
    );
    if failed > 0 {
        1
    } else {
        0
    }
}

struct Report {
    parts: usize,
    measures: usize,
    notes: usize,
    map: Option<(usize, String)>,
    cleaned: bool,
    skipped: Vec<u32>,
}

impl Report {
    fn text(&self, l: Lang) -> String {
        let mut s = match l {
            Lang::Sv => format!(
                "{} stämmor, {} takter, {} noter",
                self.parts, self.measures, self.notes
            ),
            Lang::En => format!(
                "{} parts, {} measures, {} notes",
                self.parts, self.measures, self.notes
            ),
        };
        match &self.map {
            Some((n, _)) if *n == self.measures => {
                s += l.t(", markören följer PDF:en", ", cursor follows the PDF")
            }
            Some((n, _)) => {
                s += &match l {
                    Lang::Sv => format!(
                        ", obs: takt-kartan har {n} takter, markören på PDF:en kan hamna fel"
                    ),
                    Lang::En => format!(
                        ", note: the measure map has {n} measures, the PDF cursor may drift"
                    ),
                }
            }
            None => {
                s += l.t(
                    ", ingen takt-karta (visa noterna i notvyn)",
                    ", no measure map (use the notation view)",
                )
            }
        }
        if self.cleaned {
            s += l.t(", efter tvätt av skanningen", ", after scan cleanup");
        }
        if !self.skipped.is_empty() {
            let pages: Vec<String> = self.skipped.iter().map(u32::to_string).collect();
            s += &match l {
                Lang::Sv => format!(", sidor utan noter hoppades över: {}", pages.join(", ")),
                Lang::En => format!(", pages skipped (no music read): {}", pages.join(", ")),
            };
        }
        s
    }
}

/// Convert one input, then name the files after the score's title and add
/// the original PDF and the measure map.
fn convert_one(
    engine: &Audiveris,
    cleaner: Option<&dyn Preprocessor>,
    input: Input,
    from_pdf_file: bool,
    dir: &Path,
) -> Result<(Piece, Report), OmrError> {
    let Input::Pdf { title, bytes } = &input;
    let (fallback, pdf_bytes) = (title.clone(), bytes.clone());
    let work_name = safe_name(&fallback);
    let c = convert_with(engine, cleaner, input, dir)?;
    // A PDF's file name is often "scan_0042"; prefer the printed title.
    // forScore titles were chosen by the user, keep them.
    let header_title = std::fs::read(&c.musicxml)
        .ok()
        .and_then(|b| cantor_score::read_header(&b).ok())
        .and_then(|h| h.title());
    let title = match header_title {
        Some(t) if from_pdf_file => t,
        _ => fallback,
    };
    let name = safe_name(&title);
    let rename = |p: &Path| -> Result<PathBuf, OmrError> {
        let ext = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .strip_prefix(&format!("{work_name}."))
            .map(str::to_string)
            .unwrap_or_else(|| "musicxml".into());
        let dest = dir.join(format!("{name}.{ext}"));
        if dest != p {
            std::fs::rename(p, &dest)?;
        }
        Ok(dest)
    };
    let musicxml = rename(&c.musicxml)?;
    let midi = rename(&c.midi)?;
    let pdf = dir.join(format!("{name}.pdf"));
    std::fs::write(&pdf, &pdf_bytes)?;
    let mut files = vec![musicxml, pdf];
    let map = map_for_work(
        &dir.join(format!(".{work_name}.omr")),
        &work_name,
        c.measures,
    );
    let map = match map {
        Some((boxes, source)) => {
            let p = dir.join(format!("{name}.measures.json"));
            std::fs::write(&p, serde_json::to_string(&boxes).unwrap_or_default())?;
            files.push(p);
            Some((boxes.len(), source))
        }
        None => None,
    };
    files.push(midi);
    Ok((
        Piece { title, files },
        Report {
            parts: c.parts,
            measures: c.measures,
            notes: c.notes,
            map,
            cleaned: c.cleaned,
            skipped: c.skipped_pages,
        },
    ))
}

fn write_set(name: &str, pieces: &[Piece], dir: &Path) -> Result<PathBuf, String> {
    let mut scores = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        let mut files = Vec::new();
        for f in &p.files {
            let fname = f.file_name().unwrap_or_default().to_string_lossy();
            if fname.ends_with(".mid") {
                continue; // Cantor plays from the MusicXML
            }
            files.push((
                fname.into_owned(),
                std::fs::read(f).map_err(|e| e.to_string())?,
            ));
        }
        scores.push(BundleScore {
            id: format!("omr:{i}"),
            title: p.title.clone(),
            files,
            ..Default::default()
        });
    }
    let setlist = Setlist {
        id: String::new(),
        name: name.to_string(),
        items: scores
            .iter()
            .map(|s| SetItem::Score {
                score: s.id.clone(),
                start_page: None,
                note: String::new(),
            })
            .collect(),
        instructions: String::new(),
    };
    let bytes = export_bundle(&setlist, &scores)?;
    let path = dir.join(format!("{}.{BUNDLE_EXT}", safe_name(name)));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Tesseract model folder Audiveris reads (`$CANTOR_TESSDATA` overrides).
pub fn tessdata_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("CANTOR_TESSDATA") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/AudiverisLtd/audiveris/tessdata")
    } else {
        home.join(".config/AudiverisLtd/audiveris/tessdata")
    }
}

fn doctor(l: Lang, out: &mut dyn Write) -> i32 {
    let mut code = 0;
    match Audiveris::locate() {
        Some(a) if a.bin.exists() => {
            let _ = writeln!(out, "✓ Audiveris: {}", a.bin.display());
        }
        Some(a) => {
            code = 2;
            let _ = writeln!(
                out,
                "✗ Audiveris: {} {}\n  {}",
                a.bin.display(),
                l.t(
                    "finns inte (CANTOR_AUDIVERIS)",
                    "does not exist (CANTOR_AUDIVERIS)"
                ),
                audiveris_missing(l)
            );
        }
        None => {
            code = 2;
            let _ = writeln!(out, "✗ {}", audiveris_missing(l));
        }
    }
    let td = tessdata_dir();
    let langs = std::env::var("CANTOR_OCR_LANGUAGES")
        .unwrap_or_else(|_| crate::DEFAULT_OCR_LANGUAGES.to_string());
    for lang in langs.split('+').filter(|s| !s.is_empty()) {
        let f = td.join(format!("{lang}.traineddata"));
        if f.exists() {
            let _ = writeln!(out, "✓ {} {lang}", l.t("Textspråk", "Text language"));
        } else {
            let _ = writeln!(
                out,
                "! {} {lang} {}\n  curl -sSLfo \"{}\" https://github.com/tesseract-ocr/tessdata/raw/main/{lang}.traineddata",
                l.t("Textspråk", "Text language"),
                l.t(
                    "saknas: sångtext och tempo läses inte. Installera med:",
                    "missing: lyrics and tempo text are not read. Install with:"
                ),
                f.display()
            );
        }
    }
    if ScriptCleaner::locate(&repo_root()).is_some() {
        let _ = writeln!(out, "✓ {}", l.t("Skanningstvätt", "Scan cleanup"));
    } else {
        let _ = writeln!(
            out,
            "! {}",
            l.t(
                "Skanningstvätt saknas (valfri, hjälper dåliga skanningar): se tools/omr/README.md",
                "Scan cleanup missing (optional, helps poor scans): see tools/omr/README.md"
            )
        );
    }
    let _ = writeln!(
        out,
        "{}",
        if code == 0 {
            l.t(
                "Klart att köra: cantor-omr convert noter.pdf",
                "Ready: cantor-omr convert score.pdf",
            )
        } else {
            l.t(
                "Installera det som saknas och kör doctor igen.",
                "Install what is missing and run doctor again.",
            )
        }
    );
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_cli_parses_convert_options() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_convert(&a(&["a.pdf", "--out", "o", "b.4sc", "--set", "Vår"])),
            Some(ConvertOpts {
                inputs: vec!["a.pdf".into(), "b.4sc".into()],
                out: Some("o".into()),
                set: Some("Vår".into()),
            })
        );
        assert_eq!(parse_convert(&a(&["--out"])), None);
        assert_eq!(parse_convert(&a(&["--bogus", "a.pdf"])), None);
        assert_eq!(parse_convert(&a(&["--out", "o"])), None);
    }

    #[test]
    fn lib_06_cli_explains_errors_in_both_languages() {
        let e = OmrError::Engine {
            engine: "audiveris".into(),
            detail: "no staves".into(),
        };
        assert!(explain(&e, Lang::Sv).contains("300 DPI"));
        assert!(explain(&e, Lang::En).contains("sharper scan"));
        let u = OmrError::UnsupportedInput("x.doc".into());
        assert!(explain(&u, Lang::Sv).contains("bara .pdf"));
        let nf = OmrError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(explain(&nf, Lang::En), "the file does not exist");
        let other = OmrError::Io(std::io::Error::other("disk full"));
        assert!(explain(&other, Lang::En).contains("disk full"));
        let mut o = Vec::new();
        assert_eq!(run(&["--help".into()], Lang::Sv, &mut o), 0);
        assert!(String::from_utf8_lossy(&o).contains("Användning"));
        assert_eq!(run(&[], Lang::En, &mut Vec::new()), 2);
        assert_eq!(run(&["convert".into()], Lang::En, &mut Vec::new()), 2);
    }
}
