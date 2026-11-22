//! PLAY-11 (P1): export the mix to an audio file. The exported length equals
//! the score length at the chosen tempo within 10 ms, and the file carries
//! the mixer state (what you hear is what you get).

use cantor_audio::{export_wav, Engine, Preset, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;

/// Outputs a constant level per sounding channel so the mix is measurable:
/// each held note adds 0.01 * channel gain (from CC7) to both sides.
#[derive(Default)]
struct Level {
    vol: [f32; 16],
    held: [u32; 16],
}

impl Synth for Level {
    fn message(&mut self, ch: u8, status: u8, d1: u8, d2: u8) {
        let c = ch as usize & 15;
        match status {
            0xB0 if d1 == 0x07 => self.vol[c] = d2 as f32 / 127.0,
            0xB0 if d1 == 0x7B => self.held[c] = 0,
            0x90 if d2 > 0 => self.held[c] += 1,
            0x80 => self.held[c] = self.held[c].saturating_sub(1),
            _ => {}
        }
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        let v: f32 = (0..16)
            .map(|c| self.held[c] as f32 * 0.01 * self.vol[c])
            .sum();
        l.fill(v);
        r.fill(v);
    }
}

const SR: u32 = 48_000;

/// Two voices on one staff, 4/4 at 120 bpm, 2 measures = 4.0 s.
fn engine() -> Engine<Level> {
    let n = |s: &str, v: u32| {
        format!("<note><pitch><step>{s}</step><octave>4</octave></pitch><duration>1</duration><voice>{v}</voice></note>").repeat(4)
    };
    let m = |num: u32, attrs: &str| {
        format!(
            r#"<measure number="{num}">{attrs}{}<backup><duration>4</duration></backup>{}</measure>"#,
            n("G", 1),
            n("C", 2)
        )
    };
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
        <part id="P1">{}{}</part></score-partwise>"#,
        m(
            1,
            r#"<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="120"/></direction>"#
        ),
        m(2, "")
    );
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    assert_eq!(plan.channels.len(), 2);
    Engine::new(Level::default(), &tm, &plan, SR)
}

/// (sample rate, channels, samples -1..1) from a 16-bit PCM WAV.
fn read_wav(b: &[u8]) -> (u32, u16, Vec<f32>) {
    assert_eq!(&b[0..4], b"RIFF");
    assert_eq!(&b[8..12], b"WAVE");
    let ch = u16::from_le_bytes([b[22], b[23]]);
    let sr = u32::from_le_bytes([b[24], b[25], b[26], b[27]]);
    assert_eq!(u16::from_le_bytes([b[34], b[35]]), 16);
    assert_eq!(&b[36..40], b"data");
    let len = u32::from_le_bytes([b[40], b[41], b[42], b[43]]) as usize;
    assert_eq!(b.len(), 44 + len, "header length matches data");
    let s = b[44..]
        .chunks(2)
        .map(|x| i16::from_le_bytes([x[0], x[1]]) as f32 / 32767.0)
        .collect();
    (sr, ch, s)
}

#[test]
fn play_11_export_length_within_10ms() {
    let mut e = engine();
    let (sr, ch, s) = read_wav(&export_wav(&mut e, 0.0));
    assert_eq!((sr, ch), (SR, 2));
    let secs = s.len() as f64 / 2.0 / sr as f64;
    assert!((secs - 4.0).abs() <= 0.010, "{secs}");
}

#[test]
fn play_11_export_follows_tempo() {
    let mut e = engine();
    e.set_tempo_scale(0.5);
    let (sr, _, s) = read_wav(&export_wav(&mut e, 0.0));
    let secs = s.len() as f64 / 2.0 / sr as f64;
    assert!(
        (secs - 8.0).abs() <= 0.010,
        "half speed doubles the length: {secs}"
    );
}

#[test]
fn play_11_export_tail_is_added_after_the_music() {
    let mut e = engine();
    let (sr, _, s) = read_wav(&export_wav(&mut e, 1.5));
    let secs = s.len() as f64 / 2.0 / sr as f64;
    assert!((secs - 5.5).abs() <= 0.010, "reverb tail: {secs}");
}

#[test]
fn play_11_export_carries_the_mix() {
    let level = |e: &mut Engine<Level>| {
        let (_, _, s) = read_wav(&export_wav(e, 0.0));
        s.iter().map(|x| x.abs()).sum::<f32>() / s.len() as f32
    };
    let mut e = engine();
    let both = level(&mut e);
    e.mixer_mut().apply(Preset::WithoutMe(0));
    let one = level(&mut e);
    assert!(
        one > 0.0 && one < both * 0.7,
        "one voice muted: {one} vs {both}"
    );
    e.mixer_mut().set_mute(1, true);
    assert_eq!(level(&mut e), 0.0, "everything muted is silent");
}

#[test]
fn play_11_export_leaves_live_playback_where_it_was() {
    let mut e = engine();
    e.seek(1.5);
    e.set_metronome(true);
    e.set_loop(Some((1, 1)));
    let _ = export_wav(&mut e, 0.0);
    assert!((e.position() - 1.5).abs() < 1e-9);
    assert!(!e.is_playing());
    assert!(e.metronome(), "settings kept");
    assert!(e.loop_range().is_some(), "loop kept for live playback");
}
