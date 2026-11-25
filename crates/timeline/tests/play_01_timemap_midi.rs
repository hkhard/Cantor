//! PLAY-01 (event timeline) and Standard MIDI File export from the score model.

use cantor_score::parse_musicxml;
use cantor_timeline::{build_timemap, smf::to_smf, DEFAULT_TEMPO};
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

const PPQ: u64 = cantor_score::PPQ;

const SCORE: &str = r#"<?xml version="1.0"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="T"><part-name>Tenor</part-name>
      <midi-instrument id="T-I1"><midi-channel>1</midi-channel><midi-program>53</midi-program></midi-instrument>
    </score-part>
    <score-part id="B"><part-name>Bass</part-name></score-part>
  </part-list>
  <part id="T">
    <measure number="1">
      <attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
      <direction><sound tempo="120"/></direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="start"/></note>
    </measure>
    <measure number="2">
      <direction><sound tempo="60"/></direction>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>
      <note><rest/><duration>1</duration></note>
    </measure>
  </part>
  <part id="B">
    <measure number="1">
      <attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>2</duration></note>
      <note><chord/><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration></note>
    </measure>
    <measure number="2">
      <note><pitch><step>F</step><octave>2</octave></pitch><duration>2</duration></note>
    </measure>
  </part>
</score-partwise>"#;

#[test]
fn play_01_timemap_ties_merge_and_chords_share_onset() {
    let tm = build_timemap(&parse_musicxml(SCORE.as_bytes()).unwrap());
    let tenor: Vec<_> = tm
        .notes
        .iter()
        .filter(|n| n.part == 0)
        .map(|n| (n.pitch, n.tick_on, n.tick_off))
        .collect();
    assert_eq!(
        tenor,
        vec![(60, 0, PPQ), (62, PPQ, 3 * PPQ)],
        "tied D sounds once for two beats"
    );
    let bass: Vec<_> = tm
        .notes
        .iter()
        .filter(|n| n.part == 1)
        .map(|n| (n.pitch, n.tick_on))
        .collect();
    assert_eq!(bass, vec![(48, 0), (55, 0), (41, 2 * PPQ)]);
}

#[test]
fn play_01_seconds_follow_tempo_changes() {
    let tm = build_timemap(&parse_musicxml(SCORE.as_bytes()).unwrap());
    // 120 bpm for measure 1 (2 beats = 1.0 s), then 60 bpm.
    let d = tm.notes.iter().find(|n| n.pitch == 62).unwrap();
    assert!((d.sec_on - 0.5).abs() < 1e-9);
    assert!(
        (d.sec_off - 2.0).abs() < 1e-9,
        "0.5 s at 120 + 1 beat at 60 = 2.0, got {}",
        d.sec_off
    );
    assert!((tm.duration_sec - 3.0).abs() < 1e-9);
    assert_eq!(tm.tempos, vec![(0, 120.0), (2 * PPQ, 60.0)]);
}

#[test]
fn play_01_default_tempo_when_score_has_none() {
    let xml = SCORE
        .replace(r#"<sound tempo="120"/>"#, "")
        .replace(r#"<sound tempo="60"/>"#, "");
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.tempos, vec![(0, DEFAULT_TEMPO)]);
}

/// OMR output can give parts different measure lengths (a misread rhythm).
/// Measures are aligned by index, each starting after the longest version of
/// the previous measure, so one bad part cannot shift the others.
#[test]
fn play_01_parts_realigned_at_each_barline() {
    let xml = SCORE.replace(
        r#"<note><pitch><step>F</step><octave>2</octave></pitch><duration>2</duration></note>"#,
        r#"<note><pitch><step>F</step><octave>2</octave></pitch><duration>2</duration></note>
           </measure><measure number="3"><note><pitch><step>E</step><octave>2</octave></pitch><duration>1</duration></note>"#,
    );
    // Make bass measure 1 overrun by one beat.
    let xml = xml.replacen(
        r#"<note><chord/><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration></note>"#,
        r#"<note><chord/><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration></note>
           <note><rest/><duration>1</duration></note>"#,
        1,
    );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.measure_starts, vec![0, 3 * PPQ, 5 * PPQ]);
    let tenor_d = tm
        .notes
        .iter()
        .find(|n| n.part == 0 && n.pitch == 62)
        .unwrap();
    assert_eq!(tenor_d.tick_on, PPQ);
    let e2 = tm.notes.iter().find(|n| n.pitch == 40).unwrap();
    assert_eq!(e2.tick_on, 5 * PPQ);
}

