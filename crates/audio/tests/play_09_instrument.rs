//! PLAY-09: the instrument of every voice can be changed while the score is
//! open, also during playback; notes already sounding finish on the old
//! instrument and the next ones use the new one.

use cantor_audio::{Engine, Synth};
use cantor_score::parse_musicxml;
use cantor_timeline::build_timemap;
use cantor_timeline::channels::plan_channels;

#[derive(Default)]
struct Log(Vec<(u8, u8, u8)>);
impl Synth for Log {
    fn message(&mut self, channel: u8, status: u8, d1: u8, _d2: u8) {
        self.0.push((status, channel, d1));
    }
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        l.fill(0.0);
        r.fill(0.0);
    }
}

#[test]
fn play_09_change_instrument_of_a_voice() {
    let x = r#"<score-partwise version="4.0"><part-list>
      <score-part id="P1"><part-name>Soprano</part-name></score-part>
      <score-part id="P2"><part-name>Bass</part-name></score-part></part-list>
      <part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
      <note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration></note></measure></part>
      <part id="P2"><measure number="1"><attributes><divisions>1</divisions></attributes>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration></note></measure></part></score-partwise>"#;
    let s = parse_musicxml(x.as_bytes()).unwrap();
    let tm = build_timemap(&s);
    let plan = plan_channels(&s, &tm);
    let mut e = Engine::new(Log::default(), &tm, &plan, 48_000);
    let before = e.program(1).unwrap();
    // Bass -> Church organ (GM 19, 0-based).
    assert!(e.set_program(1, 19));
    assert_eq!(e.program(1), Some(19));
    assert!(
        e.synth().0.contains(&(0xC0, 1, 19)),
        "program change sent: {:?}",
        e.synth().0
    );
    assert_ne!(before, 19);
    // Out of range: refused, nothing sent.
    let n = e.synth().0.len();
    assert!(!e.set_program(9, 0));
    assert!(!e.set_program(0, 128));
    assert_eq!(e.synth().0.len(), n);
    // Survives seeking (channels re-initialised on seek keep the choice).
    e.seek(0.0);
    assert_eq!(e.program(1), Some(19));
}
