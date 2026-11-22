//! Sample-clock timing, measured against the audio clock (rendered frames):
//!
//! - PERF-03: metronome jitter < 1 ms. Every click lands on exactly the frame
//!   computed from its beat time (0 frames of jitter), whatever the block size.
//! - PLAY-06: a tempo change mid-playback loses or doubles no click.
//! - PLAY-07: the loop wraps on the exact frame (well inside one buffer).
//! - NFR-02: the engine position is derived from rendered frames only.
//!
//! The tempo (97 bpm, then 131 bpm) is chosen so beats fall between frames,
//! and the blocks are random sizes 64..=1024 frames.

use cantor_audio::{Engine, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::channels::{plan_channels, ChannelPlan};
use cantor_timeline::{build_timemap, Timemap};

/// Records every message with the output frame it arrived at.
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
const SRF: f64 = SR as f64;
/// One millisecond in frames.
const MS: f64 = SRF / 1000.0;

/// xorshift64*: deterministic random block sizes without a dependency.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn block(&mut self) -> usize {
        64 + (self.next() % 961) as usize
    }
}

/// 3/4, 16 measures, a quarter per beat; 97 bpm, 131 bpm from measure 9.
fn score() -> (Timemap, ChannelPlan) {
    let beat =
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>"#;
    let mut body = String::new();
    for n in 1..=16 {
        let attrs = match n {
            1 => {
                r#"<attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="97"/></direction>"#
            }
            9 => r#"<direction><sound tempo="131"/></direction>"#,
            _ => "",
        };
        body.push_str(&format!(
            r#"<measure number="{n}">{attrs}{}</measure>"#,
            beat.repeat(3)
        ));
    }
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list>
        <part id="P1">{body}</part></score-partwise>"#
    );
    let s = parse_musicxml(xml.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    (tm, plan)
}

fn engine(s: &(Timemap, ChannelPlan)) -> Engine<Recorder> {
    Engine::new(Recorder::default(), &s.0, &s.1, SR)
}

/// Render `frames` in random blocks.
fn run(e: &mut Engine<Recorder>, rng: &mut Rng, frames: u64) {
    let mut left = frames;
    let (mut l, mut r) = (vec![0.0; 1024], vec![0.0; 1024]);
    while left > 0 {
        let n = rng.block().min(left as usize);
        e.render(&mut l[..n], &mut r[..n]);
        left -= n as u64;
    }
}

fn clicks(e: &Engine<Recorder>) -> Vec<(u64, u8)> {
    e.synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0 && m.2 == 9)
        .map(|m| (m.0, m.3))
        .collect()
}

fn music_ons(e: &Engine<Recorder>) -> Vec<u64> {
    e.synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90 && m.4 > 0 && m.2 != 9)
        .map(|m| m.0)
        .collect()
}

/// The audio frame a time lands on: the first frame at or after it.
fn frame_at(sec: f64) -> u64 {
    (sec * SRF - 1e-6).ceil().max(0.0) as u64
}

/// Click key for a beat (GM hi / low wood block).
fn key(accent: bool) -> u8 {
    if accent {
        76
    } else {
        77
    }
}

#[test]
fn perf_03_metronome_clicks_on_the_exact_frame_with_random_blocks() {
    let sc = score();
    let tm = &sc.0;
    let beats = tm.beat_times();
    assert_eq!(beats.len(), 48);
    for seed in [1u64, 7, 0xC0FFEE, 99991] {
        let mut rng = Rng(seed);
        let mut e = engine(&sc);
        e.set_metronome(true);
        e.play();
        run(&mut e, &mut rng, frame_at(tm.duration_sec) + SR as u64);
        let got = clicks(&e);
        let want: Vec<(u64, u8)> = beats.iter().map(|&(s, a)| (frame_at(s), key(a))).collect();
        assert_eq!(got.len(), want.len(), "seed {seed}: one click per beat");
        let jitter = got
            .iter()
            .zip(&want)
            .map(|(g, w)| g.0.abs_diff(w.0))
            .max()
            .unwrap();
        println!(
            "PERF-03 seed {seed}: {} clicks, max jitter {jitter} frames ({:.3} ms)",
            got.len(),
            jitter as f64 / MS
        );
        assert_eq!(got, want, "seed {seed}");
        assert!((jitter as f64) < MS);
    }
}

