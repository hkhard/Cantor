//! Mixer channels for playback (PLAY-03/05/09).
//!
//! Choir scores, especially OMR output, rarely say who sings what: parts are
//! called "Voice", two singers often share a staff as voices 1 and 2, and OCR
//! garbles names ("TenorZ", "Tent or"). This module turns parts and voices
//! into one channel per sung line, recognises the line's role (S/A/T/B, with
//! divisi numbers) from the part name or, failing that, from the pitch range
//! of the whole choir, and gives every line a distinct, separable timbre.
//!
//! Timbre palette (General MIDI, 0-based), chosen so neighbouring lines come
//! from different instrument families and stay easy to tell apart:
//!
//! | Line | Program |
//! | --- | --- |
//! | Soprano (1) | 73 Flute |
//! | Soprano 2 | 68 Oboe |
//! | Alto (1) | 71 Clarinet |
//! | Alto 2 | 65 Alto Sax |
//! | Tenor (1) | 56 Trumpet |
//! | Tenor 2 | 60 French Horn |
//! | Bass (1) | 42 Cello |
//! | Bass 2 | 70 Bassoon |
//! | Single unnamed voice | 73 Flute |
//!
//! Instrument parts keep their `<midi-program>` (or a guess from the name).

use crate::smf::program_for;
use crate::{TimedNote, Timemap};
use cantor_score::Score;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Role {
    Soprano,
    Alto,
    Tenor,
    Bass,
    /// The only sung line, with no hint who sings it.
    Voice,
    Piano,
    Instrument,
}

impl Role {
    fn name(self) -> &'static str {
        match self {
            Role::Soprano => "Soprano",
            Role::Alto => "Alto",
            Role::Tenor => "Tenor",
            Role::Bass => "Bass",
            Role::Voice => "Voice",
            Role::Piano => "Piano",
            Role::Instrument => "Instrument",
        }
    }

    fn is_vocal(self) -> bool {
        matches!(
            self,
            Role::Soprano | Role::Alto | Role::Tenor | Role::Bass | Role::Voice
        )
    }

    /// Typical sounding median pitch, used when only some lines are known.
    fn typical_median(self) -> f64 {
        match self {
            Role::Soprano => 71.0,
            Role::Alto => 64.0,
            Role::Tenor => 59.0,
            _ => 50.0,
        }
    }
}

/// Default timbre for a sung line (see module docs). Lines beyond the second
/// of a role take further instruments of a related colour.
pub fn voice_program(role: Role, number: u8) -> u8 {
    let options = role_palette(role);
    let i = (number.max(1) as usize - 1) % options.len();
    options[i]
}

fn role_palette(role: Role) -> &'static [u8] {
    match role {
        Role::Soprano => &[73, 68, 72, 79, 13],
        Role::Alto => &[71, 65, 74, 66, 11],
        Role::Tenor => &[56, 60, 57, 61, 69],
        Role::Bass => &[42, 70, 58, 32, 43],
        _ => &[73, 71, 56, 42],
    }
}

/// Choir type, from the title or chosen by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choir {
    Male,
    Female,
    Mixed,
}

