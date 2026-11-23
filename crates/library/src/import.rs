//! On-device library: import files into a folder and list what is there
//! (LIB-01, LIB-02, LIB-07).
//!
//! A score is a file stem with any of these siblings: `<stem>.mxl` /
//! `<stem>.musicxml` / `<stem>.xml` (plays), `<stem>.pdf` (shown) and
//! `<stem>.measures.json` (cursor on the PDF, from OMR on a desktop).
//! Import never runs OMR; a PDF-only score is readable but silent until it
//! is converted.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub stem: String,
    /// MusicXML present: the score plays.
    pub playable: bool,
    /// PDF present: the original page can be shown.
    pub pdf: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryEntry {
    /// The score's own title (MusicXML header), else the tidied file name.
    pub title: String,
    pub stem: String,
    pub dir: PathBuf,
    pub score_file: Option<String>,
    pub pdf_file: Option<String>,
    /// PERF-04 backing track `<stem>.track.<ext>` (audio recording).
    pub track_file: Option<String>,
    /// Header of the MusicXML file (empty for PDF-only scores or when the
    /// file has none).
    pub header: cantor_score::Header,
}

/// Folder (inside a library dir) holding the original forScore files.
pub const ORIGINALS_DIR: &str = ".forscore";

const SCORE_EXT: &[&str] = &["mxl", "musicxml", "xml"];

/// NFR-13: most bytes one file inside an archive (MXL, backup, bundle,
/// forScore gzip) may unpack to. A "zip bomb" that declares or inflates to
/// more is refused instead of filling memory.
pub const MAX_UNPACKED: u64 = 512 * 1024 * 1024;

/// Read at most [`MAX_UNPACKED`] bytes; more is an error.
pub fn read_limited(r: impl std::io::Read, what: &str) -> Result<Vec<u8>, String> {
    read_limited_to(r, MAX_UNPACKED, what)
}

/// Read at most `max` bytes; more is an error.
pub fn read_limited_to(r: impl std::io::Read, max: u64, what: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    r.take(max + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("{what}: {e}"))?;
    if out.len() as u64 > max {
        return Err(format!("{what}: too large to unpack (damaged file?)"));
    }
    Ok(out)
}

/// NFR-13: write a file atomically (temp file, then rename), so a crash
/// mid-write leaves the old file or the new one, never half of one.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}
/// Audio formats a backing track can have (decoded by cantor-audio).
pub const TRACK_EXT: &[&str] = &["m4a", "mp3", "wav", "flac", "aac", "aif", "aiff"];

/// File-name-safe stem: path components and reserved characters removed.
pub(crate) fn safe_stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = match base.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => base,
    };
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "Score".to_string()
    } else {
        cleaned
    }
}

/// Stem for a new file of kind `kind` ("score" or "pdf"): the plain stem if
/// a score with that stem exists but lacks this kind (the MusicXML and PDF
/// of one piece pair up, LIB-05), else the first free stem.
pub(crate) fn pairing_stem(dir: &Path, stem: &str, kind: &str) -> String {
    let has = |exts: &[&str]| {
        exts.iter()
            .any(|e| dir.join(format!("{stem}.{e}")).exists())
    };
    let (mine, other): (&[&str], &[&str]) = if kind == "pdf" {
        (&["pdf"], SCORE_EXT)
    } else {
        (SCORE_EXT, &["pdf"])
    };
    if has(other) && !has(mine) {
        return stem.to_string();
    }
    free_stem(dir, stem)
}

/// First stem not used by any file in `dir`: "Que", "Que (2)", ...
pub(crate) fn free_stem(dir: &Path, stem: &str) -> String {
    let taken = |s: &str| {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.rsplit_once('.').is_some_and(|(st, _)| {
                        st == s || st.strip_suffix(".measures").is_some_and(|x| x == s)
                    })
                })
            })
            .unwrap_or(false)
    };
    if !taken(stem) {
        return stem.to_string();
    }
    (2..)
        .map(|k| format!("{stem} ({k})"))
        .find(|s| !taken(s))
        .expect("unbounded")
}

