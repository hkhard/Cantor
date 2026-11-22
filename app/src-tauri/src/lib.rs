//! Cantor demo app backend: score library, playback engine, mixer commands.
//!
//! The UI (Svelte + Verovio) asks for the score list, opens a score (gets its
//! MusicXML for engraving plus the mixer channels and measure times), and
//! drives transport and mixer. Audio runs in Rust: cpal output stream ->
//! `cantor_audio::Engine` (rustysynth + GeneralUser GS).

mod audio;

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
                && p.extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("4sc") || x.eq_ignore_ascii_case("4ss"))
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

#[derive(Default)]
struct Player {
    engine: Option<Arc<Mutex<Engine<SoundFontSynth>>>>,
    stream: Option<audio::Output>,
    /// Channel labels of the open score, for presets.
    channels: usize,
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
    let mut out = Vec::new();
    for (d, dir) in paths.dirs().iter().enumerate() {
        for e in cantor_library::import::list_library(&[dir]) {
            out.push(ScoreEntry {
                file: format!("{d}:{}", e.stem),
                title: e.title,
                playable: e.score_file.is_some(),
                pdf: e.pdf_file.is_some(),
                user: d == 1,
            });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

/// Import a file picked in the app (raw bytes, name in the `x-name` header).
#[tauri::command]
fn import_bytes(request: tauri::ipc::Request<'_>, paths: State<'_, Paths>) -> Result<Vec<String>, String> {
    let name = request
        .headers()
        .get("x-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or("missing file name")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".into());
    };
    let items = cantor_library::import::import_file(&paths.user, &name, bytes)?;
    Ok(items.into_iter().map(|i| format!("1:{}", i.stem)).collect())
}

/// Remove an imported score (all its files) from the user library.
#[tauri::command]
fn delete_score(file: String, paths: State<'_, Paths>) -> Result<(), String> {
    if !file.starts_with("1:") {
        return Err("bundled scores cannot be removed".into());
    }
    let e = paths.entry(&file)?;
    for f in [e.score_file, e.pdf_file, Some(format!("{}.measures.json", e.stem))]
        .into_iter()
        .flatten()
    {
        let _ = std::fs::remove_file(e.dir.join(f));
    }
    Ok(())
}

#[tauri::command]
fn open_score(
    file: String,
    choir: Option<String>,
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
) -> Result<Opened, String> {
    let entry = paths.entry(&file)?;
    let dir_index = file.split(':').next().unwrap_or("0").to_string();
    let pdf_url = entry
        .pdf_file
        .as_ref()
        .map(|f| format!("{dir_index}/{f}"));
    let Some(score_file) = entry.score_file.clone() else {
        // PDF only (e.g. a forScore .4sc): readable, no sound until converted.
        let mut p = player.lock().unwrap();
        p.stream = None;
        p.engine = None;
        p.channels = 0;
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
    let score = cantor_score::parse_score_file(&bytes).map_err(|e| e.to_string())?;
    // OMR files: drop OCR text that is not lyrics (dynamics, credits) and
    // chord symbols made from misread text before showing them.
    let musicxml = cantor_lyrics::clean_omr_musicxml(&String::from_utf8_lossy(
        &cantor_score::musicxml_bytes(&bytes).map_err(|e| e.to_string())?,
    ));
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
    p.engine = Some(engine);
    p.channels = plan.channels.len();

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
    with_engine(&player, |e| Status {
        playing: e.is_playing(),
        position: e.position(),
        duration: e.duration(),
        tempo: e.tempo_scale(),
    })
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
            delete_score
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