/// OMR rarely marks pickups `implicit="yes"`. A measure whose content is
/// shorter than the time signature in every part is played at its content
/// length (what MuseScore does), so the downbeat lands where it is written.
#[test]
fn play_01_unmarked_pickup_uses_content_length() {
    let xml = SCORE.replace(
        r#"<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="start"/></note>"#,
        r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="start"/></note>"#,
    )
    .replacen(
        r#"<note><pitch><step>C</step><octave>3</octave></pitch><duration>2</duration></note>
      <note><chord/><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration></note>"#,
        r#"<note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration></note>"#,
        1,
    );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.measure_starts, vec![0, PPQ], "1-beat pickup in 2/4");
    let f2 = tm.notes.iter().find(|n| n.pitch == 41).unwrap();
    assert_eq!(f2.tick_on, PPQ);
}

/// Same rule mid-piece: when every part of a measure is short (a written
/// partial measure, or a time change OMR missed), play the longest content.
/// A measure short in only one part keeps the full length (missing rest).
#[test]
fn play_01_short_measure_in_all_parts_is_shortened() {
    // Measure 2: tenor 1 beat (D tie stop only), bass 1 beat.
    let xml = SCORE
        .replace(
            r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>
      <note><rest/><duration>1</duration></note>"#,
            r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>"#,
        )
        .replace(
            r#"<note><pitch><step>F</step><octave>2</octave></pitch><duration>2</duration></note>"#,
            r#"<note><pitch><step>F</step><octave>2</octave></pitch><duration>1</duration></note>"#,
        );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.duration_ticks, 3 * PPQ);

    // Only the tenor short: bass still fills 2 beats, so the measure is 2 beats.
    let xml = SCORE.replace(
        r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>
      <note><rest/><duration>1</duration></note>"#,
        r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>"#,
    );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.duration_ticks, 4 * PPQ);
}

/// An empty measure (no notes, no rests) still lasts its time signature.
#[test]
fn play_01_empty_measure_keeps_nominal_length() {
    let xml = SCORE.replace(
        r#"<note><pitch><step>F</step><octave>2</octave></pitch><duration>2</duration></note>"#,
        "",
    )
    .replace(
        r#"<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><tie type="stop"/></note>
      <note><rest/><duration>1</duration></note>"#,
        "",
    );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    assert_eq!(tm.duration_ticks, 4 * PPQ);
}

#[test]
fn play_09_smf_has_track_per_part_programs_and_notes() {
    let score = parse_musicxml(SCORE.as_bytes()).unwrap();
    let bytes = to_smf(&score, &build_timemap(&score));
    let smf = Smf::parse(&bytes).expect("valid SMF");
    assert_eq!(smf.tracks.len(), 3, "conductor + one per part");

    let tempos: Vec<u32> = smf.tracks[0]
        .iter()
        .filter_map(|e| match e.kind {
            TrackEventKind::Meta(MetaMessage::Tempo(t)) => Some(t.as_int()),
            _ => None,
        })
        .collect();
    assert_eq!(tempos, vec![500_000, 1_000_000]);

    let notes_and_programs = |t: usize| {
        let mut abs = 0u64;
        let mut ons = Vec::new();
        let mut prog = None;
        let mut chan = None;
        for e in &smf.tracks[t] {
            abs += e.delta.as_int() as u64;
            if let TrackEventKind::Midi { channel, message } = e.kind {
                chan = Some(channel.as_int());
                match message {
                    MidiMessage::NoteOn { key, vel } if vel > 0 => ons.push((abs, key.as_int())),
                    MidiMessage::ProgramChange { program } => prog = Some(program.as_int()),
                    _ => {}
                }
            }
        }
        (ons, prog, chan)
    };
    let (ons, prog, chan) = notes_and_programs(1);
    assert_eq!(ons, vec![(0, 60), (PPQ, 62)]);
    assert_eq!(prog, Some(52));
    assert_eq!(chan, Some(0));
    let (ons, prog, chan) = notes_and_programs(2);
    assert_eq!(ons.len(), 3);
    assert_eq!(prog, Some(52), "unnamed part defaults to choir aahs");
    assert_eq!(
        chan,
        Some(1),
        "parts without a channel get the next free one"
    );
}

