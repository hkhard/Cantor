//! Transposition by semitones (VIEW-05), on the model and on MusicXML text.
//!
//! # Spelling rule
//!
//! A transposition by `n` semitones moves the key signature around the
//! circle of fifths by `7·n mod 12`. Of the two enharmonic keys this gives
//! (`f` and `f - 12`) the one with fewer accidentals wins, so D major down a
//! semitone is Db (5 flats), not C# (7 sharps). On a tie (6 sharps or 6
//! flats) sharps are used when transposing up, flats when down
//! ([`transposed_fifths`]).
//!
//! Every note is then moved by the *diatonic* interval implied by the actual
//! key change `D = new - old` fifths: its letter moves `4·D mod 7` steps up,
//! and `alter` and `octave` are chosen so the MIDI number moves by exactly
//! `n`. Because notes and key move by the same interval, a note that was
//! diatonic stays diatonic and a chromatic note keeps its relation to the key
//! (F# in C becomes G# in D). Should that produce a triple accidental, the note
//! is respelled to the nearest letter with |alter| <= 2.
//!
//! Measures without a key signature are spelled as if in C (fifths 0) and keep
//! `key_fifths: None`. The rule is deterministic, and `+n` then `-n` restores
//! the original model for keys of up to 6 accidentals.

use crate::model::{EventKind, Pitch, Score, Step};
use crate::parse::{parse_musicxml, ParseError};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;

const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];

fn step_index(s: Step) -> i32 {
    STEPS.iter().position(|&x| x == s).unwrap_or(0) as i32
}

fn step_name(s: Step) -> &'static str {
    ["C", "D", "E", "F", "G", "A", "B"][step_index(s) as usize]
}

/// Key signature (`<fifths>`) after transposing by `semitones`.
pub fn transposed_fifths(fifths: i32, semitones: i32) -> i32 {
    let f = fifths + (7 * semitones).rem_euclid(12);
    let (sharp, flat) = (f, f - 12);
    // Fewest accidentals; on a tie (F# vs Gb) follow the direction.
    match sharp.abs().cmp(&flat.abs()) {
        std::cmp::Ordering::Less => sharp,
        std::cmp::Ordering::Greater => flat,
        std::cmp::Ordering::Equal if semitones >= 0 => sharp,
        std::cmp::Ordering::Equal => flat,
    }
}

/// Transpose one pitch by `semitones`, in a measure whose key is `fifths`.
fn transpose_pitch(p: Pitch, fifths: i32, semitones: i32) -> Pitch {
    let delta_fifths = transposed_fifths(fifths, semitones) - fifths;
    let letters = (4 * delta_fifths).rem_euclid(7);
    let target = p.midi() + semitones;
    spell(step_index(p.step) + letters, p.octave, target)
}

/// Pitch with letter index `idx` (may exceed 6, carries into the octave)
/// near `octave`, sounding `target`; respelled if the alter would exceed ±2.
fn spell(idx: i32, octave: i32, target: i32) -> Pitch {
    let step = STEPS[idx.rem_euclid(7) as usize];
    let mut octave = octave + idx.div_euclid(7);
    let mut alter = target - (octave + 1) * 12 - step.semitone();
    while alter > 6 {
        alter -= 12;
        octave += 1;
    }
    while alter < -6 {
        alter += 12;
        octave -= 1;
    }
    match alter {
        a if a > 2 => spell(idx + 1, octave - idx.div_euclid(7), target),
        a if a < -2 => spell(idx - 1, octave - idx.div_euclid(7), target),
        _ => Pitch {
            step,
            alter,
            octave,
        },
    }
}

/// BAND-02: a chord root or bass `(step, alter)` moved like a note in the
/// same key, so it follows the key's spelling (Bb in flat keys, A# never).
fn transpose_chord_step(sa: (Step, i32), fifths: i32, semitones: i32) -> (Step, i32) {
    let p = transpose_pitch(
        Pitch {
            step: sa.0,
            alter: sa.1,
            octave: 4,
        },
        fifths,
        semitones,
    );
    (p.step, p.alter)
}

/// The new `<root>` / `<bass>` element of a chord symbol, or `None` if the
/// element has no readable step.
fn chord_step_xml(
    name: &str,
    fields: &[(String, String)],
    fifths: i32,
    semitones: i32,
) -> Option<String> {
    let get = |k: String| {
        fields
            .iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.trim().to_string())
    };
    let step = Step::parse(&get(format!("{name}-step"))?)?;
    let alter = get(format!("{name}-alter"))
        .and_then(|a| a.parse::<f64>().ok())
        .map_or(0, |a| a.round() as i32);
    let (st, al) = transpose_chord_step((step, alter), fifths, semitones);
    let mut out = format!("<{name}><{name}-step>{}</{name}-step>", step_name(st));
    if al != 0 {
        out.push_str(&format!("<{name}-alter>{al}</{name}-alter>"));
    }
    out.push_str(&format!("</{name}>"));
    Some(out)
}

