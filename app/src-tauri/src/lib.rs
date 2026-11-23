//! Cantor demo app backend: score library, playback engine, mixer commands.
//!
//! The UI (Svelte + Verovio) asks for the score list, opens a score (gets its
//! MusicXML for engraving plus the mixer channels and measure times), and
//! drives transport and mixer. Audio runs in Rust: cpal output stream ->
//! `cantor_audio::Engine` (rustysynth + GeneralUser GS).

mod audio;
mod folder;
mod guard;
mod m4a;
mod remote;
mod share;

use cantor_audio::{Engine, Preset, SoundFontSynth};
use cantor_timeline::channels::{choir_from_title, plan_channels_for, Choir};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

/// Where the scores and SoundFont live: bundled demo scores (read-only) and
/// the user's library (files received from the share sheet, Files or the
/// import button). On iOS the user library is the app's Documents folder, so
/// it also shows in the Files app under "On My iPad > Cantor".
struct Paths {
    scores: PathBuf,
    user: PathBuf,
    soundfont: PathBuf,
}

impl Paths {
    /// Library folders by index (the `dir` part of a score id).
    fn dirs(&self) -> [&std::path::Path; 2] {
        [self.scores.as_path(), self.user.as_path()]
    }

    /// Resolve a score id `"<dir>:<stem>"`.
    fn entry(&self, id: &str) -> Result<cantor_library::import::LibraryEntry, String> {
        let (d, stem) = id.split_once(':').ok_or("bad score id")?;
        let d: usize = d.parse().map_err(|_| "bad score id")?;
        let dir = self.dirs().get(d).copied().ok_or("bad score id")?;
        let mut e = cantor_library::import::list_library(&[dir])
            .into_iter()
            .find(|e| e.stem == stem)
            .ok_or_else(|| format!("{stem}: finns inte i biblioteket"))?;
        // A bundled score's backing track lives in the user folder (the
        // bundle is read-only): `<stem>.track.<ext>` there belongs to it.
        if d == 0 && e.track_file.is_none() {
            if let Ok(rd) = std::fs::read_dir(&self.user) {
                for f in rd.flatten() {
                    let n = f.file_name().to_string_lossy().to_string();
                    if let Some((st, ext)) = n.rsplit_once('.') {
                        if st == format!("{stem}.track")
                            && cantor_library::import::TRACK_EXT
                                .contains(&ext.to_lowercase().as_str())
                        {
                            e.track_file = Some(n);
                        }
                    }
                }
            }
        }
        Ok(e)
    }

    /// Directory a score's track (and track settings) live in.
    fn track_dir(&self, id: &str, e: &cantor_library::import::LibraryEntry) -> PathBuf {
        if id.starts_with("1:") {
            e.dir.clone()
        } else {
            self.user.clone()
        }
    }
}

/// Files the share sheet / "Open in" delivered (copied into Documents/Inbox),
/// and score files dropped into Documents via the Files app that need
/// unpacking (.4sc, .4ss): import them into the user library. Originals are
/// moved to Documents/.imported, never deleted.
fn sweep_incoming(paths: &Paths) -> Vec<String> {
    let mut added = Vec::new();
    let done = paths.user.join(".imported");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(paths.user.join("Inbox")) {
        candidates.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_file()));
    }
    if let Ok(rd) = std::fs::read_dir(&paths.user) {
        candidates.extend(rd.flatten().map(|e| e.path()).filter(|p| {
            p.is_file()
                && p.extension().is_some_and(|x| {
                    x.eq_ignore_ascii_case("4sc")
                        || x.eq_ignore_ascii_case("4ss")
                        || x.eq_ignore_ascii_case(cantor_library::bundle::BUNDLE_EXT)
                        || x.eq_ignore_ascii_case(cantor_library::backup::BACKUP_EXT)
                })
        }));
    }
    for path in candidates {
        added.extend(import_path(paths, &path));
        let _ = std::fs::create_dir_all(&done);
        if let Some(name) = path.file_name() {
            let _ = std::fs::rename(&path, done.join(name));
        }
    }
    added
}

/// Import one file into the user library; returns the new score ids.
fn import_path(paths: &Paths, path: &std::path::Path) -> Vec<String> {
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(cantor_library::backup::BACKUP_EXT))
    {
        // DATA-03: a backup received from Files/AirDrop is never restored
        // silently (it replaces the library); it waits in Documents/Backups
        // and the library screen offers to restore it.
        let dir = paths.user.join("Backups");
        let _ = std::fs::create_dir_all(&dir);
        if let Some(n) = path.file_name() {
            let _ = std::fs::copy(path, dir.join(n));
        }
        return Vec::new();
    }
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case(cantor_library::bundle::BUNDLE_EXT))
    {
        // SET-02: a shared setlist bundle (scores, annotations, mixes).
        let Ok(bytes) = std::fs::read(path) else {
            return Vec::new();
        };
        let mut st = store(paths);
        return match cantor_library::bundle::import_bundle(
            &paths.user,
            &mut st,
            &paths.user.join("Annotations"),
            &bytes,
        ) {
            Ok(b) => {
                if let Err(e) = st.save() {
                    log::warn!("bundle store save: {e:?}");
                }
                // SET-04: the library opens this set and shows its
                // instructions next time it looks (take_received_set).
                let marker = paths.user.join(".incoming");
                let _ = std::fs::create_dir_all(&marker);
                let _ = std::fs::write(marker.join(RECEIVED_SET), &b.setlist_id);
                b.score_ids
            }
            Err(e) => {
                log::warn!("bundle import: {e}");
                Vec::new()
            }
        };
    }
    let ids = import_path_inner(paths, path);
    if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("4ss"))
    {
        if let Ok(bytes) = std::fs::read(path) {
            let name = path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            setlist_from_4ss(paths, &name, &bytes, &ids);
            // Keep the original for fields not imported yet (see
            // docs/forscore-samples-needed.md).
            let keep = paths.user.join(cantor_library::import::ORIGINALS_DIR);
            let _ = std::fs::create_dir_all(&keep);
            let _ = std::fs::write(keep.join(format!("{}.4ss", file_safe(&name))), &bytes);
        }
    }
    ids
}

/// LIB-08: a forScore setlist also becomes a Cantor setlist, in the same
/// order, pointing at the scores just imported from it.
fn setlist_from_4ss(paths: &Paths, name: &str, bytes: &[u8], ids: &[String]) {
    let Ok(set) = cantor_library::forscore::parse_4ss(bytes) else {
        return;
    };
    let titles = set.items.iter().filter_map(|it| match it.entry {
        cantor_library::forscore::SetlistEntry::Score(_) => Some(it.title.clone()),
        _ => None,
    });
    let imported: Vec<(String, String)> = titles.zip(ids.iter().cloned()).collect();
    let sl = cantor_library::store::setlist_from_forscore(name, &set, &imported);
    let mut st = cantor_library::store::Store::open(&paths.user);
    st.put_setlist(sl);
    if let Err(e) = st.save() {
        log::warn!("setlist save: {e:?}");
    }
}

fn import_path_inner(paths: &Paths, path: &std::path::Path) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else {
        log::warn!("import: cannot read {}", path.display());
        return Vec::new();
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    match cantor_library::import::import_file(&paths.user, &name, &bytes) {
        Ok(items) => items.into_iter().map(|i| format!("1:{}", i.stem)).collect(),
        Err(e) => {
            log::warn!("import {name}: {e}");
            Vec::new()
        }
    }
}

fn store(paths: &Paths) -> cantor_library::store::Store {
    cantor_library::store::Store::open(&paths.user)
}

fn save_store(st: &cantor_library::store::Store) -> Result<(), String> {
    st.save().map_err(|e| format!("{e:?}"))
}

#[derive(Serialize)]
struct LibraryData {
    metadata: std::collections::BTreeMap<String, cantor_library::store::Metadata>,
    setlists: Vec<cantor_library::store::Setlist>,
    home: cantor_library::store::HomeView,
    /// One-time hints already dismissed (first run).
    hints: Vec<String>,
}

/// Metadata (LIB-03) and setlists (SET-01) for the library screen.
#[tauri::command]
fn library_data(paths: State<'_, Paths>) -> LibraryData {
    let mut st = store(&paths);
    // LIB-03: fill empty metadata from each MusicXML file's own header
    // (title, composer, arranger, lyricist) once; user edits win.
    let mut changed = false;
    for (d, dir) in paths.dirs().iter().enumerate() {
        for e in cantor_library::import::list_library(&[dir]) {
            changed |= st.seed_metadata(&format!("{d}:{}", e.stem), &e.header);
        }
    }
    if changed {
        let _ = save_store(&st);
    }
    LibraryData {
        metadata: st.all_metadata().clone(),
        setlists: st.setlists_ordered().into_iter().cloned().collect(),
        home: st.home_view().clone(),
        hints: st.hints_seen(),
    }
}

#[tauri::command]
fn set_metadata(
    id: String,
    metadata: cantor_library::store::Metadata,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    let mut st = store(&paths);
    st.set_metadata(&id, metadata);
    save_store(&st)
}

/// SET-04: file (in `.incoming`, never synced or backed up) naming the
/// setlist of the last received `.cantorset`.
const RECEIVED_SET: &str = "received-set";

/// SET-04: the setlist id of a just-received set, once (then forgotten).
#[tauri::command]
fn take_received_set(paths: State<'_, Paths>) -> Option<String> {
    let f = paths.user.join(".incoming").join(RECEIVED_SET);
    let id = std::fs::read_to_string(&f).ok()?;
    let _ = std::fs::remove_file(&f);
    Some(id.trim().to_string()).filter(|s| !s.is_empty())
}

/// SET-04: directory of a score's practice tracks (one file per part).
fn practice_dir(paths: &Paths, id: &str) -> PathBuf {
    paths
        .user
        .join(cantor_library::bundle::PRACTICE_DIR)
        .join(cantor_library::bundle::safe_id(id))
}

/// SET-04: `(part, file name)` of a score's practice tracks, by part.
fn practice_tracks(paths: &Paths, id: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(practice_dir(paths, id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            let (stem, ext) = n.rsplit_once('.')?;
            (!stem.starts_with('.')
                && ["m4a", "wav", "mp3", "aac"].contains(&ext.to_lowercase().as_str()))
            .then(|| (stem.to_string(), n.clone()))
        })
        .collect();
    v.sort();
    v
}

#[derive(Serialize)]
struct PracticeInfo {
    part: String,
    /// Path for the `cantor://` scheme: `practice/<safe id>/<file>`.
    url: String,
}

/// SET-03: a score's instructions, the member and their part, and the
/// practice tracks that came with it (SET-04).
#[derive(Serialize)]
struct InstructionsData {
    text: String,
    parts: Vec<cantor_library::store::PartHint>,
    member: String,
    member_part: Option<String>,
    practice: Vec<PracticeInfo>,
}

