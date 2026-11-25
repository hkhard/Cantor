//! `score-partwise` parser and MXL container (LIB-01). Timewise follows in WP-1.2.

use crate::dom::{self, DomError, Element};
use crate::model::{
    Articulation, Degree, DegreeKind, Event, EventKind, Harmony, Header, Lyric, Measure,
    Navigation, Part, Pitch, Score, Step, Swing, Syllabic, Tick, PPQ,
};
use std::io::Read;

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
    #[error("broken MXL container: {0}")]
    Mxl(String),
}

impl From<DomError> for ParseError {
    fn from(e: DomError) -> Self {
        ParseError::Xml(e.to_string())
    }
}

/// Parse a score file of any supported kind, sniffed from its bytes:
/// compressed MXL (zip) or uncompressed MusicXML.
pub fn parse_score_file(bytes: &[u8]) -> Result<Score, ParseError> {
    if bytes.starts_with(b"PK\x03\x04") {
        parse_mxl(bytes)
    } else {
        parse_musicxml(bytes)
    }
}

/// Only the header of a score file (MXL or MusicXML), cheaply: the document
/// is cut before `<part-list>` (the header elements come first by schema), so
/// listing a large library does not parse every note.
pub fn read_header(bytes: &[u8]) -> Result<Header, ParseError> {
    let xml = musicxml_bytes(bytes)?;
    let text = String::from_utf8_lossy(&xml);
    let Some(root_at) = ["<score-partwise", "<score-timewise"]
        .iter()
        .find_map(|r| text.find(r))
    else {
        return Err(ParseError::UnsupportedRoot("no score root".into()));
    };
    let root_name = if text[root_at..].starts_with("<score-partwise") {
        "score-partwise"
    } else {
        "score-timewise"
    };
    let head = match text[root_at..].find("<part-list") {
        Some(i) => &text[..root_at + i],
        None => &text[..],
    };
    let doc = format!("{head}</{root_name}>");
    let root = dom::parse(doc.as_bytes())?;
    Ok(header(&root))
}

/// Parse a compressed MusicXML (`.mxl`) container.
pub fn parse_mxl(bytes: &[u8]) -> Result<Score, ParseError> {
    parse_musicxml(&mxl_rootfile(bytes)?)
}

/// The uncompressed MusicXML document of a score file: `.mxl` containers are
/// unpacked, plain MusicXML is returned as is. Used to feed engravers that
/// want plain XML.
pub fn musicxml_bytes(bytes: &[u8]) -> Result<Vec<u8>, ParseError> {
    if bytes.starts_with(b"PK\x03\x04") {
        mxl_rootfile(bytes)
    } else {
        Ok(bytes.to_vec())
    }
}

fn mxl_rootfile(bytes: &[u8]) -> Result<Vec<u8>, ParseError> {
    let mxl = |e: &dyn std::fmt::Display| ParseError::Mxl(e.to_string());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| mxl(&e))?;
    let mut read = |name: &str| -> Result<Vec<u8>, ParseError> {
        let f = zip.by_name(name).map_err(|e| mxl(&e))?;
        read_capped(f)
    };

    let rootfile = match read("META-INF/container.xml") {
        Ok(c) => {
            let c = dom::parse(&c)?;
            c.child("rootfiles")
                .and_then(|r| r.child("rootfile"))
                .and_then(|r| r.attr("full-path"))
                .map(str::to_string)
                .ok_or_else(|| ParseError::Mxl("container.xml has no rootfile".into()))?
        }
        Err(_) => {
            // No container: accept a single score file at any path.
            let names: Vec<String> = zip
                .file_names()
                .filter_map(|n| n.ok().map(|n| n.into_owned()))
                .filter(|n| !n.starts_with("META-INF/"))
                .filter(|n| n.ends_with(".xml") || n.ends_with(".musicxml"))
                .collect();
            match names.as_slice() {
                [one] => one.clone(),
                _ => {
                    return Err(ParseError::Mxl(
                        "no container.xml and no single score file".into(),
                    ))
                }
            }
        }
    };
    let f = zip.by_name(&rootfile).map_err(|e| mxl(&e))?;
    read_capped(f)
}

