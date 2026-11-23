//! Setlist bundles (SET-02, SET-04): share a setlist with its scores,
//! annotations, per-score settings (incl. mixer presets), instructions and
//! part hints (SET-03), item notes (SET-05) and practice tracks as one
//! `.cantorset` zip. No accounts: AirDrop or Files carries the file.
//!
//! Layout: `manifest.json`, `scores/<name>.<ext>` (`mxl`, `musicxml`, `xml`,
//! `pdf`, `measures.json`), `annotations/<name>.json` and, since version 2,
//! `practice/<name>/<part>.m4a` (one practice track per part). `<name>` is a
//! bundle-local name derived from the score title; the manifest's setlist
//! refers to scores by that name. Every file carries its sha256 in the
//! manifest. Zip entries use a fixed timestamp so exports are deterministic,
//! and [`content_hash`] only covers file hashes, so it survives re-zipping.
//!
//! Versions: 1 had no instructions, hints, notes or practice tracks; all new
//! manifest fields default to empty, so a version 1 bundle still imports.
//! A bundle newer than [`BUNDLE_VERSION`] is refused with a clear message.

use crate::import::{free_stem, safe_stem};
use crate::store::{new_id, Metadata, ScoreInstructions, ScoreSettings, SetItem, Setlist, Store};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::path::Path;

/// Bundle file extension (without dot).
pub const BUNDLE_EXT: &str = "cantorset";
/// Manifest format version.
pub const BUNDLE_VERSION: u32 = 2;
/// Audio formats accepted as practice tracks.
const PRACTICE_EXT: &[&str] = &["m4a", "wav", "mp3", "aac"];

const FILE_EXT: &[&str] = &["mxl", "musicxml", "xml", "pdf", "measures.json"];

/// One score to put in a bundle. `files` are `(file name, bytes)`, e.g.
/// `("Que.mxl", ..)`; only the extension of the name is used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BundleScore {
    pub id: String,
    pub title: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub annotations_json: Option<String>,
    pub metadata: Option<Metadata>,
    pub settings: Option<ScoreSettings>,
    /// SET-03: instructions and part hints.
    pub instructions: Option<ScoreInstructions>,
    /// SET-04: practice tracks, `(part label, file name, bytes)`, e.g.
    /// `("Tenor 1", "Que - Tenor 1.m4a", ..)`.
    pub practice: Vec<(String, String, Vec<u8>)>,
}

/// A practice track that landed in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PracticeTrack {
    /// Part label ("Tenor 1").
    pub part: String,
    /// File name inside the library's practice dir of the score.
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBundle {
    pub setlist_id: String,
    /// New score ids (`"1:<stem>"`) in manifest order.
    pub score_ids: Vec<String>,
    pub content_hash: String,
    /// Manifest version of the bundle that was read.
    pub version: u32,
    /// Practice tracks per new score id.
    pub practice: Vec<(String, Vec<PracticeTrack>)>,
}

