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

/// Where the bundled demo scores and SoundFont live.
struct Paths {
    scores: PathBuf,
    soundfont: PathBuf,
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
    file: String,
    title: String,
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
    let mut out = Vec::new();
    let dir = std::fs::read_dir(&paths.scores).map_err(|e| format!("{}: {e}", paths.scores.display()))?;
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".mxl") || name.ends_with(".musicxml") || name.ends_with(".xml") {
            let title = name
                .rsplit_once('.')
                .map_or(name.as_str(), |(t, _)| t)
                .replace('_', " ");
            out.push(ScoreEntry { file: name, title });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(out)
}

#[tauri::command]
fn open_score(
    file: String,
    choir: Option<String>,
    paths: State<'_, Paths>,
    player: State<'_, Shared>,
) -> Result<Opened, String> {
    if file.contains('/') || file.contains("..") {
        return Err("bad file name".into());
    }
    let bytes = std::fs::read(paths.scores.join(&file)).map_err(|e| e.to_string())?;
    let score = cantor_score::parse_score_file(&bytes).map_err(|e| e.to_string())?;
    let musicxml = String::from_utf8_lossy(
        &cantor_score::musicxml_bytes(&bytes).map_err(|e| e.to_string())?,
    )
    .into_owned();
    let tm = cantor_timeline::build_timemap(&score);
    let title = file
        .rsplit_once('.')
        .map_or(file.as_str(), |(t, _)| t)
        .replace('_', " ");
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
    Ok(Opened {
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
    Paths {
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
        .invoke_handler(tauri::generate_handler![
            list_scores,
            open_score,
            play,
            pause,
            seek,
            set_tempo,
            status,
            set_channel,
            apply_preset
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cantor");
}
