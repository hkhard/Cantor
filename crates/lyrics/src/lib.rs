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
    extract_with(score, tm, part, voice, verse, false)
}

/// Like [`extract`] for verse 1, but a note without a verse-1 lyric takes any
/// lyric it has. For through-composed songs whose OCR spread one text over
/// several lyric lines (OCR numbers lines by height, not by verse).
pub fn extract_merged(score: &Score, tm: &Timemap, part: usize, voice: Option<u32>) -> Vec<Phrase> {
    extract_with(score, tm, part, voice, 1, true)
}

/// Dynamics and expression marks OCR reads into lyric lines.
const DYNAMICS: &[&str] = &[
    "p", "pp", "ppp", "mp", "mf", "f", "ff", "fff", "sf", "sfz", "fz", "fp", "rf", "rfz", "cresc",
    "dim", "rit", "rall", "accel",
];

/// Whether OCR'd lyric text is something a singer sings. Rejects dynamics,
/// URLs, publisher credits (one long capitalised word with trailing
/// punctuation, all-caps codes) and fragments without letters. Short real
/// syllables, including single vowels, are kept.
pub fn is_lyric_text(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_lowercase();
    let letters: String = t.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return false;
    }
    if DYNAMICS.contains(&letters.to_lowercase().as_str())
        && letters.len() == t.trim_matches(|c: char| !c.is_alphanumeric()).len()
    {
        return false;
    }
    if lower.contains("www")
        || lower.contains("http")
        || lower.contains(".org")
        || lower.contains(".com")
    {
        return false;
    }
    // Stray single consonants are OCR specks; single vowels are real syllables.
    if letters.chars().count() == 1 {
        let c = letters.to_lowercase().chars().next().unwrap_or(' ');
        return "aeiouyåäöæøéèü".contains(c);
    }
    // All-caps codes of 3+ letters (ISMN, SATB) are not sung.
    if letters.chars().count() >= 3 && letters.chars().all(|c| c.is_uppercase()) {
        return false;
    }
    // A syllable longer than any sung syllable is a credit line or title.
    if letters.chars().count() > 14 {
        return false;
    }
    // Text mixing in symbols other than lyric punctuation is noise ("+T2 '").
    if t.chars()
        .any(|c| !(c.is_alphanumeric() || " ,.;:!?’'\"-()–".contains(c)))
    {
        return false;
    }
    true
}

/// Lyric text as sung: verse numbers ("1.", "1.3.") stripped, noise dropped.
pub fn clean_lyric(text: &str) -> Option<String> {
    let t = text.trim();
    let stripped = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
    let t = if stripped.len() < t.len() && t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        stripped
    } else {
        t
    };
    is_lyric_text(t).then(|| t.to_string())
}

/// Performer cues printed in the lyric line (`Female:`, `Children:`, `Solo:`).
fn is_role_label(text: &str) -> bool {
    let t = text.trim();
    t.len() > 2
        && t.ends_with(':')
        && t[..t.len() - 1]
            .chars()
            .next()
            .is_some_and(char::is_uppercase)
        && matches!(
            t[..t.len() - 1].to_lowercase().as_str(),
            "female"
                | "male"
                | "children"
                | "solo"
                | "all"
                | "men"
                | "women"
                | "tutti"
                | "choir"
                | "chorus"
                | "soprano"
                | "alto"
                | "tenor"
                | "bass"
                | "baritone"
                | "unison"
                | "descant"
                | "girls"
                | "boys"
        )
}

fn extract_with(
    score: &Score,
    tm: &Timemap,
    part: usize,
    voice: Option<u32>,
    verse: u32,
    merged: bool,
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
                .filter(|l| !is_role_label(&l.text))
                .find(|l| l.verse == wanted)
                .or_else(|| {
                    e.lyrics
                        .iter()
                        .find(|l| l.verse == verse && !is_role_label(&l.text))
                })
                .or_else(|| {
                    merged
                        .then(|| {
                            let mut ls: Vec<_> = e
                                .lyrics
                                .iter()
                                .filter(|l| !is_role_label(&l.text))
                                .collect();
                            ls.sort_by_key(|l| l.verse);
                            ls.first().copied()
                        })
                        .flatten()
                })
                .and_then(|l| {
                    // OCR noise becomes "no lyric" (extender-only lyrics stay).
                    match clean_lyric(&l.text) {
                        Some(t) => Some((t, l.syllabic, l.extend)),
                        None if l.text.trim().is_empty() && l.extend => {
                            Some((String::new(), l.syllabic, l.extend))
                        }
                        None => None,
                    }
                });
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
    let mut last_end: Option<Tick> = None;
    let mut word = String::new();
    for (onset, cands) in by_onset {
        let top = cands.iter().max_by_key(|c| c.pitch).expect("non-empty");
        let lyric = top
            .lyric
            .clone()
            .or_else(|| cands.iter().find_map(|c| c.lyric.clone()));
        let end = onset + top.dur;
        // A silence that breaks the phrase also ends any held syllable.
        if last_end.is_some_and(|e| onset >= e + PHRASE_BREAK) {
            held = None;
        }
        last_end = Some(last_end.map_or(end, |e| e.max(end)));

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

/// Clean OMR MusicXML for display: drop `<lyric>` elements whose text is not
/// sung (dynamics, credits, URLs, specks), strip verse-number prefixes, and
/// remove `<harmony>` chord symbols (OCR turns stray text into them). Only
/// applies to files written by an OMR engine (Audiveris); files from notation
/// programs are returned unchanged. Text-level edit so everything else in the
/// file (layout, ids) is kept for the engraver.
pub fn clean_omr_musicxml(xml: &str) -> String {
    let head = &xml[..xml.len().min(4000)];
    if !head.contains("<software>Audiveris") {
        return xml.to_string();
    }
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    loop {
        let next_lyric = rest.find("<lyric");
        let next_harmony = rest.find("<harmony");
        let (start, is_lyric) = match (next_lyric, next_harmony) {
            (None, None) => break,
            (Some(l), Some(h)) if h < l => (h, false),
            (Some(l), _) => (l, true),
            (None, Some(h)) => (h, false),
        };
        let close = if is_lyric { "</lyric>" } else { "</harmony>" };
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        // Self-closing elements carry no text: keep them.
        let tag_end = tail.find('>').map_or(tail.len(), |i| i + 1);
        if tail[..tag_end].ends_with("/>") {
            out.push_str(&tail[..tag_end]);
            rest = &tail[tag_end..];
            continue;
        }
        let Some(end) = tail.find(close).map(|i| i + close.len()) else {
            out.push_str(tail);
            rest = "";
            break;
        };
        let element = &tail[..end];
        if is_lyric {
            if let Some(cleaned) = clean_lyric_element(element) {
                out.push_str(&cleaned);
            }
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// One `<lyric>…</lyric>` element: cleaned, or `None` to drop it.
fn clean_lyric_element(el: &str) -> Option<String> {
    let (Some(a), Some(b)) = (el.find("<text"), el.find("</text>")) else {
        return Some(el.to_string()); // extender-only lyric
    };
    let open_end = a + el[a..].find('>')? + 1;
    let raw = &el[open_end..b];
    let text = raw
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&quot;", "\"");
    let cleaned = clean_lyric(&text)?;
    if cleaned == text {
        return Some(el.to_string());
    }
    let escaped = cleaned
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    Some(format!("{}{}{}", &el[..open_end], escaped, &el[b..]))
}
