//! `score-partwise` parser (LIB-01). Timewise and MXL follow in WP-1.2.

use crate::dom::{self, DomError, Element};
use crate::model::{Event, EventKind, Measure, Part, Pitch, Score, Step, Tick, PPQ};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("not a readable XML document: {0}")]
    Xml(String),
    #[error("unsupported root element <{0}>, expected <score-partwise>")]
    UnsupportedRoot(String),
    #[error("part {part}, measure {measure}: {msg}")]
    Invalid {
        part: String,
        measure: String,
        msg: String,
    },
}

impl From<DomError> for ParseError {
    fn from(e: DomError) -> Self {
        ParseError::Xml(e.to_string())
    }
}

/// Parse an uncompressed MusicXML document.
pub fn parse_musicxml(bytes: &[u8]) -> Result<Score, ParseError> {
    let root = dom::parse(bytes)?;
    match root.name.as_str() {
        "score-partwise" => parse_partwise(&root),
        other => Err(ParseError::UnsupportedRoot(other.to_string())),
    }
}

fn parse_partwise(root: &Element) -> Result<Score, ParseError> {
    let names: Vec<(String, String)> = root
        .child("part-list")
        .map(|pl| {
            pl.children("score-part")
                .map(|sp| {
                    let id = sp.attr("id").unwrap_or_default().to_string();
                    let name = sp.child_text("part-name").unwrap_or_default().to_string();
                    (id, name)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut parts = Vec::new();
    for p in root.children("part") {
        let id = p.attr("id").unwrap_or_default().to_string();
        let name = names
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        let measures = parse_part_measures(&id, p)?;
        parts.push(Part { id, name, measures });
    }
    Ok(Score { parts })
}

struct Ctx<'a> {
    part: &'a str,
    measure: String,
}

impl Ctx<'_> {
    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError::Invalid {
            part: self.part.to_string(),
            measure: self.measure.clone(),
            msg: msg.into(),
        }
    }

    fn num<T: std::str::FromStr>(&self, el: &Element, name: &str) -> Result<Option<T>, ParseError> {
        match el.child_text(name) {
            None => Ok(None),
            Some(t) => t
                .parse()
                .map(Some)
                .map_err(|_| self.err(format!("<{name}> is not a number: {t:?}"))),
        }
    }
}

fn parse_part_measures(part_id: &str, part: &Element) -> Result<Vec<Measure>, ParseError> {
    // Divisions persist across measures until changed.
    let mut divisions: u64 = 1;
    let mut out = Vec::new();

    for m in part.children("measure") {
        let number = m.attr("number").unwrap_or_default().to_string();
        let ctx = Ctx {
            part: part_id,
            measure: number.clone(),
        };
        let to_ticks = |d: u64, div: u64| d * PPQ / div;

        let mut cursor: Tick = 0;
        let mut last_onset: Tick = 0;
        let mut events = Vec::new();

        for child in &m.children {
            match child.name.as_str() {
                "attributes" => {
                    if let Some(d) = ctx.num::<u64>(child, "divisions")? {
                        if d == 0 {
                            return Err(ctx.err("<divisions> must be positive"));
                        }
                        divisions = d;
                    }
                }
                "backup" => {
                    let d = to_ticks(ctx.num::<u64>(child, "duration")?.unwrap_or(0), divisions);
                    cursor = cursor.saturating_sub(d);
                }
                "forward" => {
                    cursor += to_ticks(ctx.num::<u64>(child, "duration")?.unwrap_or(0), divisions);
                }
                "note" => {
                    let is_chord = child.child("chord").is_some();
                    let is_grace = child.child("grace").is_some();
                    let duration =
                        to_ticks(ctx.num::<u64>(child, "duration")?.unwrap_or(0), divisions);
                    let voice = ctx.num::<u32>(child, "voice")?.unwrap_or(1);
                    let staff = ctx.num::<u32>(child, "staff")?.unwrap_or(1);
                    let kind = if child.child("rest").is_some() {
                        EventKind::Rest
                    } else if let Some(p) = child.child("pitch") {
                        EventKind::Note(parse_pitch(&ctx, p)?)
                    } else {
                        // Unpitched percussion: modelled in WP-1.1 follow-up.
                        continue;
                    };
                    let offset = if is_chord { last_onset } else { cursor };
                    events.push(Event {
                        offset,
                        duration,
                        voice,
                        staff,
                        kind,
                    });
                    if !is_chord && !is_grace {
                        last_onset = cursor;
                        cursor += duration;
                    }
                }
                _ => {}
            }
        }
        out.push(Measure { number, events });
    }
    Ok(out)
}

fn parse_pitch(ctx: &Ctx<'_>, p: &Element) -> Result<Pitch, ParseError> {
    let step_text = p
        .child_text("step")
        .ok_or_else(|| ctx.err("<pitch> without <step>"))?;
    let step =
        Step::parse(step_text).ok_or_else(|| ctx.err(format!("bad <step> {step_text:?}")))?;
    let octave = ctx
        .num::<i32>(p, "octave")?
        .ok_or_else(|| ctx.err("<pitch> without <octave>"))?;
    // <alter> may be fractional for microtones; round to nearest semitone for now.
    let alter = match p.child_text("alter") {
        None => 0,
        Some(t) => t
            .parse::<f64>()
            .map(|a| a.round() as i32)
            .map_err(|_| ctx.err(format!("bad <alter> {t:?}")))?,
    };
    Ok(Pitch {
        step,
        alter,
        octave,
    })
}
