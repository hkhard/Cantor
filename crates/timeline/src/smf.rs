//! Standard MIDI File (format 1) export from the timemap.
//!
//! Track 0 is the conductor track (tempo, time signature); then one track per
//! part with its name, program and notes. Used for MIDI export and as a
//! reference rendering until the audio engine (M2) exists.

use crate::Timemap;
use cantor_score::{Score, PPQ};
use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// General MIDI program (0-based) for a part: `<midi-program>` if present,
/// else a guess from the part name (PLAY-09), else Choir Aahs.
pub fn program_for(name: &str, explicit: Option<u8>) -> u8 {
    if let Some(p) = explicit {
        return p.min(127);
    }
    let n = name.to_lowercase();
    let table: &[(&[&str], u8)] = &[
        (&["piano", "pno", "klaver", "flygel"], 0),
        (&["organ", "orgel"], 19),
        (&["guitar", "gitarr"], 24),
        (&["violin", "fiol"], 40),
        (&["viola"], 41),
        (&["cello"], 42),
        (&["contrabass", "double bass", "kontrabas"], 43),
        (&["flute", "flöjt"], 73),
        (&["clarinet", "klarinett"], 71),
        (&["trumpet", "trumpet"], 56),
        (&["horn"], 60),
    ];
    table
        .iter()
        .find(|(keys, _)| keys.iter().any(|k| n.contains(k)))
        .map_or(52, |(_, p)| *p)
}

/// Encode the score's timemap as a format 1 Standard MIDI File at [`PPQ`].
pub fn to_smf(score: &Score, tm: &Timemap) -> Vec<u8> {
    let mut tracks: Vec<Vec<TrackEvent<'_>>> = Vec::new();

    // Conductor track.
    let mut conductor: Vec<(u64, TrackEventKind<'_>)> = Vec::new();
    for &(tick, bpm) in &tm.tempos {
        let us = (60_000_000.0 / bpm).round().clamp(1.0, 16_777_215.0) as u32;
        conductor.push((tick, TrackEventKind::Meta(MetaMessage::Tempo(u24::new(us)))));
    }
    if let Some((num, den)) = score
        .parts
        .first()
        .and_then(|p| p.measures.first())
        .and_then(|m| m.time_signature)
    {
        if den.is_power_of_two() && num <= 255 {
            conductor.push((
                0,
                TrackEventKind::Meta(MetaMessage::TimeSignature(
                    num as u8,
                    den.trailing_zeros() as u8,
                    24,
                    8,
                )),
            ));
        }
    }
    tracks.push(to_delta(conductor, tm.duration_ticks));

    // Channels: explicit ones first, then the next free channel, skipping 9 (drums).
    let mut used: Vec<u8> = score.parts.iter().filter_map(|p| p.midi_channel).collect();
    let mut channels = Vec::new();
    for p in &score.parts {
        let ch = match p.midi_channel {
            Some(c) => c,
            None => {
                let c = (0u8..16)
                    .find(|c| *c != 9 && !used.contains(c))
                    .unwrap_or((channels.len() % 15) as u8);
                used.push(c);
                c
            }
        };
        channels.push(ch);
    }

    for (pi, p) in score.parts.iter().enumerate() {
        let ch = u4::new(channels[pi] & 0x0f);
        let mut evs: Vec<(u64, TrackEventKind<'_>)> = vec![
            (
                0,
                TrackEventKind::Meta(MetaMessage::TrackName(p.name.as_bytes())),
            ),
            (
                0,
                TrackEventKind::Midi {
                    channel: ch,
                    message: MidiMessage::ProgramChange {
                        program: u7::new(program_for(&p.name, p.midi_program)),
                    },
                },
            ),
        ];
        for n in tm.notes.iter().filter(|n| n.part == pi) {
            let key = u7::new(n.pitch);
            evs.push((
                n.tick_on,
                TrackEventKind::Midi {
                    channel: ch,
                    message: MidiMessage::NoteOn {
                        key,
                        vel: u7::new(n.velocity.clamp(1, 127)),
                    },
                },
            ));
            evs.push((
                n.tick_off,
                TrackEventKind::Midi {
                    channel: ch,
                    message: MidiMessage::NoteOff {
                        key,
                        vel: u7::new(0),
                    },
                },
            ));
        }
        tracks.push(to_delta(evs, tm.duration_ticks));
    }

    let smf = Smf {
        header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ as u16))),
        tracks,
    };
    let mut out = Vec::new();
    smf.write_std(&mut out)
        .expect("writing to a Vec cannot fail");
    out
}

/// Sort absolute-time events (note-offs before note-ons at the same tick so a
/// repeated pitch re-attacks) and convert to delta times, ending the track at `end`.
fn to_delta(mut evs: Vec<(u64, TrackEventKind<'_>)>, end: u64) -> Vec<TrackEvent<'_>> {
    let rank = |k: &TrackEventKind<'_>| match k {
        TrackEventKind::Meta(_) => 0,
        TrackEventKind::Midi {
            message: MidiMessage::ProgramChange { .. },
            ..
        } => 1,
        TrackEventKind::Midi {
            message: MidiMessage::NoteOff { .. },
            ..
        } => 2,
        _ => 3,
    };
    evs.sort_by_key(|(t, k)| (*t, rank(k)));
    let mut out = Vec::with_capacity(evs.len() + 1);
    let mut last = 0u64;
    for (t, kind) in evs {
        out.push(TrackEvent {
            delta: u28::new((t - last) as u32),
            kind,
        });
        last = t;
    }
    out.push(TrackEvent {
        delta: u28::new(end.saturating_sub(last) as u32),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    out
}
