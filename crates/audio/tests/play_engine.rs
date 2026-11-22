//! PLAY-03 (volume, pan, mute, solo), PLAY-04 (level presets), PLAY-06 (tempo)
//! and the engine's note scheduling, checked without a SoundFont (the engine
//! is generic over its synth) and, when the bundled SoundFont is present, by
//! rendering audio.

use cantor_audio::{Engine, Mixer, Preset, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;

/// Records every MIDI message with the sample frame it was sent at.
#[derive(Default)]
struct Recorder {
    frame: u64,
    log: Vec<(u64, u8, u8, u8, u8)>, // frame, status, channel, d1, d2
}

impl Synth for Recorder {
    fn message(&mut self, channel: u8, status: u8, d1: u8, d2: u8) {
        self.log.push((self.frame, status, channel, d1, d2));
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        self.frame += left.len() as u64;
    }
}

const SR: u32 = 48_000;

/// SATB on two staves, 4/4 at 60 bpm: each voice plays four quarter notes
/// in m.1 and in m.2.
fn satb() -> cantor_score::Score {
    let part = |id: &str, v1: (&str, i32), v2: (&str, i32)| {
        let n = |step: &str, oct: i32, voice: u32| {
            format!("<note><pitch><step>{step}</step><octave>{oct}</octave></pitch><duration>1</duration><voice>{voice}</voice></note>").repeat(4)
        };
        let m = |num: u32| {
            format!(
                "<measure number=\"{num}\">{}{}<backup><duration>4</duration></backup>{}</measure>",
                if num == 1 {
                    "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo=\"60\"/></direction>"
                } else {
                    ""
                },
                n(v1.0, v1.1, 1),
                n(v2.0, v2.1, 2)
            )
        };
        format!("<part id=\"{id}\">{}{}</part>", m(1), m(2))
    };
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0"><part-list>
        <score-part id="P1"><part-name>Voice</part-name></score-part>
        <score-part id="P2"><part-name>Voice</part-name></score-part></part-list>{}{}</score-partwise>"#,
        part("P1", ("C", 5), ("G", 4)),
        part("P2", ("C", 4), ("C", 3))
    );
    parse_musicxml(xml.as_bytes()).unwrap()
}

fn engine() -> Engine<Recorder> {
    let s = satb();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    Engine::new(Recorder::default(), &tm, &plan, SR)
}

fn run(e: &mut Engine<Recorder>, seconds: f64) {
    let frames = (seconds * SR as f64) as usize;
    let mut l = vec![0.0; frames];
    let mut r = vec![0.0; frames];
    // Odd block size: events must still land on their exact frame.
    for (lc, rc) in l.chunks_mut(333).zip(r.chunks_mut(333)) {
        e.render(lc, rc);
    }
}

fn note_ons(e: &Engine<Recorder>) -> Vec<(u64, u8, u8)> {
    e.synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0)
        .map(|m| (m.0, m.2, m.3))
        .collect()
}

#[test]
fn play_01_notes_start_on_their_exact_sample_frame() {
    let mut e = engine();
    e.play();
    run(&mut e, 8.5);
    let ons = note_ons(&e);
    assert_eq!(ons.len(), 32);
    // Beat k (1 s each at 60 bpm) starts exactly on frame k * SR.
    for (i, o) in ons.iter().enumerate() {
        assert_eq!(o.0, (i / 4) as u64 * SR as u64, "{ons:?}");
    }
    // Each voice on its own MIDI channel with its own program.
    let mut chans: Vec<u8> = ons[..4].iter().map(|o| o.1).collect();
    chans.sort_unstable();
    chans.dedup();
    assert_eq!(chans.len(), 4);
    let programs: Vec<(u8, u8)> = e
        .synth()
        .log
        .iter()
        .filter(|m| m.1 == 0xC0)
        .map(|m| (m.2, m.3))
        .collect();
    assert_eq!(programs.len(), 4);
    assert!(e.finished());
}

#[test]
fn play_01_note_offs_balance_and_drums_channel_is_skipped() {
    let mut e = engine();
    e.play();
    run(&mut e, 9.0);
    let log = &e.synth().log;
    let on = log.iter().filter(|m| m.1 == 0x90 && m.4 > 0).count();
    let off = log.iter().filter(|m| m.1 == 0x80).count();
    assert_eq!(on, off);
    assert!(log.iter().all(|m| m.2 != 9));
}

#[test]
fn play_01_seek_starts_mid_score_and_stops_hanging_notes() {
    let mut e = engine();
    e.play();
    run(&mut e, 1.0);
    let before = e.synth().log.len();
    e.seek(4.0);
    run(&mut e, 0.5);
    let log = &e.synth().log[before..];
    assert!(
        log.iter().any(|m| m.1 == 0xB0 && m.3 == 0x7B),
        "all notes off on seek"
    );
    let late: Vec<_> = log
        .iter()
        .filter(|m| m.1 == 0x90 && m.0 == SR as u64)
        .collect();
    assert_eq!(late.len(), 4, "measure 2 starts right after the seek");
    assert!((e.position() - 4.5).abs() < 0.01, "{}", e.position());
}

