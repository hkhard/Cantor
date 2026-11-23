//! Persisted library store: per-score metadata (LIB-03), search (LIB-04),
//! setlists (SET-01, LIB-08 import) and per-score playback settings incl.
//! custom mixer presets (PLAY-04).
//!
//! Everything lives in one versioned JSON file, [`STORE_FILE`], inside the
//! user's library folder. Saves are atomic (write `library.json.tmp`, then
//! rename). Opening never fails: a missing file gives an empty store, an
//! unreadable/corrupt one is copied to `library.json.bak` and an empty store
//! is returned.
//!
//! Score ids (e.g. `"1:Olaf Trygvason"`) are opaque strings.
//!
//! Search normalizes text by NFD-decomposing, dropping combining marks and
//! lowercasing, so `å/ä/ö` match `a/a/o` as well as themselves. All query
//! words must match some field; results rank title-prefix > title-contains >
//! other fields, then by title.

use crate::forscore::ForscoreSetlist;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// File name of the store inside the library folder.
pub const STORE_FILE: &str = "library.json";
/// Current on-disk format version.
pub const STORE_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported store version {0}")]
    Version(u32),
}

/// Editable score metadata (LIB-03). `rating`/`difficulty` are clamped to 0..=5.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub title: String,
    pub composer: String,
    pub arranger: String,
    pub key: String,
    pub tempo: Option<f64>,
    pub genre: String,
    pub tags: Vec<String>,
    pub rating: Option<u8>,
    pub difficulty: Option<u8>,
    pub part_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SetItem {
    Score {
        score: String,
        start_page: Option<u32>,
    },
    Placeholder {
        title: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setlist {
    pub id: String,
    pub name: String,
    pub items: Vec<SetItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MixerStrip {
    pub volume_db: f64,
    pub mute: bool,
    pub solo: bool,
}

/// A user-saved mixer preset (PLAY-04).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub strips: Vec<MixerStrip>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    #[default]
    Pdf,
    Notation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choir {
    #[default]
    Auto,
    Male,
    Female,
    Mixed,
}

/// What the library screen opens on (LIB-04): a grouping and the selected
/// tab in it. `tab` "" = all scores; otherwise a setlist id or a group value
/// such as a genre or tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeView {
    pub group: String,
    pub tab: String,
}

/// Groupings the library can show.
pub const HOME_GROUPS: &[&str] = &[
    "setlists",
    "genre",
    "tags",
    "composer",
    "arranger",
    "rating",
    "difficulty",
];

impl Default for HomeView {
    fn default() -> Self {
        HomeView {
            group: "setlists".into(),
            tab: String::new(),
        }
    }
}

/// Per-score playback/view settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreSettings {
    /// Channel index of "my part".
    pub my_part: Option<usize>,
    pub strips: Vec<MixerStrip>,
    pub tempo_percent: f64,
    pub view: View,
    pub choir: Choir,
    pub presets: Vec<Preset>,
    pub transpose: i32,
    pub hidden_parts: Vec<usize>,
    /// VIEW-05: show transposing instruments at concert pitch.
    pub concert_pitch: bool,
}

impl Default for ScoreSettings {
    fn default() -> Self {
        Self {
            my_part: None,
            strips: Vec::new(),
            tempo_percent: 100.0,
            view: View::default(),
            choir: Choir::default(),
            presets: Vec::new(),
            transpose: 0,
            hidden_parts: Vec::new(),
            concert_pitch: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    version: u32,
    metadata: BTreeMap<String, Metadata>,
    setlists: Vec<Setlist>,
    settings: BTreeMap<String, ScoreSettings>,
    /// Manual order of the main library view (score ids), set by dragging.
    /// Scores not listed come after, by title.
    score_order: Vec<String>,
    /// Manual order of the setlist tabs (setlist ids).
    setlist_order: Vec<String>,
    /// Scores whose metadata was filled from the file header once. They are
    /// never seeded again, so a field the user cleared stays cleared.
    seeded: std::collections::BTreeSet<String>,
    /// The library's start view.
    home: HomeView,
    /// "score|pdf" pairs the user chose not to combine.
    dismissed_pairs: std::collections::BTreeSet<String>,
}

/// The persisted library store.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
    data: Data,
}

/// Generate a unique-ish id from time + process counter (no extra deps).
pub fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{:x}-{c:x}", std::process::id())
}

fn clamp(m: &mut Metadata) {
    m.rating = m.rating.map(|r| r.min(5));
    m.difficulty = m.difficulty.map(|r| r.min(5));
}

fn parse(json: &str) -> Result<Data, StoreError> {
    let mut d: Data = serde_json::from_str(json)?;
    if d.version > STORE_VERSION {
        return Err(StoreError::Version(d.version));
    }
    d.version = STORE_VERSION;
    d.metadata.values_mut().for_each(clamp);
    Ok(d)
}

impl Store {
    /// Open (or create in memory) the store in `dir`. Never fails or panics.
    pub fn open(dir: impl AsRef<Path>) -> Store {
        let dir = dir.as_ref().to_path_buf();
        let path = dir.join(STORE_FILE);
        let data = match std::fs::read_to_string(&path) {
            Ok(s) => parse(&s).unwrap_or_else(|_| {
                let _ = std::fs::copy(&path, dir.join(format!("{STORE_FILE}.bak")));
                Data::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
            Err(_) => {
                let _ = std::fs::copy(&path, dir.join(format!("{STORE_FILE}.bak")));
                Data::default()
            }
        };
        Store {
            dir,
            data: Data {
                version: STORE_VERSION,
                ..data
            },
        }
    }

    /// Path of the backing JSON file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(STORE_FILE)
    }

    /// Atomically write the store to disk.
    pub fn save(&self) -> Result<(), StoreError> {
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{STORE_FILE}.tmp"));
        std::fs::write(&tmp, self.export_json())?;
        std::fs::rename(&tmp, self.path())?;
        Ok(())
    }

    /// Whole store as pretty JSON (`"version": 1`).
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.data).expect("store serializes")
    }

    /// Import an exported store. `merge` overlays entries (imported wins,
    /// setlists matched by id); otherwise replaces everything.
    pub fn import_json(&mut self, json: &str, merge: bool) -> Result<(), StoreError> {
        let d = parse(json)?;
        if !merge {
            self.data = d;
            return Ok(());
        }
        self.data.metadata.extend(d.metadata);
        self.data.settings.extend(d.settings);
        for sl in d.setlists {
            match self.data.setlists.iter_mut().find(|s| s.id == sl.id) {
                Some(s) => *s = sl,
                None => self.data.setlists.push(sl),
            }
        }
        Ok(())
    }

    // --- metadata ---
    pub fn metadata(&self, id: &str) -> Option<&Metadata> {
        self.data.metadata.get(id)
    }
    pub fn all_metadata(&self) -> &BTreeMap<String, Metadata> {
        &self.data.metadata
    }
    /// Fill empty metadata fields of `id` from the score file's own header
    /// (LIB-03): title, composer, arranger; the lyricist becomes a
    /// "Text: …" tag. Runs once per score; user edits are never overwritten.
    /// Returns whether anything changed (the caller saves).
    pub fn seed_metadata(&mut self, id: &str, h: &cantor_score::Header) -> bool {
        if self.data.seeded.contains(id) {
            return false;
        }
        let title = h.title().unwrap_or_default();
        if title.is_empty()
            && h.composer.is_empty()
            && h.arranger.is_empty()
            && h.lyricist.is_empty()
        {
            return false;
        }
        let mut m = self.data.metadata.get(id).cloned().unwrap_or_default();
        let before = m.clone();
        for (slot, v) in [
            (&mut m.title, &title),
            (&mut m.composer, &h.composer),
            (&mut m.arranger, &h.arranger),
        ] {
            if slot.trim().is_empty() && !v.trim().is_empty() {
                *slot = v.trim().to_string();
            }
        }
        if !h.lyricist.trim().is_empty() && !m.tags.iter().any(|t| t.starts_with("Text: ")) {
            m.tags.push(format!("Text: {}", h.lyricist.trim()));
        }
        self.data.seeded.insert(id.to_string());
        if m != before {
            self.set_metadata(id, m);
        }
        true
    }
    pub fn set_metadata(&mut self, id: &str, mut m: Metadata) {
        clamp(&mut m);
        self.data.metadata.insert(id.to_string(), m);
    }

    // --- settings ---
    pub fn settings(&self, id: &str) -> Option<&ScoreSettings> {
        self.data.settings.get(id)
    }
    /// Settings for `id`, inserting defaults if absent.
    pub fn settings_mut(&mut self, id: &str) -> &mut ScoreSettings {
        self.data.settings.entry(id.to_string()).or_default()
    }
    pub fn set_settings(&mut self, id: &str, s: ScoreSettings) {
        self.data.settings.insert(id.to_string(), s);
    }

    // --- combine suggestions (LIB-05) ---
    pub fn dismissed_pairs(&self) -> &std::collections::BTreeSet<String> {
        &self.data.dismissed_pairs
    }
    pub fn dismiss_pair(&mut self, key: &str) {
        self.data.dismissed_pairs.insert(key.to_string());
    }

    // --- start view ---
    pub fn home_view(&self) -> &HomeView {
        &self.data.home
    }
    pub fn set_home_view(&mut self, v: HomeView) {
        self.data.home = if HOME_GROUPS.contains(&v.group.as_str()) {
            v
        } else {
            HomeView::default()
        };
    }

    // --- manual order (drag to rearrange) ---
    pub fn score_order(&self) -> &[String] {
        &self.data.score_order
    }
    pub fn set_score_order(&mut self, ids: Vec<String>) {
        self.data.score_order = ids;
    }
    pub fn set_setlist_order(&mut self, ids: Vec<String>) {
        self.data.setlist_order = ids;
    }
    /// `(id, title)` entries in the manual order: ordered ids first, the
    /// rest by title (case-insensitive) after them.
    pub fn in_score_order(&self, mut entries: Vec<(String, String)>) -> Vec<(String, String)> {
        let pos: BTreeMap<&str, usize> = self
            .data
            .score_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        entries.sort_by(
            |a, b| match (pos.get(a.0.as_str()), pos.get(b.0.as_str())) {
                (Some(x), Some(y)) => x.cmp(y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
            },
        );
        entries
    }
    /// Setlists in the user's tab order (unlisted ones after, as created).
    pub fn setlists_ordered(&self) -> Vec<&Setlist> {
        let pos = |id: &str| {
            self.data
                .setlist_order
                .iter()
                .position(|x| x == id)
                .unwrap_or(usize::MAX)
        };
        let mut v: Vec<(usize, usize, &Setlist)> = self
            .data
            .setlists
            .iter()
            .enumerate()
            .map(|(i, s)| (pos(&s.id), i, s))
            .collect();
        v.sort_by_key(|x| (x.0, x.1));
        v.into_iter().map(|x| x.2).collect()
    }

    // --- setlists ---
    pub fn setlists(&self) -> &[Setlist] {
        &self.data.setlists
    }
    pub fn setlist(&self, id: &str) -> Option<&Setlist> {
        self.data.setlists.iter().find(|s| s.id == id)
    }
    pub fn setlist_mut(&mut self, id: &str) -> Option<&mut Setlist> {
        self.data.setlists.iter_mut().find(|s| s.id == id)
    }
    /// Create an empty setlist and return its id.
    pub fn create_setlist(&mut self, name: &str) -> String {
        let id = new_id();
        self.data.setlists.push(Setlist {
            id: id.clone(),
            name: name.to_string(),
            items: Vec::new(),
        });
        id
    }
    /// Insert or replace (by id) a setlist, e.g. one from [`setlist_from_forscore`].
    pub fn put_setlist(&mut self, sl: Setlist) {
        match self.setlist_mut(&sl.id) {
            Some(s) => *s = sl,
            None => self.data.setlists.push(sl),
        }
    }
    pub fn delete_setlist(&mut self, id: &str) -> bool {
        let n = self.data.setlists.len();
        self.data.setlists.retain(|s| s.id != id);
        n != self.data.setlists.len()
    }

    /// Move metadata/settings to a new id and update setlist references.
    pub fn rename_score_id(&mut self, old: &str, new: &str) {
        if let Some(m) = self.data.metadata.remove(old) {
            self.data.metadata.insert(new.to_string(), m);
        }
        if let Some(s) = self.data.settings.remove(old) {
            self.data.settings.insert(new.to_string(), s);
        }
        for it in self
            .data
            .setlists
            .iter_mut()
            .flat_map(|s| s.items.iter_mut())
        {
            if let SetItem::Score { score, .. } = it {
                if score == old {
                    *score = new.to_string();
                }
            }
        }
    }

    /// Forget a score. Setlist items pointing at it become placeholders with
    /// its title (metadata title if set, else `fallback_title`).
    pub fn remove_score(&mut self, id: &str, fallback_title: &str) {
        let title = self
            .data
            .metadata
            .remove(id)
            .map(|m| m.title)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| fallback_title.to_string());
        self.data.settings.remove(id);
        for it in self
            .data
            .setlists
            .iter_mut()
            .flat_map(|s| s.items.iter_mut())
        {
            if matches!(it, SetItem::Score { score, .. } if score == id) {
                *it = SetItem::Placeholder {
                    title: title.clone(),
                };
            }
        }
    }

    // --- search ---
    /// Build a reusable search index over `entries` (`(id, display title)`);
    /// metadata title, when non-empty, overrides the display title.
    pub fn search_index(&self, entries: &[(String, String)]) -> SearchIndex {
        let items = entries
            .iter()
            .map(|(id, title)| {
                let m = self.data.metadata.get(id);
                let t = m
                    .map(|m| m.title.as_str())
                    .filter(|t| !t.is_empty())
                    .unwrap_or(title);
                let mut other = String::new();
                if let Some(m) = m {
                    for f in [&m.composer, &m.arranger, &m.genre, &m.key]
                        .into_iter()
                        .chain(&m.tags)
                        .chain(&m.part_names)
                    {
                        other.push_str(&normalize(f));
                        other.push('\u{1}');
                    }
                    if !m.title.is_empty() && m.title != *title {
                        other.push_str(&normalize(title));
                    }
                }
                IndexItem {
                    id: id.clone(),
                    title: normalize(t),
                    other,
                }
            })
            .collect();
        SearchIndex { items }
    }

    /// One-shot search (builds an index). Empty query returns all ids.
    pub fn search(&self, query: &str, entries: &[(String, String)]) -> Vec<String> {
        self.search_index(entries).search(query)
    }
}