/// Read the choir type from a score title ("TTBB", "Manskör", "SSA", "SATB").
pub fn choir_from_title(title: &str) -> Option<Choir> {
    let lower = title.to_lowercase();
    if [
        "manskör",
        "manskor",
        "male choir",
        "men's choir",
        "männerchor",
    ]
    .iter()
    .any(|k| lower.contains(k))
    {
        return Some(Choir::Male);
    }
    if [
        "damkör",
        "damkor",
        "female choir",
        "women's choir",
        "frauenchor",
    ]
    .iter()
    .any(|k| lower.contains(k))
    {
        return Some(Choir::Female);
    }
    for w in title.split(|c: char| !c.is_alphanumeric()) {
        if w.len() < 2 || !w.chars().all(|c| "SATB".contains(c)) {
            continue;
        }
        let has = |c| w.contains(c);
        return Some(match (has('S') || has('A'), has('T') || has('B')) {
            (false, true) => Choir::Male,
            (true, false) => Choir::Female,
            _ => Choir::Mixed,
        });
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    /// Shown in the mixer: "Tenor 1", "Alto", "Piano".
    pub label: String,
    pub role: Role,
    /// Divisi number (1, 2, ...) when a role has several lines, else 0.
    pub number: u8,
    /// General MIDI program, 0-based.
    pub program: u8,
    /// -1 (left) .. 1 (right).
    pub pan: f32,
    /// Part this channel plays from (index into `Score::parts`).
    pub part: usize,
    /// Median sounding pitch (MIDI) of its notes.
    pub median_pitch: f64,
    pub note_count: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChannelPlan {
    pub channels: Vec<Channel>,
    /// (part, staff, voice) -> channel index.
    pub routes: BTreeMap<(usize, u32, u32), usize>,
}

impl ChannelPlan {
    /// Channel that plays this note.
    pub fn channel_of(&self, n: &TimedNote) -> usize {
        self.routes
            .get(&(n.part, n.staff, n.voice))
            .copied()
            .or_else(|| self.channels.iter().position(|c| c.part == n.part))
            .unwrap_or(0)
    }
}

/// A voice with fewer notes than this share of its part's busiest voice is
/// an OMR artefact (stray chord tone, cue note) and joins the nearest line.
const MINOR_VOICE_SHARE: f64 = 0.4;
const MIN_LINE_NOTES: usize = 4;

/// Role hint from a part name, tolerant of abbreviations, Swedish and OCR noise.
fn role_from_name(name: &str) -> Option<Role> {
    let n = name.to_lowercase();
    let words: Vec<&str> = n
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    let starts = |prefixes: &[&str]| {
        words
            .iter()
            .any(|w| prefixes.iter().any(|p| w.starts_with(p)))
    };
    let exact = |ws: &[&str]| words.iter().any(|w| ws.contains(w));
    if starts(&["sop", "dis"]) || exact(&["s"]) {
        Some(Role::Soprano)
    } else if starts(&["alt", "mezzo"]) || exact(&["a"]) {
        Some(Role::Alto)
    } else if starts(&["ten"]) || exact(&["t"]) {
        Some(Role::Tenor)
    } else if starts(&["bas", "bar", "basso"]) || exact(&["b"]) {
        Some(Role::Bass)
    } else {
        None
    }
}

fn is_vocal_program(p: Option<u8>) -> bool {
    p.is_none_or(|p| (52..=54).contains(&p))
}

/// Voice labels when a part name is a list of them, one per line or
/// separated by commas/slashes ("T1\nT2\nB1\nB2", "S/A", "Tenor 1, Tenor 2").
/// MuseScore choir arrangements often keep the default Piano instrument and
/// name the part this way. Returns `(role, divisi number)` per label, or
/// `None` when any piece is not a voice label.
fn voice_labels(name: &str) -> Option<Vec<(Role, u8)>> {
    let pieces: Vec<&str> = name
        .split(['\n', ',', '/', '&', '+'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if pieces.is_empty() {
        return None;
    }
    pieces
        .iter()
        .map(|p| {
            let role = role_from_name(p)?;
            let number = p
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            Some((role, number))
        })
        .collect()
}

fn instrument_role(name: &str, explicit: Option<u8>) -> Option<(Role, u8)> {
    if voice_labels(name).is_some() {
        return None; // sung lines, whatever the instrument says
    }
    let generic = program_for(name, None);
    let named_instrument = generic != 52;
    if !named_instrument && is_vocal_program(explicit) {
        return None;
    }
    let prog = program_for(name, explicit);
    let role = if prog < 8 {
        Role::Piano
    } else {
        Role::Instrument
    };
    Some((role, prog))
}

fn median(v: &mut [u8]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_unstable();
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m] as f64
    } else {
        (v[m - 1] as f64 + v[m] as f64) / 2.0
    }
}

/// One sung line before roles are assigned.
struct Line {
    part: usize,
    keys: Vec<(u32, u32)>,
    pitches: Vec<u8>,
    median: f64,
    hint: Option<Role>,
    /// Divisi number given by the part name ("T2"), when names say so.
    number: Option<u8>,
}

/// Plan mixer channels for a score.
pub fn plan_channels(score: &Score, tm: &Timemap) -> ChannelPlan {
    plan_channels_for(score, tm, None)
}

/// Plan mixer channels, with the choir type when known (title or user choice).
pub fn plan_channels_for(score: &Score, tm: &Timemap, choir: Option<Choir>) -> ChannelPlan {
    // Pitches per (part, staff, voice), in first-appearance order.
    let mut by_voice: Vec<BTreeMap<(u32, u32), Vec<u8>>> = vec![BTreeMap::new(); score.parts.len()];
    for n in &tm.notes {
        if let Some(m) = by_voice.get_mut(n.part) {
            m.entry((n.staff, n.voice)).or_default().push(n.pitch);
        }
    }

    enum Slot {
        Inst(Channel, Vec<(u32, u32)>),
        Lines(Vec<usize>),
    }
    let mut slots: Vec<Slot> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();

    for (pi, part) in score.parts.iter().enumerate() {
        let voices = &by_voice[pi];
        let count: usize = voices.values().map(Vec::len).sum();
        if count == 0 {
            continue;
        }
        if let Some((role, program)) = instrument_role(&part.name, part.midi_program) {
            let mut all: Vec<u8> = voices.values().flatten().copied().collect();
            let label = if part.name.trim().is_empty() {
                role.name().to_string()
            } else {
                part.name.trim().to_string()
            };
            slots.push(Slot::Inst(
                Channel {
                    label,
                    role,
                    number: 0,
                    program,
                    pan: 0.0,
                    part: pi,
                    median_pitch: median(&mut all),
                    note_count: count,
                },
                voices.keys().copied().collect(),
            ));
            continue;
        }

        // Substantive voices become lines; minor ones join the nearest line.
        let busiest = voices.values().map(Vec::len).max().unwrap_or(0);
        let mut main: Vec<((u32, u32), Vec<u8>)> = Vec::new();
        let mut minor: Vec<((u32, u32), Vec<u8>)> = Vec::new();
        for (k, v) in voices {
            let share = v.len() as f64 / busiest.max(1) as f64;
            if v.len() >= MIN_LINE_NOTES && share >= MINOR_VOICE_SHARE {
                main.push((*k, v.clone()));
            } else {
                minor.push((*k, v.clone()));
            }
        }
        if main.is_empty() {
            main = std::mem::take(&mut minor);
            let keys: Vec<(u32, u32)> = main.iter().map(|(k, _)| *k).collect();
            let pitches: Vec<u8> = main.into_iter().flat_map(|(_, v)| v).collect();
            main = vec![(keys[0], pitches)];
            // Remember every key on the merged line.
            let mut tmp_line = Line {
                part: pi,
                keys,
                pitches: main[0].1.clone(),
                median: 0.0,
                hint: role_from_name(&part.name),
                number: None,
            };
            tmp_line.median = median(&mut tmp_line.pitches.clone());
            slots.push(Slot::Lines(vec![lines.len()]));
            lines.push(tmp_line);
            continue;
        }
        let mut part_lines: Vec<Line> = main
            .into_iter()
            .map(|(k, v)| {
                let mut p = v.clone();
                Line {
                    part: pi,
                    keys: vec![k],
                    median: median(&mut p),
                    pitches: v,
                    hint: role_from_name(&part.name),
                    number: None,
                }
            })
            .collect();
        for (k, mut v) in minor {
            let m = median(&mut v);
            let target = part_lines
                .iter_mut()
                .min_by(|a, b| {
                    (a.median - m)
                        .abs()
                        .partial_cmp(&(b.median - m).abs())
                        .unwrap()
                        .then(b.median.partial_cmp(&a.median).unwrap())
                })
                .expect("at least one line");
            target.keys.push(k);
            target.pitches.extend(v);
        }
        // Top line first within a staff group.
        part_lines.sort_by(|a, b| b.median.partial_cmp(&a.median).unwrap());
        // A name listing one label per line ("T1\nT2\nB1\nB2") labels the
        // lines top to bottom.
        // A name may repeat a label ("T1\nT1\nT2"); then its distinct labels
        // in order are tried.
        let named = voice_labels(&part.name).and_then(|l| {
            if l.len() == part_lines.len() {
                return Some(l);
            }
            let mut d: Vec<(Role, u8)> = Vec::new();
            for x in l {
                if !d.contains(&x) {
                    d.push(x);
                }
            }
            (d.len() == part_lines.len()).then_some(d)
        });
        if let Some(labels) = named {
            for (line, (role, number)) in part_lines.iter_mut().zip(labels) {
                line.hint = Some(role);
                line.number = (number > 0).then_some(number);
            }
        }
        let idx: Vec<usize> = (lines.len()..lines.len() + part_lines.len()).collect();
        lines.extend(part_lines);
        slots.push(Slot::Lines(idx));
    }

    let roles = assign_roles(&lines, choir);

    // Divisi numbers in score order.
    let mut totals: BTreeMap<Role, u8> = BTreeMap::new();
    for r in &roles {
        *totals.entry(*r).or_default() += 1;
    }
    let mut seen: BTreeMap<Role, u8> = BTreeMap::new();
    let mut numbers = vec![0u8; lines.len()];
    for slot in &slots {
        if let Slot::Lines(ix) = slot {
            for &i in ix {
                let r = roles[i];
                if let Some(n) = lines[i].number {
                    numbers[i] = n;
                    seen.insert(r, n.max(seen.get(&r).copied().unwrap_or(0)));
                } else if totals[&r] > 1 {
                    let c = seen.entry(r).or_default();
                    *c += 1;
                    numbers[i] = *c;
                }
            }
        }
    }

    let mut plan = ChannelPlan::default();
    for slot in slots {
        match slot {
            Slot::Inst(ch, keys) => {
                let ci = plan.channels.len();
                for k in keys {
                    plan.routes.insert((ch.part, k.0, k.1), ci);
                }
                plan.channels.push(ch);
            }
            Slot::Lines(ix) => {
                for i in ix {
                    let l = &lines[i];
                    let (role, number) = (roles[i], numbers[i]);
                    let label = if number > 0 {
                        format!("{} {number}", role.name())
                    } else {
                        role.name().to_string()
                    };
                    let ci = plan.channels.len();
                    for k in &l.keys {
                        plan.routes.insert((l.part, k.0, k.1), ci);
                    }
                    plan.channels.push(Channel {
                        label,
                        role,
                        number,
                        program: voice_program(role, number),
                        pan: 0.0,
                        part: l.part,
                        median_pitch: l.median,
                        note_count: l.pitches.len(),
                    });
                }
            }
        }
    }

    // Every label must be unique. When two parts claim the same voice (a
    // lead "T1 T1 T2" part above a "T1 T2 B1 B2" choir), the part with more
    // lines keeps the plain names and the other is marked as the lead:
    // "Tenor 1 (solo)".
    let mut by_label: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, c) in plan.channels.iter().enumerate() {
        by_label.entry(c.label.clone()).or_default().push(i);
    }
    let lines_in_part =
        |p: usize, plan: &ChannelPlan| plan.channels.iter().filter(|c| c.part == p).count();
    let mut rename: Vec<(usize, String)> = Vec::new();
    for (label, idx) in &by_label {
        if idx.len() < 2 {
            continue;
        }
        let keep = *idx
            .iter()
            .max_by_key(|&&i| {
                (
                    lines_in_part(plan.channels[i].part, &plan),
                    std::cmp::Reverse(i),
                )
            })
            .expect("non-empty");
        for (k, &i) in idx.iter().filter(|&&i| i != keep).enumerate() {
            let suffix = if k == 0 {
                "solo".to_string()
            } else {
                format!("solo {}", k + 1)
            };
            rename.push((i, format!("{label} ({suffix})")));
        }
    }
    for (i, l) in rename {
        plan.channels[i].label = l;
    }

    // Every channel must sound different: resolve any clash with the next
    // unused instrument from the role's palette, then from the voice set.
    let mut used: Vec<u8> = Vec::new();
    for c in &mut plan.channels {
        if used.contains(&c.program) && c.role.is_vocal() {
            let pal = role_palette(c.role);
            if let Some(p) = pal
                .iter()
                .chain(
                    [
                        73u8, 71, 56, 42, 68, 65, 60, 70, 72, 74, 57, 58, 40, 41, 21, 22,
                    ]
                    .iter(),
                )
                .find(|p| !used.contains(p))
            {
                c.program = *p;
            }
        }
        used.push(c.program);
    }

    // Spread sung lines across the stereo field, top voice left.
    let vocal: Vec<usize> = (0..plan.channels.len())
        .filter(|&i| plan.channels[i].role.is_vocal())
        .collect();
    if vocal.len() > 1 {
        let n = vocal.len() as f32;
        for (k, &i) in vocal.iter().enumerate() {
            plan.channels[i].pan = -0.4 + 0.8 * k as f32 / (n - 1.0);
        }
    }
    plan
}

/// Roles for sung lines: the name wins; otherwise the choir type (male,
/// female, mixed) is read from the range of all lines and unknown lines are
/// assigned by rank, top to bottom.
fn assign_roles(lines: &[Line], choir: Option<Choir>) -> Vec<Role> {
    if lines.is_empty() {
        return Vec::new();
    }
    if lines.len() == 1 && lines[0].hint.is_none() && choir.is_none() {
        return vec![Role::Voice];
    }
    let top = lines.iter().map(|l| l.median).fold(f64::MIN, f64::max);
    let bottom = lines.iter().map(|l| l.median).fold(f64::MAX, f64::min);
    let choir = choir.unwrap_or(if top <= 66.0 {
        Choir::Male
    } else if bottom >= 60.0 {
        Choir::Female
    } else {
        Choir::Mixed
    });
    let palette: &[Role] = match choir {
        Choir::Male => &[Role::Tenor, Role::Bass],
        Choir::Female => &[Role::Soprano, Role::Alto],
        Choir::Mixed => &[Role::Soprano, Role::Alto, Role::Tenor, Role::Bass],
    };
    // A name hint outside the choir type is OCR noise ("I 8" read as alto).
    let hint = |l: &Line| l.hint.filter(|h| palette.contains(h));

    let unknown: Vec<usize> = (0..lines.len())
        .filter(|&i| hint(&lines[i]).is_none())
        .collect();
    let mut roles: Vec<Role> = lines
        .iter()
        .map(|l| hint(l).unwrap_or(Role::Voice))
        .collect();
    if unknown.len() == lines.len() && lines.len() >= palette.len() {
        // Rank: split the lines, highest first, evenly over the palette.
        let mut order = unknown.clone();
        order.sort_by(|&a, &b| lines[b].median.partial_cmp(&lines[a].median).unwrap());
        let n = order.len();
        for (rank, &i) in order.iter().enumerate() {
            roles[i] = palette[rank * palette.len() / n];
        }
    } else {
        for &i in &unknown {
            roles[i] = *palette
                .iter()
                .min_by(|a, b| {
                    (a.typical_median() - lines[i].median)
                        .abs()
                        .partial_cmp(&(b.typical_median() - lines[i].median).abs())
                        .unwrap()
                })
                .unwrap();
        }
    }
    roles
}