#[test]
fn play_09_smf_note_offs_balance_note_ons() {
    let score = parse_musicxml(SCORE.as_bytes()).unwrap();
    let bytes = to_smf(&score, &build_timemap(&score));
    let smf = Smf::parse(&bytes).unwrap();
    for t in &smf.tracks[1..] {
        let (mut on, mut off) = (0, 0);
        for e in t {
            if let TrackEventKind::Midi { message, .. } = e.kind {
                match message {
                    MidiMessage::NoteOn { vel, .. } if vel > 0 => on += 1,
                    MidiMessage::NoteOff { .. } | MidiMessage::NoteOn { .. } => off += 1,
                    _ => {}
                }
            }
        }
        assert_eq!(on, off);
    }
}

#[test]
fn play_01_same_pitch_tied_in_two_voices_sounds_once_per_voice() {
    // The Anchor Song bar 1-2: voices 1 and 2 of one part both hold E4 tied
    // over the bar line. Each voice's tie continues its own note; nothing is
    // struck again in bar 2. Was: the second voice's E4 was struck again.
    let note = |voice: u32, tie: &str| {
        format!(
            r#"<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration>{tie}<voice>{voice}</voice></note>"#
        )
    };
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
        <part id="P1">
          <measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
            {}<backup><duration>4</duration></backup>{}</measure>
          <measure number="2">{}<backup><duration>4</duration></backup>{}</measure>
        </part></score-partwise>"#,
        note(1, r#"<tie type="start"/>"#),
        note(2, r#"<tie type="start"/>"#),
        note(1, r#"<tie type="stop"/>"#),
        note(2, r#"<tie type="stop"/>"#),
    );
    let tm = build_timemap(&parse_musicxml(xml.as_bytes()).unwrap());
    let es: Vec<_> = tm
        .notes
        .iter()
        .map(|n| (n.voice, n.tick_on, n.tick_off))
        .collect();
    assert_eq!(es, vec![(1, 0, 8 * PPQ), (2, 0, 8 * PPQ)]);
}

/// One staff of 4/4 with `divisions` 1 from measure bodies.
fn one_voice(measures: &[&str]) -> cantor_score::Score {
    let body: String = measures
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let attrs = if i == 0 {
                "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>"
            } else {
                ""
            };
            format!(r#"<measure number="{}">{attrs}{m}</measure>"#, i + 1)
        })
        .collect();
    let xml = format!(
        r#"<?xml version="1.0"?><score-partwise version="4.0">
        <part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list>
        <part id="P1">{body}</part></score-partwise>"#
    );
    parse_musicxml(xml.as_bytes()).unwrap()
}

fn g4(dur: u32, extra: &str) -> String {
    format!(
        r#"<note><pitch><step>G</step><octave>4</octave></pitch><duration>{dur}</duration>{extra}</note>"#
    )
}

#[test]
fn play_01_tie_into_a_skipped_volta_is_held_its_written_length() {
    // Only You bars 19-21 on the last pass: bar 19 ends with G4 tied into
    // bar 20 (volta 1-3), but the music goes on to bar 21 (volta 4). MuseScore
    // holds the G4 for the written tied length anyway (1/2 + 1 beat).
    let s = one_voice(&[
        &format!(
            r#"<note><rest/><duration>3</duration></note>{}"#,
            g4(1, r#"<tie type="start"/>"#)
        ),
        &format!(
            r#"<barline location="left"><ending number="1" type="start"/></barline>{}<note><rest/><duration>2</duration></note><barline location="right"><ending number="1" type="stop"/><repeat direction="backward"/></barline>"#,
            g4(2, r#"<tie type="stop"/>"#)
        ),
        r#"<barline location="left"><ending number="2" type="start"/></barline><note><rest/><duration>4</duration></note>"#,
    ]);
    let tm = build_timemap(&s);
    assert_eq!(tm.measure_order, vec![0, 1, 0, 2]);
    let g: Vec<_> = tm.notes.iter().map(|n| (n.tick_on, n.tick_off)).collect();
    // Pass 1: tied into bar 2 (3 beats total ending at beat 2 of bar 2).
    // Pass 2: bar 2 is skipped, still held 3 beats into bar 3.
    assert_eq!(g, vec![(3 * PPQ, 6 * PPQ), (11 * PPQ, 14 * PPQ)], "{g:?}");
}