#[tauri::command]
fn score_instructions(id: String, paths: State<'_, Paths>) -> InstructionsData {
    let st = store(&paths);
    let ins = st.instructions(&id).cloned().unwrap_or_default();
    let safe = cantor_library::bundle::safe_id(&id);
    InstructionsData {
        member_part: st.member_part(&id).map(str::to_string),
        member: st.member().to_string(),
        text: ins.text,
        parts: ins.parts,
        practice: practice_tracks(&paths, &id)
            .into_iter()
            .map(|(part, f)| PracticeInfo {
                part,
                url: format!("practice/{safe}/{f}"),
            })
            .collect(),
    }
}

/// SET-03: save (or clear, when empty) a score's instructions and hints.
#[tauri::command]
fn set_instructions(
    id: String,
    instructions: cantor_library::store::ScoreInstructions,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    let mut st = store(&paths);
    st.set_instructions(&id, instructions);
    save_store(&st)
}

/// SET-03: who uses this library ("" = nobody); picks their part.
#[tauri::command]
fn set_member(name: String, paths: State<'_, Paths>) -> Result<(), String> {
    let mut st = store(&paths);
    st.set_member(&name);
    save_store(&st)
}

/// LIB-04: search titles, composers, tags, genres; returns score ids.
#[tauri::command]
fn search_scores(query: String, paths: State<'_, Paths>) -> Vec<String> {
    let entries: Vec<(String, String)> = paths
        .dirs()
        .iter()
        .enumerate()
        .flat_map(|(d, dir)| {
            cantor_library::import::list_library(&[dir])
                .into_iter()
                .map(move |e| (format!("{d}:{}", e.stem), e.title))
        })
        .collect();
    store(&paths).search(&query, &entries)
}

/// Create (no id) or replace a setlist; returns its id.
#[tauri::command]
fn save_setlist(
    setlist: Option<cantor_library::store::Setlist>,
    name: Option<String>,
    paths: State<'_, Paths>,
) -> Result<String, String> {
    let mut st = store(&paths);
    let id = match setlist {
        Some(sl) => {
            let id = sl.id.clone();
            st.put_setlist(sl);
            id
        }
        None => st.create_setlist(name.as_deref().unwrap_or("Ny lista")),
    };
    save_store(&st)?;
    Ok(id)
}

#[tauri::command]
fn delete_setlist(id: String, paths: State<'_, Paths>) -> Result<(), String> {
    let mut st = store(&paths);
    st.delete_setlist(&id);
    save_store(&st)
}

/// Settings remembered per score (mixer, my part, tempo, view, presets,
/// transposition, hidden parts).
#[tauri::command]
fn score_settings(id: String, paths: State<'_, Paths>) -> cantor_library::store::ScoreSettings {
    store(&paths).settings(&id).cloned().unwrap_or_default()
}

#[tauri::command]
fn save_score_settings(
    id: String,
    settings: cantor_library::store::ScoreSettings,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    let mut st = store(&paths);
    st.set_settings(&id, settings);
    save_store(&st)
}

/// PERF-03: give the starting note of one part, or all parts as a chord.
#[tauri::command]
fn pitch_pipe(channel: Option<usize>, player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| e.pitch_pipe(channel, 1.6))
}

#[derive(Default)]
struct Player {
    engine: Option<Arc<Mutex<Engine<SoundFontSynth>>>>,
    stream: Option<audio::Output>,
    /// Channel labels of the open score, for presets.
    channels: usize,
    /// What is needed to render the open score offline (export).
    render: Option<RenderSource>,
}

/// The open score, kept so exports can render with fresh engines.
struct RenderSource {
    title: String,
    /// Score id, so part exports also become its practice tracks (SET-04).
    id: String,
    timemap: cantor_timeline::Timemap,
    plan: cantor_timeline::channels::ChannelPlan,
    /// The backing track, so exports include it.
    backing: Option<cantor_audio::BackingTrack>,
}

type Shared = Mutex<Player>;

#[derive(Serialize)]
struct ScoreEntry {
    /// Score id `"<dir>:<stem>"` (0 = bundled, 1 = user library).
    file: String,
    title: String,
    /// Has MusicXML: plays.
    playable: bool,
    /// Has the original PDF.
    pdf: bool,
    /// In the user's own library (imported), not a bundled demo.
    user: bool,
}

/// PLAY-09: the instrument of each voice. In order: the instrument chosen for
/// this channel of this score, else the score's voice set, else the default
/// voice set for the choir type (mixed/male/female, from the user's choice,
/// the title, or which voices the score has).
fn apply_instruments(
    paths: &Paths,
    file: &str,
    plan: &mut cantor_timeline::channels::ChannelPlan,
    choir: Option<Choir>,
) -> &'static str {
    use cantor_timeline::channels::Role;
    let st = store(paths);
    let settings = st.settings(file).cloned().unwrap_or_default();
    let roles: Vec<Role> = plan.channels.iter().map(|c| c.role).collect();
    let choir_key = match choir {
        Some(Choir::Male) => "male",
        Some(Choir::Female) => "female",
        Some(Choir::Mixed) => "mixed",
        None => {
            let has = |r: Role| roles.contains(&r);
            if has(Role::Soprano) && (has(Role::Tenor) || has(Role::Bass)) {
                "mixed"
            } else if has(Role::Tenor) || has(Role::Bass) {
                "male"
            } else if has(Role::Soprano) || has(Role::Alto) {
                "female"
            } else {
                "mixed"
            }
        }
    };
    let set = settings
        .voice_set
        .as_deref()
        .and_then(|id| st.voice_sets().into_iter().find(|v| v.id == id))
        .or_else(|| st.default_voice_set(choir_key));
    for (i, c) in plan.channels.iter_mut().enumerate() {
        if let Some(Some(p)) = settings.programs.get(i) {
            if *p < 128 {
                c.program = *p;
                continue;
            }
        }
        let key = match c.role {
            Role::Soprano => "soprano",
            Role::Alto => "alto",
            Role::Tenor => "tenor",
            Role::Bass => "bass",
            Role::Voice => "voice",
            Role::Piano => "piano",
            Role::Instrument => "instrument",
        };
        if let Some(p) = set.as_ref().and_then(|v| v.program_for(key, c.number)) {
            c.program = p;
        }
    }
    choir_key
}

/// PLAY-09: change one voice's instrument now (also while playing) and
/// remember it for this score. `None` forgets the choice; the voice gets
/// its voice set's instrument when the score is opened again.
#[tauri::command]
fn set_instrument(
    file: String,
    index: usize,
    program: Option<u8>,
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
) -> Result<(), String> {
    let program = program.filter(|p| *p < 128);
    let mut st = store(&paths);
    let s = st.settings_mut(&file);
    if s.programs.len() <= index {
        s.programs.resize(index + 1, None);
    }
    s.programs[index] = program;
    save_store(&st)?;
    if let Some(p) = program {
        with_engine(&player, |e| {
            e.set_program(index, p);
        })?;
    }
    Ok(())
}

/// PLAY-09: all voice sets (built-in and own) and the default per choir type.
#[derive(Serialize)]
struct VoiceSets {
    sets: Vec<cantor_library::store::VoiceSet>,
    defaults: std::collections::BTreeMap<String, String>,
}

#[tauri::command]
fn voice_sets(paths: State<'_, Paths>) -> VoiceSets {
    let st = store(&paths);
    let defaults = ["mixed", "male", "female"]
        .iter()
        .filter_map(|c| st.default_voice_set(c).map(|v| (c.to_string(), v.id)))
        .collect();
    VoiceSets {
        sets: st.voice_sets(),
        defaults,
    }
}

/// PLAY-09: use a voice set for this score (clears per-voice choices), or,
/// with `default_for`, make it the default for a choir type.
#[tauri::command]
fn apply_voice_set(
    file: Option<String>,
    id: String,
    default_for: Option<String>,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    let mut st = store(&paths);
    if !st.voice_sets().iter().any(|v| v.id == id) {
        return Err(format!("unknown voice set {id}"));
    }
    if let Some(c) = default_for {
        st.set_default_voice_set(&c, &id);
    }
    if let Some(f) = file {
        let s = st.settings_mut(&f);
        s.voice_set = Some(id);
        s.programs.clear();
    }
    save_store(&st)
}

/// PLAY-09: save the current instruments of an open score as an own voice
/// set (one program per role, from its first line of each role).
#[tauri::command]
fn save_voice_set(
    set: cantor_library::store::VoiceSet,
    paths: State<'_, Paths>,
) -> Result<String, String> {
    let mut st = store(&paths);
    let id = st.save_voice_set(set);
    save_store(&st)?;
    Ok(id)
}

#[tauri::command]
fn remove_voice_set(id: String, paths: State<'_, Paths>) -> Result<(), String> {
    let mut st = store(&paths);
    st.remove_voice_set(&id);
    save_store(&st)
}

/// General MIDI instruments (0-based program, English name) for pickers.
#[tauri::command]
fn instruments() -> Vec<(u8, &'static str)> {
    (0..128u8).map(|p| (p, gm_full_name(p))).collect()
}

#[derive(Serialize)]
struct ChannelInfo {
    label: String,
    role: String,
    /// Divisi number within its role (1, 2 ...), 0 when the role has one line.
    number: u8,
    program: u8,
    instrument: String,
    pan: f32,
}

#[derive(Serialize)]
struct Opened {
    title: String,
    /// Plain MusicXML for Verovio.
    musicxml: String,
    channels: Vec<ChannelInfo>,
    /// Start time in seconds of each played measure, with the written
    /// measure index it comes from (repeats unfolded).
    measures: Vec<(f64, usize)>,
    duration: f64,
    choir: String,
    /// PLAY-09: choir type the instruments were chosen for ("mixed",
    /// "male", "female"; detected from the voices when `choir` is "auto").
    voice_choir: String,
    sample_rate: u32,
    /// Original PDF next to the score (`<name>.pdf`), if any: the page the
    /// singer reads. Fetched through the `cantor` URI scheme.
    pdf_url: Option<String>,
    /// Box of each written measure on the PDF pages (LIB-05), when it was
    /// recognised with the same measure count as the MusicXML.
    measure_boxes: Option<Vec<serde_json::Value>>,
    /// Has transposing instruments (`<transpose>`): offer the concert pitch toggle.
    has_transposing: bool,
    /// PERF-04 backing track settings, when the score has a recording.
    track: Option<TrackInfo>,
}