impl Score {
    /// A copy with every pitch, chord symbol and key signature moved by
    /// `semitones` (see the module docs for the spelling rule).
    pub fn transposed(&self, semitones: i32) -> Score {
        let mut s = self.clone();
        for m in s.parts.iter_mut().flat_map(|p| &mut p.measures) {
            let fifths = m.key_fifths.unwrap_or(0);
            for e in &mut m.events {
                if let EventKind::Note(p) = &mut e.kind {
                    *p = transpose_pitch(*p, fifths, semitones);
                }
            }
            for h in &mut m.harmonies {
                h.root = h.root.map(|r| transpose_chord_step(r, fifths, semitones));
                h.bass = h.bass.map(|b| transpose_chord_step(b, fifths, semitones));
            }
            m.key_fifths = m.key_fifths.map(|f| transposed_fifths(f, semitones));
        }
        s
    }
}

fn accidental_name(alter: i32) -> &'static str {
    match alter {
        a if a <= -2 => "flat-flat",
        -1 => "flat",
        0 => "natural",
        1 => "sharp",
        _ => "double-sharp",
    }
}

/// Transpose a MusicXML document in place, for display. Only the contents of
/// `<pitch>`, `<key><fifths>` and note `<accidental>` elements change; all
/// other bytes (layout, lyrics, comments) are preserved, so
/// `parse(musicxml_transposed(x, n)) == parse(x).transposed(n)`.
pub fn musicxml_transposed(xml: &str, semitones: i32) -> Result<String, ParseError> {
    // Validate first so the text pass may assume a well-formed score.
    parse_musicxml(xml.as_bytes())?;
    let xml_err = |e: &dyn std::fmt::Display| ParseError::Xml(e.to_string());

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut fifths = 0; // key in effect in the current part
    let mut content_start = 0; // byte after the start tag of fifths/accidental
    let mut pitch_start = 0;
    let mut pitch_text: Vec<(String, String)> = Vec::new();
    let mut text = String::new();
    let mut note_alter: Option<i32> = None;

    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader.read_event().map_err(|e| xml_err(&e))?;
        let after = reader.buffer_position() as usize;
        match ev {
            XmlEvent::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "part" => fifths = 0,
                    "note" => note_alter = None,
                    "pitch" | "root" | "bass" => {
                        pitch_start = before;
                        pitch_text.clear();
                    }
                    _ => {}
                }
                content_start = after;
                text.clear();
                stack.push(name);
            }
            XmlEvent::Text(t) => text.push_str(&t.unescape().map_err(|e| xml_err(&e))?),
            XmlEvent::End(_) => {
                let name = stack.pop().unwrap_or_default();
                let parent = stack.last().map(String::as_str);
                match (name.as_str(), parent) {
                    ("fifths", Some("key")) => {
                        if let Ok(f) = text.trim().parse::<i32>() {
                            fifths = f;
                            let new = transposed_fifths(f, semitones).to_string();
                            edits.push((content_start, before, new));
                        }
                    }
                    ("root" | "bass", Some("harmony")) => {
                        if let Some(out) = chord_step_xml(&name, &pitch_text, fifths, semitones) {
                            edits.push((pitch_start, after, out));
                        }
                    }
                    ("pitch", _) => {
                        let get = |k: &str| {
                            pitch_text
                                .iter()
                                .find(|(n, _)| n == k)
                                .map(|(_, v)| v.trim())
                        };
                        let step = get("step").and_then(Step::parse).unwrap_or(Step::C);
                        let alter = get("alter")
                            .and_then(|a| a.parse::<f64>().ok())
                            .map_or(0, |a| a.round() as i32);
                        let octave = get("octave").and_then(|o| o.parse().ok()).unwrap_or(4);
                        let p = transpose_pitch(
                            Pitch {
                                step,
                                alter,
                                octave,
                            },
                            fifths,
                            semitones,
                        );
                        note_alter = Some(p.alter);
                        let mut out = format!("<pitch><step>{}</step>", step_name(p.step));
                        if p.alter != 0 {
                            out.push_str(&format!("<alter>{}</alter>", p.alter));
                        }
                        out.push_str(&format!("<octave>{}</octave></pitch>", p.octave));
                        edits.push((pitch_start, after, out));
                    }
                    (child, Some("pitch" | "root" | "bass")) => {
                        pitch_text.push((child.to_string(), std::mem::take(&mut text)));
                    }
                    ("accidental", Some("note")) => {
                        if let Some(a) = note_alter {
                            edits.push((content_start, before, accidental_name(a).into()));
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }

    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for (start, end, rep) in edits {
        out.push_str(&xml[last..start]);
        out.push_str(&rep);
        last = end;
    }
    out.push_str(&xml[last..]);
    Ok(out)
}

/// VIEW-05 concert pitch: rewrite the written notes of transposing parts
/// (MusicXML `<transpose>`, e.g. a B-flat clarinet or a tenor written an
/// octave up) to the pitches that sound, and drop the `<transpose>` elements,
/// so notation and playback show the same notes. Parts without `<transpose>`
/// are untouched; a document without any is returned unchanged.
pub fn musicxml_concert_pitch(xml: &str) -> Result<String, ParseError> {
    if !xml.contains("<transpose") {
        return Ok(xml.to_string());
    }
    parse_musicxml(xml.as_bytes())?;
    let xml_err = |e: &dyn std::fmt::Display| ParseError::Xml(e.to_string());
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    // Current transposition of the part and its <transpose> fields.
    let mut semis = 0i32;
    let mut tr_start = 0usize;
    let mut tr_fields: Vec<(String, String)> = Vec::new();
    let mut fifths = 0i32;
    let mut content_start = 0usize;
    let mut pitch_start = 0usize;
    let mut pitch_text: Vec<(String, String)> = Vec::new();
    let mut note_alter: Option<i32> = None;
    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader.read_event().map_err(|e| xml_err(&e))?;
        let after = reader.buffer_position() as usize;
        match ev {
            XmlEvent::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "part" => {
                        semis = 0;
                        fifths = 0;
                    }
                    "note" => note_alter = None,
                    "pitch" | "root" | "bass" => {
                        pitch_start = before;
                        pitch_text.clear();
                    }
                    "transpose" => {
                        tr_start = before;
                        tr_fields.clear();
                    }
                    _ => {}
                }
                content_start = after;
                text.clear();
                stack.push(name);
            }
            XmlEvent::Text(t) => text.push_str(&t.unescape().map_err(|e| xml_err(&e))?),
            XmlEvent::End(_) => {
                let name = stack.pop().unwrap_or_default();
                let parent = stack.last().map(String::as_str);
                match (name.as_str(), parent) {
                    (child, Some("transpose")) => {
                        tr_fields.push((child.to_string(), std::mem::take(&mut text)));
                    }
                    ("transpose", _) => {
                        let n = |k: &str| {
                            tr_fields
                                .iter()
                                .find(|(f, _)| f == k)
                                .and_then(|(_, v)| v.trim().parse::<i32>().ok())
                                .unwrap_or(0)
                        };
                        let double = tr_fields.iter().any(|(f, _)| f == "double");
                        semis =
                            n("chromatic") + 12 * n("octave-change") - if double { 12 } else { 0 };
                        // Remove the element (and the whitespace before it on its line).
                        edits.push((tr_start, after, String::new()));
                    }
                    ("fifths", Some("key")) => {
                        if let Ok(f) = text.trim().parse::<i32>() {
                            fifths = f;
                            if semis != 0 {
                                edits.push((
                                    content_start,
                                    before,
                                    transposed_fifths(f, semis).to_string(),
                                ));
                            }
                        }
                    }
                    ("root" | "bass", Some("harmony")) if semis != 0 => {
                        if let Some(out) = chord_step_xml(&name, &pitch_text, fifths, semis) {
                            edits.push((pitch_start, after, out));
                        }
                    }
                    ("pitch", _) if semis != 0 => {
                        let get = |k: &str| {
                            pitch_text
                                .iter()
                                .find(|(n, _)| n == k)
                                .map(|(_, v)| v.trim())
                        };
                        let step = get("step").and_then(Step::parse).unwrap_or(Step::C);
                        let alter = get("alter")
                            .and_then(|a| a.parse::<f64>().ok())
                            .map_or(0, |a| a.round() as i32);
                        let octave = get("octave").and_then(|o| o.parse().ok()).unwrap_or(4);
                        let p = transpose_pitch(
                            Pitch {
                                step,
                                alter,
                                octave,
                            },
                            fifths,
                            semis,
                        );
                        note_alter = Some(p.alter);
                        let mut out = format!("<pitch><step>{}</step>", step_name(p.step));
                        if p.alter != 0 {
                            out.push_str(&format!("<alter>{}</alter>", p.alter));
                        }
                        out.push_str(&format!("<octave>{}</octave></pitch>", p.octave));
                        edits.push((pitch_start, after, out));
                    }
                    (child, Some("pitch" | "root" | "bass")) => {
                        pitch_text.push((child.to_string(), std::mem::take(&mut text)));
                    }
                    ("accidental", Some("note")) if semis != 0 => {
                        if let Some(a) = note_alter {
                            edits.push((content_start, before, accidental_name(a).into()));
                        }
                    }
                    _ => {}
                }
                text.clear();
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for (start, end, rep) in edits {
        if start < last {
            continue;
        }
        out.push_str(&xml[last..start]);
        out.push_str(&rep);
        last = end;
    }
    out.push_str(&xml[last..]);
    Ok(out)
}
