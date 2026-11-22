//! Cantor demo app backend: score library, playback engine, mixer commands.
//!
//! The UI (Svelte + Verovio) asks for the score list, opens a score (gets its
//! MusicXML for engraving plus the mixer channels and measure times), and
//! drives transport and mixer. Audio runs in Rust: cpal output stream ->
//! `cantor_audio::Engine` (rustysynth + GeneralUser GS).

mod audio;
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
        cantor_library::import::list_library(&[dir])
            .into_iter()
            .find(|e| e.stem == stem)
            .ok_or_else(|| format!("{stem}: finns inte i biblioteket"))
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
}

/// Metadata (LIB-03) and setlists (SET-01) for the library screen.
#[tauri::command]
fn library_data(paths: State<'_, Paths>) -> LibraryData {
    let st = store(&paths);
    LibraryData {
        metadata: st.all_metadata().clone(),
        setlists: st.setlists().to_vec(),
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
    timemap: cantor_timeline::Timemap,
    plan: cantor_timeline::channels::ChannelPlan,
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

#[derive(Serialize)]
struct ChannelInfo {
    label: String,
    role: String,
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
    sample_rate: u32,
    /// Original PDF next to the score (`<name>.pdf`), if any: the page the
    /// singer reads. Fetched through the `cantor` URI scheme.
    pdf_url: Option<String>,
    /// Box of each written measure on the PDF pages (LIB-05), when it was
    /// recognised with the same measure count as the MusicXML.
    measure_boxes: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct Status {
    playing: bool,
    position: f64,
    duration: f64,
    tempo: f64,
}

fn gm_name(p: u8) -> &'static str {
    match p {
        0 => "Grand Piano",
        11 => "Vibraphone",
        13 => "Xylophone",
        19 => "Church Organ",
        24 => "Nylon Guitar",
        32 => "Acoustic Bass",
        40 => "Violin",
        41 => "Viola",
        42 => "Cello",
        43 => "Contrabass",
        52 => "Choir Aahs",
        53 => "Voice Oohs",
        56 => "Trumpet",
        57 => "Trombone",
        58 => "Tuba",
        60 => "French Horn",
        61 => "Brass Section",
        65 => "Alto Sax",
        66 => "Tenor Sax",
        68 => "Oboe",
        69 => "English Horn",
        70 => "Bassoon",
        71 => "Clarinet",
        72 => "Piccolo",
        73 => "Flute",
        74 => "Recorder",
        79 => "Ocarina",
        _ => "Instrument",
    }
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
    out.sort_by_key(|a| a.title.to_lowercase());
    Ok(out)
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
async fn export_audio(what: String, app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let (jobs, out_dir) = {
        let paths = app.state::<Paths>();
        let player = app.state::<Shared>();
        let p = player.lock().unwrap();
        let src = p.render.as_ref().ok_or("no score open")?;
        let engine = p.engine.as_ref().ok_or("no score open")?;
        let live = engine.lock().unwrap();
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
                    jobs.push((
                        format!("{} - {}{}.wav", safe(&src.title), safe(&c.label), tempo_tag),
                        Some(i),
                    ));
                }
            }
        } else {
            jobs.push((format!("{}{}.wav", safe(&src.title), tempo_tag), None));
        }
        let out_dir = paths.user.join("Export");
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        let render = (
            src.timemap.clone(),
            src.plan.clone(),
            paths.soundfont.clone(),
            tempo,
            mix,
        );
        (
            jobs.into_iter()
                .map(|j| (j, render.clone()))
                .collect::<Vec<_>>(),
            out_dir,
        )
    };
    // Render off the UI thread; each job gets its own synth and engine.
    let out = out_dir.clone();
    let names = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        for ((name, part), (tm, plan, sf, tempo, mix)) in jobs {
            let sr = 44_100;
            let synth = SoundFontSynth::open(&sf, sr)?;
            let mut e = Engine::new(synth, &tm, &plan, sr);
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
            std::fs::write(out.join(&name), wav).map_err(|e| e.to_string())?;
            names.push(name);
        }
        Ok(names)
    })
    .await
    .map_err(|e| e.to_string())??;
    let files: Vec<PathBuf> = names.iter().map(|n| out_dir.join(n)).collect();
    share_on_main(&app, files)?;
    Ok(names)
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
    let map: Vec<cantor_annotations::Rect> =
        std::fs::read(e.dir.join(format!("{}.measures.json", e.stem)))
            .ok()
            .and_then(|b| serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok())
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
    ann.lock().unwrap().open = Some((path, cantor_annotations::Editor::new(doc)));
    Ok(out)
}

