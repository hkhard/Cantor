//! Lyric extraction (VOX-01): one part and verse become phrases of sung notes,
//! timed with the unfolded timemap so sung tracks line up with playback.
//!
//! Output is engine-neutral: each sung note is a new syllable, a melisma
//! continuation of the previous syllable, or a note without lyrics. Engine
//! adapters (SoulX-Singer, OmniSinger, ...) serialise phrases into their format.

use cantor_score::{EventKind, Score, Syllabic, Tick, PPQ};
use cantor_timeline::Timemap;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// Starts a new syllable.
    Syllable,
    /// Continues the previous syllable on a new note (slur / extender).
    Melisma,
    /// Sung note with no lyric in this verse (vocalise, missing OCR).
    NoLyric,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SungNote {
    pub kind: NoteKind,
    /// Syllable text; for `Melisma` the syllable being held.
    pub text: String,
    pub syllabic: Syllabic,
    /// The whole word, set on the syllable that completes it.
    pub word: Option<String>,
    /// MIDI pitch.
    pub pitch: u8,
    pub start_tick: Tick,
    pub end_tick: Tick,
    pub start_sec: f64,
    pub duration_sec: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phrase {
    pub notes: Vec<SungNote>,
    pub start_sec: f64,
    pub end_sec: f64,
}

impl Phrase {
    /// Words completed in this phrase, in order.
    pub fn words(&self) -> Vec<String> {
        self.notes.iter().filter_map(|n| n.word.clone()).collect()
    }
}

/// Minimum silence between notes that starts a new phrase.
pub const PHRASE_BREAK: Tick = PPQ;

/// Candidate note at one onset before choosing a line.
struct Cand {
    pitch: u8,
    dur: Tick,
    tie_stop: bool,
    lyric: Option<(String, Syllabic, bool)>,
}

/// Extract the sung line of `part` (index into `score.parts`).
///
/// `voice`: `Some(v)` sings MusicXML voice `v` only; `None` sings the top line
/// (highest note at each onset). `verse` is the 1-based verse for the first
/// pass; on the n-th pass through a repeated measure verse `verse + n - 1` is
/// used when the note has it.
pub fn extract(
    score: &Score,
    tm: &Timemap,
    part: usize,
    voice: Option<u32>,
    verse: u32,
) -> Vec<Phrase> {
    let Some(p) = score.parts.get(part) else {
        return Vec::new();
    };

    // 1. Collect candidate notes per absolute onset, following the unfolded order.
    let mut by_onset: BTreeMap<Tick, Vec<Cand>> = BTreeMap::new();
    let mut passes: BTreeMap<usize, u32> = BTreeMap::new();
    for (visit, &mi) in tm.measure_order.iter().enumerate() {
        let pass = {
            let c = passes.entry(mi).or_insert(0);
            *c += 1;
            *c
        };
        let Some(m) = p.measures.get(mi) else {
            continue;
        };
        for e in &m.events {
            if voice.is_some_and(|v| v != e.voice) || e.duration == 0 {
                continue;
            }
            let EventKind::Note(pitch) = &e.kind else {
                continue;
            };
            let Ok(pitch) = u8::try_from(pitch.midi()) else {
                continue;
            };
            let wanted = verse + pass - 1;
            let lyric = e
                .lyrics
                .iter()
                .find(|l| l.verse == wanted)
                .or_else(|| e.lyrics.iter().find(|l| l.verse == verse))
                .map(|l| (l.text.clone(), l.syllabic, l.extend));
            by_onset
                .entry(tm.measure_starts[visit] + e.offset)
                .or_default()
                .push(Cand {
                    pitch,
                    dur: e.duration,
                    tie_stop: e.tie_stop,
                    lyric,
                });
        }
    }

    // 2. One note per onset: the highest; its lyric, else any lyric at that onset.
    let mut notes: Vec<SungNote> = Vec::new();
    let mut held: Option<String> = None; // syllable available for melismas
    let mut word = String::new();
    for (onset, cands) in by_onset {
        let top = cands.iter().max_by_key(|c| c.pitch).expect("non-empty");
        let lyric = top
            .lyric
            .clone()
            .or_else(|| cands.iter().find_map(|c| c.lyric.clone()));
        let end = onset + top.dur;

        // Tie continuation: extend the previous note, no new attack.
        if top.tie_stop {
            if let Some(prev) = notes.last_mut() {
                if prev.pitch == top.pitch && prev.end_tick == onset {
                    prev.end_tick = end;
                    continue;
                }
            }
        }

        let (kind, text, syllabic, completed) = match lyric {
            Some((text, syl, _)) if !text.is_empty() => {
                word.push_str(&text);
                let done = (!syl.continues_word()).then(|| std::mem::take(&mut word));
                held = Some(text.clone());
                (NoteKind::Syllable, text, syl, done)
            }
            // Extender-only lyric or no lyric after a syllable: melisma.
            _ => match &held {
                Some(h) => (NoteKind::Melisma, h.clone(), Syllabic::Single, None),
                None => (NoteKind::NoLyric, String::new(), Syllabic::Single, None),
            },
        };
        notes.push(SungNote {
            kind,
            text,
            syllabic,
            word: completed,
            pitch: top.pitch,
            start_tick: onset,
            end_tick: end,
            start_sec: 0.0,
            duration_sec: 0.0,
        });
    }

    // 3. Seconds from the tempo map, then split into phrases at silences.
    for n in &mut notes {
        n.start_sec = tm.seconds_at(n.start_tick);
        n.duration_sec = tm.seconds_at(n.end_tick) - n.start_sec;
    }
    let mut phrases: Vec<Phrase> = Vec::new();
    let mut cur: Vec<SungNote> = Vec::new();
    for n in notes {
        if let Some(last) = cur.last() {
            if n.start_tick >= last.end_tick + PHRASE_BREAK {
                phrases.push(finish(std::mem::take(&mut cur)));
            }
        }
        cur.push(n);
    }
    if !cur.is_empty() {
        phrases.push(finish(cur));
    }
    phrases
}

fn finish(notes: Vec<SungNote>) -> Phrase {
    let start_sec = notes.first().map_or(0.0, |n| n.start_sec);
    let end_sec = notes.last().map_or(0.0, |n| n.start_sec + n.duration_sec);
    Phrase {
        notes,
        start_sec,
        end_sec,
    }
}
