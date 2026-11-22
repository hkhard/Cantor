//! LIB-06: correct single notes in a MusicXML document (OMR mistakes) and
//! re-save. Text-level edit: only the addressed `<note>` changes.

use crate::model::{Pitch, Step};
use crate::parse::{parse_musicxml, ParseError};
use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;

/// What to do with a note.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteEdit {
    /// Move by diatonic steps (+1 = next line/space up), taking the key
    /// signature's accidental for the new step.
    Step(i32),
    /// Set an exact pitch (a rest becomes a note).
    Set(Pitch),
    /// Make it a rest (keeps duration, type, voice, staff).
    Rest,
}

const STEPS: [&str; 7] = ["C", "D", "E", "F", "G", "A", "B"];

/// Accidental the key signature `fifths` gives a step.
fn key_alter(step: usize, fifths: i32) -> i32 {
    // Order of sharps F C G D A E B; flats reversed.
    const SHARPS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
    if fifths > 0 && SHARPS[..fifths.min(7) as usize].contains(&step) {
        1
    } else if fifths < 0
        && SHARPS
            .iter()
            .rev()
            .take((-fifths).min(7) as usize)
            .any(|&s| s == step)
    {
        -1
    } else {
        0
    }
}

fn pitch_xml(step: usize, alter: i32, octave: i32) -> String {
    let mut s = format!("<pitch><step>{}</step>", STEPS[step]);
    if alter != 0 {
        s.push_str(&format!("<alter>{alter}</alter>"));
    }
    s.push_str(&format!("<octave>{octave}</octave></pitch>"));
    s
}

/// Apply `edit` to note `note` (0-based, every `<note>` of the measure in
/// document order: chord notes, grace notes and rests count) of written
/// measure `measure` in part `part`. Returns the new document.
pub fn edit_note(
    xml: &str,
    part: usize,
    measure: usize,
    note: usize,
    edit: NoteEdit,
) -> Result<String, ParseError> {
    parse_musicxml(xml.as_bytes())?;
    let err = |m: &str| ParseError::Xml(m.to_string());
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let (mut p_i, mut m_i, mut n_i) = (-1i64, -1i64, -1i64);
    let mut fifths = 0i32;
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    // Inside the target note: span of <pitch>...</pitch> or <rest/> and parsed pitch.
    let mut in_target = false;
    let mut target_span: Option<(usize, usize)> = None;
    let mut pitch_fields: Vec<(String, String)> = Vec::new();
    let mut pitch_start = 0usize;
    let mut was_rest = false;
    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader.read_event().map_err(|e| err(&e.to_string()))?;
        let after = reader.buffer_position() as usize;
        match ev {
            XmlEvent::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match name.as_str() {
                    "part" => {
                        p_i += 1;
                        m_i = -1;
                        fifths = 0;
                    }
                    "measure" => {
                        m_i += 1;
                        n_i = -1;
                    }
                    "note" => {
                        n_i += 1;
                        in_target =
                            p_i == part as i64 && m_i == measure as i64 && n_i == note as i64;
                    }
                    "pitch" if in_target => {
                        pitch_start = before;
                        pitch_fields.clear();
                    }
                    "rest" if in_target => {
                        pitch_start = before;
                        was_rest = true;
                    }
                    _ => {}
                }
                text.clear();
                stack.push(name);
            }
            XmlEvent::Empty(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if in_target && name == "rest" {
                    was_rest = true;
                    target_span = Some((before, after));
                }
            }
            XmlEvent::Text(t) => text.push_str(&t.unescape().map_err(|e| err(&e.to_string()))?),
            XmlEvent::End(_) => {
                let name = stack.pop().unwrap_or_default();
                let parent = stack.last().cloned();
                match (name.as_str(), parent.as_deref()) {
                    ("fifths", Some("key")) => {
                        if let Ok(f) = text.trim().parse() {
                            fifths = f;
                        }
                    }
                    (c, Some("pitch")) if in_target => {
                        pitch_fields.push((c.to_string(), text.clone()))
                    }
                    ("pitch", _) if in_target => target_span = Some((pitch_start, after)),
                    ("rest", _) if in_target => target_span = Some((pitch_start, after)),
                    ("note", _) if in_target => {
                        let (start, end) =
                            target_span.ok_or_else(|| err("note has no pitch or rest"))?;
                        let get = |k: &str| {
                            pitch_fields
                                .iter()
                                .find(|(n, _)| n == k)
                                .map(|(_, v)| v.trim().to_string())
                        };
                        let cur_step = get("step")
                            .and_then(|s| STEPS.iter().position(|x| *x == s))
                            .unwrap_or(0);
                        let cur_oct: i32 = get("octave").and_then(|o| o.parse().ok()).unwrap_or(4);
                        let rep = match edit {
                            NoteEdit::Rest => "<rest/>".to_string(),
                            NoteEdit::Set(p) => {
                                let idx = STEPS
                                    .iter()
                                    .position(|x| *x == step_str(p.step))
                                    .unwrap_or(0);
                                pitch_xml(idx, p.alter, p.octave)
                            }
                            NoteEdit::Step(d) => {
                                if was_rest {
                                    return Err(err("cannot step a rest; set a pitch"));
                                }
                                let abs = cur_oct * 7 + cur_step as i32 + d;
                                let (oct, st) = (abs.div_euclid(7), abs.rem_euclid(7) as usize);
                                pitch_xml(st, key_alter(st, fifths), oct)
                            }
                        };
                        let mut out = String::with_capacity(xml.len() + 16);
                        out.push_str(&xml[..start]);
                        out.push_str(&rep);
                        out.push_str(&xml[end..]);
                        // Drop a now-wrong <accidental> in this note.
                        return Ok(drop_accidental(&out, start + rep.len()));
                    }
                    _ => {}
                }
                text.clear();
            }
            XmlEvent::Eof => break,
            _ => {}
        }
    }
    Err(err(&format!(
        "no note {note} in measure {measure} of part {part}"
    )))
}

fn step_str(s: Step) -> &'static str {
    STEPS[[
        Step::C,
        Step::D,
        Step::E,
        Step::F,
        Step::G,
        Step::A,
        Step::B,
    ]
    .iter()
    .position(|&x| x == s)
    .unwrap_or(0)]
}

/// Remove the `<accidental>` of the note that continues at byte `from`.
fn drop_accidental(xml: &str, from: usize) -> String {
    let end = xml[from..]
        .find("</note>")
        .map(|i| from + i)
        .unwrap_or(from);
    if let Some(i) = xml[from..end].find("<accidental") {
        let a = from + i;
        if let Some(j) = xml[a..end].find("</accidental>") {
            let b = a + j + "</accidental>".len();
            return format!("{}{}", &xml[..a], &xml[b..]);
        }
    }
    xml.to_string()
}