/// Apply an edit (`"undo"`/`"redo"` or a command) and return the document.
#[tauri::command]
fn annotations_edit(
    action: String,
    command: Option<cantor_annotations::Command>,
    ann: State<'_, Mutex<Annotations>>,
) -> Result<cantor_annotations::Document, String> {
    let mut a = ann.lock().unwrap();
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
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, doc.to_json().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &*path).map_err(|e| e.to_string())?;
    Ok(doc)
}

/// Remove an imported score (all its files) from the user library.
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
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
    remote_slot: State<'_, remote::SharedEngine>,
) -> Result<Opened, String> {
    let entry = paths.entry(&file)?;
    let dir_index = file.split(':').next().unwrap_or("0").to_string();
    let pdf_url = entry.pdf_file.as_ref().map(|f| format!("{dir_index}/{f}"));
    let Some(score_file) = entry.score_file.clone() else {
        // PDF only (e.g. a forScore .4sc): readable, no sound until converted.
        let mut p = player.lock().unwrap();
        p.stream = None;
        p.engine = None;
        p.channels = 0;
        p.render = None;
        *remote_slot.lock().unwrap() = None;
        return Ok(Opened {
            title: entry.title,
            musicxml: String::new(),
            channels: Vec::new(),
            measures: Vec::new(),
            duration: 0.0,
            choir: "auto".into(),
            sample_rate: 0,
            pdf_url,
            measure_boxes: None,
        });
    };
    let bytes = std::fs::read(entry.dir.join(&score_file)).map_err(|e| e.to_string())?;
    let mut score = cantor_score::parse_score_file(&bytes).map_err(|e| e.to_string())?;
    // OMR files: drop OCR text that is not lyrics (dynamics, credits) and
    // chord symbols made from misread text before showing them.
    let mut musicxml = cantor_lyrics::clean_omr_musicxml(&String::from_utf8_lossy(
        &cantor_score::musicxml_bytes(&bytes).map_err(|e| e.to_string())?,
    ));
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
    let plan = plan_channels_for(&score, &tm, choir);

    let mut p = player.lock().unwrap();
    // Stop the old stream before opening a new one.
    p.stream = None;
    let sr = audio::output_sample_rate().unwrap_or(48_000);
    let synth = SoundFontSynth::open(&paths.soundfont, sr)?;
    let engine = Arc::new(Mutex::new(Engine::new(synth, &tm, &plan, sr)));
    p.stream = Some(audio::Output::start(engine.clone())?);
    *remote_slot.lock().unwrap() = Some(engine.clone());
    p.engine = Some(engine);
    p.channels = plan.channels.len();
    p.render = Some(RenderSource {
        title: title.clone(),
        timemap: tm.clone(),
        plan: plan.clone(),
    });

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
    let measure_boxes = std::fs::read(entry.dir.join(format!("{}.measures.json", entry.stem)))
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok())
        .filter(|b| b.len() == score.measure_count());
    Ok(Opened {
        pdf_url,
        measure_boxes,
        title,
        musicxml,
        channels: plan
            .channels
            .iter()
            .map(|c| ChannelInfo {
                label: c.label.clone(),
                role: format!("{:?}", c.role),
                program: c.program,
                instrument: gm_name(c.program).to_string(),
                pan: c.pan,
            })
            .collect(),
        measures,
        duration: tm.duration_sec,
        choir: choir_name.into(),
        sample_rate: sr,
    })
}

fn with_engine<T>(
    player: &State<'_, Shared>,
    f: impl FnOnce(&mut Engine<SoundFontSynth>) -> T,
) -> Result<T, String> {
    let p = player.lock().unwrap();
    let e = p.engine.as_ref().ok_or("no score open")?;
    let mut e = e.lock().unwrap();
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
    let n = player.lock().unwrap().channels;
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
        .invoke_handler(tauri::generate_handler![
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
            export_item
        ])
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