#[test]
fn perf_03_metronome_exact_at_a_slower_tempo_scale() {
    let sc = score();
    let tm = &sc.0;
    let mut rng = Rng(42);
    let mut e = engine(&sc);
    e.set_tempo_scale(0.73);
    e.set_metronome(true);
    e.play();
    run(
        &mut e,
        &mut rng,
        frame_at(tm.duration_sec / 0.73) + SR as u64,
    );
    let want: Vec<(u64, u8)> = tm
        .beat_times()
        .iter()
        .map(|&(s, a)| (frame_at(s / 0.73), key(a)))
        .collect();
    assert_eq!(clicks(&e), want);
}

/// Clicks as (output frame, GM key).
type Clicks = Vec<(u64, u8)>;

/// Change the tempo scale at `change` output frames and return the clicks
/// together with the frames the test computes for them.
fn tempo_change(seed: u64, change: u64, from: f64, to: f64) -> (Clicks, Clicks) {
    let sc = score();
    let tm = &sc.0;
    let mut rng = Rng(seed);
    let mut e = engine(&sc);
    e.set_tempo_scale(from);
    e.set_metronome(true);
    e.play();
    run(&mut e, &mut rng, change);
    e.set_tempo_scale(to);
    let pos = change as f64 * from / SRF; // score seconds at the change
    let rest = frame_at((tm.duration_sec - pos) / to) + SR as u64;
    run(&mut e, &mut rng, rest);
    let want = tm
        .beat_times()
        .iter()
        .map(|&(s, a)| {
            let f = if frame_at(s / from) <= change {
                frame_at(s / from)
            } else {
                change + frame_at((s - pos) / to)
            };
            (f, key(a))
        })
        .collect();
    (clicks(&e), want)
}

#[test]
fn play_06_tempo_change_mid_playback_no_click_lost_or_doubled() {
    let sc = score();
    let tm = &sc.0;
    let beats = tm.beat_times();
    let mut changes = vec![];
    // Mid-beat, exactly on a beat frame, one frame before and after a beat,
    // and in the 131 bpm section.
    let b5 = frame_at(beats[5].0);
    changes.extend([12_345, b5, b5 - 1, b5 + 1, frame_at(beats[30].0) + 777]);
    for (i, &c) in changes.iter().enumerate() {
        for &(from, to) in &[(1.0, 1.5), (1.0, 0.5), (0.8, 2.0)] {
            let c = (c as f64 / from) as u64;
            let (got, want) = tempo_change(i as u64 + 3, c, from, to);
            assert_eq!(
                got.len(),
                beats.len(),
                "change at {c} ({from}->{to}): every beat clicks once"
            );
            assert_eq!(got, want, "change at {c} ({from}->{to})");
            // Accent pattern unbroken: downbeat every third click.
            assert!(got.iter().enumerate().all(|(k, g)| g.1 == key(k % 3 == 0)));
            // Strictly increasing: no doubled click at the seam.
            assert!(got.windows(2).all(|w| w[0].0 < w[1].0));
        }
    }
}

#[test]
fn play_06_tempo_change_applies_from_the_next_beat_on() {
    // Clicks after the change are spaced at the new tempo.
    let sc = score();
    let tm = &sc.0;
    let beats = tm.beat_times();
    let change = frame_at(beats[3].0) + 5_000;
    let (got, _) = tempo_change(11, change, 1.0, 1.25);
    let beat = 60.0 / 97.0 * SRF;
    for w in got.windows(2).take(7) {
        let gap = (w[1].0 - w[0].0) as f64;
        if w[0].0 >= change {
            assert!((gap - beat / 1.25).abs() <= 1.0, "new tempo: {gap}");
        } else if w[1].0 < change {
            assert!((gap - beat).abs() <= 1.0, "old tempo: {gap}");
        }
    }
}