/// Lowercase, NFD, strip combining marks: "Vår" -> "var".
pub fn normalize(s: &str) -> String {
    s.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

#[derive(Debug, Clone)]
struct IndexItem {
    id: String,
    title: String,
    other: String,
}

/// Precomputed normalized search strings.
#[derive(Debug, Clone)]
pub struct SearchIndex {
    items: Vec<IndexItem>,
}

impl SearchIndex {
    /// Ids matching every word of `query`, best first.
    pub fn search(&self, query: &str) -> Vec<String> {
        let q = normalize(query);
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut hits: Vec<(u8, &IndexItem)> = self
            .items
            .iter()
            .filter(|it| {
                words
                    .iter()
                    .all(|w| it.title.contains(w) || it.other.contains(w))
            })
            .map(|it| {
                let rank = if q.trim().is_empty() || it.title.starts_with(q.trim()) {
                    0
                } else if it.title.contains(q.trim()) || words.iter().all(|w| it.title.contains(w))
                {
                    1
                } else {
                    2
                };
                (rank, it)
            })
            .collect();
        hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.title.cmp(&b.1.title)));
        hits.into_iter().map(|(_, it)| it.id.clone()).collect()
    }
}

/// Convert a parsed forScore setlist (LIB-08). `imported` maps a forScore
/// item (by title or by file name / file stem) to the score id it was
/// imported as. Unmatched items and forScore placeholders become
/// [`SetItem::Placeholder`]. Start page: forScore's `.4ss` archives seen so
/// far carry no start-page key; if an integer `StartPage`/`startPage`/`Page`
/// key shows up among the item's unknown fields it is used, else `None`.
pub fn setlist_from_forscore(
    name: &str,
    set: &ForscoreSetlist,
    imported: &[(String, String)],
) -> Setlist {
    let items = set
        .items
        .iter()
        .map(|it| {
            let stem = it
                .file_path
                .rsplit_once('.')
                .map(|(s, _)| s)
                .unwrap_or(&it.file_path);
            let hit = imported.iter().find(|(t, _)| {
                !t.is_empty() && (*t == it.title || *t == it.file_path || *t == stem)
            });
            match hit {
                Some((_, id)) => SetItem::Score {
                    score: id.clone(),
                    start_page: ["StartPage", "startPage", "Page", "page"]
                        .iter()
                        .find_map(|k| it.unknown.get(*k))
                        .and_then(|v| v.as_unsigned_integer())
                        .and_then(|p| u32::try_from(p).ok()),
                },
                None => SetItem::Placeholder {
                    title: it.title.clone(),
                },
            }
        })
        .collect();
    Setlist {
        id: new_id(),
        name: name.to_string(),
        items,
    }
}
