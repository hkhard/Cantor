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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub title: String,
    pub stem: String,
    pub dir: PathBuf,
    pub score_file: Option<String>,
    pub pdf_file: Option<String>,
    /// PERF-04 backing track `<stem>.track.<ext>` (audio recording).
    pub track_file: Option<String>,
}

/// Folder (inside a library dir) holding the original forScore files.
pub const ORIGINALS_DIR: &str = ".forscore";

const SCORE_EXT: &[&str] = &["mxl", "musicxml", "xml"];
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
        std::fs::write(dir.join(format!("{stem}.{ext}")), data).map_err(|e| e.to_string())
    };
    match ext {
        "mxl" | "musicxml" | "xml" => {
            check_musicxml(bytes)?;
            let stem = free_stem(dir, &safe_stem(name));
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
            let stem = free_stem(dir, &safe_stem(name));
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
        _ => Err(format!(
            "{name}: Cantor kan öppna MusicXML (.musicxml, .mxl), PDF och forScore (.4sc, .4ss)"
        )),
    }
}

/// Minimal validity check without depending on the score crate: an MXL is a
/// zip, MusicXML has a complete `score-partwise`/`score-timewise` root.
fn check_musicxml(bytes: &[u8]) -> Result<(), String> {
    if bytes.starts_with(b"PK\x03\x04") {
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
    out.sort_by_key(|e| e.title.to_lowercase());
    out
}