#[test]
fn play_07_loop_wrap_frame_measured_with_random_blocks() {
    let sc = score();
    let tm = &sc.0;
    // Loop measures 3..=10 (0-based 2..=9): crosses the tempo change, and
    // neither end is on a whole frame.
    let spans: Vec<f64> = tm
        .measure_starts
        .iter()
        .map(|&t| tm.seconds_at(t))
        .collect();
    let (a, b) = (spans[2], spans[10]);
    let pass = frame_at(b - a);
    for seed in [5u64, 17, 2024] {
        let mut rng = Rng(seed);
        let mut e = engine(&sc);
        e.set_loop(Some((2, 9)));
        e.seek(a);
        e.play();
        run(&mut e, &mut rng, 5 * pass - 1000);
        let ons = music_ons(&e);
        // The loop's first beat sounds at the start of every pass.
        let starts: Vec<u64> = (0..5).map(|k| ons[k * 24]).collect();
        let want: Vec<u64> = (0..5).map(|k| k as u64 * pass).collect();
        assert_eq!(starts, want, "seed {seed}");
        // Every pass plays the same 24 beats at the same offsets.
        let per_pass = 24;
        assert_eq!(ons.len(), 5 * per_pass, "seed {seed}");
        for k in 1..5 {
            let base = k * per_pass;
            for j in 0..per_pass {
                assert_eq!(
                    ons[base + j] - ons[j],
                    k as u64 * pass,
                    "seed {seed} pass {k} beat {j}"
                );
            }
        }
        let jitter = 0; // exact equality asserted above
        println!(
            "PLAY-07 seed {seed}: wrap every {pass} frames, jitter {jitter} frames (<= 64-frame buffer)"
        );
    }
}

#[test]
fn nfr_02_position_is_rendered_frames_over_sample_rate() {
    let sc = score();
    // At most 50 x 20 000 frames (20.8 s) x 1.2 = 25 s of score, inside the
    // 25.8 s score, so the engine never stops at the end mid-test.
    assert!(sc.0.duration_sec > 25.0);
    for &scale in &[1.0f64, 0.75, 1.2] {
        let mut rng = Rng(scale.to_bits());
        let mut e = engine(&sc);
        e.set_tempo_scale(scale);
        e.play();
        let mut rendered = 0u64;
        for _ in 0..50 {
            let n = rng.next() % 20_000;
            run(&mut e, &mut rng, n);
            rendered += n;
            let want = rendered as f64 / SRF * scale;
            assert!(
                (e.position() - want).abs() < 1e-9,
                "{} vs {want}",
                e.position()
            );
            assert_eq!(e.synth().frame, rendered, "synth saw every frame");
        }
    }
}

#[test]
fn nfr_02_position_does_not_move_with_wall_time() {
    let sc = score();
    let mut e = engine(&sc);
    e.play();
    let mut rng = Rng(9);
    run(&mut e, &mut rng, 12_000);
    let p = e.position();
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert_eq!(e.position(), p, "no wall clock in the scheduler");
    assert_eq!(p, 12_000.0 / SRF);
}

#[test]
fn nfr_02_notes_scheduled_by_frame_not_by_block() {
    // The same score rendered with different block sizes produces the same
    // note-on frames.
    let sc = score();
    let tm = &sc.0;
    let mut reference = None;
    for seed in [1u64, 2, 3] {
        let mut rng = Rng(seed);
        let mut e = engine(&sc);
        e.play();
        run(&mut e, &mut rng, frame_at(tm.duration_sec) + 100);
        let ons = music_ons(&e);
        assert_eq!(ons.len(), 48);
        match &reference {
            None => reference = Some(ons),
            Some(r) => assert_eq!(&ons, r, "seed {seed}"),
        }
    }
}