/// Import one file (MusicXML, MXL, PDF, forScore `.4sc`/`.4ss`) into `dir`.
/// Returns what was added, or a message for the user.
pub fn import_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<Vec<Imported>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let write = |stem: &str, ext: &str, data: &[u8]| -> Result<(), String> {
        write_atomic(&dir.join(format!("{stem}.{ext}")), data).map_err(|e| e.to_string())
    };
    match ext {
        "mxl" | "musicxml" | "xml" => {
            check_musicxml(bytes)?;
            let stem = pairing_stem(dir, &safe_stem(name), "score");
            write(&stem, ext, bytes)?;
            Ok(vec![Imported {
                stem,
                playable: true,
                pdf: false,
            }])
        }
        "pdf" => {
            if !bytes.starts_with(b"%PDF") {
                return Err(format!("{name}: not a PDF file"));
            }
            let stem = pairing_stem(dir, &safe_stem(name), "pdf");
            write(&stem, "pdf", bytes)?;
            Ok(vec![Imported {
                stem,
                playable: false,
                pdf: true,
            }])
        }
        "4sc" => {
            let s = crate::forscore::parse_4sc(bytes).map_err(|e| format!("{name}: {e}"))?;
            let title = s.title.clone().unwrap_or_else(|| safe_stem(name));
            let stem = free_stem(dir, &safe_stem(&format!("{title}.pdf")));
            write(&stem, "pdf", &s.pdf)?;
            // Keep the original: fields not imported yet (text and stamp
            // annotations, bookmarks, links, attachments) stay available.
            // In a hidden folder so the app's import sweep does not see it as new.
            let keep = dir.join(ORIGINALS_DIR);
            std::fs::create_dir_all(&keep).map_err(|e| e.to_string())?;
            std::fs::write(keep.join(format!("{stem}.4sc")), bytes).map_err(|e| e.to_string())?;
            Ok(vec![Imported {
                stem,
                playable: false,
                pdf: true,
            }])
        }
        "4ss" => {
            let set = crate::forscore::parse_4ss(bytes).map_err(|e| format!("{name}: {e}"))?;
            let mut out = Vec::new();
            for it in set.items {
                if let crate::forscore::SetlistEntry::Score(s) = it.entry {
                    let stem = free_stem(dir, &safe_stem(&format!("{}.pdf", it.title)));
                    write(&stem, "pdf", &s.pdf)?;
                    out.push(Imported {
                        stem,
                        playable: false,
                        pdf: true,
                    });
                }
            }
            Ok(out)
        }
        "mscz" => {
            // MuseScore source: kept beside the score with the same stem so
            // it travels with it (backup, sync). Cantor cannot play it;
            // MuseScore exports the MusicXML and PDF that it uses.
            if !bytes.starts_with(b"PK\x03\x04") {
                return Err(format!("{name}: not a MuseScore file"));
            }
            let stem = safe_stem(name);
            let has_score = SCORE_EXT.iter().chain(["pdf"].iter()).any(|e| dir.join(format!("{stem}.{e}")).exists());
            if !has_score {
                return Err(format!(
                    "{name}: MuseScore-filer kan inte spelas direkt. Exportera MusicXML (och gärna PDF) från MuseScore och importera dem; .mscz-filen sparas då med noterna."
                ));
            }
            write(&stem, "mscz", bytes)?;
            Ok(vec![Imported {
                stem,
                playable: dir.join(format!("{}.mxl", safe_stem(name))).exists()
                    || dir.join(format!("{}.musicxml", safe_stem(name))).exists(),
                pdf: dir.join(format!("{}.pdf", safe_stem(name))).exists(),
            }])
        }
        _ => Err(format!(
            "{name}: Cantor kan öppna MusicXML (.musicxml, .mxl), PDF, MuseScore (.mscz, tillsammans med MusicXML) och forScore (.4sc, .4ss)"
        )),
    }
}

/// Minimal validity check without depending on the score crate: an MXL is a
/// zip, MusicXML has a complete `score-partwise`/`score-timewise` root.
fn check_musicxml(bytes: &[u8]) -> Result<(), String> {
    if bytes.starts_with(b"PK\x03\x04") {
        // MXL: the zip must open and hold a score file that unpacks.
        let bad = || "Filen är inte en hel MusicXML-fil (skadad .mxl)".to_string();
        let mut zr = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| bad())?;
        let name = zr
            .file_names()
            .filter_map(|n| n.ok().map(|n| n.into_owned()))
            .find(|n| {
                !n.starts_with("META-INF/") && (n.ends_with(".xml") || n.ends_with(".musicxml"))
            })
            .ok_or_else(bad)?;
        let f = zr.by_name(&name).map_err(|_| bad())?;
        read_limited(f, &name).map_err(|_| bad())?;
        return Ok(());
    }
    let text = String::from_utf8_lossy(bytes);
    let ok = ["score-partwise", "score-timewise"]
        .iter()
        .any(|r| text.contains(&format!("<{r}")) && text.contains(&format!("</{r}>")));
    if ok {
        Ok(())
    } else {
        Err("Filen är inte en hel MusicXML-fil".into())
    }
}

