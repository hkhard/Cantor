//! PLAY-06 (count-in, metronome click) and PLAY-07 (A-B loop), engine level.

use cantor_audio::{Engine, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;

#[derive(Default)]
struct Recorder {
    frame: u64,
    log: Vec<(u64, u8, u8, u8, u8)>,
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
const S: u64 = SR as u64;

/// 3/4 at 60 bpm, four measures, one quarter note per beat (C4).
fn engine() -> Engine<Recorder> {
    let beat =
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>"#;
    let m = |n: u32, attrs: &str| {
        format!(
            r#"<measure number="{n}">{attrs}{}</measure>"#,
            beat.repeat(3)
        )
    };
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
        <part id="P1">{}{}{}{}</part></score-partwise>"#,
        m(
            1,
            r#"<attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="60"/></direction>"#
        ),
        m(2, ""),
        m(3, ""),
        m(4, "")
    );
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    Engine::new(Recorder::default(), &tm, &plan, SR)
}

fn run(e: &mut Engine<Recorder>, seconds: f64) {
    let n = (seconds * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    for (lc, rc) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        e.render(lc, rc);
    }
}

/// Frames of note-ons on the music channels (not the click).
fn music_ons(e: &Engine<Recorder>) -> Vec<u64> {
    e.synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0 && m.2 != 9)
        .map(|m| m.0)
        .collect()
}

fn click_ons(e: &Engine<Recorder>) -> Vec<(u64, u8)> {
    e.synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0 && m.2 == 9)
        .map(|m| (m.0, m.3))
        .collect()
}

#[test]
fn play_07_loop_wraps_on_the_exact_frame() {
    let mut e = engine();
    // Loop measures 2..=3 (0-based written measures 1 and 2): 3 s to 9 s.
    e.set_loop(Some((1, 2)));
    e.seek(3.0);
    e.play();
    run(&mut e, 14.0);
    let ons = music_ons(&e);
    // 3 beats x 2 measures per pass, passes start at frames 0, 6 s, 12 s.
    let want: Vec<u64> = (0..14).map(|b| b * S).filter(|f| *f < 14 * S).collect();
    assert_eq!(ons[..14], want[..], "{ons:?}");
    assert!(
        e.position() < 9.0 && e.position() >= 3.0,
        "{}",
        e.position()
    );
}

#[test]
fn play_07_loop_releases_notes_at_the_wrap() {
    let mut e = engine();
    e.set_loop(Some((0, 0)));
    e.play();
    run(&mut e, 3.5);
    let log = &e.synth().log;
    let ons = log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0 && m.2 != 9)
        .count();
    let offs = log.iter().filter(|m| m.1 == 0x80 && m.2 != 9).count();
    assert_eq!(ons, 4);
    assert_eq!(offs, 3, "the last beat before the wrap was released");
}

#[test]
fn play_07_loop_off_plays_through() {
    let mut e = engine();
    e.set_loop(Some((0, 0)));
    e.set_loop(None);
    e.play();
    run(&mut e, 12.5);
    assert_eq!(music_ons(&e).len(), 12);
    assert!(e.finished());
}

#[test]
fn play_06_metronome_clicks_every_beat_with_accent_on_one() {
    let mut e = engine();
    e.set_metronome(true);
    e.play();
    run(&mut e, 6.5);
    let clicks = click_ons(&e);
    let frames: Vec<u64> = clicks.iter().map(|c| c.0).collect();
    assert_eq!(frames, (0..7).map(|b| b * S).collect::<Vec<_>>());
    // GM percussion: 76 hi wood block on the downbeat, 77 low wood block else.
    let keys: Vec<u8> = clicks.iter().map(|c| c.1).collect();
    assert_eq!(keys, [76, 77, 77, 76, 77, 77, 76]);
}

#[test]
fn play_06_count_in_one_measure_before_the_music() {
    let mut e = engine();
    e.set_count_in(true);
    e.play();
    run(&mut e, 4.5);
    let clicks = click_ons(&e);
    assert_eq!(
        clicks.iter().map(|c| c.0).collect::<Vec<_>>()[..3],
        [0, S, 2 * S],
        "three count-in clicks in 3/4"
    );
    let ons = music_ons(&e);
    assert_eq!(ons[0], 3 * S, "music starts after the count-in");
    assert!(
        e.position() > 1.4 && e.position() < 1.6,
        "score time excludes the count-in"
    );
}

#[test]
fn play_06_count_in_follows_tempo_scale_and_seek() {
    let mut e = engine();
    e.set_count_in(true);
    e.set_tempo_scale(2.0);
    e.seek(6.0); // measure 3
    e.play();
    run(&mut e, 2.0);
    let clicks = click_ons(&e);
    assert_eq!(
        clicks.iter().map(|c| c.0).collect::<Vec<_>>()[..3],
        [0, S / 2, S],
        "count-in at double speed"
    );
    assert_eq!(music_ons(&e)[0], 3 * S / 2);
}
