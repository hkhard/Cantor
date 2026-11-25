//! Repeat unfolding, tempo map and timemap.
//!
//! The [`Timemap`] is the single source of time (PRD section 6): playback,
//! cursor, MIDI export and sung lyrics all read note times from it.
//! Repeats, voltas and D.C./D.S./Coda/Fine are unfolded first (PLAY-02).

pub mod channels;
pub mod smf;
mod unfold;

pub use unfold::unfold;

use cantor_score::{EventKind, Measure, Score, Swing, Tick};
use std::collections::{BTreeMap, HashMap};

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
    /// MIDI velocity from dynamics and accents (PLAY-10).
    pub velocity: u8,
    /// Position in its chord (same part, staff, voice and onset), 0 = top
    /// note (PLAY-05: two singers written as chords on one staff).
    pub chord_index: u8,
    /// Notes in that chord (1 for a single note).
    pub chord_size: u8,
}

/// Velocity when a score has no dynamics (mezzo-forte).
pub const DEFAULT_VELOCITY: u8 = 78;

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
    /// Beat grid per played measure (parallel to `measure_order`): the beat
    /// unit in ticks from the time signature (a quarter in 4/4, a dotted
    /// quarter in 6/8) and the number of beats. Drives the metronome click
    /// and the count-in (PLAY-06).
    pub beats: Vec<(Tick, u32)>,
}

impl Timemap {
    /// Seconds of every beat in the played order, with `true` on downbeats.
    pub fn beat_times(&self) -> Vec<(f64, bool)> {
        let mut out = Vec::new();
        for (visit, &(unit, count)) in self.beats.iter().enumerate() {
            let start = self.measure_starts[visit];
            let end = self
                .measure_starts
                .get(visit + 1)
                .copied()
                .unwrap_or(self.duration_ticks);
            for b in 0..count {
                let t = start + unit * b as Tick;
                if t >= end && b > 0 {
                    break; // short (pickup) measure
                }
                out.push((self.seconds_at(t), b == 0));
            }
        }
        out
    }
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

    // Swing in effect at each played measure's start, and changes inside it.
    // Swing marks apply to all parts (a "Swing" over the top staff swings
    // the whole band), carried across barlines until "Straight".
    let swing_at = swing_map(score, &measure_order);

