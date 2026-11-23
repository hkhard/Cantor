//! Persisted library store: per-score metadata (LIB-03), search (LIB-04),
//! setlists (SET-01, LIB-08 import) and per-score playback settings incl.
//! custom mixer presets (PLAY-04), instructions and part hints per score and
//! per setlist, and notes per setlist item (SET-03, SET-05).
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
        /// SET-05: a note for this item ("start at bar 9", "skip verse 2").
        #[serde(default, skip_serializing_if = "String::is_empty")]
        note: String,
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
    /// SET-03: the director's instructions for the whole set (markdown-light
    /// text, see [`ScoreInstructions::text`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instructions: String,
}

/// SET-03: who sings what. `who` is a member or a group ("Anna", "Tenor 1
/// group"); `part` is the mixer channel label it maps to ("Tenor 1").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartHint {
    pub who: String,
    pub part: String,
}

/// SET-03: instructions for one score: free text (paragraphs separated by a
/// blank line, `**bold**`, `[text](url)` and bare links, bar references such
/// as "t. 12" / "m. 12"; the UI renders it, the store keeps it verbatim) and
/// part hints for members.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScoreInstructions {
    pub text: String,
    pub parts: Vec<PartHint>,
}

impl ScoreInstructions {
    /// Nothing written and no hints.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.parts.is_empty()
    }
    /// The part hinted for `member` (case- and accent-insensitive), if any.
    pub fn part_for(&self, member: &str) -> Option<&str> {
        let m = normalize(member.trim());
        if m.is_empty() {
            return None;
        }
        self.parts
            .iter()
            .find(|h| normalize(h.who.trim()) == m)
            .map(|h| h.part.as_str())
    }
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

/// PLAY-09: a "voice set": which instrument plays each choir voice. Roles
/// are "soprano", "alto", "tenor", "bass", "voice", "piano", "instrument";
/// each has a list of GM programs (0-based) for divisi lines 1, 2, ... (the
/// last one repeats). Roles left out keep the default instrument.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSet {
    pub id: String,
    pub name_sv: String,
    pub name_en: String,
    /// "mixed", "male", "female", or "" for any choir.
    pub choir: String,
    pub roles: BTreeMap<String, Vec<u8>>,
}

impl VoiceSet {
    /// Program for divisi line `number` (1-based) of `role`, if the set
    /// says.
    pub fn program_for(&self, role: &str, number: u8) -> Option<u8> {
        let v = self.roles.get(role)?;
        let i = (number.max(1) as usize - 1).min(v.len().checked_sub(1)?);
        v.get(i).copied()
    }
}

fn vs(id: &str, sv: &str, en: &str, choir: &str, roles: &[(&str, &[u8])]) -> VoiceSet {
    VoiceSet {
        id: id.into(),
        name_sv: sv.into(),
        name_en: en.into(),
        choir: choir.into(),
        roles: roles
            .iter()
            .map(|(r, p)| (r.to_string(), p.to_vec()))
            .collect(),
    }
}

