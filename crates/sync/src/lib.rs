//! Sync through a user-chosen folder (DATA-02): iCloud Drive, OneDrive,
//! Google Drive, a USB stick, anything that shows up as a directory.
//!
//! # Layout
//!
//! Every device keeps its own library dir (as without sync). The shared
//! folder holds
//!
//! * `cantor-sync/devices/<device-id>.jsonl`: one append-only operation log
//!   per device. Only that device ever writes its file, so cloud drives never
//!   see two writers on one file (no "conflicted copy" files).
//! * `cantor-sync/blobs/<sha256>`: content-addressed score files (`.mxl`,
//!   `.musicxml`, PDFs, `*.measures.json`, kept forScore originals...). Written
//!   via a temp file plus rename, so a reader never sees half a blob; readers
//!   also verify the hash and simply retry on the next sync if a blob has not
//!   arrived yet.
//!
//! Per library, `<library>/.sync/state.json` remembers the last synced local
//! state (to diff against), the mapping of local annotation ids to sync ids,
//! the clock and a file-hash cache. `.sync/` is per device and must be
//! excluded from backups (DATA-03).
//!
//! # Operations and clock
//!
//! Each log line is one operation stamped with a hybrid logical clock
//! `(wall ms, counter, device id)`; the triple is a total order. A device's
//! clock never goes below anything it has seen, so an edit made after a sync
//! is ordered after everything that sync brought in, whatever the wall
//! clocks say. Concurrent edits are ordered by wall time (then device id).
//!
//! # Merge rules (CRDT)
//!
//! Every sync re-reads all logs and folds them into one state; the fold is
//! commutative, associative and idempotent, so all devices that have read
//! the same logs materialise byte-identical libraries.
//!
//! * **Library files** (everything except `library.json`, `Annotations/*.json`
//!   and non-content dirs such as `Export/`, `Inbox/`, `Backups/`, `.sync/`):
//!   one last-writer-wins register per relative path holding the blob hash; a
//!   delete is a tombstone in the same register.
//! * **`library.json`**: LWW registers per key. `metadata` per score id and
//!   `settings` per score id are each one register (a concurrent title edit
//!   and tag edit on the same score do not merge field by field: the later
//!   whole record wins). A setlist is two registers: its head (name and any
//!   other fields) and its item list (order included, LWW as a whole). Setlist
//!   order in the file is by creation. Deletes are tombstones; delete vs edit
//!   is decided per register by the clock. Instructions (SET-03) are one
//!   register per score id (`instructions`) and one per setlist (apart
//!   from its name and items). Deleting a score on one device
//!   while another edits its metadata later leaves that metadata record
//!   (an orphan without files) on both devices; when the delete is later the
//!   score is gone everywhere. Other top-level keys are one register each.
//! * **Annotations** (`Annotations/<name>.json`, a [`cantor_annotations::Document`]):
//!   annotations form an observed-remove set keyed by a globally unique sync
//!   id (`<ms>.<counter>.<device>` of the add). Edits (move, restyle) are a LWW
//!   register per annotation; removing wins over a concurrent edit and edits
//!   never bring an annotation back. Items are ordered by creation. Local
//!   `u64` ids are assigned deterministically: in creation order every
//!   annotation keeps the id it had on its origin device unless an earlier one
//!   took it, then the rest get fresh ids above the maximum; `next_id` is one
//!   above all ids ever assigned. So ids can change after a sync (only on
//!   collisions); reload the document after syncing. Layers are LWW per
//!   index. A deleted document (score removed) removes the annotations and
//!   layers it had seen; annotations added concurrently elsewhere survive and
//!   bring the document back with default layers.
//!
//! # Limitations
//!
//! * Logs are re-read in full each sync and never compacted.
//! * A crash between appending to the log and saving `state.json` makes the
//!   next sync re-send the same changes: harmless for files and store keys,
//!   but annotations added in that window are duplicated.
//! * The first sync of a library that already has annotations treats them
//!   all as new, so the same document annotated on two unsynced devices ends
//!   up with both copies.