/// PERF-04: settings of a score's backing track, kept in `<stem>.track.json`.
#[derive(Serialize, serde::Deserialize, Clone, Default)]
struct TrackInfo {
    /// Track second at score second 0 (the recording's lead-in).
    #[serde(default)]
    offset_sec: f64,
    /// Page turns on the track timeline: `(track second, page)`.
    #[serde(default)]
    markers: Vec<(f64, u32)>,
    /// Length of the recording in seconds (filled when opened).
    #[serde(default)]
    duration_sec: f64,
}

fn load_track_info(
    paths: &Paths,
    id: &str,
    entry: &cantor_library::import::LibraryEntry,
) -> TrackInfo {
    std::fs::read(
        paths
            .track_dir(id, entry)
            .join(format!("{}.track.json", entry.stem)),
    )
    .ok()
    .and_then(|b| serde_json::from_slice(&b).ok())
    .unwrap_or_default()
}

/// Build the engine for an opened score: synth, score channels, and the
/// backing track as an extra channel. Returns the mixer channels, the
/// duration and the sample rate.
#[allow(clippy::too_many_arguments)]
fn start_engine(
    paths: &Paths,
    player: &State<'_, Shared>,
    remote_slot: &State<'_, remote::SharedEngine>,
    entry: &cantor_library::import::LibraryEntry,
    tm: &cantor_timeline::Timemap,
    plan: &cantor_timeline::channels::ChannelPlan,
    track: Option<&String>,
    info: &TrackInfo,
    track_dir: &std::path::Path,
) -> Result<(Vec<ChannelInfo>, f64, u32), String> {
    let mut p = guard::lock(player);
    // Stop the old stream before opening a new one.
    p.stream = None;
    let sr = audio::output_sample_rate().unwrap_or(48_000);
    let synth = SoundFontSynth::open(&paths.soundfont, sr)?;
    let mut engine = Engine::new(synth, tm, plan, sr);
    let mut channels: Vec<ChannelInfo> = plan
        .channels
        .iter()
        .map(|c| ChannelInfo {
            label: c.label.clone(),
            role: format!("{:?}", c.role),
            number: c.number,
            program: c.program,
            instrument: gm_full_name(c.program).to_string(),
            pan: c.pan,
        })
        .collect();
    let mut backing = None;
    if let Some(t) = track {
        match cantor_audio::decode_file(&track_dir.join(t), sr) {
            Ok(clip) => {
                let bt = cantor_audio::BackingTrack::new(clip)
                    .with_offset(info.offset_sec)
                    .with_markers(
                        info.markers
                            .iter()
                            .map(|&(at_sec, page)| cantor_audio::TurnMarker { at_sec, page })
                            .collect(),
                    );
                backing = Some(bt.clone());
                engine.set_backing_track(bt);
                channels.push(ChannelInfo {
                    label: cantor_audio::BACKING_TRACK_LABEL.into(),
                    role: "Track".into(),
                    number: 0,
                    program: 0,
                    instrument: t
                        .rsplit_once('.')
                        .map(|(_, e)| e.to_uppercase())
                        .unwrap_or_default(),
                    pan: 0.0,
                });
            }
            Err(e) => log::warn!("backing track {t}: {e}"),
        }
    }
    let duration = engine.duration();
    let engine = Arc::new(Mutex::new(engine));
    p.stream = Some(audio::Output::start(engine.clone())?);
    *guard::lock(remote_slot) = Some(engine.clone());
    p.engine = Some(engine);
    p.channels = channels.len();
    p.render = Some(RenderSource {
        id: String::new(),
        title: entry.title.clone(),
        timemap: tm.clone(),
        plan: plan.clone(),
        backing,
    });
    Ok((channels, duration, sr))
}

#[derive(Serialize)]
struct Status {
    playing: bool,
    position: f64,
    duration: f64,
    tempo: f64,
    /// PERF-04: position on the backing track (seconds), when there is one.
    track_position: Option<f64>,
    /// PERF-04: page-turn markers that fired since the last poll (pages).
    turns: Vec<u32>,
}

/// The 128 General MIDI instrument names (program 0-based).
const GM_NAMES: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavinet",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Acoustic Guitar (nylon)",
    "Acoustic Guitar (steel)",
    "Electric Guitar (jazz)",
    "Electric Guitar (clean)",
    "Electric Guitar (muted)",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Electric Bass (finger)",
    "Electric Bass (pick)",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Lead 1 (square)",
    "Lead 2 (sawtooth)",
    "Lead 3 (calliope)",
    "Lead 4 (chiff)",
    "Lead 5 (charang)",
    "Lead 6 (voice)",
    "Lead 7 (fifths)",
    "Lead 8 (bass + lead)",
    "Pad 1 (new age)",
    "Pad 2 (warm)",
    "Pad 3 (polysynth)",
    "Pad 4 (choir)",
    "Pad 5 (bowed)",
    "Pad 6 (metallic)",
    "Pad 7 (halo)",
    "Pad 8 (sweep)",
    "FX 1 (rain)",
    "FX 2 (soundtrack)",
    "FX 3 (crystal)",
    "FX 4 (atmosphere)",
    "FX 5 (brightness)",
    "FX 6 (goblins)",
    "FX 7 (echoes)",
    "FX 8 (sci-fi)",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bagpipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

fn gm_full_name(p: u8) -> &'static str {
    GM_NAMES[(p as usize).min(127)]
}

#[tauri::command]
fn list_scores(paths: State<'_, Paths>) -> Result<Vec<ScoreEntry>, String> {
    sweep_incoming(&paths);
    let st = store(&paths);
    let mut out = Vec::new();
    for (d, dir) in paths.dirs().iter().enumerate() {
        for e in cantor_library::import::list_library(&[dir]) {
            let file = format!("{d}:{}", e.stem);
            let title = st
                .metadata(&file)
                .map(|m| m.title.clone())
                .filter(|t| !t.is_empty())
                .unwrap_or(e.title);
            out.push(ScoreEntry {
                file,
                title,
                playable: e.score_file.is_some(),
                pdf: e.pdf_file.is_some(),
                user: d == 1,
            });
        }
    }
    // The user's own order (dragged on the library screen), then by title.
    let order = st.in_score_order(
        out.iter()
            .map(|e| (e.file.clone(), e.title.clone()))
            .collect(),
    );
    let mut by_id: std::collections::HashMap<String, ScoreEntry> =
        out.into_iter().map(|e| (e.file.clone(), e)).collect();
    Ok(order
        .into_iter()
        .filter_map(|(id, _)| by_id.remove(&id))
        .collect())
}

/// URL path of a score's original PDF (for the cantor:// scheme), if any.
#[tauri::command]
fn pdf_url_of(file: String, paths: State<'_, Paths>) -> Result<Option<String>, String> {
    let entry = paths.entry(&file)?;
    let dir_index = file.split(':').next().unwrap_or("0").to_string();
    Ok(entry.pdf_file.as_ref().map(|f| format!("{dir_index}/{f}")))
}

/// LIB-03 from the page: recognise the text on a rendered first page (PNG
/// from pdf.js) on the device (Apple Vision, Swedish + English) and read
/// title, composer, arranger and lyricist from it. Runs off the main thread.
#[tauri::command]
async fn read_title_page(
    png: Vec<u8>,
    paths: State<'_, Paths>,
) -> Result<cantor_ocr::TitlePage, String> {
    let user = paths.user.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let t = std::time::Instant::now();
        let lines = cantor_ocr::recognize(&png, &["sv-SE", "en-US"]).map_err(|e| e.to_string())?;
        let ms = t.elapsed().as_millis();
        let m = cantor_ocr::guess_metadata(&lines);
        // Timing and result in Documents/.perf.log (read off the iPad).
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(user.join(".perf.log"))
        {
            use std::io::Write;
            let _ = writeln!(
                f,
                "ocr title page\t{ms} ms\t{} lines\t{:?}",
                lines.len(),
                m.title
            );
        }
        Ok(m)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// LIB-04: remember what the library opens on (grouping + selected tab).
#[tauri::command]
fn set_home_view(
    view: cantor_library::store::HomeView,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    let mut st = store(&paths);
    st.set_home_view(view);
    save_store(&st)
}

/// First run: remember that a one-time hint was dismissed.
#[tauri::command]
fn mark_hint_seen(id: String, paths: State<'_, Paths>) -> Result<(), String> {
    let mut st = store(&paths);
    st.mark_hint_seen(&id);
    save_store(&st)
}

/// Save the manual order of the library (score ids) or of the setlist tabs.
#[tauri::command]
fn set_order(kind: String, ids: Vec<String>, paths: State<'_, Paths>) -> Result<(), String> {
    let mut st = store(&paths);
    match kind.as_str() {
        "scores" => st.set_score_order(ids),
        "setlists" => st.set_setlist_order(ids),
        _ => return Err(format!("unknown order {kind}")),
    }
    save_store(&st)
}

/// Import a file picked in the app (raw bytes, name in the `x-name` header).
#[tauri::command]
fn import_bytes(
    request: tauri::ipc::Request<'_>,
    paths: State<'_, Paths>,
) -> Result<Vec<String>, String> {
    let name = request
        .headers()
        .get("x-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or("missing file name")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".into());
    };
    let lower = name.to_lowercase();
    if lower.ends_with(&format!(".{}", cantor_library::backup::BACKUP_EXT)) {
        // Parked in Documents/Backups; the library screen asks before restoring.
        let dir = paths.user.join("Backups");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(file_safe(&name)), bytes).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    if lower.ends_with(".4ss")
        || lower.ends_with(&format!(".{}", cantor_library::bundle::BUNDLE_EXT))
    {
        // Setlists (forScore .4ss, Cantor bundles) also create a setlist.
        let tmp = paths.user.join(".incoming");
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let path = tmp.join(file_safe(&name));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        let ids = import_path(&paths, &path);
        let _ = std::fs::remove_file(&path);
        if ids.is_empty() {
            return Err(format!("{name}: kunde inte läsas"));
        }
        return Ok(ids);
    }
    let items = cantor_library::import::import_file(&paths.user, &name, bytes)?;
    Ok(items.into_iter().map(|i| format!("1:{}", i.stem)).collect())
}