/// All scores in the given folders (later folders win on equal stems),
/// sorted by title.
pub fn list_library(dirs: &[&Path]) -> Vec<LibraryEntry> {
    let mut map: std::collections::BTreeMap<String, LibraryEntry> = Default::default();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut names: Vec<String> = rd
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        for n in names {
            let Some((stem, ext)) = n.rsplit_once('.') else {
                continue;
            };
            if stem.ends_with(".measures") || stem.starts_with('.') {
                continue;
            }
            let ext = ext.to_lowercase();
            // "<stem>.track.<audio>" is the backing track of <stem>.
            if let Some(owner) = stem.strip_suffix(".track") {
                if TRACK_EXT.contains(&ext.as_str()) {
                    let key = format!("{}\u{0}{}", dir.display(), owner);
                    map.entry(key)
                        .or_insert_with(|| LibraryEntry {
                            title: owner.replace('_', " ").trim().to_string(),
                            stem: owner.to_string(),
                            dir: dir.to_path_buf(),
                            score_file: None,
                            pdf_file: None,
                            track_file: None,
                            header: Default::default(),
                        })
                        .track_file = Some(n.clone());
                }
                continue;
            }
            let is_score = SCORE_EXT.contains(&ext.as_str());
            if !is_score && ext != "pdf" {
                continue;
            }
            let key = format!("{}\u{0}{}", dir.display(), stem);
            let e = map.entry(key).or_insert_with(|| LibraryEntry {
                title: stem.replace('_', " ").trim().to_string(),
                stem: stem.to_string(),
                dir: dir.to_path_buf(),
                score_file: None,
                pdf_file: None,
                track_file: None,
                header: Default::default(),
            });
            if is_score {
                // Prefer .mxl over .musicxml over .xml.
                let rank = |f: &str| SCORE_EXT.iter().position(|x| f.ends_with(x)).unwrap_or(9);
                if e.score_file.as_deref().is_none_or(|f| rank(&n) < rank(f)) {
                    e.score_file = Some(n.clone());
                }
            } else {
                e.pdf_file = Some(n.clone());
            }
        }
    }
    // A recording alone is not a score.
    let mut out: Vec<LibraryEntry> = map
        .into_values()
        .filter(|e| e.score_file.is_some() || e.pdf_file.is_some())
        .collect();
    // The title the file itself carries (LIB-01): read only the header.
    for e in &mut out {
        if let Some(f) = &e.score_file {
            if let Ok(h) = std::fs::read(e.dir.join(f))
                .map_err(|_| ())
                .and_then(|b| cantor_score::read_header(&b).map_err(|_| ()))
            {
                if let Some(t) = h.title() {
                    e.title = t;
                }
                e.header = h;
            }
        }
    }
    out.sort_by_key(|e| e.title.to_lowercase());
    out
}

/// A library entry as seen by [`pair_suggestions`]: its stem, displayed
/// title, whether it has MusicXML / a PDF, and (PDF only) the words on the
/// PDF's first page.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub stem: String,
    pub title: String,
    pub playable: bool,
    pub pdf: bool,
    pub pdf_text: String,
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// LIB-05: MusicXML-only and PDF-only entries that are the same piece:
/// equal titles, or the score's title (at least two words or 6 letters)
/// printed on the PDF's first page. Returns `(score stem, pdf stem)` pairs,
/// each entry used once, sorted by score stem.
pub fn pair_suggestions(c: &[Candidate]) -> Vec<(String, String)> {
    let scores: Vec<&Candidate> = c.iter().filter(|x| x.playable && !x.pdf).collect();
    let mut pdfs: Vec<&Candidate> = c.iter().filter(|x| x.pdf && !x.playable).collect();
    let mut out = Vec::new();
    let mut sorted = scores.clone();
    sorted.sort_by(|a, b| a.stem.cmp(&b.stem));
    for s in sorted {
        let t = norm(&s.title);
        if t.is_empty() {
            continue;
        }
        let long_enough = t.contains(' ') || t.chars().count() >= 6;
        let hit = pdfs.iter().position(|p| {
            norm(&p.title) == t
                || (long_enough && format!(" {} ", norm(&p.pdf_text)).contains(&format!(" {t} ")))
        });
        if let Some(i) = hit {
            out.push((s.stem.clone(), pdfs.remove(i).stem.clone()));
        }
    }
    out
}

/// Combine a PDF-only entry into a score: rename `<pdf_stem>.pdf` (and its
/// `.measures.json`, if any) to `<score_stem>.*`. Fails without touching
/// anything if the score has no MusicXML or already has a PDF.
pub fn combine(dir: &Path, score_stem: &str, pdf_stem: &str) -> Result<(), String> {
    if !SCORE_EXT
        .iter()
        .any(|e| dir.join(format!("{score_stem}.{e}")).exists())
    {
        return Err(format!("{score_stem}: no MusicXML to combine with"));
    }
    let to = dir.join(format!("{score_stem}.pdf"));
    if to.exists() {
        return Err(format!("{score_stem} already has a PDF"));
    }
    let from = dir.join(format!("{pdf_stem}.pdf"));
    if !from.exists() {
        return Err(format!("{pdf_stem}.pdf not found"));
    }
    std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
    let m_from = dir.join(format!("{pdf_stem}.measures.json"));
    let m_to = dir.join(format!("{score_stem}.measures.json"));
    if m_from.exists() && !m_to.exists() {
        let _ = std::fs::rename(m_from, m_to);
    }
    Ok(())
}