use cantor_annotations::{Document, DOCUMENT_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Directory created inside the user-chosen shared folder.
pub const SYNC_DIR: &str = "cantor-sync";
/// Per-device sync state directory inside the library dir (exclude from backups).
pub const STATE_DIR: &str = ".sync";
const STATE_FILE: &str = "state.json";
const STATE_VERSION: u32 = 1;
const STORE_FILE: &str = "library.json";
const ANN_DIR: &str = "Annotations";
/// Top-level library entries that are not library content (as in backup.rs, plus `.sync`).
const EXCLUDED_DIRS: &[&str] = &[
    "Export",
    "Inbox",
    "Backups",
    ".imported",
    ".incoming",
    STATE_DIR,
];
const MAX_LAYERS: usize = 4096;

type Key = Vec<String>;

fn key(kind: &str, parts: &[&str]) -> Key {
    std::iter::once(kind)
        .chain(parts.iter().copied())
        .map(String::from)
        .collect()
}

/// What one [`SyncFolder::sync`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Operations appended to this device's log (local changes).
    pub sent: usize,
    /// Operations from other devices read for the first time.
    pub received: usize,
    /// Library files written (added or changed by others).
    pub files_written: usize,
    /// Library files deleted (deleted by others).
    pub files_removed: usize,
    /// Annotation documents written.
    pub docs_written: usize,
    /// Annotation documents deleted.
    pub docs_removed: usize,
    /// Whether `library.json` was rewritten.
    pub store_written: bool,
    /// Files whose blob has not arrived in the shared folder yet (retried next sync).
    pub pending: usize,
}

/// A user-chosen shared folder, seen from one device.
pub struct SyncFolder {
    root: PathBuf,
    device: String,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for SyncFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncFolder")
            .field("root", &self.root)
            .field("device", &self.device)
            .finish()
    }
}

/// Device ids: 1..=64 of `[A-Za-z0-9._-]`, not starting with a dot.
pub fn valid_device_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn err(ctx: impl std::fmt::Display, e: impl std::fmt::Display) -> String {
    format!("{ctx}: {e}")
}