/// Directory (inside the library) that holds practice tracks of received
/// sets: `Practice/<safe score id>/<part>.m4a`.
pub const PRACTICE_DIR: &str = "Practice";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    /// Path inside the zip.
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PracticeEntry {
    part: String,
    #[serde(flatten)]
    file: FileEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoreEntry {
    name: String,
    title: String,
    metadata: Option<Metadata>,
    settings: Option<ScoreSettings>,
    files: Vec<FileEntry>,
    annotations: Option<FileEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instructions: Option<ScoreInstructions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    practice: Vec<PracticeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    setlist: Setlist,
    scores: Vec<ScoreEntry>,
}

fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn ext_of(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    // Longest first so `.measures.json` wins over a bare `.json`.
    let mut exts = FILE_EXT.to_vec();
    exts.sort_by_key(|e| std::cmp::Reverse(e.len()));
    exts.into_iter().find(|e| lower.ends_with(&format!(".{e}")))
}

/// Build a `.cantorset` zip. Setlist items pointing at scores not in `files`
/// become placeholders titled with the score id.
pub fn export_bundle(setlist: &Setlist, files: &[BundleScore]) -> Result<Vec<u8>, String> {
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let put = |zw: &mut zip::ZipWriter<Cursor<Vec<u8>>>, path: &str, data: &[u8]| {
        zw.start_file(path, opts).map_err(|e| e.to_string())?;
        zw.write_all(data).map_err(|e| e.to_string())
    };

    let mut used = BTreeSet::new();
    let mut scores = Vec::new();
    for s in files {
        let base = safe_stem(&format!("{}.x", s.title));
        let name = std::iter::once(base.clone())
            .chain((2..).map(|k| format!("{base} ({k})")))
            .find(|n| !used.contains(n))
            .expect("unbounded");
        used.insert(name.clone());
        let mut entries = Vec::new();
        for (fname, data) in &s.files {
            let ext = ext_of(fname).ok_or_else(|| format!("{fname}: unsupported file type"))?;
            let path = format!("scores/{name}.{ext}");
            if entries.iter().any(|e: &FileEntry| e.path == path) {
                return Err(format!("{}: two .{ext} files", s.title));
            }
            put(&mut zw, &path, data)?;
            entries.push(FileEntry {
                path,
                sha256: sha256_hex(data),
            });
        }
        let annotations = match &s.annotations_json {
            Some(a) => {
                let path = format!("annotations/{name}.json");
                put(&mut zw, &path, a.as_bytes())?;
                Some(FileEntry {
                    path,
                    sha256: sha256_hex(a.as_bytes()),
                })
            }
            None => None,
        };
        let mut practice = Vec::new();
        for (part, fname, data) in &s.practice {
            let lower = fname.to_lowercase();
            let ext = PRACTICE_EXT
                .iter()
                .find(|e| lower.ends_with(&format!(".{e}")))
                .ok_or_else(|| format!("{fname}: unsupported practice track"))?;
            let base = safe_stem(&format!("{part}.x"));
            let pname = std::iter::once(base.clone())
                .chain((2..).map(|k| format!("{base} ({k})")))
                .find(|n| {
                    !practice.iter().any(|p: &PracticeEntry| {
                        p.file.path.starts_with(&format!("practice/{name}/{n}."))
                    })
                })
                .expect("unbounded");
            let path = format!("practice/{name}/{pname}.{ext}");
            put(&mut zw, &path, data)?;
            practice.push(PracticeEntry {
                part: part.clone(),
                file: FileEntry {
                    path,
                    sha256: sha256_hex(data),
                },
            });
        }
        scores.push((
            s.id.clone(),
            ScoreEntry {
                name,
                title: s.title.clone(),
                metadata: s.metadata.clone(),
                settings: s.settings.clone(),
                files: entries,
                annotations,
                instructions: s.instructions.clone().filter(|i| !i.is_empty()),
                practice,
            },
        ));
    }

    let items = setlist
        .items
        .iter()
        .map(|it| match it {
            SetItem::Score {
                score,
                start_page,
                note,
            } => match scores.iter().find(|(id, _)| id == score) {
                Some((_, e)) => SetItem::Score {
                    score: e.name.clone(),
                    start_page: *start_page,
                    note: note.clone(),
                },
                None => SetItem::Placeholder {
                    title: score.clone(),
                },
            },
            p => p.clone(),
        })
        .collect();
    let manifest = Manifest {
        version: BUNDLE_VERSION,
        setlist: Setlist {
            id: String::new(),
            name: setlist.name.clone(),
            items,
            instructions: setlist.instructions.clone(),
        },
        scores: scores.into_iter().map(|(_, e)| e).collect(),
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    put(&mut zw, "manifest.json", &json)?;
    Ok(zw.finish().map_err(|e| e.to_string())?.into_inner())
}

fn hash_of(m: &Manifest) -> String {
    let mut hs: Vec<&str> = m
        .scores
        .iter()
        .flat_map(|s| {
            s.files
                .iter()
                .chain(&s.annotations)
                .chain(s.practice.iter().map(|p| &p.file))
        })
        .map(|f| f.sha256.as_str())
        .collect();
    hs.sort_unstable();
    sha256_hex(hs.join("\n").as_bytes())
}

fn read_manifest(zr: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<Manifest, String> {
    let mut s = String::new();
    zr.by_name("manifest.json")
        .map_err(|_| "not a Cantor setlist bundle (no manifest.json)".to_string())?
        .read_to_string(&mut s)
        .map_err(|e| e.to_string())?;
    let m: Manifest = serde_json::from_str(&s).map_err(|e| format!("manifest: {e}"))?;
    if m.version > BUNDLE_VERSION {
        return Err(format!("unsupported bundle version {}", m.version));
    }
    Ok(m)
}

/// Deterministic content hash: sha256 over the manifest's sorted per-file
/// hashes. Independent of zip timestamps, compression and names. Returns an
/// empty string if `bundle` is not a readable bundle.
pub fn content_hash(bundle: &[u8]) -> String {
    zip::ZipArchive::new(Cursor::new(bundle))
        .ok()
        .and_then(|mut zr| read_manifest(&mut zr).ok())
        .map(|m| hash_of(&m))
        .unwrap_or_default()
}

/// Annotation (and practice dir) name the app uses for a score id.
pub fn safe_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_().".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Import a bundle: score files into `dir` (never overwriting, clashes get
/// `" (2)"`), annotations into `annotations_dir/<safe(id)>.json`, metadata,
/// settings and a new setlist into `store` (the caller saves the store).
/// All file hashes are verified before anything is written.
pub fn import_bundle(
    dir: &Path,
    store: &mut Store,
    annotations_dir: &Path,
    bytes: &[u8],
) -> Result<ImportedBundle, String> {
    let mut zr = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("bundle: {e}"))?;
    let m = read_manifest(&mut zr)?;
    let mut read = |f: &FileEntry| -> Result<Vec<u8>, String> {
        let mut b = Vec::new();
        zr.by_name(&f.path)
            .map_err(|_| format!("bundle is missing {}", f.path))?
            .read_to_end(&mut b)
            .map_err(|e| e.to_string())?;
        if sha256_hex(&b) != f.sha256 {
            return Err(format!(
                "{}: checksum mismatch (damaged or modified)",
                f.path
            ));
        }
        Ok(b)
    };
    let mut loaded = Vec::new();
    for s in &m.scores {
        let mut fs = Vec::new();
        for f in &s.files {
            let ext = ext_of(&f.path).ok_or_else(|| format!("{}: unsupported file", f.path))?;
            fs.push((ext, read(f)?));
        }
        let ann = s.annotations.as_ref().map(&mut read).transpose()?;
        let mut pr = Vec::new();
        for p in &s.practice {
            let ext = p
                .file
                .path
                .rsplit_once('.')
                .map(|(_, e)| e.to_lowercase())
                .filter(|e| PRACTICE_EXT.contains(&e.as_str()))
                .ok_or_else(|| format!("{}: unsupported practice track", p.file.path))?;
            pr.push((p.part.clone(), ext, read(&p.file)?));
        }
        loaded.push((s, fs, ann, pr));
    }

    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    let mut practice = Vec::new();
    for (s, fs, ann, pr) in loaded {
        let stem = free_stem(dir, &safe_stem(&format!("{}.x", s.name)));
        for (ext, data) in &fs {
            std::fs::write(dir.join(format!("{stem}.{ext}")), data).map_err(|e| e.to_string())?;
        }
        let id = format!("1:{stem}");
        if let Some(a) = ann {
            std::fs::create_dir_all(annotations_dir).map_err(|e| e.to_string())?;
            std::fs::write(annotations_dir.join(format!("{}.json", safe_id(&id))), a)
                .map_err(|e| e.to_string())?;
        }
        if let Some(md) = &s.metadata {
            store.set_metadata(&id, md.clone());
        }
        if let Some(st) = &s.settings {
            store.set_settings(&id, st.clone());
        }
        if let Some(ins) = &s.instructions {
            store.set_instructions(&id, ins.clone());
        }
        if !pr.is_empty() {
            let pdir = dir.join(PRACTICE_DIR).join(safe_id(&id));
            std::fs::create_dir_all(&pdir).map_err(|e| e.to_string())?;
            let mut tracks = Vec::new();
            for (part, ext, data) in pr {
                let file = format!(
                    "{}.{ext}",
                    free_stem(&pdir, &safe_stem(&format!("{part}.x")))
                );
                std::fs::write(pdir.join(&file), data).map_err(|e| e.to_string())?;
                tracks.push(PracticeTrack { part, file });
            }
            practice.push((id.clone(), tracks));
        }
        ids.push((s.name.clone(), id));
    }
    let items = m
        .setlist
        .items
        .iter()
        .map(|it| match it {
            SetItem::Score {
                score,
                start_page,
                note,
            } => match ids.iter().find(|(n, _)| n == score) {
                Some((_, id)) => SetItem::Score {
                    score: id.clone(),
                    start_page: *start_page,
                    note: note.clone(),
                },
                None => SetItem::Placeholder {
                    title: score.clone(),
                },
            },
            p => p.clone(),
        })
        .collect();
    let setlist_id = new_id();
    store.put_setlist(Setlist {
        id: setlist_id.clone(),
        name: m.setlist.name.clone(),
        items,
        instructions: m.setlist.instructions.clone(),
    });
    Ok(ImportedBundle {
        setlist_id,
        score_ids: ids.into_iter().map(|(_, id)| id).collect(),
        content_hash: hash_of(&m),
        version: m.version,
        practice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Preset;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cantor-bundle-{tag}-{}", new_id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample() -> (Setlist, Vec<BundleScore>) {
        let settings = ScoreSettings {
            presets: vec![Preset {
                name: "Tenor".into(),
                strips: vec![],
            }],
            ..Default::default()
        };
        let scores = vec![
            BundleScore {
                id: "1:Que".into(),
                title: "Que".into(),
                files: vec![
                    ("Que.mxl".into(), b"PK fake mxl".to_vec()),
                    ("Que.pdf".into(), b"%PDF-1.4 que".to_vec()),
                    ("Que.measures.json".into(), b"[]".to_vec()),
                ],
                annotations_json: Some(r#"{"strokes":[]}"#.into()),
                metadata: Some(Metadata {
                    title: "Que".into(),
                    composer: "Anon".into(),
                    ..Default::default()
                }),
                settings: Some(settings),
                ..Default::default()
            },
            BundleScore {
                id: "1:Ave".into(),
                title: "Ave".into(),
                files: vec![("Ave.pdf".into(), b"%PDF-1.4 ave".to_vec())],
                ..Default::default()
            },
        ];
        let sl = Setlist {
            id: "x".into(),
            name: "Concert".into(),
            items: vec![
                SetItem::Score {
                    score: "1:Ave".into(),
                    start_page: Some(2),
                    note: String::new(),
                },
                SetItem::Placeholder {
                    title: "Break".into(),
                },
                SetItem::Score {
                    score: "1:Que".into(),
                    start_page: None,
                    note: String::new(),
                },
            ],
            instructions: String::new(),
        };
        (sl, scores)
    }

    #[test]
    fn set_02_export_is_deterministic() {
        let (sl, sc) = sample();
        assert_eq!(
            export_bundle(&sl, &sc).unwrap(),
            export_bundle(&sl, &sc).unwrap()
        );
    }

    #[test]
    fn set_02_round_trip_same_content_hash() {
        let (sl, sc) = sample();
        let b = export_bundle(&sl, &sc).unwrap();
        let h = content_hash(&b);
        assert_eq!(h.len(), 64);
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let mut store = Store::open(&dir);
        let imp = import_bundle(&dir, &mut store, &ann, &b).unwrap();
        assert_eq!(imp.content_hash, h);
        assert_eq!(imp.score_ids, vec!["1:Que", "1:Ave"]);
        assert!(ann.join("1_Que.json").exists());
        assert_eq!(store.settings("1:Que").unwrap().presets[0].name, "Tenor");
        assert_eq!(store.metadata("1:Que").unwrap().composer, "Anon");
        let set = store.setlist(&imp.setlist_id).unwrap();
        assert_eq!(set.name, "Concert");
        assert_eq!(
            set.items[0],
            SetItem::Score {
                score: "1:Ave".into(),
                start_page: Some(2),
                note: String::new(),
            }
        );

        // Re-export from what landed on disk.
        let rebuilt: Vec<BundleScore> = imp
            .score_ids
            .iter()
            .map(|id| {
                let stem = id.strip_prefix("1:").unwrap();
                let files = FILE_EXT
                    .iter()
                    .filter_map(|e| {
                        let n = format!("{stem}.{e}");
                        std::fs::read(dir.join(&n)).ok().map(|b| (n, b))
                    })
                    .collect();
                BundleScore {
                    id: id.clone(),
                    title: stem.into(),
                    files,
                    annotations_json: std::fs::read_to_string(
                        ann.join(format!("{}.json", safe_id(id))),
                    )
                    .ok(),
                    metadata: store.metadata(id).cloned(),
                    settings: store.settings(id).cloned(),
                    ..Default::default()
                }
            })
            .collect();
        let b2 = export_bundle(set, &rebuilt).unwrap();
        assert_eq!(content_hash(&b2), h);
    }

    #[test]
    fn set_02_tampered_file_fails() {
        let (sl, sc) = sample();
        let b = export_bundle(&sl, &sc).unwrap();
        // Rebuild the zip with one file altered but the original manifest.
        let mut zr = zip::ZipArchive::new(Cursor::new(&b[..])).unwrap();
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..zr.len() {
            let mut f = zr.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            if name == "scores/Ave.pdf" {
                data.extend_from_slice(b" evil");
            }
            zw.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(&data).unwrap();
        }
        let bad = zw.finish().unwrap().into_inner();
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let mut store = Store::open(&dir);
        let e = import_bundle(&dir, &mut store, &ann, &bad).unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        assert!(store.setlists().is_empty());
    }

    #[test]
    fn set_02_name_clash_gets_suffix() {
        let (sl, sc) = sample();
        let b = export_bundle(&sl, &sc).unwrap();
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        std::fs::write(dir.join("Que.pdf"), b"%PDF existing").unwrap();
        let mut store = Store::open(&dir);
        let imp = import_bundle(&dir, &mut store, &ann, &b).unwrap();
        assert_eq!(imp.score_ids[0], "1:Que (2)");
        assert_eq!(
            std::fs::read(dir.join("Que.pdf")).unwrap(),
            b"%PDF existing"
        );
        assert!(dir.join("Que (2).mxl").exists());
        assert!(ann.join("1_Que (2).json").exists());
    }

    /// A director's set: instructions on the set and on a piece, part hints,
    /// a note on an item and two practice tracks.
    fn director_set() -> (Setlist, Vec<BundleScore>) {
        let (mut sl, mut sc) = sample();
        sl.instructions = "Konsert **18.00**. Se [noter](https://example.org).".into();
        if let SetItem::Score { note, .. } = &mut sl.items[2] {
            *note = "Börja i t. 9, hoppa över vers 2".into();
        }
        sc[0].instructions = Some(ScoreInstructions {
            text: "Tenorerna: andas före t. 12.".into(),
            parts: vec![
                PartHint {
                    who: "Anna".into(),
                    part: "Soprano".into(),
                },
                PartHint {
                    who: "Bo".into(),
                    part: "Tenor 1".into(),
                },
            ],
        });
        sc[0].practice = vec![
            (
                "Soprano".into(),
                "Que - Soprano.m4a".into(),
                b"m4a soprano".to_vec(),
            ),
            (
                "Tenor 1".into(),
                "Que - Tenor 1.m4a".into(),
                b"m4a tenor".to_vec(),
            ),
        ];
        (sl, sc)
    }

    use crate::store::PartHint;

    #[test]
    fn set_04_round_trip_keeps_instructions_hints_notes_and_tracks() {
        let (sl, sc) = director_set();
        let b = export_bundle(&sl, &sc).unwrap();
        assert_eq!(b, export_bundle(&sl, &sc).unwrap(), "deterministic");
        let h = content_hash(&b);
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let mut store = Store::open(&dir);
        store.set_member("anna");
        let imp = import_bundle(&dir, &mut store, &ann, &b).unwrap();
        assert_eq!(imp.version, BUNDLE_VERSION);
        assert_eq!(imp.content_hash, h);
        let set = store.setlist(&imp.setlist_id).unwrap().clone();
        assert_eq!(set.instructions, sl.instructions);
        assert!(
            matches!(&set.items[2], SetItem::Score { note, .. } if note == "Börja i t. 9, hoppa över vers 2")
        );
        let ins = store.instructions("1:Que").unwrap();
        assert_eq!(ins.text, "Tenorerna: andas före t. 12.");
        assert_eq!(store.member_part("1:Que"), Some("Soprano"));
        assert_eq!(imp.practice.len(), 1);
        let (id, tracks) = &imp.practice[0];
        assert_eq!(id, "1:Que");
        assert_eq!(
            tracks.iter().map(|t| t.part.as_str()).collect::<Vec<_>>(),
            vec!["Soprano", "Tenor 1"]
        );
        let pdir = dir.join(PRACTICE_DIR).join(safe_id(id));
        assert_eq!(
            std::fs::read(pdir.join(&tracks[1].file)).unwrap(),
            b"m4a tenor"
        );
        // Practice tracks are not scores in the library listing.
        let listed = crate::import::list_library(&[&dir]);
        assert_eq!(listed.len(), 2);

        // Re-export what landed: same content hash.
        let rebuilt: Vec<BundleScore> = imp
            .score_ids
            .iter()
            .map(|id| {
                let stem = id.strip_prefix("1:").unwrap();
                let files = FILE_EXT
                    .iter()
                    .filter_map(|e| {
                        let n = format!("{stem}.{e}");
                        std::fs::read(dir.join(&n)).ok().map(|b| (n, b))
                    })
                    .collect();
                let practice = imp
                    .practice
                    .iter()
                    .filter(|(p, _)| p == id)
                    .flat_map(|(_, ts)| ts.iter())
                    .map(|t| {
                        let pd = dir.join(PRACTICE_DIR).join(safe_id(id));
                        (
                            t.part.clone(),
                            t.file.clone(),
                            std::fs::read(pd.join(&t.file)).unwrap(),
                        )
                    })
                    .collect();
                BundleScore {
                    id: id.clone(),
                    title: stem.into(),
                    files,
                    annotations_json: std::fs::read_to_string(
                        ann.join(format!("{}.json", safe_id(id))),
                    )
                    .ok(),
                    metadata: store.metadata(id).cloned(),
                    settings: store.settings(id).cloned(),
                    instructions: store.instructions(id).cloned(),
                    practice,
                }
            })
            .collect();
        assert_eq!(content_hash(&export_bundle(&set, &rebuilt).unwrap()), h);
    }

    #[test]
    fn set_04_version_1_bundle_still_imports() {
        // A manifest exactly as version 1 wrote it: no instructions, notes,
        // hints or practice tracks.
        let pdf = b"%PDF-1.4 old";
        let manifest = format!(
            r#"{{"version":1,"setlist":{{"id":"","name":"Gammal","items":[{{"kind":"score","score":"Old","start_page":3}},{{"kind":"placeholder","title":"Paus"}}]}},"scores":[{{"name":"Old","title":"Old","metadata":null,"settings":null,"files":[{{"path":"scores/Old.pdf","sha256":"{}"}}],"annotations":null}}]}}"#,
            sha256_hex(pdf)
        );
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let o = zip::write::SimpleFileOptions::default();
        zw.start_file("scores/Old.pdf", o).unwrap();
        zw.write_all(pdf).unwrap();
        zw.start_file("manifest.json", o).unwrap();
        zw.write_all(manifest.as_bytes()).unwrap();
        let old = zw.finish().unwrap().into_inner();
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let mut store = Store::open(&dir);
        let imp = import_bundle(&dir, &mut store, &ann, &old).unwrap();
        assert_eq!(imp.version, 1);
        assert!(imp.practice.is_empty());
        let set = store.setlist(&imp.setlist_id).unwrap();
        assert_eq!(set.name, "Gammal");
        assert!(set.instructions.is_empty());
        assert_eq!(
            set.items[0],
            SetItem::Score {
                score: "1:Old".into(),
                start_page: Some(3),
                note: String::new(),
            }
        );
        assert!(store.instructions("1:Old").is_none());
        assert_eq!(std::fs::read(dir.join("Old.pdf")).unwrap(), pdf);
    }

    #[test]
    fn set_04_newer_bundle_is_refused_clearly() {
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zw.start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(br#"{"version":99,"setlist":{"id":"","name":"x","items":[]},"scores":[]}"#)
            .unwrap();
        let b = zw.finish().unwrap().into_inner();
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let e = import_bundle(&dir, &mut Store::open(&dir), &ann, &b).unwrap_err();
        assert!(e.contains("version 99"), "{e}");
    }

    #[test]
    fn set_04_tampered_practice_track_fails() {
        let (sl, sc) = director_set();
        let b = export_bundle(&sl, &sc).unwrap();
        let mut zr = zip::ZipArchive::new(Cursor::new(&b[..])).unwrap();
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..zr.len() {
            let mut f = zr.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            if name.starts_with("practice/") {
                data.push(b'!');
            }
            zw.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zw.write_all(&data).unwrap();
        }
        let bad = zw.finish().unwrap().into_inner();
        let (dir, ann) = (tmp("lib"), tmp("ann"));
        let e = import_bundle(&dir, &mut Store::open(&dir), &ann, &bad).unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert!(!dir.join(PRACTICE_DIR).exists());
    }

    #[test]
    fn set_04_unsupported_practice_format_is_rejected_on_export() {
        let (sl, mut sc) = director_set();
        sc[0].practice = vec![("S".into(), "S.exe".into(), vec![1])];
        assert!(export_bundle(&sl, &sc).unwrap_err().contains("practice"));
    }
}