/// NFR-13: an MXL entry may unpack to at most 512 MB (zip bombs are refused).
fn read_capped(f: impl Read) -> Result<Vec<u8>, ParseError> {
    const MAX: u64 = 512 * 1024 * 1024;
    let mut buf = Vec::new();
    f.take(MAX + 1)
        .read_to_end(&mut buf)
        .map_err(|e| ParseError::Mxl(e.to_string()))?;
    if buf.len() as u64 > MAX {
        return Err(ParseError::Mxl("score file too large to unpack".into()));
    }
    Ok(buf)
}

/// Parse an uncompressed MusicXML document.
pub fn parse_musicxml(bytes: &[u8]) -> Result<Score, ParseError> {
    let root = dom::parse(bytes)?;
    match root.name.as_str() {
        "score-partwise" => parse_partwise(&root),
        other => Err(ParseError::UnsupportedRoot(other.to_string())),
    }
}

struct PartInfo {
    id: String,
    name: String,
    program: Option<u8>,
    channel: Option<u8>,
}

fn part_list(root: &Element) -> Vec<PartInfo> {
    let Some(pl) = root.child("part-list") else {
        return Vec::new();
    };
    pl.children("score-part")
        .map(|sp| {
            let midi = sp.child("midi-instrument");
            let one_based = |name: &str| {
                midi.and_then(|m| m.child_text(name))
                    .and_then(|t| t.parse::<u16>().ok())
                    .filter(|v| (1..=128).contains(v))
                    .map(|v| (v - 1) as u8)
            };
            PartInfo {
                id: sp.attr("id").unwrap_or_default().to_string(),
                name: sp.child_text("part-name").unwrap_or_default().to_string(),
                program: one_based("midi-program"),
                channel: one_based("midi-channel").filter(|c| *c < 16),
            }
        })
        .collect()
}

/// Strip "arr." / "Music:" style prefixes from credit text.
fn credit_name(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = t.to_lowercase();
    for p in [
        "arranged by",
        "arr.",
        "arr",
        "music by",
        "music:",
        "musik:",
        "words by",
        "text:",
        "lyrics by",
        "words:",
        "by",
    ] {
        if let Some(rest) = lower.strip_prefix(p) {
            if p.ends_with(['.', ':']) || rest.starts_with([' ', ':', '.']) {
                return t[t.len() - rest.len()..]
                    .trim_start_matches([':', '.', ' '])
                    .trim()
                    .to_string();
            }
        }
    }
    t
}