/// Export rehearsal audio (PLAY-11): `"mix"` renders what you hear (mixer,
/// tempo); `"parts"` renders one file per voice, each with that voice loud and
/// the others at -12 dB (the "My part" preset), so every singer gets a
/// learning track. Files go to Documents/Export (visible in Files) and the
/// share sheet opens. Returns the file names.
#[tauri::command]
async fn export_audio(
    what: String,
    format: Option<String>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    // PLAY-11: AAC (.m4a, small, the default) or 16-bit WAV.
    let ext = if format.as_deref() == Some("wav") {
        "wav"
    } else {
        "m4a"
    };
    let (jobs, out_dir, practice) = {
        let paths = app.state::<Paths>();
        let player = app.state::<Shared>();
        let p = guard::lock(&player);
        let src = p.render.as_ref().ok_or("no score open")?;
        // SET-04: learning tracks at normal tempo of a library score are
        // kept as its practice tracks, so a shared set carries them.
        let mut practice: Vec<(String, String)> = Vec::new();
        let engine = p.engine.as_ref().ok_or("no score open")?;
        let live = guard::lock(engine);
        let tempo = live.tempo_scale();
        let mix: Vec<(f32, bool, bool)> = (0..p.channels)
            .map(|i| {
                let m = live.mixer();
                (m.volume_db(i), m.muted(i), m.soloed(i))
            })
            .collect();
        let safe = |s: &str| {
            s.chars()
                .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
                .collect::<String>()
        };
        let tempo_tag = if (tempo - 1.0).abs() > 1e-6 {
            format!(" {}%", (tempo * 100.0).round())
        } else {
            String::new()
        };
        let mut jobs: Vec<(String, Option<usize>)> = Vec::new();
        if what == "parts" {
            for (i, c) in src.plan.channels.iter().enumerate() {
                if c.role.is_vocal() {
                    let name = format!(
                        "{} - {}{}.{ext}",
                        safe(&src.title),
                        safe(&c.label),
                        tempo_tag
                    );
                    if tempo_tag.is_empty() && !src.id.is_empty() {
                        practice.push((name.clone(), format!("{}.{ext}", safe(&c.label))));
                    }
                    jobs.push((name, Some(i)));
                }
            }
        } else {
            jobs.push((format!("{}{}.{ext}", safe(&src.title), tempo_tag), None));
        }
        let out_dir = paths.user.join("Export");
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let render = (
            src.timemap.clone(),
            src.plan.clone(),
            paths.soundfont.clone(),
            tempo,
            mix,
            src.backing.clone(),
        );
        let pdir = (!practice.is_empty()).then(|| practice_dir(&paths, &src.id));
        (
            jobs.into_iter()
                .map(|j| (j, render.clone()))
                .collect::<Vec<_>>(),
            out_dir,
            pdir.map(|d| (d, practice)),
        )
    };
    // Render off the UI thread; each job gets its own synth and engine.
    let out = out_dir.clone();
    let names = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for ((name, part), (tm, plan, sf, tempo, mix, backing)) in jobs {
            let sr = 44_100;
            let synth = SoundFontSynth::open(&sf, sr)?;
            let mut e = Engine::new(synth, &tm, &plan, sr);
            if let Some(mut bt) = backing {
                // The live track was decoded at the device rate; match the export.
                if bt.clip.sample_rate != sr {
                    bt.clip = std::sync::Arc::new(bt.clip.resampled(sr));
                }
                e.set_backing_track(bt);
            }
            e.set_tempo_scale(tempo);
            match part {
                Some(i) => e.mixer_mut().apply(Preset::MyPart(i)),
                None => {
                    for (i, &(db, mute, solo)) in mix.iter().enumerate() {
                        let m = e.mixer_mut();
                        m.set_volume_db(i, db);
                        m.set_mute(i, mute);
                        m.set_solo(i, solo);
                    }
                }
            }
            let wav = cantor_audio::export_wav(&mut e, 2.0);
            if name.ends_with(".m4a") {
                let (pcm, rate) = m4a::wav_pcm(&wav).ok_or("render produced no audio")?;
                m4a::write_m4a(&out.join(&name), &pcm, rate)?;
            } else {
                std::fs::write(out.join(&name), wav).map_err(|e| e.to_string())?;
            }
            names.push(name);
        }
        Ok(names)
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Some((dir, list)) = practice {
        // Replace the score's practice tracks with this export.
        let _ = std::fs::remove_dir_all(&dir);
        if std::fs::create_dir_all(&dir).is_ok() {
            for (from, to) in list {
                if let Err(e) = std::fs::copy(out_dir.join(&from), dir.join(&to)) {
                    log::warn!("practice track {to}: {e}");
                }
            }
        }
    }
    let files: Vec<PathBuf> = names.iter().map(|n| out_dir.join(n)).collect();
    share_on_main(&app, files)?;
    Ok(names)
}

/// DATA-03: back up the whole library (scores, notes, setlists, mixes) to
/// Documents/Export/Cantor <date>.cantorbackup and open the share sheet so it
/// can go to iCloud Drive, AirDrop or a computer. Returns the file name.
#[tauri::command]
async fn backup_library(app: tauri::AppHandle) -> Result<String, String> {
    let paths = app.state::<Paths>();
    let user = paths.user.clone();
    let out_dir = user.join("Export");
    let (name, path) =
        tauri::async_runtime::spawn_blocking(move || -> Result<(String, PathBuf), String> {
            let bytes = cantor_library::backup::backup(&user)?;
            std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let name = format!(
                "Cantor {}.{}",
                ymd(secs),
                cantor_library::backup::BACKUP_EXT
            );
            let path = out_dir.join(&name);
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            Ok((name, path))
        })
        .await
        .map_err(|e| e.to_string())??;
    share_on_main(&app, vec![path])?;
    Ok(name)
}

/// `YYYY-MM-DD` (UTC) for a Unix time, without a date crate.
fn ymd(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// DATA-03: restore a backup picked in the app (raw bytes). The current
/// library is moved aside into Documents/.before-restore-<time>, never deleted.
#[tauri::command]
fn restore_library(
    request: tauri::ipc::Request<'_>,
    paths: State<'_, Paths>,
) -> Result<usize, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".into());
    };
    let r = cantor_library::backup::restore(&paths.user, bytes)?;
    Ok(r.files)
}

/// Backups waiting to be restored (received from Files, AirDrop, Open in).
#[tauri::command]
fn pending_backups(paths: State<'_, Paths>) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(paths.user.join("Backups"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| n.ends_with(&format!(".{}", cantor_library::backup::BACKUP_EXT)))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Restore a waiting backup by name, or discard it (`discard: true`).
#[tauri::command]
fn restore_pending(name: String, discard: bool, paths: State<'_, Paths>) -> Result<usize, String> {
    let p = paths.user.join("Backups").join(file_safe(&name));
    if discard {
        return std::fs::remove_file(&p)
            .map(|_| 0)
            .map_err(|e| e.to_string());
    }
    let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
    let r = cantor_library::backup::restore(&paths.user, &bytes)?;
    let _ = std::fs::remove_file(&p);
    Ok(r.files)
}

/// NFR-03/04 probe: the UI reports a timing; the backend adds the process
/// memory footprint and writes both to the log (Console / devicectl).
#[tauri::command]
fn perf_mark(name: String, ms: f64, paths: State<'_, Paths>) -> serde_json::Value {
    let mem = resident_mb();
    log::info!("perf {name}: {ms:.0} ms, memory {mem:.0} MB");
    // Also appended to Documents/.perf.log, readable off the device.
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.user.join(".perf.log"))
    {
        use std::io::Write;
        let _ = writeln!(f, "{name}\t{ms:.0} ms\t{mem:.0} MB");
    }
    serde_json::json!({ "name": name, "ms": ms, "memory_mb": mem })
}

