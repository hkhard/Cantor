//! Test helpers: temp library dirs, fake clocks, edits through the real
//! library/annotation crates, and a library hash that ignores `.sync/`.
#![allow(dead_code)]

use cantor_annotations::{Anchor, Command, Document, Editor, Ink, InkPoint, InkTool, Item};
use cantor_library::store::{Metadata, Store};
use cantor_sync::{SyncFolder, SyncReport};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub fn temp_dir(name: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "cantor-sync-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One device: its library dir, its sync folder handle and a settable clock.
pub struct Device {
    pub lib: PathBuf,
    pub sync: SyncFolder,
    pub clock: Arc<AtomicU64>,
}

impl Device {
    pub fn new(shared: &Path, id: &str, start_ms: u64) -> Device {
        let lib = temp_dir(&format!("lib-{id}"));
        let clock = Arc::new(AtomicU64::new(start_ms));
        let c = clock.clone();
        let sync = SyncFolder::open(shared, id)
            .unwrap()
            .with_clock(move || c.load(Ordering::SeqCst));
        Device { lib, sync, clock }
    }
    pub fn at(&self, ms: u64) -> &Self {
        self.clock.store(ms, Ordering::SeqCst);
        self
    }
    pub fn sync(&self) -> SyncReport {
        self.sync.sync(&self.lib).unwrap()
    }
    pub fn add_file(&self, rel: &str, data: &[u8]) {
        let p = self.lib.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    pub fn has_file(&self, rel: &str) -> bool {
        self.lib.join(rel).is_file()
    }
    pub fn store(&self) -> Store {
        Store::open(&self.lib)
    }
    pub fn edit_store(&self, f: impl FnOnce(&mut Store)) {
        let mut s = self.store();
        f(&mut s);
        s.save().unwrap();
    }
    pub fn metadata(&self, id: &str) -> Option<Metadata> {
        self.store().metadata(id).cloned()
    }
    pub fn doc_path(&self, score: &str) -> PathBuf {
        self.lib.join("Annotations").join(format!("{score}.json"))
    }
    pub fn doc(&self, score: &str) -> Option<Document> {
        let s = std::fs::read_to_string(self.doc_path(score)).ok()?;
        Some(Document::from_json(&s).unwrap())
    }
    /// Apply editor commands to a score's annotation document, as the app does.
    pub fn edit_doc(&self, score: &str, cmds: Vec<Command>) {
        let mut ed = Editor::new(self.doc(score).unwrap_or_default());
        for c in cmds {
            let _ = ed.apply(c);
        }
        let p = self.doc_path(score);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, ed.document().to_json().unwrap()).unwrap();
    }
    pub fn hash(&self) -> String {
        hash_without_sync(&self.lib)
    }
}

pub fn ink(page: u32, x: f32, y: f32, color: &str) -> Command {
    Command::Add {
        layer: 0,
        anchor: Anchor::Page { page, x, y },
        item: Item::Ink(Ink {
            points: vec![
                InkPoint {
                    x: 0.0,
                    y: 0.0,
                    pressure: 1.0,
                },
                InkPoint {
                    x: 0.05,
                    y: 0.01,
                    pressure: 0.5,
                },
            ],
            color: color.into(),
            width: 0.004,
            tool: InkTool::Pen,
        }),
    }
}

/// `cantor_library::backup::library_hash` of a copy of `dir` without the
/// per-device `.sync/` state (which backup.rs does not exclude yet).
pub fn hash_without_sync(dir: &Path) -> String {
    let copy = temp_dir("hash");
    fn cp(from: &Path, to: &Path, top: bool) {
        for e in std::fs::read_dir(from).unwrap().flatten() {
            if top && e.file_name() == ".sync" {
                continue;
            }
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                std::fs::create_dir_all(&t).unwrap();
                cp(&e.path(), &t, false);
            } else {
                std::fs::copy(e.path(), t).unwrap();
            }
        }
    }
    cp(dir, &copy, true);
    let h = cantor_library::backup::library_hash(&copy).unwrap();
    let _ = std::fs::remove_dir_all(&copy);
    h
}
