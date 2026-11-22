//! Repeat unfolding, tempo map and timemap.
//!
//! The [`Timemap`] is the single source of time (PRD section 6): playback,
//! cursor, MIDI export and sung lyrics all read note times from it.
//! Repeats, voltas and D.C./D.S./Coda/Fine are unfolded first (PLAY-02).

pub mod channels;
pub mod smf;
mod unfold;

pub use unfold::unfold;

use cantor_score::{EventKind, Score, Tick};
use std::collections::HashMap;

/// Quarter notes per minute used when a score has no tempo marking
/// (common for OMR output, which rarely reads metronome marks).
pub const DEFAULT_TEMPO: f64 = 100.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TimedNote {
    /// Index into `Score::parts`.
    pub part: usize,
    pub staff: u32,
    pub voice: u32,
    /// Written measure index the note comes from.
    pub measure: usize,
    pub pitch: u8,
    pub tick_on: Tick,
    pub tick_off: Tick,
    pub sec_on: f64,
    pub sec_off: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timemap {
    /// Notes sorted by onset, then part, then pitch.
    pub notes: Vec<TimedNote>,
    /// Written measure index of each played measure (repeats unfolded).
    pub measure_order: Vec<usize>,
    /// Absolute start tick of each played measure, parallel to `measure_order`.
    pub measure_starts: Vec<Tick>,
    /// Tempo changes `(absolute tick, quarter notes per minute)`, first at tick 0.
    pub tempos: Vec<(Tick, f64)>,
    pub duration_ticks: Tick,
    pub duration_sec: f64,
}

impl Timemap {
    /// Convert an absolute tick to seconds using the tempo map.
    pub fn seconds_at(&self, tick: Tick) -> f64 {
        seconds_at(&self.tempos, tick)
    }
}

fn seconds_at(tempos: &[(Tick, f64)], tick: Tick) -> f64 {
    let ppq = cantor_score::PPQ as f64;
    let mut sec = 0.0;
    for (i, &(start, bpm)) in tempos.iter().enumerate() {
        if start >= tick {
            break;
        }
        let end = tempos.get(i + 1).map_or(tick, |n| n.0.min(tick));
        sec += (end - start) as f64 / ppq * 60.0 / bpm;
    }
    sec
}

/// Build the timemap for a score in playback order (repeats unfolded).
///
/// Measures are aligned by index across parts: each measure starts where the
/// longest part's previous measure ended. A part with a misread rhythm (common
/// in OMR output) therefore cannot shift the other parts.
pub fn build_timemap(score: &Score) -> Timemap {
    let n_measures = score
        .parts
        .iter()
        .map(|p| p.measures.len())
        .max()
        .unwrap_or(0);
    // Played length of each written measure.
    let lengths: Vec<Tick> = (0..n_measures)
        .map(|i| {
            let ms: Vec<_> = score
                .parts
                .iter()
                .filter_map(|p| p.measures.get(i))
                .collect();
            let nominal = ms.iter().map(|m| m.duration).max().unwrap_or(0);
            let content = ms.iter().map(|m| m.content_duration).max().unwrap_or(0);
            // Every part shorter than the time signature: a pickup, a written
            // partial measure, or a time change OMR missed. Play the content
            // (MuseScore does the same). Empty measures keep their full length.
            if content > 0 && content < nominal {
                content
            } else {
                nominal
            }
        })
        .collect();

    let measure_order = unfold(score);
    let mut measure_starts = Vec::with_capacity(measure_order.len());
    let mut t: Tick = 0;
    for &mi in &measure_order {
        measure_starts.push(t);
        t += lengths[mi];
    }
    let duration_ticks = t;

    // Tempo map: union of tempo marks from all parts, first one at a tick wins.
    let mut tempos: Vec<(Tick, f64)> = Vec::new();
    for p in &score.parts {
        for (visit, &mi) in measure_order.iter().enumerate() {
            if let Some(m) = p.measures.get(mi) {
                for &(off, bpm) in &m.tempos {
                    tempos.push((measure_starts[visit] + off, bpm));
                }
            }
        }
    }
    tempos.sort_by_key(|x| x.0);
    tempos.dedup_by_key(|x| x.0);
    if tempos.first().is_none_or(|x| x.0 != 0) {
        tempos.insert(0, (0, DEFAULT_TEMPO));
    }
    tempos.dedup_by(|b, a| a.1 == b.1);

    let mut notes: Vec<TimedNote> = Vec::new();
    for (pi, p) in score.parts.iter().enumerate() {
        // Open ties: pitch -> index of the sounding note.
        let mut open: HashMap<u8, usize> = HashMap::new();
        for (visit, &mi) in measure_order.iter().enumerate() {
            let Some(m) = p.measures.get(mi) else {
                continue;
            };
            for e in &m.events {
                let EventKind::Note(pitch) = &e.kind else {
                    continue;
                };
                if e.duration == 0 {
                    continue; // grace notes: no playback time yet
                }
                let Ok(midi) = u8::try_from(pitch.midi()) else {
                    continue;
                };
                if midi > 127 {
                    continue;
                }
                let on = measure_starts[visit] + e.offset;
                let off = on + e.duration;
                if e.tie_stop {
                    if let Some(&idx) = open.get(&midi) {
                        // Only continue a tie that ends where this note starts.
                        if notes[idx].tick_off == on {
                            notes[idx].tick_off = off;
                            if !e.tie_start {
                                open.remove(&midi);
                            }
                            continue;
                        }
                    }
                }
                notes.push(TimedNote {
                    part: pi,
                    staff: e.staff,
                    voice: e.voice,
                    measure: mi,
                    pitch: midi,
                    tick_on: on,
                    tick_off: off,
                    sec_on: 0.0,
                    sec_off: 0.0,
                });
                if e.tie_start {
                    open.insert(midi, notes.len() - 1);
                } else {
                    open.remove(&midi);
                }
            }
        }
    }
    for n in &mut notes {
        n.sec_on = seconds_at(&tempos, n.tick_on);
        n.sec_off = seconds_at(&tempos, n.tick_off);
    }
    notes.sort_by_key(|n| (n.tick_on, n.part, n.pitch));

    Timemap {
        duration_sec: seconds_at(&tempos, duration_ticks),
        notes,
        measure_starts,
        measure_order,
        tempos,
        duration_ticks,
    }
}