/// The voice sets Cantor ships with (GM programs, 0-based). "Classic" is
/// the built-in choice of `cantor_timeline::channels::voice_program`:
/// neighbouring voices get different instrument families.
pub fn builtin_voice_sets() -> Vec<VoiceSet> {
    let all = |p: u8| -> Vec<(&'static str, Vec<u8>)> {
        ["soprano", "alto", "tenor", "bass", "voice"]
            .iter()
            .map(|r| (*r, vec![p]))
            .collect()
    };
    let one = |id: &str, sv: &str, en: &str, p: u8| {
        let r = all(p);
        let refs: Vec<(&str, &[u8])> = r.iter().map(|(a, b)| (*a, b.as_slice())).collect();
        vs(id, sv, en, "", &refs)
    };
    vec![
        vs(
            "classic",
            "Klassisk (olika färger)",
            "Classic (distinct colours)",
            "",
            &[
                ("soprano", &[73, 68, 72, 79, 13]),
                ("alto", &[71, 65, 74, 66, 11]),
                ("tenor", &[56, 60, 57, 61, 69]),
                ("bass", &[42, 70, 58, 32, 43]),
                ("voice", &[73]),
            ],
        ),
        one("choir-aahs", "Kör (aah)", "Choir aahs", 52),
        one("voice-oohs", "Röster (ooh)", "Voice oohs", 53),
        one("piano", "Piano", "Piano", 0),
        vs(
            "organ",
            "Orgel",
            "Organ",
            "",
            &[
                ("soprano", &[19]),
                ("alto", &[20]),
                ("tenor", &[19]),
                ("bass", &[20]),
                ("voice", &[19]),
            ],
        ),
        vs(
            "strings-mixed",
            "Stråkar (SATB)",
            "Strings (SATB)",
            "mixed",
            &[
                ("soprano", &[40, 40]),
                ("alto", &[40, 41]),
                ("tenor", &[41, 42]),
                ("bass", &[42, 43]),
                ("voice", &[40]),
            ],
        ),
        vs(
            "strings-male",
            "Stråkar (TTBB)",
            "Strings (TTBB)",
            "male",
            &[
                ("soprano", &[40]),
                ("alto", &[41]),
                ("tenor", &[41, 42]),
                ("bass", &[42, 43]),
                ("voice", &[42]),
            ],
        ),
        vs(
            "strings-female",
            "Stråkar (SSAA)",
            "Strings (SSAA)",
            "female",
            &[
                ("soprano", &[40, 40]),
                ("alto", &[41, 41]),
                ("tenor", &[42]),
                ("bass", &[42]),
                ("voice", &[40]),
            ],
        ),
        vs(
            "woodwinds-mixed",
            "Träblås (SATB)",
            "Woodwinds (SATB)",
            "mixed",
            &[
                ("soprano", &[73, 68]),
                ("alto", &[68, 71]),
                ("tenor", &[71, 69]),
                ("bass", &[70]),
                ("voice", &[73]),
            ],
        ),
        vs(
            "woodwinds-female",
            "Träblås (SSAA)",
            "Woodwinds (SSAA)",
            "female",
            &[
                ("soprano", &[73, 72]),
                ("alto", &[68, 71]),
                ("tenor", &[71]),
                ("bass", &[70]),
                ("voice", &[73]),
            ],
        ),
        vs(
            "brass-male",
            "Bleckblås (TTBB)",
            "Brass (TTBB)",
            "male",
            &[
                ("soprano", &[56]),
                ("alto", &[60]),
                ("tenor", &[56, 60]),
                ("bass", &[57, 58]),
                ("voice", &[60]),
            ],
        ),
        vs(
            "brass-mixed",
            "Bleckblås (SATB)",
            "Brass (SATB)",
            "mixed",
            &[
                ("soprano", &[56]),
                ("alto", &[60]),
                ("tenor", &[57]),
                ("bass", &[58]),
                ("voice", &[56]),
            ],
        ),
    ]
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
    /// PLAY-09: instrument (GM program, 0-based) chosen per mixer channel;
    /// `None` = the default for that voice.
    pub programs: Vec<Option<u8>>,
    /// PLAY-09: voice set used for this score (`None` = the default for its
    /// choir type).
    pub voice_set: Option<String>,
    /// BAND-03: key of this player's instrument ("C", "Bb", "Eb", "F"):
    /// notation is shown written for it, sound stays at concert pitch.
    pub instrument_key: String,
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
            programs: Vec::new(),
            voice_set: None,
            instrument_key: "C".into(),
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
    /// PLAY-09: the user's own voice sets.
    voice_sets: Vec<VoiceSet>,
    /// Default voice set id per choir type ("mixed", "male", "female").
    default_voice_sets: BTreeMap<String, String>,
    /// "score|pdf" pairs the user chose not to combine.
    dismissed_pairs: std::collections::BTreeSet<String>,
    /// One-time hints the user has dismissed (first run, e.g. "reader").
    hints_seen: std::collections::BTreeSet<String>,
    /// SET-03: instructions and part hints per score id.
    instructions: BTreeMap<String, ScoreInstructions>,
    /// SET-03: the member using this library (matches [`PartHint::who`]).
    member: String,
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
        self.data.instructions.extend(d.instructions);
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

    // --- instructions (SET-03) ---
    pub fn instructions(&self, id: &str) -> Option<&ScoreInstructions> {
        self.data.instructions.get(id)
    }
    pub fn all_instructions(&self) -> &BTreeMap<String, ScoreInstructions> {
        &self.data.instructions
    }
    /// Set (or, when empty, remove) the instructions of a score. Blank hint
    /// rows are dropped.
    pub fn set_instructions(&mut self, id: &str, mut ins: ScoreInstructions) {
        ins.parts
            .retain(|h| !h.who.trim().is_empty() && !h.part.trim().is_empty());
        if ins.is_empty() {
            self.data.instructions.remove(id);
        } else {
            self.data.instructions.insert(id.to_string(), ins);
        }
    }
    /// The member name chosen on this library ("" = nobody).
    pub fn member(&self) -> &str {
        &self.data.member
    }
    pub fn set_member(&mut self, name: &str) {
        self.data.member = name.trim().to_string();
    }
    /// The part hinted for the current member in score `id`.
    pub fn member_part(&self, id: &str) -> Option<&str> {
        self.instructions(id)?.part_for(&self.data.member)
    }

    // --- voice sets (PLAY-09) ---
    /// Built-in sets first, then the user's own.
    pub fn voice_sets(&self) -> Vec<VoiceSet> {
        let mut v = builtin_voice_sets();
        v.extend(self.data.voice_sets.iter().cloned());
        v
    }
    /// Save (new: empty id) or replace an own voice set; returns its id.
    pub fn save_voice_set(&mut self, mut set: VoiceSet) -> String {
        if set.id.is_empty() || builtin_voice_sets().iter().any(|b| b.id == set.id) {
            let n = self.data.voice_sets.len() + 1;
            let mut k = n;
            while self
                .data
                .voice_sets
                .iter()
                .any(|v| v.id == format!("own-{k}"))
            {
                k += 1;
            }
            set.id = format!("own-{k}");
        }
        let id = set.id.clone();
        match self.data.voice_sets.iter_mut().find(|v| v.id == id) {
            Some(v) => *v = set,
            None => self.data.voice_sets.push(set),
        }
        id
    }
    pub fn remove_voice_set(&mut self, id: &str) {
        self.data.voice_sets.retain(|v| v.id != id);
        self.data.default_voice_sets.retain(|_, v| v != id);
    }
    pub fn set_default_voice_set(&mut self, choir: &str, id: &str) {
        self.data
            .default_voice_sets
            .insert(choir.to_string(), id.to_string());
    }
    /// The default set for a choir type; "classic" when none was chosen or
    /// the chosen one is gone.
    pub fn default_voice_set(&self, choir: &str) -> Option<VoiceSet> {
        let all = self.voice_sets();
        let id = self
            .data
            .default_voice_sets
            .get(choir)
            .map(String::as_str)
            .unwrap_or("classic");
        all.iter()
            .find(|v| v.id == id)
            .or_else(|| all.iter().find(|v| v.id == "classic"))
            .cloned()
    }

    // --- combine suggestions (LIB-05) ---
    pub fn dismissed_pairs(&self) -> &std::collections::BTreeSet<String> {
        &self.data.dismissed_pairs
    }
    pub fn dismiss_pair(&mut self, key: &str) {
        self.data.dismissed_pairs.insert(key.to_string());
    }

    // --- one-time hints ---
    pub fn hint_seen(&self, id: &str) -> bool {
        self.data.hints_seen.contains(id)
    }
    pub fn mark_hint_seen(&mut self, id: &str) {
        self.data.hints_seen.insert(id.to_string());
    }
    pub fn hints_seen(&self) -> Vec<String> {
        self.data.hints_seen.iter().cloned().collect()
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
            instructions: String::new(),
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
        if let Some(i) = self.data.instructions.remove(old) {
            self.data.instructions.insert(new.to_string(), i);
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
        self.data.instructions.remove(id);
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
                    note: String::new(),
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
        instructions: String::new(),
    }
}