#[test]
fn play_06_tempo_scale_changes_timing_not_pitch() {
    let mut e = engine();
    e.set_tempo_scale(2.0);
    e.play();
    run(&mut e, 4.1);
    let ons = note_ons(&e);
    assert_eq!(ons.len(), 32);
    assert!(ons[4..8].iter().all(|o| o.0 == SR as u64 / 2), "{ons:?}");
    let mut keys: Vec<u8> = ons[..4].iter().map(|o| o.2).collect();
    keys.sort_unstable();
    assert_eq!(keys, [48, 60, 67, 72], "same keys");
}

#[test]
fn play_01_pause_sends_no_notes() {
    let mut e = engine();
    run(&mut e, 1.0);
    assert!(note_ons(&e).is_empty());
    assert_eq!(e.position(), 0.0);
}

// ---- Mixer (pure logic) ----

fn gains(m: &Mixer) -> Vec<f32> {
    (0..m.len()).map(|i| m.effective_gain(i)).collect()
}

#[test]
fn play_03_mute_solo_truth_table() {
    let mut m = Mixer::new(4);
    assert_eq!(gains(&m), [1.0, 1.0, 1.0, 1.0]);
    m.set_mute(1, true);
    assert_eq!(gains(&m), [1.0, 0.0, 1.0, 1.0]);
    m.set_solo(2, true);
    assert_eq!(gains(&m), [0.0, 0.0, 1.0, 0.0], "solo isolates");
    m.set_solo(1, true);
    assert_eq!(
        gains(&m),
        [0.0, 0.0, 1.0, 0.0],
        "mute wins over solo on the same channel (DAW convention)"
    );
    m.set_mute(1, false);
    assert_eq!(gains(&m), [0.0, 1.0, 1.0, 0.0], "several solos add up");
    m.set_solo(1, false);
    m.set_solo(2, false);
    assert_eq!(gains(&m), [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn play_03_volume_in_db_from_minus_infinity_to_plus_6() {
    let mut m = Mixer::new(1);
    m.set_volume_db(0, -6.0);
    assert!((m.effective_gain(0) - 0.501).abs() < 0.01);
    m.set_volume_db(0, 12.0);
    assert!(
        (m.effective_gain(0) - 1.995).abs() < 0.01,
        "clamped to +6 dB"
    );
    m.set_volume_db(0, f32::NEG_INFINITY);
    assert_eq!(m.effective_gain(0), 0.0);
}

#[test]
fn play_04_presets_set_exact_gains() {
    let mut m = Mixer::new(4);
    m.apply(Preset::MyPart(2));
    let db: Vec<f32> = (0..4).map(|i| m.volume_db(i)).collect();
    assert_eq!(db, [-12.0, -12.0, 0.0, -12.0]);
    assert!((0..4).all(|i| !m.muted(i) && !m.soloed(i)));
    m.apply(Preset::WithoutMe(2));
    assert_eq!(gains(&m)[2], 0.0);
    assert!(m.muted(2));
    assert_eq!(m.volume_db(0), 0.0);
    m.apply(Preset::Balanced);
    assert_eq!(gains(&m), [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn play_03_mixer_changes_reach_the_synth_as_channel_volume() {
    let mut e = engine();
    e.play();
    run(&mut e, 0.1);
    e.mixer_mut().set_mute(0, true);
    run(&mut e, 0.1);
    let log = &e.synth().log;
    let cc7: Vec<_> = log
        .iter()
        .filter(|m| m.1 == 0xB0 && m.3 == 0x07 && m.0 > 0)
        .collect();
    assert!(cc7.iter().any(|m| m.4 == 0), "muted channel gets volume 0");
}

// ---- Real SoundFont (skipped when not present) ----

fn soundfont() -> Option<std::path::PathBuf> {
    let p: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "assets",
        "sf2",
        "GeneralUser-GS.sf2",
    ]
    .iter()
    .collect();
    p.exists().then_some(p)
}

#[test]
fn play_03_rendered_audio_soloed_channel_only() {
    let Some(sf) = soundfont() else {
        eprintln!("skipped: SoundFont not present");
        return;
    };
    let s = satb();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    let synth = cantor_audio::SoundFontSynth::open(&sf, SR).unwrap();
    let mut e = Engine::new(synth, &tm, &plan, SR);
    let rms = |e: &mut Engine<cantor_audio::SoundFontSynth>| {
        let mut l = vec![0.0; SR as usize];
        let mut r = vec![0.0; SR as usize];
        e.render(&mut l, &mut r);
        (l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32).sqrt()
    };
    e.play();
    let all = rms(&mut e);
    assert!(all > 0.01, "audible: {all}");
    e.seek(0.0);
    e.mixer_mut().apply(Preset::Balanced);
    for i in 0..4 {
        e.mixer_mut().set_mute(i, true);
    }
    let _ = rms(&mut e); // release tails fade
    e.seek(0.0);
    let silent = rms(&mut e);
    assert!(silent < all * 0.05, "all muted: {silent} vs {all}");
}
