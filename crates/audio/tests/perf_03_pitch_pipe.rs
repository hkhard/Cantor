//! PERF-03: pitch pipe. Each sung line's first note (or all of them as a
//! chord) sounds on demand, also while paused, without moving the transport.

use cantor_audio::{Engine, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;

#[derive(Default)]
struct Recorder {
    frame: u64,
    log: Vec<(u64, u8, u8, u8)>,
}

impl Synth for Recorder {
    fn message(&mut self, channel: u8, status: u8, d1: u8, _d2: u8) {
        if status == 0x90 || status == 0x80 {
            self.log.push((self.frame, status, channel, d1));
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        self.frame += left.len() as u64;
    }
}

const SR: u32 = 48_000;

/// Two parts: S starts on E5 after a rest, B starts on C3.
fn engine() -> Engine<Recorder> {
    let xml = r#"<?xml version="1.0"?><score-partwise version="4.0">
      <part-list><score-part id="P1"><part-name>Soprano</part-name></score-part>
      <score-part id="P2"><part-name>Bass</part-name></score-part></part-list>
      <part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
        <note><rest/><duration>1</duration></note>
        <note><pitch><step>E</step><octave>5</octave></pitch><duration>3</duration></note></measure></part>
      <part id="P2"><measure number="1"><attributes><divisions>1</divisions></attributes>
        <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration></note></measure></part>
    </score-partwise>"#;
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

#[test]
fn perf_03_starting_pitches_per_channel() {
    let e = engine();
    // From the start: S's first sounding note is E5 (76), B's is C3 (48).
    assert_eq!(e.starting_pitches(0.0), vec![Some(76), Some(48)]);
    // From a later position: the note sounding or next to come.
    assert_eq!(e.starting_pitches(1.9), vec![Some(76), Some(48)]);
}

#[test]
fn perf_03_pipe_sounds_while_paused_and_stops() {
    let mut e = engine();
    assert!(!e.is_playing());
    e.pitch_pipe(None, 1.0);
    run(&mut e, 1.5);
    let log = &e.synth().log;
    let ons: Vec<u8> = log.iter().filter(|m| m.1 == 0x90).map(|m| m.3).collect();
    assert_eq!(ons.len(), 2, "{log:?}");
    assert!(ons.contains(&76) && ons.contains(&48));
    // Released after one second, the transport did not move.
    let offs: Vec<&(u64, u8, u8, u8)> = log
        .iter()
        .filter(|m| m.1 == 0x80 && m.0 > 0 && (m.3 == 76 || m.3 == 48))
        .collect();
    assert_eq!(offs.len(), 2);
    assert!(
        offs.iter().all(|m| (m.0 as i64 - SR as i64).abs() <= 256),
        "{offs:?}"
    );
    assert_eq!(e.position(), 0.0);
}

#[test]
fn perf_03_pipe_single_channel() {
    let mut e = engine();
    e.pitch_pipe(Some(1), 0.5);
    run(&mut e, 1.0);
    let ons: Vec<u8> = e
        .synth()
        .log
        .iter()
        .filter(|m| m.1 == 0x90)
        .map(|m| m.3)
        .collect();
    assert_eq!(ons, vec![48]);
}