fn header(root: &Element) -> Header {
    let text = |e: Option<&Element>| e.map(|e| e.text.trim().to_string()).unwrap_or_default();
    let mut h = Header {
        work_title: text(root.child("work").and_then(|w| w.child("work-title"))),
        work_number: text(root.child("work").and_then(|w| w.child("work-number"))),
        movement_title: text(root.child("movement-title")),
        movement_number: text(root.child("movement-number")),
        ..Header::default()
    };
    if let Some(id) = root.child("identification") {
        for c in id.children("creator") {
            let v = c.text.trim().to_string();
            let slot = match c.attr("type").unwrap_or("").to_lowercase().as_str() {
                "composer" => &mut h.composer,
                "arranger" => &mut h.arranger,
                "lyricist" | "poet" | "words" | "librettist" => &mut h.lyricist,
                _ => continue,
            };
            if slot.is_empty() && !v.is_empty() {
                *slot = v;
            }
        }
        h.rights = text(id.child("rights"));
    }
    // <credit>: typed credits first, then the biggest text on page 1.
    let mut biggest: Option<(f64, String)> = None;
    for c in root.children("credit") {
        if c.attr("page").is_some_and(|p| p != "1") {
            continue;
        }
        let words: Vec<&Element> = c.children("credit-words").collect();
        let joined = words
            .iter()
            .map(|w| w.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if joined.is_empty() {
            continue;
        }
        match c.child_text("credit-type").map(|t| t.trim().to_lowercase()) {
            Some(t) if t == "title" && h.credit_title.is_empty() => h.credit_title = joined,
            Some(t) if t == "composer" && h.composer.is_empty() => {
                h.composer = credit_name(&joined)
            }
            Some(t) if t == "arranger" && h.arranger.is_empty() => {
                h.arranger = credit_name(&joined)
            }
            Some(t) if (t == "lyricist" || t == "poet") && h.lyricist.is_empty() => {
                h.lyricist = credit_name(&joined)
            }
            Some(t) if t == "rights" && h.rights.is_empty() => h.rights = joined,
            Some(_) => {}
            None => {
                let size = words
                    .first()
                    .and_then(|w| w.attr("font-size"))
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if biggest.as_ref().is_none_or(|(b, _)| size > *b) {
                    biggest = Some((size, joined));
                }
            }
        }
    }
    if h.credit_title.is_empty() {
        if let Some((size, t)) = biggest {
            if size > 0.0 {
                h.credit_title = t;
            }
        }
    }
    h
}

fn parse_partwise(root: &Element) -> Result<Score, ParseError> {
    let infos = part_list(root);
    let mut parts = Vec::new();
    for p in root.children("part") {
        let id = p.attr("id").unwrap_or_default().to_string();
        let info = infos.iter().find(|i| i.id == id);
        let measures = parse_part_measures(&id, p)?;
        parts.push(Part {
            name: info.map(|i| i.name.clone()).unwrap_or_default(),
            midi_program: info.and_then(|i| i.program),
            midi_channel: info.and_then(|i| i.channel),
            id,
            measures,
        });
    }
    Ok(Score {
        header: header(root),
        parts,
    })
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

/// `<sound><swing>`: `Some(None)` = straight, `Some(Some(_))` = swing,
/// `None` = no swing information in this sound.
fn swing_of(sound: &Element) -> Option<Option<Swing>> {
    let sw = sound.child("swing")?;
    if sw.child("straight").is_some() {
        return Some(None);
    }
    let num = |n: &str| sw.child_text(n).and_then(|t| t.trim().parse::<u32>().ok());
    let unit = match sw.child_text("swing-type").map(str::trim) {
        Some("16th") => PPQ / 4,
        _ => PPQ / 2,
    };
    Some(Some(match (num("first"), num("second")) {
        (Some(first), Some(second)) if first > 0 && second > 0 => Swing {
            first,
            second,
            unit,
        },
        _ => Swing {
            unit,
            ..Swing::TRIPLET_EIGHTHS
        },
    }))
}

/// Swing from the words of a direction ("Swing", "Straight", "Swing feel").
fn swing_from_words(dir: &Element) -> Option<Option<Swing>> {
    let dt = dir.child("direction-type")?;
    let words: String = dt
        .children("words")
        .map(|w| w.text.trim().to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if words.contains("straight") || words.contains("no swing") {
        Some(None)
    } else if words.contains("swing") {
        Some(Some(Swing::TRIPLET_EIGHTHS))
    } else {
        None
    }
}

/// Velocity of a dynamic mark (`<dynamics><mf/>`), MIDI 1..127.
fn dynamic_velocity(mark: &str) -> Option<u8> {
    Some(match mark {
        "pppp" => 16,
        "ppp" => 24,
        "pp" => 36,
        "p" => 50,
        "mp" => 64,
        "mf" => 78,
        "f" => 90,
        "ff" => 104,
        "fff" => 116,
        "ffff" => 124,
        "sf" | "sfz" | "fz" | "rf" | "rfz" | "sffz" => 100,
        "fp" => 90,
        _ => return None,
    })
}

/// Dynamic from a `<direction>`: `<sound dynamics>` (percent of forte = 90)
/// wins over the printed mark.
fn direction_dynamic(dir: &Element) -> Option<u8> {
    if let Some(v) = dir
        .child("sound")
        .and_then(|s| s.attr("dynamics"))
        .and_then(|d| d.trim().parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d >= 0.0)
    {
        return Some((v * 0.9).round().clamp(1.0, 127.0) as u8);
    }
    let dt = dir.child("direction-type")?;
    let d = dt.child("dynamics")?;
    d.children
        .iter()
        .find_map(|c| dynamic_velocity(c.name.as_str()))
}

fn articulation_of(note: &Element) -> Articulation {
    let mut a = Articulation::NORMAL;
    for n in note.children("notations") {
        for arts in n.children("articulations") {
            for c in &arts.children {
                match c.name.as_str() {
                    "staccato" | "spiccato" => a.length_pct = a.length_pct.min(50),
                    "staccatissimo" => a.length_pct = a.length_pct.min(25),
                    "detached-legato" => a.length_pct = a.length_pct.min(75),
                    "accent" => a.accent = a.accent.max(15),
                    "strong-accent" => a.accent = a.accent.max(25),
                    _ => {}
                }
            }
        }
    }
    a
}

fn tempo_of(sound: &Element) -> Option<f64> {
    sound
        .attr("tempo")
        .and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|t| *t > 0.0 && t.is_finite())
}

fn parse_part_measures(part_id: &str, part: &Element) -> Result<Vec<Measure>, ParseError> {
    // Divisions and time signature persist across measures until changed.
    let mut divisions: u64 = 1;
    let mut time_sig: Option<(u32, u32)> = None;
    let mut key_fifths: Option<i32> = None;
    // Written-to-sounding interval of a transposing instrument (`<transpose>`).
    let mut transpose: i32 = 0;
    // Volta numbers open across measures (ending start .. stop/discontinue).
    let mut open_ending: Vec<u32> = Vec::new();
    // Dynamic velocity in effect per staff, carried across measures.
    let mut dynamics: std::collections::HashMap<u32, u8> = std::collections::HashMap::new();
    let mut out = Vec::new();

    for m in part.children("measure") {
        let number = m.attr("number").unwrap_or_default().to_string();
        let ctx = Ctx {
            part: part_id,
            measure: number.clone(),
        };
        let to_ticks = |d: u64, div: u64| d * PPQ / div;

        let mut cursor: Tick = 0;
        let mut high_water: Tick = 0;
        let mut last_onset: Tick = 0;
        let mut events = Vec::new();
        let mut tempos = Vec::new();
        let mut swing: Vec<(Tick, Option<Swing>)> = Vec::new();
        let mut harmonies = Vec::new();
        let mut nav = Navigation {
            ending: open_ending.clone(),
            ..Navigation::default()
        };

        for child in &m.children {
            match child.name.as_str() {
                "attributes" => {
                    if let Some(d) = ctx.num::<u64>(child, "divisions")? {
                        if d == 0 {
                            return Err(ctx.err("<divisions> must be positive"));
                        }
                        divisions = d;
                    }
                    if let Some(t) = child.child("transpose") {
                        let n = |k: &str| {
                            t.child_text(k)
                                .and_then(|v| v.trim().parse::<i32>().ok())
                                .unwrap_or(0)
                        };
                        transpose = n("chromatic")
                            + 12 * n("octave-change")
                            + if t.child("double").is_some() { -12 } else { 0 };
                    }
                    if let Some(f) = child.child("key").and_then(|k| k.child_text("fifths")) {
                        key_fifths = Some(
                            f.parse()
                                .map_err(|_| ctx.err(format!("bad <fifths> {f:?}")))?,
                        );
                    }
                    if let Some(t) = child.child("time") {
                        let beats = t.child_text("beats").and_then(|b| {
                            // Compound beats like "3+2" add up.
                            b.split('+')
                                .map(|x| x.trim().parse::<u32>().ok())
                                .sum::<Option<u32>>()
                        });
                        let beat_type = t
                            .child_text("beat-type")
                            .and_then(|b| b.parse::<u32>().ok());
                        if let (Some(b), Some(bt)) = (beats, beat_type) {
                            if b > 0 && bt > 0 {
                                time_sig = Some((b, bt));
                            }
                        }
                    }
                }
                "harmony" => {
                    if let Some(h) = parse_harmony(child, cursor, divisions) {
                        harmonies.push(h);
                    }
                }
                "backup" => {
                    let d = to_ticks(ctx.num::<u64>(child, "duration")?.unwrap_or(0), divisions);
                    cursor = cursor.saturating_sub(d);
                }
                "forward" => {
                    cursor += to_ticks(ctx.num::<u64>(child, "duration")?.unwrap_or(0), divisions);
                    high_water = high_water.max(cursor);
                }
                "direction" => {
                    // The encoded <swing> wins over the words.
                    if let Some(sw) = child
                        .child("sound")
                        .and_then(swing_of)
                        .or_else(|| swing_from_words(child))
                    {
                        swing.push((cursor, sw));
                    }
                    if let Some(v) = direction_dynamic(child) {
                        // A dynamic applies to its staff, or all staves.
                        match child
                            .child_text("staff")
                            .and_then(|s| s.trim().parse::<u32>().ok())
                        {
                            Some(st) => {
                                dynamics.insert(st, v);
                            }
                            None => {
                                for st in 1..=8 {
                                    dynamics.insert(st, v);
                                }
                            }
                        }
                    }
                    if let Some(s) = child.child("sound") {
                        if let Some(t) = tempo_of(s) {
                            tempos.push((cursor, t));
                        }
                        apply_sound_nav(s, &mut nav);
                    }
                    // Graphic segno/coda without <sound> still mark targets.
                    if let Some(dt) = child.child("direction-type") {
                        if dt.child("segno").is_some() && nav.segno.is_none() {
                            nav.segno = Some(String::new());
                        }
                        if dt.child("coda").is_some() && nav.coda.is_none() {
                            nav.coda = Some(String::new());
                        }
                    }
                }
                "sound" => {
                    if let Some(sw) = swing_of(child) {
                        swing.push((cursor, sw));
                    }
                    if let Some(t) = tempo_of(child) {
                        tempos.push((cursor, t));
                    }
                    apply_sound_nav(child, &mut nav);
                }
                "barline" => {
                    if let Some(r) = child.child("repeat") {
                        match r.attr("direction") {
                            Some("forward") => nav.repeat_start = true,
                            Some("backward") => {
                                let times = r
                                    .attr("times")
                                    .and_then(|t| t.trim().parse::<u32>().ok())
                                    .unwrap_or(2)
                                    .max(1);
                                nav.repeat_end = Some(times);
                            }
                            _ => {}
                        }
                    }
                    if let Some(e) = child.child("ending") {
                        let nums = parse_ending_numbers(e.attr("number").unwrap_or_default());
                        match e.attr("type") {
                            Some("start") => {
                                open_ending = nums.clone();
                                nav.ending = nums;
                            }
                            Some("stop") | Some("discontinue") => {
                                if nav.ending.is_empty() {
                                    nav.ending = nums;
                                }
                                open_ending.clear();
                            }
                            _ => {}
                        }
                    }
                    if child.child("segno").is_some() && nav.segno.is_none() {
                        nav.segno = Some(String::new());
                    }
                    if child.child("coda").is_some() && nav.coda.is_none() {
                        nav.coda = Some(String::new());
                    }
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
                        // Unpitched percussion: modelled in a WP-1.1 follow-up. Still
                        // occupies time so following notes keep their positions.
                        if !is_chord && !is_grace {
                            last_onset = cursor;
                            cursor += duration;
                            high_water = high_water.max(cursor);
                        }
                        continue;
                    };
                    let tie = |ty: &str| child.children("tie").any(|t| t.attr("type") == Some(ty));
                    let slur = |ty: &str| {
                        child
                            .children("notations")
                            .flat_map(|n| n.children("slur"))
                            .any(|s| s.attr("type") == Some(ty))
                    };
                    let offset = if is_chord { last_onset } else { cursor };
                    events.push(Event {
                        offset,
                        duration,
                        voice,
                        staff,
                        kind,
                        tie_start: tie("start"),
                        tie_stop: tie("stop"),
                        slur_start: slur("start"),
                        slur_stop: slur("stop"),
                        lyrics: parse_lyrics(child),
                        dynamic: dynamics.get(&staff).copied(),
                        articulation: articulation_of(child),
                    });
                    if !is_chord && !is_grace {
                        last_onset = cursor;
                        cursor += duration;
                        high_water = high_water.max(cursor);
                    }
                }
                _ => {}
            }
        }

        let nominal = time_sig.map(|(b, bt)| b as u64 * 4 * PPQ / bt as u64);
        let implicit = m.attr("implicit") == Some("yes");
        let duration = match nominal {
            Some(n) if !implicit && high_water <= n => n,
            _ => high_water,
        };
        out.push(Measure {
            number,
            duration,
            content_duration: high_water,
            implicit,
            time_signature: time_sig,
            key_fifths,
            transpose,
            tempos,
            nav,
            swing,
            events,
            harmonies,
        });
    }
    Ok(out)
}

/// `(step, alter)` from e.g. `<root><root-step>B</root-step><root-alter>-1`.
fn step_alter(el: &Element, step: &str, alter: &str) -> Option<(Step, i32)> {
    let st = Step::parse(el.child_text(step)?)?;
    let al = el
        .child_text(alter)
        .and_then(|a| a.parse::<f64>().ok())
        .map_or(0, |a| a.round() as i32);
    Some((st, al))
}

/// BAND-01: a `<harmony>` at `cursor` (ticks). Lenient: a chord symbol the
/// model cannot hold is skipped, never an error.
fn parse_harmony(h: &Element, cursor: Tick, divisions: u64) -> Option<Harmony> {
    let kind_el = h.child("kind")?;
    let kind = kind_el.text.trim().to_string();
    let root = h
        .child("root")
        .and_then(|r| step_alter(r, "root-step", "root-alter"));
    let bass = h
        .child("bass")
        .and_then(|b| step_alter(b, "bass-step", "bass-alter"));
    let degrees = h
        .children("degree")
        .filter_map(|d| {
            Some(Degree {
                value: d.child_text("degree-value")?.parse().ok()?,
                alter: d
                    .child_text("degree-alter")
                    .and_then(|a| a.parse::<f64>().ok())
                    .map_or(0, |a| a.round() as i32),
                kind: DegreeKind::parse(d.child_text("degree-type").unwrap_or("add")),
            })
        })
        .collect();
    let shift = h
        .child_text("offset")
        .and_then(|o| o.parse::<f64>().ok())
        .map_or(0, |o| (o * PPQ as f64 / divisions as f64).round() as i64);
    Some(Harmony {
        offset: (cursor as i64 + shift).max(0) as Tick,
        root,
        kind,
        kind_text: kind_el.attr("text").map(str::to_string),
        bass,
        degrees,
        staff: h
            .child_text("staff")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1),
    })
}

