//! Full library backup and restore (DATA-03).
//!
//! A backup is one `.cantorbackup` zip of the user library directory:
//! scores (`.mxl`, `.musicxml`, `.xml`), PDFs, measure maps, the library store
//! (`library.json`: metadata, setlists, per-score settings and mixes),
//! annotations (`Annotations/`) and kept forScore originals (`.forscore/`).
//! Not included, because they are not library content: exports (`Export/`),
//! the share-sheet inbox (`Inbox/`), received backups waiting (`Backups/`),
//! sync state and the chosen sync folder (`.sync/`), already-imported originals
//! (`.imported/`), temporary files (`.incoming/`, `*.tmp`) and earlier
//! restore leftovers (`.before-restore-*`).
//!
//! `manifest.json` lists every file with its sha256. [`library_hash`] is the
//! sha256 over the sorted `(path, file hash)` pairs, so "restore yields
//! identical library hash" is checkable. Restore verifies every hash before
//! touching anything, then moves the current library aside into
//! `.before-restore-<time>/` (never deletes it) and writes the backup.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};

/// File extension of a backup (without dot).
pub const BACKUP_EXT: &str = "cantorbackup";
/// Manifest format version.
pub const BACKUP_VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

/// Top-level entries of the library dir that are not library content.
const EXCLUDED_DIRS: &[&str] = &[
    "Export",
    "Inbox",
    "Backups",
    ".imported",
    ".incoming",
    ".sync",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Manifest {
    version: u32,
    /// `(relative path with '/', sha256 hex)`, sorted by path.
    files: Vec<(String, String)>,
    library_hash: String,
}

/// What [`restore`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    /// Number of files restored.
    pub files: usize,
    /// Where the previous library was moved, if there was one.
    pub previous: Option<PathBuf>,
    pub library_hash: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn excluded(rel: &str) -> bool {
    let top = rel.split('/').next().unwrap_or("");
    EXCLUDED_DIRS.contains(&top)
        || top.starts_with(".before-restore-")
        || rel.ends_with(".tmp")
        || rel == ".perf.log"
        || rel.split('/').any(|p| p == ".DS_Store")
}

/// Library files under `dir` as sorted relative paths with '/' separators.
fn library_files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let rd = match std::fs::read_dir(d) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", d.display())),
        };
        for e in rd.flatten() {
            let p = e.path();
            let rel = p
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            if excluded(&rel) {
                continue;
            }
            let ft = e.file_type().map_err(|e| e.to_string())?;
            if ft.is_dir() {
                walk(base, &p, out)?;
            } else if ft.is_file() {
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn hash_of(files: &[(String, String)]) -> String {
    let mut h = Sha256::new();
    for (p, f) in files {
        h.update(p.as_bytes());
        h.update([0]);
        h.update(f.as_bytes());
        h.update(b"\n");
    }
    hex(&h.finalize())
}

/// Content hash of the library in `dir` (paths and file contents; exports,
/// inbox and temporary files excluded).
pub fn library_hash(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for rel in library_files(dir)? {
        let data = std::fs::read(dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        files.push((rel, sha(&data)));
    }
    Ok(hash_of(&files))
}

/// Make a backup of the library in `dir`.
pub fn backup(dir: &Path) -> Result<Vec<u8>, String> {
    // Stored, not deflated: PDFs and MXL are already compressed, and a stored
    // archive is fast to write on the device.
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default())
        .large_file(true);
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut files = Vec::new();
    for rel in library_files(dir)? {
        let data = std::fs::read(dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        files.push((rel.clone(), sha(&data)));
        zw.start_file(format!("library/{rel}"), opts)
            .map_err(|e| e.to_string())?;
        zw.write_all(&data).map_err(|e| e.to_string())?;
    }
    let manifest = Manifest {
        version: BACKUP_VERSION,
        library_hash: hash_of(&files),
        files,
    };
    zw.start_file(MANIFEST, opts).map_err(|e| e.to_string())?;
    zw.write_all(
        serde_json::to_string_pretty(&manifest)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    Ok(zw.finish().map_err(|e| e.to_string())?.into_inner())
}

/// A relative path that stays inside the library.
fn safe_rel(rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if rel.is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("backup: unsafe path {rel:?}"));
    }
    Ok(p.to_path_buf())
}

/// Files of a backup: `(relative path, bytes)`.
type BackupFiles = Vec<(PathBuf, Vec<u8>)>;

/// Read and verify a backup: `(relative path, bytes)` for every file.
fn read_backup(bytes: &[u8]) -> Result<(Manifest, BackupFiles), String> {
    let mut zr = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a Cantor backup: {e}"))?;
    let manifest: Manifest = {
        let mut f = zr
            .by_name(MANIFEST)
            .map_err(|_| "not a Cantor backup: no manifest".to_string())?;
        let mut s = String::new();
        f.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("backup manifest: {e}"))?
    };
    if manifest.version > BACKUP_VERSION {
        return Err(format!(
            "backup made by a newer Cantor (format {}), update the app",
            manifest.version
        ));
    }
    let mut out = Vec::new();
    for (rel, want) in &manifest.files {
        let path = safe_rel(rel)?;
        let mut f = zr
            .by_name(&format!("library/{rel}"))
            .map_err(|_| format!("backup is missing {rel}"))?;
        let data = crate::import::read_limited(&mut f, rel)?;
        if &sha(&data) != want {
            return Err(format!(
                "backup is damaged: {rel} does not match its checksum"
            ));
        }
        out.push((path, data));
    }
    if hash_of(&manifest.files) != manifest.library_hash {
        return Err("backup is damaged: manifest checksum".into());
    }
    Ok((manifest, out))
}

/// Restore a backup into the library dir `dir`. The current library files
/// are moved to `dir/.before-restore-<unix time>/` first (exports and inbox
/// stay). Nothing is changed if the backup is invalid.
pub fn restore(dir: &Path, bytes: &[u8]) -> Result<RestoreReport, String> {
    let (manifest, files) = read_backup(bytes)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let current = library_files(dir)?;
    let previous = if current.is_empty() {
        None
    } else {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut aside = dir.join(format!(".before-restore-{stamp}"));
        let mut n = 1;
        while aside.exists() {
            n += 1;
            aside = dir.join(format!(".before-restore-{stamp}-{n}"));
        }
        for rel in &current {
            let to = aside.join(rel);
            if let Some(p) = to.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            std::fs::rename(dir.join(rel), &to).map_err(|e| format!("{rel}: {e}"))?;
        }
        Some(aside)
    };
    for (rel, data) in &files {
        let to = dir.join(rel);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        std::fs::write(&to, data).map_err(|e| format!("{}: {e}", rel.display()))?;
    }
    Ok(RestoreReport {
        files: files.len(),
        previous,
        library_hash: manifest.library_hash,
    })
}

/// Fixture builders for tests.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// A backup whose manifest lists one file at `rel` (any path, even an
    /// unsafe one) with a correct checksum.
    pub fn backup_with_entry(rel: &str, data: &[u8]) -> Vec<u8> {
        let opts = zip::write::SimpleFileOptions::default();
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zw.start_file(format!("library/{rel}"), opts).unwrap();
        zw.write_all(data).unwrap();
        let files = vec![(rel.to_string(), sha(data))];
        let m = Manifest {
            version: BACKUP_VERSION,
            library_hash: hash_of(&files),
            files,
        };
        zw.start_file(MANIFEST, opts).unwrap();
        zw.write_all(serde_json::to_string(&m).unwrap().as_bytes())
            .unwrap();
        zw.finish().unwrap().into_inner()
    }
}