fn sha(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn is_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

impl SyncFolder {
    /// Use `shared/cantor-sync/` (created if missing) as device `device_id`.
    pub fn open(shared: &Path, device_id: &str) -> Result<SyncFolder, String> {
        if !valid_device_id(device_id) {
            return Err(format!("invalid device id {device_id:?}"));
        }
        let root = shared.join(SYNC_DIR);
        for d in ["devices", "blobs"] {
            std::fs::create_dir_all(root.join(d)).map_err(|e| err(root.join(d).display(), e))?;
        }
        Ok(SyncFolder {
            root,
            device: device_id.to_string(),
            clock: Box::new(wall_ms),
        })
    }

    /// Replace the wall clock (milliseconds), e.g. for tests.
    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    pub fn device_id(&self) -> &str {
        &self.device
    }

    /// This device's operation log.
    pub fn log_path(&self) -> PathBuf {
        self.root
            .join("devices")
            .join(format!("{}.jsonl", self.device))
    }

    /// Forget the sync state of a library (e.g. after it was copied from
    /// another device). The next sync treats everything in it as new.
    pub fn reset_state(library_dir: &Path) -> Result<(), String> {
        match std::fs::remove_dir_all(library_dir.join(STATE_DIR)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    /// Sync the library in `library_dir`: send local changes since the last
    /// sync, then merge every device's log and write the merged library.
    /// Idempotent; safe while other devices sync against the same folder.
    pub fn sync(&self, library_dir: &Path) -> Result<SyncReport, String> {
        let lib = library_dir;
        let state_path = lib.join(STATE_DIR).join(STATE_FILE);
        let mut state = match std::fs::read_to_string(&state_path) {
            Ok(s) => serde_json::from_str::<State>(&s).map_err(|e| err("sync state", e))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => return Err(err(state_path.display(), e)),
        };
        if state.version > STATE_VERSION {
            return Err("sync state written by a newer Cantor".into());
        }
        if !state.device.is_empty() && state.device != self.device {
            return Err(format!(
                "this library was synced as device {:?}, not {:?}",
                state.device, self.device
            ));
        }
        let mut report = SyncReport::default();
        let scan = self.scan(lib, &state)?;
        let ops = self.diff(&scan, &mut state);
        report.sent = ops.len();
        self.append(&ops)?;
        let (merged, seen) = self.read_logs()?;
        report.received = seen
            .iter()
            .filter(|(d, _)| **d != self.device)
            .map(|(d, n)| n.saturating_sub(state.seen.get(d).copied().unwrap_or(0)))
            .sum();
        self.materialise(lib, &scan, &merged, &mut state, &mut report)?;
        if let Some(max) = &merged.max {
            if (max.ms, max.counter) > (state.clock.ms, state.clock.c) {
                state.clock = Stamp {
                    ms: max.ms,
                    c: max.counter,
                };
            }
        }
        state.seen = seen;
        state.device = self.device.clone();
        state.version = STATE_VERSION;
        let json = serde_json::to_vec(&state).map_err(|e| err("sync state", e))?;
        write_atomic(lib, &state_path, &json)?;
        Ok(report)
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        self.root.join("blobs").join(hash)
    }

    fn put_blob(&self, data: &[u8]) -> Result<String, String> {
        let hash = sha(data);
        let to = self.blob_path(&hash);
        if !to.is_file() {
            static N: AtomicU64 = AtomicU64::new(0);
            let tmp = self.root.join("blobs").join(format!(
                ".tmp-{}-{}-{}-{hash}",
                self.device,
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::write(&tmp, data).map_err(|e| err(tmp.display(), e))?;
            std::fs::rename(&tmp, &to).map_err(|e| err(to.display(), e))?;
        }
        Ok(hash)
    }

    fn get_blob(&self, hash: &str) -> Option<Vec<u8>> {
        let data = std::fs::read(self.blob_path(hash)).ok()?;
        (sha(&data) == hash).then_some(data)
    }

    /// Read the library: file hashes (writing blobs of changed files), the
    /// store and the annotation documents.
    fn scan(&self, lib: &Path, state: &State) -> Result<Scan, String> {
        let mut scan = Scan::default();
        let mut rels = Vec::new();
        walk(lib, lib, &mut rels)?;
        for rel in rels {
            let path = lib.join(&rel);
            if rel == STORE_FILE {
                let bytes = std::fs::read(&path).map_err(|e| err(&rel, e))?;
                scan.store = match serde_json::from_slice::<Value>(&bytes) {
                    Ok(Value::Object(o)) => StoreScan::Ok(o, bytes),
                    _ => StoreScan::Unreadable,
                };
            } else if let Some(name) = doc_name(&rel) {
                let bytes = std::fs::read(&path).map_err(|e| err(&rel, e))?;
                let doc = match std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| Document::from_json(s).ok())
                {
                    Some(d) => LocalDoc::Doc(doc_parts(&d), bytes),
                    None => LocalDoc::Unreadable,
                };
                scan.docs.insert(name.to_string(), doc);
            } else {
                let md = std::fs::metadata(&path).map_err(|e| err(&rel, e))?;
                let (len, mtime) = (md.len(), mtime_ns(&md));
                let synced = state
                    .regs
                    .get(&key("file", &[&rel]))
                    .and_then(Value::as_str);
                let cached = state.files.get(&rel).filter(|c| {
                    c.len == len
                        && c.mtime == mtime
                        && mtime.saturating_add(2_000_000_000) < state.cached_at
                });
                let hash = match cached {
                    Some(c) if synced == Some(c.hash.as_str()) => c.hash.clone(),
                    _ => {
                        let data = std::fs::read(&path).map_err(|e| err(&rel, e))?;
                        let h = sha(&data);
                        if synced != Some(h.as_str()) {
                            self.put_blob(&data)?;
                        }
                        h
                    }
                };
                scan.files.insert(rel, hash);
            }
        }
        Ok(scan)
    }

    /// Operations for local changes since the last sync. Updates the
    /// annotation id mapping in `state` for new annotations.
    fn diff(&self, scan: &Scan, state: &mut State) -> Vec<(Hlc, Op)> {
        let mut clock = Clock {
            last: state.clock,
            device: self.device.clone(),
            wall: (self.clock)(),
        };
        let mut ops = Vec::new();
        let mut local: BTreeMap<Key, Value> = BTreeMap::new();
        for (rel, h) in &scan.files {
            local.insert(key("file", &[rel]), Value::String(h.clone()));
        }
        match &scan.store {
            StoreScan::Ok(o, _) => store_view(o, &mut local),
            StoreScan::Missing | StoreScan::Unreadable => {
                for (k, v) in &state.regs {
                    if is_store_key(k) {
                        local.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        let names: BTreeSet<String> = scan.docs.keys().chain(state.docs.keys()).cloned().collect();
        for n in names {
            let old = state.docs.get(&n).cloned().unwrap_or_default();
            let mut mapping = BTreeMap::new();
            match scan.docs.get(&n) {
                Some(LocalDoc::Unreadable) => {
                    for (k, v) in &state.regs {
                        if is_doc_key(k, &n) {
                            local.insert(k.clone(), v.clone());
                        }
                    }
                    mapping = old.clone();
                }
                Some(LocalDoc::Doc(parts, _)) => {
                    local.insert(key("doc", &[&n]), Value::Bool(true));
                    for (i, l) in parts.layers.iter().enumerate() {
                        local.insert(key("layer", &[&n, &i.to_string()]), l.clone());
                    }
                    let mut used = BTreeSet::new();
                    for (id, body) in &parts.items {
                        let sid = match old.get(id) {
                            Some(s) if !used.contains(s) => s.clone(),
                            _ => {
                                let h = clock.tick();
                                let sid = format!("{}.{}.{}", h.ms, h.counter, h.device);
                                ops.push((
                                    h,
                                    Op::AnnAdd {
                                        doc: n.clone(),
                                        sid: sid.clone(),
                                        pref: *id,
                                    },
                                ));
                                sid
                            }
                        };
                        used.insert(sid.clone());
                        local.insert(key("ann", &[&n, &sid]), body.clone());
                        mapping.insert(*id, sid);
                    }
                }
                None => {}
            }
            let kept: BTreeSet<&String> = mapping.values().collect();
            let removed: Vec<String> = old
                .values()
                .filter(|s| !kept.contains(s))
                .cloned()
                .collect();
            if !removed.is_empty() {
                ops.push((
                    clock.tick(),
                    Op::AnnRemove {
                        doc: n.clone(),
                        sids: removed,
                    },
                ));
            }
            if mapping.is_empty() {
                state.docs.remove(&n);
            } else {
                state.docs.insert(n, mapping);
            }
        }
        let keys: BTreeSet<&Key> = local.keys().chain(state.regs.keys()).collect();
        for k in keys {
            let (now, was) = (local.get(k), state.regs.get(k));
            if now == was || (k[0] == "ann" && now.is_none()) {
                continue;
            }
            ops.push((
                clock.tick(),
                Op::Set {
                    key: k.clone(),
                    value: now.cloned(),
                },
            ));
        }
        state.clock = clock.last;
        ops
    }

    fn append(&self, ops: &[(Hlc, Op)]) -> Result<(), String> {
        if ops.is_empty() {
            return Ok(());
        }
        let path = self.log_path();
        let mut buf = Vec::new();
        // Never glue onto a line cut short by an earlier crash.
        if std::fs::read(&path).is_ok_and(|b| b.last().is_some_and(|c| *c != b'\n')) {
            buf.push(b'\n');
        }
        for (h, op) in ops {
            let line = Line {
                t: h.ms,
                c: h.counter,
                d: h.device.clone(),
                op: op.clone(),
            };
            serde_json::to_writer(&mut buf, &line).map_err(|e| err("sync log", e))?;
            buf.push(b'\n');
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| err(path.display(), e))?;
        f.write_all(&buf).map_err(|e| err(path.display(), e))?;
        f.sync_data().map_err(|e| err(path.display(), e))?;
        Ok(())
    }

    /// Fold every device log. Also returns the number of lines per device.
    fn read_logs(&self) -> Result<(Merged, BTreeMap<String, usize>), String> {
        let dir = self.root.join("devices");
        let mut merged = Merged::default();
        let mut seen = BTreeMap::new();
        let rd = std::fs::read_dir(&dir).map_err(|e| err(dir.display(), e))?;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(dev) = name.strip_suffix(".jsonl").filter(|d| valid_device_id(d)) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(e.path()) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            let mut n = 0;
            // A last line without newline may still be being written: skip it.
            for line in text.split_inclusive('\n').filter(|l| l.ends_with('\n')) {
                n += 1;
                if let Ok(l) = serde_json::from_str::<Line>(line) {
                    if valid_device_id(&l.d) && l.op.is_valid() {
                        let h = Hlc {
                            ms: l.t,
                            counter: l.c,
                            device: l.d,
                        };
                        merged.apply(h, l.op);
                    }
                }
            }
            seen.insert(dev.to_string(), n);
        }
        Ok((merged, seen))
    }

    /// Write the merged state into the library; record what is now local in `state`.
    fn materialise(
        &self,
        lib: &Path,
        scan: &Scan,
        merged: &Merged,
        state: &mut State,
        report: &mut SyncReport,
    ) -> Result<(), String> {
        let mut view: BTreeMap<Key, Value> = BTreeMap::new();
        // Files.
        let mut files = scan.files.clone();
        for (k, (_, v)) in &merged.regs {
            if k[0] != "file" {
                continue;
            }
            let rel = &k[1];
            let path = lib.join(rel);
            match v.as_ref().and_then(Value::as_str) {
                Some(h) if files.get(rel).map(String::as_str) == Some(h) => {}
                Some(h) => match self.get_blob(h) {
                    Some(data) => {
                        write_atomic(lib, &path, &data)?;
                        files.insert(rel.clone(), h.to_string());
                        report.files_written += 1;
                    }
                    None => report.pending += 1,
                },
                None => {
                    if files.remove(rel).is_some() {
                        std::fs::remove_file(&path).map_err(|e| err(rel, e))?;
                        report.files_removed += 1;
                    }
                }
            }
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        state.files.clear();
        for (rel, h) in &files {
            view.insert(key("file", &[rel]), Value::String(h.clone()));
            if let Ok(md) = std::fs::metadata(lib.join(rel)) {
                state.files.insert(
                    rel.clone(),
                    Cached {
                        len: md.len(),
                        mtime: mtime_ns(&md),
                        hash: h.clone(),
                    },
                );
            }
        }
        state.cached_at = now;

        // Store.
        match (&scan.store, store_object(merged)) {
            (StoreScan::Unreadable, _) => {
                for (k, v) in &state.regs {
                    if is_store_key(k) {
                        view.insert(k.clone(), v.clone());
                    }
                }
            }
            (local, Some(obj)) => {
                let mut bytes = serde_json::to_vec_pretty(&Value::Object(obj.clone()))
                    .map_err(|e| err(STORE_FILE, e))?;
                bytes.push(b'\n');
                if !matches!(local, StoreScan::Ok(_, b) if *b == bytes) {
                    write_atomic(lib, &lib.join(STORE_FILE), &bytes)?;
                    report.store_written = true;
                }
                store_view(&obj, &mut view);
            }
            (StoreScan::Ok(o, _), None) => store_view(o, &mut view),
            (StoreScan::Missing, None) => {}
        }

        // Annotation documents.
        let mut names: BTreeSet<&String> = merged.anns.keys().collect();
        for k in merged.regs.keys() {
            if matches!(k[0].as_str(), "doc" | "layer" | "ann") {
                names.insert(&k[1]);
            }
        }
        // Local documents are always in the logs after the diff; keep any
        // stragglers as they are.
        names.extend(scan.docs.keys());
        let mut docs = BTreeMap::new();
        for n in names {
            let path = lib.join(ANN_DIR).join(format!("{n}.json"));
            let local = scan.docs.get(n);
            let keep_local = |view: &mut BTreeMap<Key, Value>, docs: &mut BTreeMap<_, _>| {
                for (k, v) in &state.regs {
                    if is_doc_key(k, n) {
                        view.insert(k.clone(), v.clone());
                    }
                }
                if let Some(m) = state.docs.get(n) {
                    docs.insert(n.clone(), m.clone());
                }
            };
            if matches!(local, Some(LocalDoc::Unreadable)) {
                keep_local(&mut view, &mut docs);
                continue;
            }
            match build_doc(n, merged) {
                Built::Absent => {
                    if local.is_some() {
                        std::fs::remove_file(&path).map_err(|e| err(path.display(), e))?;
                        report.docs_removed += 1;
                    }
                }
                Built::Unbuildable => keep_local(&mut view, &mut docs),
                Built::Doc(bytes, parts, sids) => {
                    if !matches!(local, Some(LocalDoc::Doc(_, b)) if *b == bytes) {
                        write_atomic(lib, &path, &bytes)?;
                        report.docs_written += 1;
                    }
                    view.insert(key("doc", &[n]), Value::Bool(true));
                    for (i, l) in parts.layers.iter().enumerate() {
                        view.insert(key("layer", &[n, &i.to_string()]), l.clone());
                    }
                    let mut mapping = BTreeMap::new();
                    for ((id, body), sid) in parts.items.iter().zip(sids) {
                        view.insert(key("ann", &[n, &sid]), body.clone());
                        mapping.insert(*id, sid);
                    }
                    if !mapping.is_empty() {
                        docs.insert(n.clone(), mapping);
                    }
                }
            }
        }
        state.regs = view;
        state.docs = docs;
        Ok(())
    }
}

fn mtime_ns(md: &std::fs::Metadata) -> u64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Write `path` (inside `lib`) via a temp file in `lib/.sync/` and rename.
fn write_atomic(lib: &Path, path: &Path, data: &[u8]) -> Result<(), String> {
    static N: AtomicU64 = AtomicU64::new(0);
    let tmpdir = lib.join(STATE_DIR);
    std::fs::create_dir_all(&tmpdir).map_err(|e| err(tmpdir.display(), e))?;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| err(p.display(), e))?;
    }
    let tmp = tmpdir.join(format!(
        "write-{}-{}.tmp",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, data).map_err(|e| err(tmp.display(), e))?;
    std::fs::rename(&tmp, path).map_err(|e| err(path.display(), e))
}

fn excluded(rel: &str) -> bool {
    let top = rel.split('/').next().unwrap_or("");
    EXCLUDED_DIRS.contains(&top)
        || top.starts_with(".before-restore-")
        || rel.ends_with(".tmp")
        || rel == ".perf.log"
        || rel == "library.json.bak"
        || rel.split('/').any(|p| p == ".DS_Store")
}

/// A relative library path that is safe to write and is a synced file.
fn safe_file_rel(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.contains('\\')
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !excluded(rel)
        && rel != STORE_FILE
        && doc_name(rel).is_none()
}

fn safe_doc_name(n: &str) -> bool {
    !n.is_empty() && !n.starts_with('.') && !n.contains(['/', '\\']) && !n.contains("..")
}

/// `Annotations/<name>.json` -> `name`.
fn doc_name(rel: &str) -> Option<&str> {
    let n = rel.strip_prefix("Annotations/")?.strip_suffix(".json")?;
    safe_doc_name(n).then_some(n)
}

fn walk(base: &Path, d: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let rd = match std::fs::read_dir(d) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(err(d.display(), e)),
    };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(rel) = p.strip_prefix(base) else {
            continue;
        };
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        if excluded(&rel) {
            continue;
        }
        let ft = e.file_type().map_err(|e| err(&rel, e))?;
        if ft.is_dir() {
            walk(base, &p, out)?;
        } else if ft.is_file() {
            out.push(rel);
        }
    }
    Ok(())
}

fn is_store_key(k: &Key) -> bool {
    matches!(
        k[0].as_str(),
        "store" | "meta" | "settings" | "instr" | "setlist" | "setlist-items" | "setlist-instr"
    )
}

fn is_doc_key(k: &Key, n: &str) -> bool {
    matches!(k[0].as_str(), "doc" | "layer" | "ann") && k[1] == n
}

/// Registers of a `library.json` object.
fn store_view(obj: &Map<String, Value>, out: &mut BTreeMap<Key, Value>) {
    for (k, v) in obj {
        match k.as_str() {
            "metadata" | "settings" | "instructions" => {
                let kind = match k.as_str() {
                    "metadata" => "meta",
                    "settings" => "settings",
                    _ => "instr",
                };
                for (id, val) in v.as_object().into_iter().flatten() {
                    out.insert(key(kind, &[id]), val.clone());
                }
            }
            "setlists" => {
                for sl in v.as_array().into_iter().flatten() {
                    let Some(o) = sl.as_object() else { continue };
                    let Some(id) = o.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    let hk = key("setlist", &[id]);
                    if out.contains_key(&hk) {
                        continue;
                    }
                    let mut head = o.clone();
                    head.remove("id");
                    let items = head.remove("items").unwrap_or(Value::Array(Vec::new()));
                    // SET-03: the set's instructions are their own register,
                    // so a rename and an instructions edit both survive.
                    let instr = head
                        .remove("instructions")
                        .unwrap_or(Value::String(String::new()));
                    out.insert(hk, Value::Object(head));
                    out.insert(key("setlist-items", &[id]), items);
                    out.insert(key("setlist-instr", &[id]), instr);
                }
            }
            _ => {
                out.insert(key("store", &[k]), v.clone());
            }
        }
    }
}

/// The merged `library.json`, or `None` if no device ever had one.
fn store_object(m: &Merged) -> Option<Map<String, Value>> {
    let mut any = false;
    let mut obj = Map::new();
    let (mut metadata, mut settings, mut instructions) = (Map::new(), Map::new(), Map::new());
    let mut setlists = Vec::new();
    for (k, (_, v)) in &m.regs {
        if !is_store_key(k) {
            continue;
        }
        any = true;
        let Some(v) = v else { continue };
        let id = &k[1];
        match k[0].as_str() {
            "store"
                if !matches!(
                    id.as_str(),
                    "metadata" | "settings" | "setlists" | "instructions"
                ) =>
            {
                obj.insert(id.clone(), v.clone());
            }
            "meta" => {
                metadata.insert(id.clone(), v.clone());
            }
            "settings" => {
                settings.insert(id.clone(), v.clone());
            }
            "instr" => {
                instructions.insert(id.clone(), v.clone());
            }
            "setlist" => {
                let Value::Object(head) = v else { continue };
                let mut o = head.clone();
                o.insert("id".into(), Value::String(id.clone()));
                let items = m
                    .regs
                    .get(&key("setlist-items", &[id]))
                    .and_then(|(_, v)| v.clone())
                    .unwrap_or(Value::Array(Vec::new()));
                o.insert("items".into(), items);
                if let Some(Value::String(t)) = m
                    .regs
                    .get(&key("setlist-instr", &[id]))
                    .and_then(|(_, v)| v.clone())
                {
                    if !t.is_empty() {
                        o.insert("instructions".into(), Value::String(t));
                    }
                }
                setlists.push((m.first.get(k).cloned(), id.clone(), Value::Object(o)));
            }
            _ => {}
        }
    }
    if !any {
        return None;
    }
    setlists.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    obj.insert("metadata".into(), Value::Object(metadata));
    obj.insert("settings".into(), Value::Object(settings));
    obj.insert("instructions".into(), Value::Object(instructions));
    obj.insert(
        "setlists".into(),
        Value::Array(setlists.into_iter().map(|s| s.2).collect()),
    );
    Some(obj)
}

/// Layers and `(local id, body without id)` of a document.
struct DocParts {
    layers: Vec<Value>,
    items: Vec<(u64, Value)>,
}

fn doc_parts(d: &Document) -> DocParts {
    DocParts {
        layers: d
            .layers
            .iter()
            .map(|l| serde_json::to_value(l).unwrap_or(Value::Null))
            .collect(),
        items: d
            .items
            .iter()
            .map(|a| {
                let mut v = serde_json::to_value(a).unwrap_or(Value::Null);
                if let Value::Object(o) = &mut v {
                    o.remove("id");
                }
                (a.id, v)
            })
            .collect(),
    }
}

enum Built {
    Absent,
    /// Merged content that this version cannot represent (newer format).
    Unbuildable,
    /// Bytes, their parts, and the sync id of each item.
    Doc(Vec<u8>, DocParts, Vec<String>),
}

fn build_doc(n: &str, m: &Merged) -> Built {
    let empty = BTreeMap::new();
    let anns = m.anns.get(n).unwrap_or(&empty);
    let mut added: Vec<(&Hlc, u64, &String, bool)> = anns
        .iter()
        .filter_map(|(sid, e)| e.created.as_ref().map(|(h, p)| (h, *p, sid, e.removed)))
        .collect();
    added.sort();
    // Deterministic local ids: keep the origin id unless taken, then fresh ones.
    let mut ids: BTreeMap<&String, u64> = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for (_, pref, sid, _) in &added {
        if *pref != 0 && taken.insert(*pref) {
            ids.insert(sid, *pref);
        }
    }
    let mut next = taken.iter().next_back().copied().unwrap_or(0) + 1;
    for (_, _, sid, _) in &added {
        ids.entry(sid).or_insert_with(|| {
            next += 1;
            next - 1
        });
    }
    let mut items = Vec::new();
    let mut sids = Vec::new();
    for (_, _, sid, removed) in &added {
        if *removed {
            continue;
        }
        let body = m
            .regs
            .get(&key("ann", &[n, sid]))
            .and_then(|(_, v)| v.as_ref());
        if let Some(Value::Object(b)) = body {
            let mut o = b.clone();
            o.insert("id".into(), Value::from(ids[sid]));
            items.push(Value::Object(o));
            sids.push((*sid).clone());
        }
    }
    let flagged = m
        .regs
        .get(&key("doc", &[n]))
        .is_some_and(|(_, v)| *v == Some(Value::Bool(true)));
    if !flagged && items.is_empty() {
        return Built::Absent;
    }
    let mut layers: BTreeMap<usize, Value> = BTreeMap::new();
    for (k, (_, v)) in m.regs.range(key("layer", &[n])..) {
        if k[0] != "layer" || k[1] != n {
            break;
        }
        if let (Some(i), Some(v)) = (k.get(2).and_then(|i| i.parse().ok()), v) {
            layers.insert(i, v.clone());
        }
    }
    let layers: Vec<Value> = match layers.keys().next_back() {
        None => doc_parts(&Document::new()).layers,
        Some(&max) => (0..=max)
            .map(|i| {
                layers.get(&i).cloned().unwrap_or_else(
                    || serde_json::json!({ "name": format!("Layer {}", i + 1), "visible": true }),
                )
            })
            .collect(),
    };
    let doc = serde_json::json!({
        "version": DOCUMENT_VERSION,
        "layers": layers,
        "items": items,
        "next_id": next,
    });
    let Ok(doc) = serde_json::from_value::<Document>(doc) else {
        return Built::Unbuildable;
    };
    let Ok(json) = doc.to_json() else {
        return Built::Unbuildable;
    };
    Built::Doc(json.into_bytes(), doc_parts(&doc), sids)
}

#[derive(Default)]
enum StoreScan {
    #[default]
    Missing,
    Unreadable,
    Ok(Map<String, Value>, Vec<u8>),
}

enum LocalDoc {
    Unreadable,
    Doc(DocParts, Vec<u8>),
}

#[derive(Default)]
struct Scan {
    files: BTreeMap<String, String>,
    store: StoreScan,
    docs: BTreeMap<String, LocalDoc>,
}

/// Hybrid logical clock stamp; ordered by `(ms, counter, device)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Hlc {
    ms: u64,
    counter: u32,
    device: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    ms: u64,
    c: u32,
}

struct Clock {
    last: Stamp,
    device: String,
    wall: u64,
}

impl Clock {
    fn tick(&mut self) -> Hlc {
        if self.wall > self.last.ms {
            self.last = Stamp {
                ms: self.wall,
                c: 0,
            };
        } else {
            self.last.c += 1;
        }
        Hlc {
            ms: self.last.ms,
            counter: self.last.c,
            device: self.device.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Op {
    /// LWW register write; `None` is a tombstone.
    Set { key: Key, value: Option<Value> },
    /// Add annotation `sid` (originally local id `pref`) to document `doc`.
    AnnAdd { doc: String, sid: String, pref: u64 },
    /// Remove observed annotations.
    AnnRemove { doc: String, sids: Vec<String> },
}

impl Op {
    fn is_valid(&self) -> bool {
        match self {
            Op::Set { key: k, value } => match (k.first().map(String::as_str), k.len()) {
                (Some("file"), 2) => {
                    safe_file_rel(&k[1])
                        && value
                            .as_ref()
                            .is_none_or(|v| v.as_str().is_some_and(is_hash))
                }
                (Some("doc" | "ann"), 2..=3) => {
                    safe_doc_name(&k[1]) && (k[0] == "doc") == (k.len() == 2)
                }
                (Some("layer"), 3) => {
                    safe_doc_name(&k[1]) && k[2].parse::<usize>().is_ok_and(|i| i < MAX_LAYERS)
                }
                (
                    Some(
                        "store" | "meta" | "settings" | "instr" | "setlist" | "setlist-items"
                        | "setlist-instr",
                    ),
                    2,
                ) => true,
                _ => false,
            },
            Op::AnnAdd { doc, .. } | Op::AnnRemove { doc, .. } => safe_doc_name(doc),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Line {
    t: u64,
    c: u32,
    d: String,
    op: Op,
}

#[derive(Default)]
struct AnnEntry {
    created: Option<(Hlc, u64)>,
    removed: bool,
}

#[derive(Default)]
struct Merged {
    regs: BTreeMap<Key, (Hlc, Option<Value>)>,
    /// Earliest write per register (setlist order).
    first: BTreeMap<Key, Hlc>,
    anns: BTreeMap<String, BTreeMap<String, AnnEntry>>,
    max: Option<Hlc>,
}

impl Merged {
    fn apply(&mut self, h: Hlc, op: Op) {
        if self.max.as_ref().is_none_or(|m| h > *m) {
            self.max = Some(h.clone());
        }
        match op {
            Op::Set { key, value } => {
                match self.first.get(&key) {
                    Some(f) if *f <= h => {}
                    _ => {
                        self.first.insert(key.clone(), h.clone());
                    }
                }
                match self.regs.get(&key) {
                    Some((old, _)) if *old >= h => {}
                    _ => {
                        self.regs.insert(key, (h, value));
                    }
                }
            }
            Op::AnnAdd { doc, sid, pref } => {
                let e = self.anns.entry(doc).or_default().entry(sid).or_default();
                if e.created.as_ref().is_none_or(|c| (&h, pref) < (&c.0, c.1)) {
                    e.created = Some((h, pref));
                }
            }
            Op::AnnRemove { doc, sids } => {
                let d = self.anns.entry(doc).or_default();
                for s in sids {
                    d.entry(s).or_default().removed = true;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cached {
    len: u64,
    mtime: u64,
    hash: String,
}

/// `<library>/.sync/state.json`.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    version: u32,
    device: String,
    /// Highest clock issued or seen.
    clock: Stamp,
    /// Log lines read per device.
    seen: BTreeMap<String, usize>,
    /// Local registers as of the last sync.
    #[serde(with = "pairs")]
    regs: BTreeMap<Key, Value>,
    /// Per document: local annotation id -> sync id.
    docs: BTreeMap<String, BTreeMap<u64, String>>,
    /// File hash cache.
    files: BTreeMap<String, Cached>,
    /// When the cache was taken (ns); entries modified within 2 s of it are rehashed.
    cached_at: u64,
}

/// A map with array keys as a JSON list of pairs.
mod pairs {
    use super::{Key, Value};
    use serde::{Deserialize, Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(m: &BTreeMap<Key, Value>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(m.iter())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<Key, Value>, D::Error> {
        Ok(Vec::<(Key, Value)>::deserialize(d)?.into_iter().collect())
    }
}