/// Physical memory footprint of this process in MB (what iOS uses for its
/// memory limit), via the Mach task_info call.
#[cfg(any(target_os = "ios", target_os = "macos"))]
fn resident_mb() -> f64 {
    #[repr(C)]
    #[derive(Default)]
    struct TaskVmInfo {
        virtual_size: u64,
        region_count: i32,
        page_size: i32,
        resident_size: u64,
        resident_size_peak: u64,
        device: u64,
        device_peak: u64,
        internal: u64,
        internal_peak: u64,
        external: u64,
        external_peak: u64,
        reusable: u64,
        reusable_peak: u64,
        purgeable_volatile_pmap: u64,
        purgeable_volatile_resident: u64,
        purgeable_volatile_virtual: u64,
        compressed: u64,
        compressed_peak: u64,
        compressed_lifetime: u64,
        phys_footprint: u64,
    }
    extern "C" {
        fn mach_task_self() -> u32;
        fn task_info(task: u32, flavor: i32, info: *mut TaskVmInfo, count: *mut u32) -> i32;
    }
    const TASK_VM_INFO: i32 = 22;
    let mut info = TaskVmInfo::default();
    let mut count = (std::mem::size_of::<TaskVmInfo>() / 4) as u32;
    // SAFETY: task_info writes at most `count` 32-bit words into `info`.
    let r = unsafe { task_info(mach_task_self(), TASK_VM_INFO, &mut info, &mut count) };
    if r == 0 {
        info.phys_footprint as f64 / 1_048_576.0
    } else {
        0.0
    }
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
fn resident_mb() -> f64 {
    0.0
}

/// LIB-06: correct one note of a user's score (OMR mistakes) and save it.
/// `edit` is `"up"`, `"down"` (a step in the key) or `"rest"`. The original
/// is kept once in Documents/.originals so the uncorrected file is never lost.
#[tauri::command]
fn correct_note(
    file: String,
    part: usize,
    measure: usize,
    note: usize,
    edit: String,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    if !file.starts_with("1:") {
        return Err("bundled scores are read-only; import a copy to correct it".into());
    }
    let e = paths.entry(&file)?;
    let name = e.score_file.ok_or("no MusicXML to correct")?;
    let path = e.dir.join(&name);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let xml =
        String::from_utf8_lossy(&cantor_score::musicxml_bytes(&bytes).map_err(|e| e.to_string())?)
            .into_owned();
    let op = match edit.as_str() {
        "up" => cantor_score::NoteEdit::Step(1),
        "down" => cantor_score::NoteEdit::Step(-1),
        "rest" => cantor_score::NoteEdit::Rest,
        _ => return Err(format!("unknown edit {edit}")),
    };
    let out = cantor_score::edit_note(&xml, part, measure, note, op).map_err(|e| e.to_string())?;
    // Keep the very first version, hidden from the library list.
    let (stem, _) = name.rsplit_once('.').unwrap_or((&name, "musicxml"));
    let keep = e.dir.join(".originals");
    std::fs::create_dir_all(&keep).map_err(|e| e.to_string())?;
    let orig = keep.join(&name);
    if !orig.exists() {
        std::fs::write(&orig, &bytes).map_err(|e| e.to_string())?;
    }
    // Saved as plain MusicXML next to the score; an .mxl is replaced by it.
    let target = e.dir.join(format!("{stem}.musicxml"));
    let tmp = target.with_extension("musicxml.tmp");
    std::fs::write(&tmp, out).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    if target != path {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}

/// PERF-04: attach a recording (raw bytes, name in `x-name`) to a score as
/// its backing track, `<stem>.track.<ext>`. Replaces an earlier track.
#[tauri::command]
fn attach_track(request: tauri::ipc::Request<'_>, paths: State<'_, Paths>) -> Result<(), String> {
    let h = |k: &str| {
        request
            .headers()
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(percent_decode)
    };
    let file = h("x-score").ok_or("missing score")?;
    let name = h("x-name").ok_or("missing file name")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".into());
    };
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    if !cantor_library::import::TRACK_EXT.contains(&ext.as_str()) {
        return Err(format!(
            "{name}: Cantor plays M4A, MP3, WAV, FLAC and AIFF recordings"
        ));
    }
    // Check it decodes before keeping it.
    cantor_audio::decode_audio(bytes, Some(&ext), 44_100).map_err(|e| format!("{name}: {e}"))?;
    let e = paths.entry(&file)?;
    // Bundled scores: the track goes to the user folder under the same name,
    // so it still belongs to the score (later folders win in the listing).
    let dir = if file.starts_with("1:") {
        e.dir.clone()
    } else {
        paths.user.clone()
    };
    if let Some(old) = &e.track_file {
        let _ = std::fs::remove_file(e.dir.join(old));
    }
    std::fs::write(dir.join(format!("{}.track.{ext}", e.stem)), bytes)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// PERF-04: remove a score's backing track (and its settings).
#[tauri::command]
fn remove_track(file: String, paths: State<'_, Paths>) -> Result<(), String> {
    let e = paths.entry(&file)?;
    let dir = paths.track_dir(&file, &e);
    if let Some(t) = &e.track_file {
        std::fs::remove_file(dir.join(t)).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(dir.join(format!("{}.track.json", e.stem)));
    Ok(())
}

/// PERF-04: save offset and page-turn markers, and apply them live.
#[tauri::command]
fn set_track_info(
    file: String,
    offset_sec: f64,
    markers: Vec<(f64, u32)>,
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
) -> Result<(), String> {
    let e = paths.entry(&file)?;
    let mut info = load_track_info(&paths, &file, &e);
    info.offset_sec = offset_sec;
    info.markers = markers.clone();
    info.markers.sort_by(|a, b| a.0.total_cmp(&b.0));
    let dir = if file.starts_with("1:") {
        e.dir.clone()
    } else {
        paths.user.clone()
    };
    cantor_library::import::write_atomic(
        &dir.join(format!("{}.track.json", e.stem)),
        &serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let _ = with_engine(&player, |eng| {
        eng.set_backing_offset(offset_sec);
        eng.set_turn_markers(
            markers
                .iter()
                .map(|&(at_sec, page)| cantor_audio::TurnMarker { at_sec, page })
                .collect(),
        );
    });
    Ok(())
}

/// DATA-02: this device's stable id for the sync log (created once).
fn device_id(paths: &Paths) -> String {
    let p = paths.user.join(".sync").join("device-id");
    if let Ok(s) = std::fs::read_to_string(&p) {
        let s = s.trim().to_string();
        if cantor_sync::valid_device_id(&s) {
            return s;
        }
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let id = format!("dev-{:x}-{:x}", secs, std::process::id());
    let _ = std::fs::create_dir_all(p.parent().unwrap_or(&paths.user));
    let _ = std::fs::write(&p, &id);
    id
}

/// The chosen sync folder, if any (resolved from its bookmark).
fn sync_folder(paths: &Paths) -> Option<PathBuf> {
    let b = std::fs::read(folder::bookmark_path(&paths.user)).ok()?;
    folder::resolve(&b)
}

#[derive(Serialize)]
struct SyncStatus {
    folder: Option<String>,
    sent: usize,
    received: usize,
    changed: usize,
    pending: usize,
}

/// DATA-02: sync the library with the chosen folder now. Returns what
/// happened; `folder: None` when no folder is chosen.
#[tauri::command]
async fn sync_now(app: tauri::AppHandle) -> Result<SyncStatus, String> {
    let paths = app.state::<Paths>();
    let Some(shared) = sync_folder(&paths) else {
        return Ok(SyncStatus {
            folder: None,
            sent: 0,
            received: 0,
            changed: 0,
            pending: 0,
        });
    };
    let user = paths.user.clone();
    let id = device_id(&paths);
    let name = shared.file_name().map(|n| n.to_string_lossy().to_string());
    let r = tauri::async_runtime::spawn_blocking(move || {
        cantor_sync::SyncFolder::open(&shared, &id)?.sync(&user)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(SyncStatus {
        folder: name,
        sent: r.sent,
        received: r.received,
        changed: r.files_written
            + r.files_removed
            + r.docs_written
            + r.docs_removed
            + usize::from(r.store_written),
        pending: r.pending,
    })
}

/// DATA-02: choose the sync folder (iOS: the Files folder picker; desktop:
/// a path argument). Returns the folder name.
#[tauri::command]
async fn choose_sync_folder(
    path: Option<String>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let paths = app.state::<Paths>();
    let bookmark: Option<Vec<u8>> = {
        #[cfg(target_os = "ios")]
        {
            let _ = &path;
            let (tx, rx) = std::sync::mpsc::channel();
            let (etx, erx) = std::sync::mpsc::channel();
            app.run_on_main_thread(move || {
                let _ = etx.send(folder::present(tx));
            })
            .map_err(|e| e.to_string())?;
            erx.recv().map_err(|e| e.to_string())??;
            tauri::async_runtime::spawn_blocking(move || rx.recv().ok().flatten())
                .await
                .map_err(|e| e.to_string())?
        }
        #[cfg(not(target_os = "ios"))]
        {
            path.map(|p| p.into_bytes())
        }
    };
    let Some(b) = bookmark else {
        return Ok(None);
    };
    let bp = folder::bookmark_path(&paths.user);
    std::fs::create_dir_all(bp.parent().unwrap_or(&paths.user)).map_err(|e| e.to_string())?;
    std::fs::write(&bp, &b).map_err(|e| e.to_string())?;
    Ok(sync_folder(&paths).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string())))
}

/// DATA-02: stop syncing (the shared folder is left as it is).
#[tauri::command]
fn forget_sync_folder(paths: State<'_, Paths>) -> Result<(), String> {
    let _ = std::fs::remove_file(folder::bookmark_path(&paths.user));
    Ok(())
}

/// Name of the chosen sync folder, if any.
#[tauri::command]
fn sync_folder_name(paths: State<'_, Paths>) -> Option<String> {
    sync_folder(&paths).and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Open the share sheet for files (on the UI thread, as UIKit requires).
fn share_on_main(app: &tauri::AppHandle, files: Vec<PathBuf>) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(share::share_files(&files));
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

fn file_safe(s: &str) -> String {
    s.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

fn read_annotations(paths: &Paths, id: &str) -> Option<cantor_annotations::Document> {
    std::fs::read_to_string(annotations_path(paths, id))
        .ok()
        .and_then(|s| cantor_annotations::Document::from_json(&s).ok())
}

/// Page count of a PDF (`/Type /Page` objects), enough to check that a PDF
/// and a MusicXML layout describe the same engraving.
fn pdf_page_count(bytes: &[u8]) -> Option<u32> {
    let pat = b"/Type";
    let mut n = 0u32;
    let mut i = 0;
    while let Some(pos) = bytes[i..].windows(pat.len()).position(|w| w == pat) {
        let rest = &bytes[i + pos + pat.len()..];
        let rest = &rest[rest.iter().take_while(|b| b.is_ascii_whitespace()).count()..];
        if rest.starts_with(b"/Page") && !rest.starts_with(b"/Pages") {
            n += 1;
        }
        i += pos + pat.len();
    }
    (n > 0).then_some(n)
}

/// LIB-05: measure boxes on the score's PDF. A `<stem>.measures.json` (from
/// OMR, or written by hand) wins. Otherwise, when the PDF was engraved from
/// the same MusicXML (MuseScore and other notation programs write page
/// layout into the file), the boxes are computed from that layout, if the
/// page counts agree, and cached next to the score.
fn measure_map(e: &cantor_library::import::LibraryEntry) -> Option<Vec<serde_json::Value>> {
    let path = e.dir.join(format!("{}.measures.json", e.stem));
    if let Some(v) = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok())
    {
        return Some(v);
    }
    let score = std::fs::read(e.dir.join(e.score_file.as_ref()?)).ok()?;
    let pdf = std::fs::read(e.dir.join(e.pdf_file.as_ref()?)).ok()?;
    let boxes = cantor_score::layout_measure_boxes(&score)?;
    let last_page = boxes.iter().map(|b| b.page).max()?;
    if pdf_page_count(&pdf) != Some(last_page) {
        return None;
    }
    let v: Vec<serde_json::Value> = boxes
        .iter()
        .map(|b| serde_json::json!({ "page": b.page, "x": b.x, "y": b.y, "w": b.w, "h": b.h, "source": "layout" }))
        .collect();
    // Cache in the user's library only (bundled scores are read-only).
    let _ =
        cantor_library::import::write_atomic(&path, &serde_json::to_vec(&v).unwrap_or_default());
    Some(v)
}

/// The PDF of a score with its annotations drawn in (ANN-05), or the plain
/// PDF when there are none.
fn annotated_pdf(paths: &Paths, id: &str) -> Result<Option<Vec<u8>>, String> {
    let e = paths.entry(id)?;
    let Some(pdf) = &e.pdf_file else {
        return Ok(None);
    };
    let bytes = std::fs::read(e.dir.join(pdf)).map_err(|e| e.to_string())?;
    // Bar-anchored notes (drawn on the notation or the page) go where the
    // PDF measure map puts their bar (ANN-04/05).
    let map: Vec<cantor_annotations::Rect> = measure_map(&e)
        .map(|v| {
            v.iter()
                .map(|b| {
                    let g = |k: &str| b.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                    cantor_annotations::Rect {
                        page: g("page") as u32,
                        x: g("x"),
                        y: g("y"),
                        w: g("w"),
                        h: g("h"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(match read_annotations(paths, id) {
        Some(doc) if !doc.items.is_empty() => {
            cantor_annotations::annotate_pdf_with_layout(&bytes, &doc, &|m| map.get(m).copied())?
        }
        _ => bytes,
    }))
}

/// Export and share: `"pdf"` the annotated PDF (ANN-05), `"4sc"` a forScore
/// score (LIB-09) of the score `id`; `"setlist"` a Cantor bundle with scores,
/// annotations and mixes (SET-02) or `"4ss"` a forScore setlist (LIB-09) of
/// the setlist `id`. Files go to Documents/Export. Returns the file name.
#[tauri::command]
async fn export_item(kind: String, id: String, app: tauri::AppHandle) -> Result<String, String> {
    let paths = app.state::<Paths>();
    let out_dir = paths.user.join("Export");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let title_of = |sid: &str| -> String {
        let st = store(&paths);
        st.metadata(sid)
            .map(|m| m.title.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| paths.entry(sid).ok().map(|e| e.title))
            .unwrap_or_else(|| sid.to_string())
    };
    let (name, bytes) = match kind.as_str() {
        "pdf" | "4sc" => {
            let pdf = annotated_pdf(&paths, &id)?.ok_or("stycket har ingen PDF")?;
            let title = title_of(&id);
            if kind == "pdf" {
                (format!("{}.pdf", file_safe(&title)), pdf)
            } else {
                (
                    format!("{}.4sc", file_safe(&title)),
                    cantor_library::forscore::write_4sc(&title, &pdf),
                )
            }
        }
        "setlist" | "4ss" => {
            let st = store(&paths);
            let sl = st.setlist(&id).ok_or("listan finns inte")?.clone();
            let ids: Vec<String> = sl
                .items
                .iter()
                .filter_map(|it| match it {
                    cantor_library::store::SetItem::Score { score, .. } => Some(score.clone()),
                    _ => None,
                })
                .collect();
            if kind == "4ss" {
                let mut items = Vec::new();
                for sid in &ids {
                    if let Some(pdf) = annotated_pdf(&paths, sid)? {
                        items.push((title_of(sid), pdf));
                    }
                }
                (
                    format!("{}.4ss", file_safe(&sl.name)),
                    cantor_library::forscore::write_4ss(&sl.name, &items),
                )
            } else {
                let mut scores = Vec::new();
                for sid in &ids {
                    let Ok(e) = paths.entry(sid) else { continue };
                    let mut files = Vec::new();
                    let measures = format!("{}.measures.json", e.stem);
                    for f in [e.score_file.clone(), e.pdf_file.clone(), Some(measures)]
                        .into_iter()
                        .flatten()
                    {
                        if let Ok(b) = std::fs::read(e.dir.join(&f)) {
                            files.push((f, b));
                        }
                    }
                    scores.push(cantor_library::bundle::BundleScore {
                        id: sid.clone(),
                        title: title_of(sid),
                        files,
                        annotations_json: std::fs::read_to_string(annotations_path(&paths, sid))
                            .ok(),
                        metadata: st.metadata(sid).cloned(),
                        settings: st.settings(sid).cloned(),
                        // SET-03/04: instructions, hints and practice tracks.
                        instructions: st.instructions(sid).cloned(),
                        practice: practice_tracks(&paths, sid)
                            .into_iter()
                            .filter_map(|(part, f)| {
                                let b = std::fs::read(practice_dir(&paths, sid).join(&f)).ok()?;
                                Some((part, f, b))
                            })
                            .collect(),
                    });
                }
                (
                    format!(
                        "{}.{}",
                        file_safe(&sl.name),
                        cantor_library::bundle::BUNDLE_EXT
                    ),
                    cantor_library::bundle::export_bundle(&sl, &scores)?,
                )
            }
        }
        _ => return Err(format!("unknown export {kind}")),
    };
    let path = out_dir.join(&name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    share_on_main(&app, vec![path])?;
    Ok(name)
}

/// Annotations of the open score (ANN-01..04), edited through the crate's
/// undoable [`cantor_annotations::Editor`] and saved after every change to
/// `Documents/Annotations/<dir>_<stem>.json` (bundled scores are read-only,
/// so annotations always live in the user folder).
#[derive(Default)]
struct Annotations {
    open: Option<(PathBuf, cantor_annotations::Editor)>,
}

fn annotations_path(paths: &Paths, file: &str) -> PathBuf {
    let safe: String = file
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_().".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    paths.user.join("Annotations").join(format!("{safe}.json"))
}

/// Load (or create) the annotation document for a score.
#[tauri::command]
fn annotations_open(
    file: String,
    paths: State<'_, Paths>,
    ann: State<'_, Mutex<Annotations>>,
) -> Result<cantor_annotations::Document, String> {
    let path = annotations_path(&paths, &file);
    let doc = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| cantor_annotations::Document::from_json(&s).ok())
        .unwrap_or_default();
    let out = doc.clone();
    guard::lock(&ann).open = Some((path, cantor_annotations::Editor::new(doc)));
    Ok(out)
}

/// Apply an edit (`"undo"`/`"redo"` or a command) and return the document.
#[tauri::command]
fn annotations_edit(
    action: String,
    command: Option<cantor_annotations::Command>,
    ann: State<'_, Mutex<Annotations>>,
) -> Result<cantor_annotations::Document, String> {
    let mut a = guard::lock(&ann);
    let (path, ed) = a.open.as_mut().ok_or("no annotations open")?;
    match action.as_str() {
        "undo" => {
            ed.undo();
        }
        "redo" => {
            ed.redo();
        }
        _ => {
            let cmd = command.ok_or("missing command")?;
            ed.apply(cmd).map_err(|e| format!("{e:?}"))?;
        }
    }
    let doc = ed.document().clone();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    cantor_library::import::write_atomic(
        path,
        doc.to_json().map_err(|e| e.to_string())?.as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    Ok(doc)
}

/// Remove an imported score (all its files) from the user library.
/// LIB-05: pairs of a MusicXML-only and a PDF-only score in the user's
/// library that look like the same piece (same title, or the title printed
/// on the PDF's first page). Returned as score ids `(score, pdf)`.
#[tauri::command]
fn pair_suggestions(paths: State<'_, Paths>) -> Vec<(String, String)> {
    let dir = &paths.user;
    let entries = cantor_library::import::list_library(&[dir.as_path()]);
    let cands: Vec<cantor_library::import::Candidate> = entries
        .iter()
        .map(|e| {
            let pdf_text = match (&e.pdf_file, &e.score_file) {
                (Some(p), None) => std::fs::read(e.dir.join(p))
                    .ok()
                    .and_then(|b| cantor_annotations::pdf_info(&b))
                    .map(|i| format!("{} {}", i.title, i.first_page_text))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            cantor_library::import::Candidate {
                stem: e.stem.clone(),
                title: e.title.clone(),
                playable: e.score_file.is_some(),
                pdf: e.pdf_file.is_some(),
                pdf_text,
            }
        })
        .collect();
    let st = store(&paths);
    let dismissed = st.dismissed_pairs();
    cantor_library::import::pair_suggestions(&cands)
        .into_iter()
        .map(|(a, b)| (format!("1:{a}"), format!("1:{b}")))
        .filter(|p| !dismissed.contains(&format!("{}|{}", p.0, p.1)))
        .collect()
}

/// Combine a PDF-only score into a MusicXML score (LIB-05), or remember
/// that the user said no (`dismiss`). The PDF's annotations, metadata and
/// setlist entries move to the combined score.
#[tauri::command]
fn combine_scores(
    score: String,
    pdf: String,
    dismiss: Option<bool>,
    paths: State<'_, Paths>,
) -> Result<(), String> {
    combine_in(&paths, score, pdf, dismiss.unwrap_or(false))
}

/// [`combine_scores`] without Tauri state. Works for any MusicXML-only and
/// PDF-only pair the user picks (Info > "Combine with"), also after they
/// answered "No, they differ" to the suggestion.
fn combine_in(paths: &Paths, score: String, pdf: String, dismiss: bool) -> Result<(), String> {
    let mut st = store(paths);
    if dismiss {
        st.dismiss_pair(&format!("{score}|{pdf}"));
        return save_store(&st);
    }
    let (Some(s), Some(p)) = (score.strip_prefix("1:"), pdf.strip_prefix("1:")) else {
        return Err("only scores in your own library can be combined".into());
    };
    cantor_library::import::combine(&paths.user, s, p)?;
    // Annotations made on the PDF while it stood alone stay with it.
    let (from, to) = (
        annotations_path(paths, &pdf),
        annotations_path(paths, &score),
    );
    if from.exists() && !to.exists() {
        let _ = std::fs::rename(from, to);
    }
    // Metadata typed on the PDF fills what the score lacks; setlists that
    // pointed at the PDF now point at the score.
    if let Some(pm) = st.metadata(&pdf).cloned() {
        let mut m = st.metadata(&score).cloned().unwrap_or_default();
        for (slot, v) in [
            (&mut m.composer, &pm.composer),
            (&mut m.arranger, &pm.arranger),
            (&mut m.genre, &pm.genre),
            (&mut m.key, &pm.key),
        ] {
            if slot.is_empty() {
                *slot = v.clone();
            }
        }
        for t in pm.tags {
            if !m.tags.contains(&t) {
                m.tags.push(t);
            }
        }
        m.rating = m.rating.or(pm.rating);
        m.difficulty = m.difficulty.or(pm.difficulty);
        st.set_metadata(&score, m);
    }
    st.rename_score_id(&pdf, &score);
    save_store(&st)
}

#[tauri::command]
fn delete_score(file: String, paths: State<'_, Paths>) -> Result<(), String> {
    if !file.starts_with("1:") {
        return Err("bundled scores cannot be removed".into());
    }
    let e = paths.entry(&file)?;
    for f in [
        e.score_file,
        e.pdf_file,
        Some(format!("{}.measures.json", e.stem)),
        Some(format!("{}.mscz", e.stem)),
    ]
    .into_iter()
    .flatten()
    {
        let _ = std::fs::remove_file(e.dir.join(f));
    }
    let _ = std::fs::remove_file(
        e.dir
            .join(cantor_library::import::ORIGINALS_DIR)
            .join(format!("{}.4sc", e.stem)),
    );
    let _ = std::fs::remove_file(annotations_path(&paths, &file));
    let _ = std::fs::remove_dir_all(practice_dir(&paths, &file));
    // Setlists keep a placeholder with the title (SET-01).
    let mut st = store(&paths);
    st.remove_score(&file, &e.title);
    save_store(&st)
}

#[tauri::command]
fn open_score(
    file: String,
    choir: Option<String>,
    transpose: Option<i32>,
    concert: Option<bool>,
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
    remote_slot: State<'_, remote::SharedEngine>,
) -> Result<Opened, String> {
    let entry = paths.entry(&file)?;
    let dir_index = file.split(':').next().unwrap_or("0").to_string();
    let pdf_url = entry.pdf_file.as_ref().map(|f| format!("{dir_index}/{f}"));
    let track_info = load_track_info(&paths, &file, &entry);
    let Some(score_file) = entry.score_file.clone() else {
        // PDF only (e.g. a forScore .4sc): readable; plays its backing track
        // if one was added (PERF-04), no notes until converted.
        if let Some(track) = &entry.track_file {
            let tm = cantor_timeline::Timemap::default();
            let plan = cantor_timeline::channels::ChannelPlan::default();
            let (channels, duration, sr) = start_engine(
                &paths,
                &player,
                &remote_slot,
                &entry,
                &tm,
                &plan,
                Some(track),
                &track_info,
                &paths.track_dir(&file, &entry),
            )?;
            return Ok(Opened {
                title: entry.title,
                musicxml: String::new(),
                channels,
                measures: Vec::new(),
                duration,
                choir: "auto".into(),
                voice_choir: "mixed".into(),
                sample_rate: sr,
                pdf_url,
                measure_boxes: None,
                has_transposing: false,
                track: Some(track_info),
            });
        }
        let mut p = guard::lock(&player);
        p.stream = None;
        p.engine = None;
        p.channels = 0;
        p.render = None;
        *guard::lock(&remote_slot) = None;
        return Ok(Opened {
            title: entry.title,
            musicxml: String::new(),
            channels: Vec::new(),
            measures: Vec::new(),
            duration: 0.0,
            choir: "auto".into(),
            voice_choir: "mixed".into(),
            sample_rate: 0,
            pdf_url,
            measure_boxes: None,
            has_transposing: false,
            track: None,
        });
    };
    let bytes = std::fs::read(entry.dir.join(&score_file)).map_err(|e| e.to_string())?;
    let mut score = cantor_score::parse_score_file(&bytes).map_err(|e| e.to_string())?;
    // OMR files: drop OCR text that is not lyrics (dynamics, credits) and
    // chord symbols made from misread text before showing them.
    let mut musicxml = cantor_lyrics::clean_omr_musicxml(&String::from_utf8_lossy(
        &cantor_score::musicxml_bytes(&bytes).map_err(|e| e.to_string())?,
    ));
    // VIEW-05: transposing instruments shown at concert pitch on request
    // (sound is concert pitch either way).
    let has_transposing = musicxml.contains("<transpose");
    if concert.unwrap_or(false) && has_transposing {
        if let Ok(x) = cantor_score::musicxml_concert_pitch(&musicxml) {
            musicxml = x;
        }
    }
    // VIEW-05: transpose both what sounds and what is shown.
    let transpose = transpose.unwrap_or(0).clamp(-12, 12);
    if transpose != 0 {
        score = score.transposed(transpose);
        if let Ok(x) = cantor_score::musicxml_transposed(&musicxml, transpose) {
            musicxml = x;
        }
    }
    let tm = cantor_timeline::build_timemap(&score);
    let title = entry.title.clone();
    let choir = match choir.as_deref() {
        Some("male") => Some(Choir::Male),
        Some("female") => Some(Choir::Female),
        Some("mixed") => Some(Choir::Mixed),
        _ => choir_from_title(&title),
    };
    let mut plan = plan_channels_for(&score, &tm, choir);
    // The choir type actually used (also when detected from the voices),
    // so the UI offers the right voice sets and defaults.
    let voice_choir = apply_instruments(&paths, &file, &mut plan, choir);

    let (channels, duration, sr) = start_engine(
        &paths,
        &player,
        &remote_slot,
        &entry,
        &tm,
        &plan,
        entry.track_file.as_ref(),
        &track_info,
        &paths.track_dir(&file, &entry),
    )?;

    let measures = tm
        .measure_starts
        .iter()
        .zip(&tm.measure_order)
        .map(|(&t, &m)| (tm.seconds_at(t), m))
        .collect();
    let choir_name = match choir {
        Some(Choir::Male) => "male",
        Some(Choir::Female) => "female",
        Some(Choir::Mixed) => "mixed",
        None => "auto",
    };
    let measure_boxes = measure_map(&entry).filter(|b| b.len() == score.measure_count());
    if let Some(r) = guard::lock(&player).render.as_mut() {
        r.id = file.clone();
    }
    Ok(Opened {
        pdf_url,
        measure_boxes,
        has_transposing,
        title,
        musicxml,
        channels,
        measures,
        duration,
        choir: choir_name.into(),
        voice_choir: voice_choir.into(),
        sample_rate: sr,
        track: entry.track_file.as_ref().map(|_| track_info.clone()),
    })
}

fn with_engine<T>(
    player: &State<'_, Shared>,
    f: impl FnOnce(&mut Engine<SoundFontSynth>) -> T,
) -> Result<T, String> {
    let p = guard::lock(player);
    let e = p.engine.as_ref().ok_or("no score open")?;
    let mut e = guard::lock(e);
    Ok(f(&mut e))
}

#[tauri::command]
fn play(player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| {
        if e.finished() {
            e.seek(0.0);
        }
        e.play()
    })
}

#[tauri::command]
fn pause(player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| e.pause())
}

#[tauri::command]
fn seek(seconds: f64, player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| e.seek(seconds))
}

#[tauri::command]
fn set_tempo(scale: f64, player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| e.set_tempo_scale(scale))
}

/// A-B loop over written measures `a..=b` (0-based), or off (PLAY-07).
#[tauri::command]
fn set_loop(a: Option<usize>, b: Option<usize>, player: State<'_, Shared>) -> Result<(), String> {
    with_engine(&player, |e| {
        e.set_loop(a.zip(b));
        if let Some((start, end)) = e.loop_range() {
            let pos = e.position();
            if pos < start || pos >= end {
                e.seek(start);
            }
        }
    })
}

/// Metronome click and count-in (PLAY-06).
#[tauri::command]
fn set_click(
    metronome: Option<bool>,
    count_in: Option<bool>,
    player: State<'_, Shared>,
) -> Result<(), String> {
    with_engine(&player, |e| {
        if let Some(m) = metronome {
            e.set_metronome(m);
        }
        if let Some(c) = count_in {
            e.set_count_in(c);
        }
    })
}

#[tauri::command]
fn status(player: State<'_, Shared>) -> Result<Status, String> {
    let title = player
        .lock()
        .unwrap()
        .render
        .as_ref()
        .map(|r| r.title.clone())
        .unwrap_or_default();
    let st = with_engine(&player, |e| Status {
        playing: e.is_playing(),
        position: e.position(),
        duration: e.duration(),
        tempo: e.tempo_scale(),
        track_position: e.backing_channel().map(|_| e.track_position()),
        turns: e
            .take_fired_markers()
            .into_iter()
            .map(|(m, _)| m.page)
            .collect(),
    })?;
    // Lock screen card, refreshed about once a second while polled.
    thread_local!(static LAST: std::cell::Cell<(bool, i64)> = const { std::cell::Cell::new((false, -1)) });
    let key = (st.playing, st.position.floor() as i64);
    if LAST.with(|l| l.replace(key)) != key {
        remote::now_playing(&title, st.duration, st.position, st.playing);
    }
    Ok(st)
}

#[tauri::command]
fn set_channel(
    index: usize,
    volume_db: Option<f32>,
    mute: Option<bool>,
    solo: Option<bool>,
    player: State<'_, Shared>,
) -> Result<(), String> {
    with_engine(&player, |e| {
        let m = e.mixer_mut();
        if let Some(v) = volume_db {
            m.set_volume_db(index, v);
        }
        if let Some(v) = mute {
            m.set_mute(index, v);
        }
        if let Some(v) = solo {
            m.set_solo(index, v);
        }
    })
}

#[derive(Serialize)]
struct Strip {
    volume_db: f32,
    mute: bool,
    solo: bool,
}

#[tauri::command]
fn apply_preset(
    preset: String,
    index: Option<usize>,
    player: State<'_, Shared>,
) -> Result<Vec<Strip>, String> {
    let n = guard::lock(&player).channels;
    with_engine(&player, |e| {
        let p = match (preset.as_str(), index) {
            ("my_part", Some(i)) => Preset::MyPart(i),
            ("without_me", Some(i)) => Preset::WithoutMe(i),
            _ => Preset::Balanced,
        };
        let m = e.mixer_mut();
        m.apply(p);
        (0..n)
            .map(|i| Strip {
                volume_db: m.volume_db(i),
                mute: m.muted(i),
                solo: m.soloed(i),
            })
            .collect()
    })
}

/// Minimal %XX decoding for file names in cantor:// URLs.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resource_paths(app: &tauri::App) -> Paths {
    let res = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let pick = |rel: &str, dev: &str| {
        // Desktop bundles put resources under resource_dir; the iOS bundle
        // puts them under Cantor.app/assets.
        for base in [res.clone(), res.join("assets")] {
            let r = base.join(rel);
            if r.exists() {
                return r;
            }
        }
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dev)
    };
    // iOS: Documents (visible in Files). Elsewhere: the app data folder.
    #[cfg(target_os = "ios")]
    let user = app.path().document_dir();
    #[cfg(not(target_os = "ios"))]
    let user = app.path().app_data_dir().map(|d| d.join("library"));
    let user = user.unwrap_or_else(|_| PathBuf::from("library"));
    let _ = std::fs::create_dir_all(&user);
    Paths {
        user,
        scores: pick("resources/scores", "resources/scores"),
        soundfont: pick(
            "resources/GeneralUser-GS.sf2",
            "../../assets/sf2/GeneralUser-GS.sf2",
        ),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let paths = resource_paths(app);
            log::info!(
                "scores {} soundfont {}",
                paths.scores.display(),
                paths.soundfont.display()
            );
            app.manage(paths);
            app.manage(Shared::default());
            app.manage(Mutex::new(Annotations::default()));
            let slot: remote::SharedEngine = Default::default();
            remote::install(slot.clone());
            app.manage(slot);
            Ok(())
        })
        .register_uri_scheme_protocol("cantor", |ctx, req| {
            // cantor://localhost/<dir>%2F<file>.pdf -> bytes of a library PDF.
            let paths = ctx.app_handle().state::<Paths>();
            let path = percent_decode(req.uri().path().trim_start_matches('/'));
            // SET-04: practice/<safe id>/<file>: a practice track.
            if let Some(rest) = path.strip_prefix("practice/") {
                let audio = rest.split_once('/').and_then(|(d, name)| {
                    let ok = [d, name].iter().all(|p| {
                        !p.is_empty()
                            && !p.contains('/')
                            && !p.contains("..")
                            && !p.starts_with('.')
                    });
                    let ext = name.rsplit_once('.')?.1.to_lowercase();
                    let mime = match ext.as_str() {
                        "m4a" | "aac" => "audio/mp4",
                        "mp3" => "audio/mpeg",
                        "wav" => "audio/wav",
                        _ => return None,
                    };
                    ok.then(|| {
                        (
                            paths
                                .user
                                .join(cantor_library::bundle::PRACTICE_DIR)
                                .join(d)
                                .join(name),
                            mime,
                        )
                    })
                });
                return match audio.and_then(|(f, m)| Some((std::fs::read(f).ok()?, m))) {
                    Some((bytes, mime)) => tauri::http::Response::builder()
                        .header("Content-Type", mime)
                        .header("Access-Control-Allow-Origin", "*")
                        .body(bytes)
                        .unwrap(),
                    None => tauri::http::Response::builder()
                        .status(404)
                        .body(Vec::new())
                        .unwrap(),
                };
            }
            let file = path.split_once('/').and_then(|(d, name)| {
                let dir = paths.dirs().get(d.parse::<usize>().ok()?).copied()?;
                let ok = !name.contains('/') && !name.contains("..") && name.ends_with(".pdf");
                ok.then(|| dir.join(name))
            });
            match file.and_then(|f| std::fs::read(f).ok()) {
                Some(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "application/pdf")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(bytes)
                    .unwrap(),
                None => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap(),
            }
        })
        .invoke_handler(guarded(tauri::generate_handler![
            list_scores,
            open_score,
            play,
            pause,
            seek,
            set_tempo,
            set_loop,
            set_click,
            status,
            set_channel,
            apply_preset,
            import_bytes,
            delete_score,
            export_audio,
            annotations_open,
            annotations_edit,
            library_data,
            set_metadata,
            search_scores,
            save_setlist,
            delete_setlist,
            score_settings,
            save_score_settings,
            pitch_pipe,
            export_item,
            backup_library,
            restore_library,
            pending_backups,
            restore_pending,
            perf_mark,
            correct_note,
            attach_track,
            remove_track,
            set_track_info,
            sync_now,
            choose_sync_folder,
            forget_sync_folder,
            sync_folder_name,
            set_order,
            set_home_view,
            mark_hint_seen,
            score_instructions,
            set_instructions,
            set_member,
            take_received_set,
            set_instrument,
            voice_sets,
            apply_voice_set,
            save_voice_set,
            remove_voice_set,
            instruments,
            pair_suggestions,
            combine_scores,
            pdf_url_of,
            read_title_page
        ]))
        .build(tauri::generate_context!())
        .expect("error while building Cantor")
        .run(|app, event| {
            // Files shared to Cantor ("Open in…", AirDrop, Files): iOS copies
            // them to Documents/Inbox and hands us their URLs.
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                use tauri::Emitter;
                let paths = app.state::<Paths>();
                let mut ids = Vec::new();
                for u in urls {
                    if let Ok(p) = u.to_file_path() {
                        ids.extend(import_path(&paths, &p));
                        if p.parent().is_some_and(|d| d.ends_with("Inbox")) {
                            let done = paths.user.join(".imported");
                            let _ = std::fs::create_dir_all(&done);
                            if let Some(n) = p.file_name() {
                                let _ = std::fs::rename(&p, done.join(n));
                            }
                        }
                    }
                }
                ids.extend(sweep_incoming(&paths));
                let _ = app.emit("library-changed", ids);
            }
            let _ = (app, event);
        });
}

#[cfg(test)]
mod data_02_app {
    use super::*;

    fn paths(name: &str) -> Paths {
        let d = std::env::temp_dir().join(format!("cantor-app-sync-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Paths {
            scores: d.join("bundled"),
            user: d,
            soundfont: PathBuf::new(),
        }
    }

    #[test]
    fn data_02_two_devices_share_a_folder_through_the_app_settings() {
        let shared = std::env::temp_dir().join(format!("cantor-app-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&shared);
        std::fs::create_dir_all(&shared).unwrap();
        let (a, b) = (paths("a"), paths("b"));
        // Device ids are stable and distinct.
        let ida = device_id(&a);
        assert_eq!(device_id(&a), ida);
        assert_ne!(device_id(&b), ida);
        // Desktop "bookmark" = the path; both devices choose the same folder.
        for p in [&a, &b] {
            let bp = folder::bookmark_path(&p.user);
            std::fs::create_dir_all(bp.parent().unwrap()).unwrap();
            std::fs::write(&bp, shared.to_string_lossy().as_bytes()).unwrap();
            assert_eq!(sync_folder(p).as_deref(), Some(shared.as_path()));
        }
        // A score imported on A arrives on B, with its metadata.
        std::fs::write(a.user.join("Olaf.pdf"), b"%PDF-1.4 olaf").unwrap();
        let mut st = store(&a);
        st.set_metadata(
            "1:Olaf",
            cantor_library::store::Metadata {
                title: "Olaf Trygvason".into(),
                ..Default::default()
            },
        );
        st.save().unwrap();
        let sync = |p: &Paths| {
            cantor_sync::SyncFolder::open(&sync_folder(p).unwrap(), &device_id(p))
                .unwrap()
                .sync(&p.user)
                .unwrap()
        };
        sync(&a);
        sync(&b);
        assert_eq!(
            std::fs::read(b.user.join("Olaf.pdf")).unwrap(),
            b"%PDF-1.4 olaf"
        );
        assert_eq!(
            store(&b).metadata("1:Olaf").unwrap().title,
            "Olaf Trygvason"
        );
        // The library hash matches; sync state and the device id are not library content.
        assert_eq!(
            cantor_library::backup::library_hash(&a.user).unwrap(),
            cantor_library::backup::library_hash(&b.user).unwrap()
        );
    }
}

#[cfg(test)]
mod lib_05_app {
    use super::*;

    fn fixture(p: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../fixtures/scores/{p}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn lib_05_pdf_cursor_from_musicxml_layout_cached_and_guarded() {
        let d = std::env::temp_dir().join(format!("cantor-l5-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("lacrymosa.mxl"),
            fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.mxl"),
        )
        .unwrap();
        std::fs::write(
            d.join("lacrymosa.pdf"),
            fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf"),
        )
        .unwrap();
        let e = cantor_library::import::list_library(&[d.as_path()]).remove(0);
        let map = measure_map(&e).expect("boxes from the layout");
        assert_eq!(map.len(), 30);
        assert_eq!(map[0]["page"], 1);
        assert_eq!(map[29]["page"], 5);
        // Cached as the measure map file; a later open reads it.
        assert!(d.join("lacrymosa.measures.json").exists());
        // A PDF with another page count is a different engraving: no map.
        std::fs::remove_file(d.join("lacrymosa.measures.json")).unwrap();
        std::fs::write(
            d.join("lacrymosa.pdf"),
            fixture("the-anchor-song/the-anchor-song.pdf"),
        )
        .unwrap();
        let e = cantor_library::import::list_library(&[d.as_path()]).remove(0);
        assert!(measure_map(&e).is_none());
        assert!(!d.join("lacrymosa.measures.json").exists());
    }
    #[test]
    fn lib_05_combine_by_hand_after_answering_no() {
        let d = std::env::temp_dir().join(format!("cantor-l5c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let paths = Paths {
            scores: d.join("bundled"),
            user: d.clone(),
            soundfont: d.join("x.sf2"),
        };
        std::fs::write(
            d.join("lacrymosa.mxl"),
            fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.mxl"),
        )
        .unwrap();
        std::fs::write(
            d.join("scan-77.pdf"),
            fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf"),
        )
        .unwrap();
        // "No, they differ": remembered, not suggested again.
        combine_in(&paths, "1:lacrymosa".into(), "1:scan-77".into(), true).unwrap();
        assert!(store(&paths)
            .dismissed_pairs()
            .contains("1:lacrymosa|1:scan-77"));
        // Later the user combines them by hand: it still works, and the PDF
        // entry is gone from the library.
        combine_in(&paths, "1:lacrymosa".into(), "1:scan-77".into(), false).unwrap();
        assert!(d.join("lacrymosa.pdf").exists());
        assert!(!d.join("scan-77.pdf").exists());
        let lib = cantor_library::import::list_library(&[d.as_path()]);
        assert_eq!(lib.len(), 1);
        assert!(lib[0].pdf_file.is_some() && lib[0].score_file.is_some());
        // A score that already has a PDF cannot take another one.
        std::fs::write(
            d.join("other.pdf"),
            fixture("mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.pdf"),
        )
        .unwrap();
        assert!(combine_in(&paths, "1:lacrymosa".into(), "1:other".into(), false).is_err());
        assert!(d.join("other.pdf").exists());
    }
}

#[cfg(test)]
mod play_09_app {
    use super::*;

    #[test]
    fn play_09_instrument_order_own_choice_then_voice_set_then_default() {
        let d = std::env::temp_dir().join(format!("cantor-p9-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let paths = Paths {
            scores: d.clone(),
            user: d.clone(),
            soundfont: d.join("x.sf2"),
        };
        let x = std::fs::read(format!(
            "{}/../../fixtures/scores/mozart-lacrymosa/mozart-lacrymosa-from-requiem-satb-with-piano.mxl",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let score = cantor_score::parse_score_file(&x).unwrap();
        let tm = cantor_timeline::build_timemap(&score);
        let plan0 = plan_channels_for(&score, &tm, None);
        let role = |p: &cantor_timeline::channels::ChannelPlan,
                    r: cantor_timeline::channels::Role| {
            p.channels.iter().position(|c| c.role == r).unwrap()
        };
        // Default (no choices): the classic set, detected as a mixed choir.
        let mut plan = plan0.clone();
        assert_eq!(apply_instruments(&paths, "1:l", &mut plan, None), "mixed");
        let bass = role(&plan, cantor_timeline::channels::Role::Bass);
        assert_eq!(plan.channels[bass].program, 42);
        // Default set for mixed choirs = organ: bass on organ (GM 20, 0-based).
        let mut st = store(&paths);
        st.set_default_voice_set("mixed", "organ");
        st.save().unwrap();
        let mut plan = plan0.clone();
        apply_instruments(&paths, "1:l", &mut plan, None);
        assert_eq!(plan.channels[bass].program, 20);
        // The score's own set wins over the default.
        let mut st = store(&paths);
        st.settings_mut("1:l").voice_set = Some("choir-aahs".into());
        st.save().unwrap();
        let mut plan = plan0.clone();
        apply_instruments(&paths, "1:l", &mut plan, None);
        assert!(plan
            .channels
            .iter()
            .filter(|c| c.role.is_vocal())
            .all(|c| c.program == 52));
        // A voice's own choice wins over everything.
        let mut st = store(&paths);
        let s = st.settings_mut("1:l");
        s.programs = vec![None; bass + 1];
        s.programs[bass] = Some(71);
        st.save().unwrap();
        let mut plan = plan0.clone();
        apply_instruments(&paths, "1:l", &mut plan, None);
        assert_eq!(plan.channels[bass].program, 71);
        // The piano part is left alone by choir sets.
        let piano = role(&plan, cantor_timeline::channels::Role::Piano);
        assert_eq!(plan.channels[piano].program, plan0.channels[piano].program);
    }
}

/// NFR-13: wrap the command handler so a panic in any command rejects its
/// promise with a readable message instead of aborting the app. Async
/// commands do their heavy work in `spawn_blocking`, whose panics already
/// come back as `Err` through `.await.map_err`.
fn guarded<R: tauri::Runtime>(
    h: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let resolver = invoke.resolver.clone();
        guard::catch(|| h(invoke), true, move |m| resolver.reject(m))
    }
}