/// `<sound>` navigation attributes (MusicXML playback semantics).
fn apply_sound_nav(s: &Element, nav: &mut Navigation) {
    let label = |k: &str| s.attr(k).map(|v| v.trim().to_string());
    if let Some(v) = label("segno") {
        nav.segno = Some(v);
    }
    if let Some(v) = label("coda") {
        nav.coda = Some(v);
    }
    if let Some(v) = label("tocoda") {
        nav.tocoda = Some(v);
    }
    if let Some(v) = label("dalsegno") {
        nav.dalsegno = Some(v);
    }
    if s.attr("dacapo") == Some("yes") {
        nav.dacapo = true;
    }
    if s.attr("fine").is_some() {
        nav.fine = true;
    }
}

/// `"1, 2"` or `"1 2"` or `"1-3"` to volta numbers.
fn parse_ending_numbers(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for tok in s.split([',', ' ']).map(str::trim).filter(|t| !t.is_empty()) {
        let tok = tok.trim_end_matches('.');
        if let Some((a, b)) = tok.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                out.extend(a..=b.min(a + 16));
            }
        } else if let Ok(n) = tok.parse() {
            out.push(n);
        }
    }
    out
}

/// `<lyric>` children of a note. Text from several `<text>` elements (elided
/// syllables) is joined with a space.
fn parse_lyrics(note: &Element) -> Vec<Lyric> {
    let mut out: Vec<Lyric> = Vec::new();
    for l in note.children("lyric") {
        let text = l
            .children("text")
            .map(|t| t.text.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let extend = l.child("extend").is_some();
        if text.is_empty() && !extend {
            continue;
        }
        let verse = l
            .attr("number")
            .and_then(|n| n.trim().parse::<u32>().ok())
            .filter(|n| *n > 0)
            .unwrap_or_else(|| out.iter().map(|x| x.verse).max().unwrap_or(0) + 1);
        out.push(Lyric {
            verse,
            text,
            syllabic: l
                .child_text("syllabic")
                .map(Syllabic::parse)
                .unwrap_or_default(),
            extend,
        });
    }
    out
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
