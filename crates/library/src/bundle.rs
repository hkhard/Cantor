//! Setlist bundles (SET-02): share a setlist with its scores, annotations and
//! per-score settings (incl. mixer presets) as one `.cantorset` zip.
//!
//! Layout: `manifest.json`, `scores/<name>.<ext>` (`mxl`, `musicxml`, `xml`,
//! `pdf`, `measures.json`) and `annotations/<name>.json`. `<name>` is a
//! bundle-local name derived from the score title; the manifest's setlist
//! refers to scores by that name. Every file carries its sha256 in the
//! manifest. Zip entries use a fixed timestamp so exports are deterministic,
//! and [`content_hash`] only covers file hashes, so it survives re-zipping.

use crate::import::{free_stem, safe_stem};
use crate::store::{new_id, Metadata, ScoreSettings, SetItem, Setlist, Store};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::path::Path;

/// Bundle file extension (without dot).
pub const BUNDLE_EXT: &str = "cantorset";
/// Manifest format version.
pub const BUNDLE_VERSION: u32 = 1;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedBundle {
    pub setlist_id: String,
    /// New score ids (`"1:<stem>"`) in manifest order.
    pub score_ids: Vec<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
    /// Path inside the zip.
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoreEntry {
    name: String,
    title: String,
    metadata: Option<Metadata>,
    settings: Option<ScoreSettings>,
    files: Vec<FileEntry>,
    annotations: Option<FileEntry>,
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
        scores.push((
            s.id.clone(),
            ScoreEntry {
                name,
                title: s.title.clone(),
                metadata: s.metadata.clone(),
                settings: s.settings.clone(),
                files: entries,
                annotations,
            },
        ));
    }

    let items = setlist
        .items
        .iter()
        .map(|it| match it {
            SetItem::Score { score, start_page } => {
                match scores.iter().find(|(id, _)| id == score) {
                    Some((_, e)) => SetItem::Score {
                        score: e.name.clone(),
                        start_page: *start_page,
                    },
                    None => SetItem::Placeholder {
                        title: score.clone(),
                    },
                }
            }
            p => p.clone(),
        })
        .collect();
    let manifest = Manifest {
        version: BUNDLE_VERSION,
        setlist: Setlist {
            id: String::new(),
            name: setlist.name.clone(),
            items,
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
        .flat_map(|s| s.files.iter().chain(&s.annotations))
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

/// Annotation file name the app uses for a score id.
fn safe_id(id: &str) -> String {
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
        loaded.push((s, fs, ann));
    }

    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    for (s, fs, ann) in loaded {
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
        ids.push((s.name.clone(), id));
    }
    let items = m
        .setlist
        .items
        .iter()
        .map(|it| match it {
            SetItem::Score { score, start_page } => match ids.iter().find(|(n, _)| n == score) {
                Some((_, id)) => SetItem::Score {
                    score: id.clone(),
                    start_page: *start_page,
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
    });
    Ok(ImportedBundle {
        setlist_id,
        score_ids: ids.into_iter().map(|(_, id)| id).collect(),
        content_hash: hash_of(&m),
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
                },
                SetItem::Placeholder {
                    title: "Break".into(),
                },
                SetItem::Score {
                    score: "1:Que".into(),
                    start_page: None,
                },
            ],
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
                start_page: Some(2)
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
}
