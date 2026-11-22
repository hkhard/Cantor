//! DATA-03: full backup / restore file. "Restore yields identical library hash".
//! A backup is one `.cantorbackup` zip of the user library: scores, PDFs,
//! measure maps, the library store (metadata, setlists, settings, mixes),
//! annotations and kept forScore originals. Exports and the share-sheet inbox
//! are not part of the library and are left out.
use cantor_library::backup::{backup, library_hash, restore, BACKUP_EXT};
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantor-bak-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(dir: &Path, rel: &str, bytes: &[u8]) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

/// A library like the app's Documents folder.
fn library(dir: &Path) {
    put(dir, "Que.mxl", b"PK\x03\x04 fake mxl");
    put(dir, "Que.pdf", b"%PDF-1.4 que");
    put(dir, "Que.measures.json", b"[]");
    put(dir, "Olaf.pdf", b"%PDF-1.4 olaf");
    put(
        dir,
        "library.json",
        br#"{"version":1,"metadata":{"1:Que":{"title":"Que?"}}}"#,
    );
    put(
        dir,
        "Annotations/1_Que.json",
        br#"{"version":1,"layers":[],"items":[],"next_id":1}"#,
    );
    put(dir, ".forscore/Olaf.4sc", b"\x1f\x8b fake");
    // Not part of the library:
    put(dir, "Export/Que.wav", b"RIFF");
    put(dir, "Inbox/new.mxl", b"x");
    put(dir, "Backups/older.cantorbackup", b"PK");
    put(dir, ".sync/state.json", b"{}");
    put(dir, ".imported/old.4sc", b"x");
    put(dir, ".incoming/tmp.4ss", b"x");
}

#[test]
fn data_03_backup_then_restore_gives_identical_library_hash() {
    let src = tmp("src");
    library(&src);
    let h = library_hash(&src).unwrap();
    let file = backup(&src).unwrap();
    let dst = tmp("dst");
    restore(&dst, &file).unwrap();
    assert_eq!(library_hash(&dst).unwrap(), h);
    // Exactly the library files, nothing else.
    assert!(dst.join("Annotations/1_Que.json").exists());
    assert!(dst.join(".forscore/Olaf.4sc").exists());
    assert!(!dst.join("Export").exists() && !dst.join("Inbox").exists());
    assert!(!dst.join("Backups").exists(), "no backups inside backups");
    assert!(!dst.join(".sync").exists(), "sync state is per device");
    assert!(BACKUP_EXT == "cantorbackup");
}

#[test]
fn data_03_hash_ignores_exports_and_changes_with_content() {
    let a = tmp("ha");
    library(&a);
    let h = library_hash(&a).unwrap();
    put(&a, "Export/other.wav", b"RIFF2");
    assert_eq!(library_hash(&a).unwrap(), h, "exports are not library");
    put(&a, "Annotations/1_Que.json", b"{}");
    assert_ne!(
        library_hash(&a).unwrap(),
        h,
        "an edited annotation changes the hash"
    );
}

#[test]
fn data_03_restore_replaces_library_and_keeps_previous_aside() {
    let src = tmp("rs");
    library(&src);
    let file = backup(&src).unwrap();
    let dst = tmp("rd");
    put(&dst, "Mine.pdf", b"%PDF-1.4 mine");
    put(&dst, "Export/keep.wav", b"RIFF");
    let report = restore(&dst, &file).unwrap();
    assert_eq!(library_hash(&dst).unwrap(), library_hash(&src).unwrap());
    // The old library is moved aside, not deleted.
    let aside = report.previous.expect("previous library moved aside");
    assert!(aside.join("Mine.pdf").exists());
    // Exports stay where they are.
    assert!(dst.join("Export/keep.wav").exists());
}

#[test]
fn data_03_corrupt_or_tampered_backup_is_rejected_without_touching_library() {
    let src = tmp("ts");
    library(&src);
    let mut file = backup(&src).unwrap();
    let dst = tmp("td");
    put(&dst, "Mine.pdf", b"%PDF-1.4 mine");
    let before = library_hash(&dst).unwrap();
    assert!(restore(&dst, b"not a zip").is_err());
    // Flip a byte inside a stored file: the manifest hash no longer matches.
    let i = file.windows(12).position(|w| w == b"%PDF-1.4 que").unwrap();
    file[i + 10] = b'X';
    assert!(restore(&dst, &file).is_err());
    assert_eq!(library_hash(&dst).unwrap(), before);
}

#[test]
fn data_03_paths_cannot_escape_the_library() {
    // A crafted backup with "../evil" must not write outside the library.
    let evil = cantor_library::backup::testing::backup_with_entry("../evil.txt", b"x");
    let dst = tmp("ev");
    assert!(restore(&dst, &evil).is_err());
    assert!(!dst.parent().unwrap().join("evil.txt").exists());
}