    let mut notes: Vec<TimedNote> = Vec::new();
    for (pi, p) in score.parts.iter().enumerate() {
        // Open ties: pitch -> index of the sounding note.
        // Open ties by (staff, voice, pitch): two voices can hold the same
        // pitch tied at once (each continues its own note).
        let mut open: HashMap<(u32, u32, u8), usize> = HashMap::new();
        // Slurs open per (staff, voice): (depth, onset of the pending change,
        // its change). Changes apply after every note of a chord has been
        // seen, so a slur mark on one chord note covers the whole chord.
        let mut slur: HashMap<(u32, u32), (i32, Tick, i32)> = HashMap::new();
        // Last note per (staff, voice, pitch), to hold on through a slur.
        let mut last: HashMap<(u32, u32, u8), usize> = HashMap::new();
        for (visit, &mi) in measure_order.iter().enumerate() {
            let Some(m) = p.measures.get(mi) else {
                continue;
            };
            // A slur does not carry over a jump (repeat, volta, D.S.).
            if visit > 0 && mi != measure_order[visit - 1] + 1 {
                slur.clear();
            }
            // A tie into a measure the music skips (a volta not taken this
            // pass) still sounds its written length, as MuseScore plays it.
            if let Some(&prev) = visit.checked_sub(1).and_then(|v| measure_order.get(v)) {
                if mi != prev + 1 {
                    if let Some(written_next) = p.measures.get(prev + 1) {
                        let start = measure_starts[visit];
                        let tie_in = |meas: &Measure, staff: u32, voice: u32, midi: u8| {
                            meas.events
                                .iter()
                                .find(|e| {
                                    e.tie_stop
                                        && e.offset == 0
                                        && e.staff == staff
                                        && e.voice == voice
                                        && e.kind.midi().map(|x| x + meas.transpose)
                                            == Some(midi as i32)
                                })
                                .map(|e| e.duration)
                        };
                        open.retain(|&(staff, voice, midi), &mut idx| {
                            // Continued by the measure played next: the normal path.
                            if notes[idx].tick_off != start
                                || tie_in(m, staff, voice, midi).is_some()
                            {
                                return true;
                            }
                            match tie_in(written_next, staff, voice, midi) {
                                Some(d) => {
                                    notes[idx].tick_off += d;
                                    false
                                }
                                None => true,
                            }
                        });
                    }
                }
            }
            for e in &m.events {
                let EventKind::Note(pitch) = &e.kind else {
                    // A rest ends a slur left open (OMR output often has one).
                    slur.remove(&(e.staff, e.voice));
                    continue;
                };
                if e.duration == 0 {
                    continue; // grace notes: no playback time yet
                }
                // Transposing instruments sound at written + <transpose>.
                let Ok(midi) = u8::try_from(pitch.midi() + m.transpose) else {
                    continue;
                };
                if midi > 127 {
                    continue;
                }
                let sw = &swing_at[visit];
                let on = measure_starts[visit] + swung(sw, e.offset);
                let written_off = measure_starts[visit] + swung(sw, e.offset + e.duration);
                // Staccato and friends shorten the sound, not the time slot.
                let pct = e.articulation.length_pct.clamp(1, 100) as Tick;
                let off = on + ((written_off - on) * pct / 100).max(1);
                let velocity = e
                    .dynamic
                    .unwrap_or(DEFAULT_VELOCITY)
                    .saturating_add(e.articulation.accent)
                    .min(127);
                // Slur depth for this staff and voice (see `slur`).
                let st = slur.entry((e.staff, e.voice)).or_insert((0, on, 0));
                if st.1 != on {
                    st.0 = (st.0 + st.2).max(0);
                    *st = (st.0, on, 0);
                }
                let st_under = st.0 > 0;
                st.2 += i32::from(e.slur_start) - i32::from(e.slur_stop);
                if e.tie_stop {
                    // This voice's own tie first; else one of the same pitch
                    // from another voice (a tie that moves between voices).
                    let key = (e.staff, e.voice, midi);
                    let found = Some(key).filter(|k| open.contains_key(k)).or_else(|| {
                        open.keys()
                            .filter(|k| k.2 == midi && notes[open[k]].tick_off == on)
                            .min()
                            .copied()
                    });
                    if let Some(k) = found {
                        let idx = open[&k];
                        // Only continue a tie that ends where this note starts.
                        if notes[idx].tick_off == on {
                            notes[idx].tick_off = off;
                            open.remove(&k);
                            if e.tie_start {
                                open.insert(key, idx);
                            }
                            continue;
                        }
                    }
                }
                let key = (e.staff, e.voice, midi);
                // Product owner, 2026-11-25: an arc between two notes of the
                // same pitch is one note, tie or slur alike. Under a slur, a
                // note that follows the same pitch without a gap holds it on.
                let under_slur = st_under;
                if under_slur && !e.tie_stop {
                    if let Some(&idx) = last.get(&key) {
                        if notes[idx].tick_off == on && notes[idx].tick_on < on {
                            notes[idx].tick_off = off;
                            if e.tie_start {
                                open.insert(key, idx);
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
                    velocity,
                    chord_index: 0,
                    chord_size: 1,
                });
                let key = (e.staff, e.voice, midi);
                last.insert(key, notes.len() - 1);
                if e.tie_start {
                    open.insert(key, notes.len() - 1);
                } else {
                    open.remove(&key);
                }
            }
        }
    }
    for n in &mut notes {
        n.sec_on = seconds_at(&tempos, n.tick_on);
        n.sec_off = seconds_at(&tempos, n.tick_off);
    }
    notes.sort_by_key(|n| (n.tick_on, n.part, n.pitch));
    // Chord position, top note first, per (onset, part, staff, voice).
    let mut chords: BTreeMap<(Tick, usize, u32, u32), Vec<usize>> = BTreeMap::new();
    for (i, n) in notes.iter().enumerate() {
        chords
            .entry((n.tick_on, n.part, n.staff, n.voice))
            .or_default()
            .push(i);
    }
    for ix in chords.values() {
        let size = ix.len().min(255) as u8;
        // Sorted by ascending pitch within the group.
        for (k, &i) in ix.iter().rev().enumerate() {
            notes[i].chord_index = k.min(255) as u8;
            notes[i].chord_size = size;
        }
    }

    let beats = measure_order
        .iter()
        .map(|&mi| {
            let ts = score
                .parts
                .iter()
                .find_map(|p| p.measures.get(mi).and_then(|m| m.time_signature))
                .unwrap_or((4, 4));
            beat_grid(ts)
        })
        .collect();

    Timemap {
        beats,
        duration_sec: seconds_at(&tempos, duration_ticks),
        notes,
        measure_starts,
        measure_order,
        tempos,
        duration_ticks,
    }
}

/// Swing changes inside one played measure: `(offset, swing)`, the first at 0.
type MeasureSwing = Vec<(Tick, Option<Swing>)>;

fn swing_map(score: &Score, order: &[usize]) -> Vec<MeasureSwing> {
    let mut current: Option<Swing> = None;
    let mut out = Vec::with_capacity(order.len());
    for &mi in order {
        let mut changes: Vec<(Tick, Option<Swing>)> = score
            .parts
            .iter()
            .filter_map(|p| p.measures.get(mi))
            .flat_map(|m| m.swing.iter().copied())
            .collect();
        changes.sort_by_key(|c| c.0);
        changes.dedup_by_key(|c| c.0);
        let mut m = vec![(0, current)];
        for (off, sw) in changes {
            if off == 0 {
                m[0].1 = sw;
            } else {
                m.push((off, sw));
            }
            current = sw;
        }
        out.push(m);
    }
    out
}

/// Played position of a written offset in a measure under swing (PLAY-10).
/// Within each pair of swing units starting on a pair boundary, the first
/// unit is stretched to `first / (first + second)` of the pair and the rest
/// compressed, linearly, so ends of notes move with their starts and every
/// pair boundary (beat) stays where it is written.
fn swung(changes: &[(Tick, Option<Swing>)], offset: Tick) -> Tick {
    let sw = changes
        .iter()
        .rev()
        .find(|c| c.0 <= offset)
        .and_then(|c| c.1);
    let Some(sw) = sw else {
        return offset;
    };
    let pair = sw.unit * 2;
    if pair == 0 || sw.first + sw.second == 0 {
        return offset;
    }
    let base = offset - offset % pair;
    let x = offset % pair;
    let long = pair * sw.first as Tick / (sw.first + sw.second) as Tick;
    let y = if x <= sw.unit {
        x * long / sw.unit
    } else {
        long + (x - sw.unit) * (pair - long) / (pair - sw.unit)
    };
    base + y
}

/// Beat unit (ticks) and beats per measure for a time signature: compound
/// meters (6/8, 9/8, 12/8) count dotted beats, others count the denominator.
fn beat_grid((num, den): (u32, u32)) -> (Tick, u32) {
    let den = den.max(1) as Tick;
    let note = cantor_score::PPQ * 4 / den;
    if den >= 8 && num > 3 && num % 3 == 0 {
        (note * 3, num / 3)
    } else {
        (note, num.max(1))
    }
}
