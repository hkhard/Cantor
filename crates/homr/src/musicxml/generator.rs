//! homr `music_xml_generator.py` (`generate_xml`): MusicXML from the symbols
//! of each voice (part). Ported function by function, with the same names
//! where it helps to compare with homr.
use std::collections::{BTreeSet, HashMap};

use super::fraction::{median, Frac};
use super::symbols::{is_lower_position, sort_token_chords, Duration, Sym, EMPTY, NONOTE};
use super::xml::{El, Node};

/// homr `constants.duration_of_quarter`.
const DURATION_OF_QUARTER: i64 = 16;
/// Used for voices and stems while a part is built, removed at the end.
const POSITION_ATTRIBUTE: &str = "homr-position";

struct State {
    beats: i64,
    division: i64,
    nominator: Frac,
    tremolo_state: &'static str,
    volta_number: i64,
    last_volta_measure: i64,
}

impl State {
    fn start_volta(&mut self, measure_no: i64) -> i64 {
        if measure_no == self.last_volta_measure + 1 {
            self.volta_number += 1;
        } else {
            self.volta_number = 1;
        }
        self.volta_number
    }
    fn stop_volta(&mut self, measure_no: i64) -> i64 {
        self.last_volta_measure = measure_no;
        self.volta_number
    }
    fn toggle_tremolo_state(&mut self) -> &'static str {
        self.tremolo_state = if self.tremolo_state == "start" {
            "stop"
        } else {
            "start"
        };
        self.tremolo_state
    }
}

/// homr `SymbolChord` (`into_positions` is [`Chord::by_positions`]).
#[derive(Clone, Debug)]
struct Chord {
    symbols: Vec<Sym>,
    /// Tuplet start or stop by position, voices have their own tuplets.
    tuplet_marks: HashMap<String, String>,
}

impl Chord {
    fn new(symbols: Vec<Sym>) -> Chord {
        Chord {
            symbols,
            tuplet_marks: HashMap::new(),
        }
    }
    fn mark_for(&self, s: &Sym) -> String {
        self.tuplet_marks
            .get(&s.position)
            .cloned()
            .unwrap_or_default()
    }
    fn is_barline(&self) -> bool {
        self.symbols.first().is_some_and(Sym::is_barline_or_repeat)
    }
    fn get_duration(&self) -> Frac {
        self.symbols
            .iter()
            .filter(|s| s.is_note_or_rest())
            .map(Sym::fraction)
            .min()
            .unwrap_or(Frac::ZERO)
    }
    fn by_positions(&self) -> Vec<Chord> {
        let mut buckets: Vec<(String, Vec<Sym>)> = Vec::new();
        for s in &self.symbols {
            match buckets.iter_mut().find(|(p, _)| *p == s.position) {
                Some(b) => b.1.push(s.clone()),
                None => buckets.push((s.position.clone(), vec![s.clone()])),
            }
        }
        let mut chords: Vec<Chord> = buckets
            .into_iter()
            .map(|(_, symbols)| Chord {
                symbols,
                tuplet_marks: self.tuplet_marks.clone(),
            })
            .collect();
        // Voices of only rests go first, the last chord is the one which advances time.
        chords.sort_by_key(|c| !c.symbols.iter().all(|s| s.rhythm.starts_with("rest")));
        chords
    }
}

/// homr `generate_xml`, with an empty title and default arguments.
pub fn generate_xml(voices: &[Vec<Sym>]) -> El {
    let mut root = El::new("score-partwise").with_attr("version", "4.0");
    let mut work = El::new("work");
    work.sub_text("work-title", "");
    root.push(work);
    let mut ident = El::new("identification");
    ident.sub(El::new("encoding")).sub_text("software", "homr");
    root.push(ident);
    root.push(El::new("defaults"));
    let two: Vec<bool> = voices
        .iter()
        .map(|v| v.iter().any(|s| is_lower_position(&s.position)))
        .collect();
    root.push(build_part_list(&two));
    for (i, voice) in voices.iter().enumerate() {
        root.push(build_part(voice, i, two[i]));
    }
    root
}

fn part_id(index: usize) -> String {
    format!("P{}", index + 1)
}

fn build_part_list(two_staves: &[bool]) -> El {
    let mut list = El::new("part-list");
    for (part, &two) in two_staves.iter().enumerate() {
        let (name, sound, program) = if two {
            ("Piano", "keyboard.piano", 1)
        } else {
            ("Voice", "voice", 54)
        };
        let id = part_id(part);
        let sp = list.sub(El::new("score-part").with_attr("id", &id));
        sp.sub_text("part-name", name);
        let si = sp.sub(El::new("score-instrument").with_attr("id", &format!("{id}-I1")));
        si.sub_text("instrument-name", name);
        si.sub_text("instrument-sound", sound);
        let mi = sp.sub(El::new("midi-instrument").with_attr("id", &format!("{id}-I1")));
        mi.sub_text("midi-channel", (part + 1).to_string());
        mi.sub_text("midi-program", program.to_string());
        mi.sub_text("volume", "100");
        mi.sub_text("pan", "0");
    }
    list
}

fn build_part(voice: &[Sym], index: usize, two_staves: bool) -> El {
    let mut part = El::new("part").with_attr("id", &part_id(index));
    for m in build_measures(voice, two_staves) {
        part.push(m);
    }
    convert_ties(&mut part);
    add_stems(&mut part);
    for m in part.elements_mut() {
        for n in m.elements_mut().filter(|n| n.tag == "note") {
            n.remove_attr(POSITION_ATTRIBUTE);
        }
    }
    part
}

fn get_staff(s: &Sym) -> i64 {
    if is_lower_position(&s.position) {
        2
    } else {
        1
    }
}

fn duration_el(tag: &str, d: Frac, state: &State) -> El {
    let mut e = El::new(tag);
    e.sub_text(
        "duration",
        (d * Frac::int(state.division)).trunc().max(1).to_string(),
    );
    e
}

fn build_measure_rest(s: &Sym, state: &State) -> El {
    let mut note = El::new("note");
    note.sub(El::new("rest").with_attr("measure", "yes"));
    let d = (state.nominator * Frac::int(state.division)).trunc().max(1);
    note.sub_text("duration", d.to_string());
    note.sub_text("voice", "1");
    note.sub_text("staff", get_staff(s).to_string());
    note
}

/// The measure being built, with the index of the attributes element in use.
struct Measures {
    done: Vec<El>,
    current: El,
    number: i64,
}

fn attributes_index(m: &mut El, last: Option<usize>, force_new: bool) -> usize {
    match last {
        Some(i) if !force_new => i,
        _ => {
            m.push(El::new("attributes"));
            m.children.len() - 1
        }
    }
}

fn barline_index(m: &mut El, location: &str) -> usize {
    if let Some(i) = m.children.iter().position(
        |n| matches!(n, Node::El(e) if e.tag == "barline" && e.get("location") == Some(location)),
    ) {
        return i;
    }
    m.push(El::new("barline").with_attr("location", location));
    m.children.len() - 1
}

fn build_repeat(rhythm: &str, barline: &mut El) {
    if barline.find("repeat").is_some() {
        return;
    }
    let dir = if rhythm == "repeatStart" {
        "forward"
    } else {
        "backward"
    };
    barline.push(El::new("repeat").with_attr("direction", dir));
}

fn build_measures(voice: &[Sym], two_staves: bool) -> Vec<El> {
    let mut clock = GroupClock::default();
    let mut rest_measures = 0;
    let mut groups: Vec<Chord> = sort_token_chords(voice)
        .into_iter()
        .map(Chord::new)
        .collect();
    add_tuplet_start_stop(&mut groups);
    let groups = split_mixed_chords(groups);
    let (division, nominators) = find_division_and_time_signature_nominators(&groups);
    let mut state = State {
        beats: 4 * DURATION_OF_QUARTER,
        division,
        nominator: nominators[0],
        tremolo_state: "stop",
        volta_number: 1,
        last_volta_measure: -10,
    };
    let positions: &[&str] = if two_staves {
        &["upper", "lower"]
    } else {
        &["upper"]
    };
    let measure_rests = |m: &mut El, state: &State| {
        for (no, p) in positions.iter().enumerate() {
            if no > 0 {
                m.push(duration_el("backup", state.nominator, state));
            }
            m.push(build_measure_rest(
                &Sym::with_position("measureRest", p),
                state,
            ));
        }
    };
    let mut ms = Measures {
        done: Vec::new(),
        current: El::new("measure").with_attr("number", "1"),
        number: 1,
    };
    let close =
        |ms: &mut Measures, clock: &mut GroupClock, rest_measures: &mut i64, state: &State| {
            let mut m = std::mem::replace(&mut ms.current, El::new("measure"));
            rebalance_measure_voices(&mut m);
            ms.done.push(m);
            *clock = GroupClock::default();
            for _ in 0..*rest_measures {
                ms.number += 1;
                let mut m = El::new("measure").with_attr("number", &ms.number.to_string());
                measure_rests(&mut m, state);
                ms.done.push(m);
            }
            *rest_measures = 0;
        };
    let new_measure = |ms: &mut Measures| {
        ms.number += 1;
        ms.current = El::new("measure").with_attr("number", &ms.number.to_string());
    };

    let first = attributes_index(&mut ms.current, None, false);
    {
        let a = ms.current.child_mut(first);
        a.sub_text("divisions", (division / 4).to_string());
        if two_staves {
            a.sub_text("staves", "2");
            a.sub_text("part-symbol", "brace");
        }
    }
    let mut attributes: Option<usize> = Some(first);
    let mut time_signature_no = 0;
    for (group_no, group) in groups.iter().enumerate() {
        let symbol = &group.symbols[0];
        let rhythm = symbol.rhythm.as_str();
        let last = attributes.take();
        if symbol.is_note_or_rest() {
            if group.symbols.len() == 1 && rhythm.ends_with('m') {
                let a = attributes_index(&mut ms.current, last, false);
                attributes = Some(a);
                let duration = build_multi_measure_rest(symbol, ms.current.child_mut(a));
                if duration > 0 {
                    measure_rests(&mut ms.current, &state);
                    rest_measures = duration - 1;
                }
            } else {
                let staff_positions = group.by_positions();
                let gap = clock.wait_for_voices(group);
                if gap > Frac::ZERO {
                    ms.current.push(duration_el("forward", gap, &state));
                }
                let advance = clock.advance(group);
                let n = staff_positions.len();
                for (pos_no, sp) in staff_positions.iter().enumerate() {
                    let cd = if pos_no == n - 1 { advance } else { Frac::ZERO };
                    for e in build_note_chord(sp, &mut state, cd) {
                        ms.current.push(e);
                    }
                }
            }
            continue;
        }
        if rhythm == "measureRest" {
            let mut rests: Vec<&Sym> = group.symbols.iter().collect();
            rests.sort_by_key(|s| get_staff(s));
            for (no, r) in rests.into_iter().enumerate() {
                if no > 0 {
                    ms.current
                        .push(duration_el("backup", state.nominator, &state));
                }
                ms.current.push(build_measure_rest(r, &state));
            }
        } else if rhythm == "newline" {
            if group_no != groups.len() - 1 {
                ms.current
                    .push(El::new("print").with_attr("new-system", "yes"));
            }
        } else if rhythm.starts_with("clef") {
            let a = attributes_index(&mut ms.current, last, true);
            attributes = Some(a);
            for c in group
                .symbols
                .iter()
                .filter(|s| s.rhythm.starts_with("clef"))
            {
                let sign_line = c.rhythm.split('_').nth(1).unwrap_or("G2");
                let mut chars = sign_line.chars();
                let clef = ms
                    .current
                    .child_mut(a)
                    .sub(El::new("clef").with_attr("number", &get_staff(c).to_string()));
                clef.sub_text("sign", chars.next().map(String::from).unwrap_or_default());
                clef.sub_text("line", chars.next().map(String::from).unwrap_or_default());
            }
        } else if rhythm.starts_with("keySignature") {
            let a = attributes_index(&mut ms.current, last, false);
            attributes = Some(a);
            let key = ms.current.child_mut(a).sub(El::new("key"));
            key.sub_text("fifths", rhythm.split('_').nth(1).unwrap_or("0"));
        } else if rhythm.starts_with("timeSignature") {
            let a = attributes_index(&mut ms.current, last, false);
            attributes = Some(a);
            state.nominator = nominators[time_signature_no.min(nominators.len() - 1)];
            time_signature_no += 1;
            let denominator = rhythm.split('/').nth(1).unwrap_or("4");
            let beats = (state.nominator * Frac::int(denominator.parse().unwrap_or(4)))
                .trunc()
                .max(1);
            let time = ms.current.child_mut(a).sub(El::new("time"));
            time.sub_text("beats", beats.to_string());
            time.sub_text("beat-type", denominator);
            state.beats = beats;
        } else if rhythm.contains("barline") {
            if rhythm != "barline" {
                let b = barline_index(&mut ms.current, "right");
                let style = if rhythm == "bolddoublebarline" {
                    "heavy-heavy"
                } else {
                    "light-light"
                };
                ms.current.child_mut(b).sub_text("bar-style", style);
            }
            close(&mut ms, &mut clock, &mut rest_measures, &state);
            new_measure(&mut ms);
        } else if rhythm == "repeatStart" {
            close(&mut ms, &mut clock, &mut rest_measures, &state);
            new_measure(&mut ms);
            let b = barline_index(&mut ms.current, "right");
            build_repeat(rhythm, ms.current.child_mut(b));
        } else if rhythm == "repeatEnd" {
            let b = barline_index(&mut ms.current, "right");
            build_repeat(rhythm, ms.current.child_mut(b));
            close(&mut ms, &mut clock, &mut rest_measures, &state);
            new_measure(&mut ms);
        } else if rhythm == "repeatEndStart" {
            let b = barline_index(&mut ms.current, "right");
            build_repeat("repeatEnd", ms.current.child_mut(b));
            close(&mut ms, &mut clock, &mut rest_measures, &state);
            new_measure(&mut ms);
            let b = barline_index(&mut ms.current, "right");
            build_repeat("repeatStart", ms.current.child_mut(b));
        } else if rhythm.starts_with("voltaStart") {
            let n = state.start_volta(ms.number);
            let b = barline_index(&mut ms.current, "left");
            build_ending(rhythm, ms.current.child_mut(b), n);
        } else if rhythm.starts_with("voltaStop") || rhythm.starts_with("voltaDiscontinue") {
            let n = state.stop_volta(ms.number);
            let b = barline_index(&mut ms.current, "right");
            build_ending(rhythm, ms.current.child_mut(b), n);
        } else if rhythm.starts_with("dynamic") || symbol.articulation.starts_with("dynamic") {
            for d in &group.symbols {
                build_dynamic(d, &mut ms.current);
            }
        }
        // Other symbols aren't supported yet (homr prints a warning).
    }
    if !ms.current.children.is_empty() {
        close(&mut ms, &mut clock, &mut rest_measures, &state);
    }
    // A clef opens a second attributes element, so look at the whole first measure.
    let has_time = ms.done[0]
        .elements()
        .filter(|e| e.tag == "attributes")
        .any(|a| a.find("time").is_some());
    if !has_time {
        let beats = (state.nominator * Frac::int(4)).trunc().max(1);
        let time = ms.done[0].child_mut(first).sub(El::new("time"));
        time.sub_text("beats", beats.to_string());
        time.sub_text("beat-type", "4");
    }
    ms.done
}

fn build_ending(rhythm: &str, barline: &mut El, number: i64) {
    let kind = if rhythm.starts_with("voltaStart") {
        "start"
    } else if rhythm.starts_with("voltaStop") {
        "stop"
    } else {
        "discontinue"
    };
    barline.push(
        El::new("ending")
            .with_attr("type", kind)
            .with_attr("number", &number.to_string()),
    );
}

fn build_multi_measure_rest(s: &Sym, attributes: &mut El) -> i64 {
    if attributes.find("measure-style").is_some() {
        return 0;
    }
    let n: i64 = s
        .rhythm
        .split('_')
        .nth(1)
        .unwrap_or("1")
        .replace('m', "")
        .parse()
        .unwrap_or(1);
    attributes
        .sub(El::new("measure-style"))
        .sub_text("multiple-rest", n.to_string());
    n
}

fn build_dynamic(s: &Sym, measure: &mut El) {
    let mut marks: Vec<El> = Vec::new();
    if s.rhythm.starts_with("dynamic") {
        let name = s.rhythm.split_once('_').map(|x| x.1).unwrap_or("");
        if name != "empty" {
            marks.push(El::new(name));
        }
    }
    if s.articulation.starts_with("dynamic") {
        let text = s.articulation.split_once('_').map(|x| x.1).unwrap_or("");
        marks.push(El::new("other-dynamics").with_text(text.replace("piu", "più")));
    }
    if marks.is_empty() {
        return;
    }
    let direction = measure.sub(El::new("direction").with_attr("placement", "below"));
    let dynamics = direction
        .sub(El::new("direction-type"))
        .sub(El::new("dynamics"));
    for m in marks {
        dynamics.push(m);
    }
    direction.sub_text("staff", get_staff(s).to_string());
}

/// homr `GroupClock`: when the groups of a measure start.
#[derive(Default)]
struct GroupClock {
    time: Frac,
    sounding: Vec<Frac>,
    voice_free: HashMap<String, Frac>,
}

impl Default for Frac {
    fn default() -> Self {
        Frac::ZERO
    }
}

fn timed(group: &Chord) -> impl Iterator<Item = &Sym> {
    group
        .symbols
        .iter()
        .filter(|s| s.is_note_or_rest() && s.fraction() > Frac::ZERO)
}

impl GroupClock {
    fn wait_for_voices(&mut self, group: &Chord) -> Frac {
        let free = timed(group)
            .map(|s| {
                self.voice_free
                    .get(&s.position)
                    .copied()
                    .unwrap_or(Frac::ZERO)
            })
            .max()
            .unwrap_or(Frac::ZERO);
        let gap = (free - self.time).max(Frac::ZERO);
        self.time = self.time + gap;
        let t = self.time;
        self.sounding.retain(|&e| e > t);
        gap
    }

    fn advance(&mut self, group: &Chord) -> Frac {
        let mut shortest: Vec<(String, Frac)> = Vec::new();
        for s in timed(group) {
            let d = s.fraction();
            match shortest.iter_mut().find(|(p, _)| *p == s.position) {
                Some(e) => e.1 = e.1.min(d),
                None => shortest.push((s.position.clone(), d)),
            }
        }
        for (p, d) in shortest {
            self.voice_free.insert(p, self.time + d);
        }
        let advance = advance_to_next_group(group, self.time, &mut self.sounding);
        self.time = self.time + advance;
        let t = self.time;
        self.sounding.retain(|&e| e > t);
        advance
    }
}

fn advance_to_next_group(group: &Chord, clock: Frac, sounding: &mut Vec<Frac>) -> Frac {
    let durations: Vec<Frac> = group
        .symbols
        .iter()
        .filter(|s| s.is_note_or_rest())
        .map(Sym::fraction)
        .filter(|&d| d > Frac::ZERO)
        .collect();
    if durations.is_empty() {
        return Frac::ZERO;
    }
    sounding.extend(durations.iter().map(|&d| clock + d));
    sounding
        .iter()
        .copied()
        .filter(|&e| e > clock)
        .min()
        .unwrap()
        - clock
}

const LIFT_TO_ALTER: [(&str, i64); 5] = [("N", 0), ("#", 1), ("##", 2), ("b", -1), ("bb", -2)];

fn duration_name(kern: i64) -> &'static str {
    match kern {
        0 => "breve",
        1 => "whole",
        2 => "half",
        4 => "quarter",
        8 => "eighth",
        16 => "16th",
        32 => "32nd",
        64 => "64th",
        128 => "128th",
        // homr raises a KeyError here.
        _ => "quarter",
    }
}

fn build_articulations(note: &mut El, articulations: &str, tuplet_mark: &str, state: &mut State) {
    let mut notation = El::new("notations");
    let mut arts: Vec<El> = Vec::new();
    let mut orns: Vec<El> = Vec::new();
    if !articulations.starts_with("dynamic") {
        for a in articulations.split('_') {
            match a {
                "" | NONOTE => {}
                "fermata" | "arpeggiate" => notation.push(El::new(a)),
                "accent" | "mordent" | "staccato" | "staccatissimo" | "tenuto" | "caesura"
                | "doit" => arts.push(El::new(a)),
                "accentStrong" => arts.push(El::new("strong-accent")),
                "breathMark" => arts.push(El::new("breath-mark")),
                "tremolo" => orns.push(
                    El::new("tremolo")
                        .with_attr("type", state.toggle_tremolo_state())
                        .with_text("3"),
                ),
                "trill" => orns.push(El::new("trill-mark")),
                "turn" => orns.push(El::new("inverted-turn")),
                "slurStart" => notation.push(El::new("slur").with_attr("type", "start")),
                "slurStop" => notation.push(El::new("slur").with_attr("type", "stop")),
                "tieStart" => notation.push(El::new("tied").with_attr("type", "start")),
                "tieStop" => notation.push(El::new("tied").with_attr("type", "stop")),
                // homr raises a ValueError for an unsupported articulation.
                _ => {}
            }
        }
    }
    for mark in tuplet_mark.split('_').filter(|m| !m.is_empty()) {
        notation.push(El::new("tuplet").with_attr("type", mark));
    }
    if !arts.is_empty() {
        let p = notation.sub(El::new("articulations"));
        for a in arts {
            p.push(a);
        }
    }
    if !orns.is_empty() {
        let p = notation.sub(El::new("ornaments"));
        for o in orns {
            p.push(o);
        }
    }
    note.push(notation);
}

fn build_slurs(note: &mut El, slurs: &str, number: i64) {
    let n = number.to_string();
    let slur = |t: &str| El::new("slur").with_attr("type", t).with_attr("number", &n);
    let notation = note.find_mut("notations").expect("notations built first");
    match slurs {
        "slurStart" => notation.push(slur("start")),
        "slurStop" => notation.push(slur("stop")),
        "slurStart_slurStop" => {
            notation.push(slur("stop"));
            notation.push(slur("start"));
        }
        _ => {}
    }
}

/// Python's `round`: halves go to the even neighbour.
fn round_half_even(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 {
        (r - x.signum()) as i64
    } else {
        r as i64
    }
}

fn build_note_or_rest(
    s: &Sym,
    layer: i64,
    is_chord: bool,
    state: &mut State,
    tuplet_mark: &str,
) -> El {
    let mut note = El::new("note");
    if is_chord {
        note.push(El::new("chord"));
    }
    let d: Duration = s.duration();
    let grace = s.rhythm.contains('G');
    if grace {
        note.push(El::new("grace"));
    }
    if s.pitch == EMPTY {
        if d.fraction.numer() == 0 {
            note.push(El::new("rest").with_attr("measure", "yes"));
        } else {
            note.push(El::new("rest"));
        }
    } else if s.pitch == NONOTE {
        note.push(El::new("rest"));
    } else {
        let mut chars = s.pitch.chars();
        let pitch = note.sub(El::new("pitch"));
        pitch.sub_text("step", chars.next().map(String::from).unwrap_or_default());
        pitch.sub_text("octave", chars.next().map(String::from).unwrap_or_default());
        if s.lift != NONOTE && s.lift != EMPTY {
            if let Some((_, alter)) = LIFT_TO_ALTER.iter().find(|(l, _)| *l == s.lift) {
                pitch.sub_text("alter", alter.to_string());
            }
        }
    }
    if grace {
        note.sub_text("type", duration_name(d.kern));
    } else if d.fraction.numer() > 0 {
        let base = if d.kern == 0 { 1 } else { d.kern };
        let n = (d.fraction * Frac::int(state.division)).trunc().max(1);
        note.sub_text("duration", n.to_string());
        note.sub_text("type", duration_name(base));
    } else {
        note.sub_text("duration", state.beats.to_string());
        note.sub_text("type", duration_name(0));
    }
    for _ in 0..d.dots {
        note.push(El::new("dot"));
    }
    if d.is_tuplet() {
        let tm = note.sub(El::new("time-modification"));
        tm.sub_text("actual-notes", d.actual_notes.to_string());
        tm.sub_text("normal-notes", d.normal_notes.to_string());
    }
    let staff = get_staff(s);
    note.sub_text("voice", ((staff - 1) * 4 + layer + 1).to_string());
    note.sub_text("staff", staff.to_string());
    note.set(POSITION_ATTRIBUTE, &s.position);
    let mark = if d.is_tuplet() { tuplet_mark } else { "" };
    build_articulations(&mut note, &s.articulation, mark, state);
    build_slurs(&mut note, &s.slur, staff);
    if let Some((x, y)) = s.image_xy {
        note.children.push(Node::Comment(format!(
            " imgpos: {}, {} ",
            round_half_even(x),
            round_half_even(y)
        )));
    }
    note
}

/// homr `_group_notes`: the notes by duration, shortest first.
fn group_notes(notes: &[Sym]) -> Vec<(Frac, Vec<Sym>)> {
    let max = notes.iter().map(Sym::fraction).max().unwrap_or(Frac::ZERO);
    let mut groups: Vec<(Frac, Vec<Sym>)> = Vec::new();
    for n in notes {
        let f = if n.rhythm.contains('G') {
            Frac::ZERO
        } else if n.fraction().numer() == 0 {
            max
        } else {
            n.fraction()
        };
        match groups.iter_mut().find(|(k, _)| *k == f) {
            Some(g) => g.1.push(n.clone()),
            None => groups.push((f, vec![n.clone()])),
        }
    }
    groups.sort_by_key(|g| g.0);
    groups
}

fn build_note_chord(chord: &Chord, state: &mut State, chord_duration: Frac) -> Vec<El> {
    let by_duration = group_notes(&chord.symbols);
    let mut out = Vec::new();
    let count = by_duration.len();
    for (i, (gd, group)) in by_duration.iter().enumerate() {
        let is_rest = |n: &&Sym| n.pitch == EMPTY || n.pitch == NONOTE;
        let notes: Vec<&Sym> = group.iter().filter(|n| !is_rest(n)).collect();
        let rests: Vec<&Sym> = group.iter().filter(is_rest).collect();
        for (k, n) in notes.iter().enumerate() {
            out.push(build_note_or_rest(
                n,
                i as i64,
                k > 0,
                state,
                &chord.mark_for(n),
            ));
        }
        if !rests.is_empty() && *gd > Frac::ZERO {
            if !notes.is_empty() {
                out.push(duration_el("backup", *gd, state));
            }
            out.push(build_note_or_rest(
                rests[0],
                i as i64,
                false,
                state,
                &chord.mark_for(rests[0]),
            ));
        }
        if i != count - 1 && *gd > Frac::ZERO {
            out.push(duration_el("backup", *gd, state));
        }
    }
    let max = by_duration.last().map(|g| g.0).unwrap_or(Frac::ZERO);
    if chord_duration < max {
        out.push(duration_el("backup", max - chord_duration, state));
    }
    out
}

fn duration_text(e: &El) -> i64 {
    e.find("duration")
        .and_then(|d| d.text.as_deref())
        .and_then(|t| t.parse().ok())
        .unwrap_or(0)
}

fn is_second_voice(note: &El) -> bool {
    note.get(POSITION_ATTRIBUTE).unwrap_or("").ends_with('2')
}

/// homr `rebalance_measure_voices`: stable non-overlapping voices per staff.
fn rebalance_measure_voices(measure: &mut El) {
    struct Event {
        staff: i64,
        start: i64,
        end: i64,
        notes: Vec<usize>,
    }
    let mut events: Vec<Event> = Vec::new();
    let (mut current, mut last_start) = (0i64, 0i64);
    for (idx, node) in measure.children.iter().enumerate() {
        let Node::El(child) = node else { continue };
        match child.tag.as_str() {
            "backup" if child.find("duration").is_some() => current -= duration_text(child),
            "forward" if child.find("duration").is_some() => current += duration_text(child),
            "note" => {
                let duration = duration_text(child);
                let staff: i64 = child
                    .find("staff")
                    .and_then(|s| s.text.as_deref()?.parse().ok())
                    .unwrap_or(1);
                let chord = child.find("chord").is_some();
                let start = if chord { last_start } else { current };
                let end = start + duration;
                if chord
                    && events
                        .last()
                        .is_some_and(|e| e.staff == staff && e.start == start && e.end == end)
                {
                    events.last_mut().unwrap().notes.push(idx);
                } else {
                    if !chord {
                        last_start = start;
                        current += duration;
                    }
                    events.push(Event {
                        staff,
                        start,
                        end,
                        notes: vec![idx],
                    });
                }
            }
            _ => {}
        }
    }
    let staffs: BTreeSet<i64> = events.iter().map(|e| e.staff).collect();
    for staff in staffs {
        let mut on: Vec<&Event> = events.iter().filter(|e| e.staff == staff).collect();
        on.sort_by_key(|e| (e.start, e.end));
        let mut active: Vec<(i64, i64)> = Vec::new();
        let mut assigned: Vec<(Vec<usize>, i64)> = Vec::new();
        for e in on {
            active.retain(|&(end, _)| end > e.start);
            let used: BTreeSet<i64> = active.iter().map(|&(_, v)| v).collect();
            let mut voice = if is_second_voice(measure.child(e.notes[0])) {
                2
            } else {
                1
            };
            if used.contains(&voice) {
                voice = 1;
                while used.contains(&voice) {
                    voice += 1;
                }
            }
            active.push((e.end, voice));
            assigned.push((e.notes.clone(), (staff - 1) * 4 + voice));
        }
        for (notes, xml_voice) in assigned {
            for i in notes {
                if let Some(v) = measure.child_mut(i).find_mut("voice") {
                    v.text = Some(xml_voice.to_string());
                }
            }
        }
    }
}

/// homr `add_stems`: two voices on a staff in a measure, first up, second down.
fn add_stems(part: &mut El) {
    for m in part.elements_mut().filter(|m| m.tag == "measure") {
        let notes: Vec<usize> = m
            .children
            .iter()
            .enumerate()
            .filter(
                |(_, n)| matches!(n, Node::El(e) if e.tag == "note" && e.find("rest").is_none()),
            )
            .map(|(i, _)| i)
            .collect();
        let staffs: BTreeSet<String> = notes
            .iter()
            .map(|&i| m.child(i).findtext("staff", "1").to_string())
            .collect();
        for staff in staffs {
            let on: Vec<usize> = notes
                .iter()
                .copied()
                .filter(|&i| m.child(i).findtext("staff", "1") == staff)
                .collect();
            let voices: BTreeSet<bool> = on
                .iter()
                .map(|&i| m.child(i))
                .filter(|n| n.get(POSITION_ATTRIBUTE).is_some_and(|p| !p.is_empty()))
                .map(is_second_voice)
                .collect();
            if voices.len() < 2 {
                continue;
            }
            for i in on {
                let note = m.child_mut(i);
                let dir = if is_second_voice(note) { "down" } else { "up" };
                let at = note.index_of("staff").unwrap_or(note.children.len());
                note.children
                    .insert(at, Node::El(El::new("stem").with_text(dir)));
            }
        }
    }
}

fn note_pitch(note: &El) -> Option<String> {
    let p = note.find("pitch")?;
    Some(
        ["step", "alter", "octave"]
            .iter()
            .map(|t| p.findtext(t, ""))
            .collect::<Vec<_>>()
            .join("|"),
    )
}

/// (index of the notations element, index of the slur in it).
fn get_slur(note: &El, kind: &str) -> Option<(usize, usize)> {
    note.children
        .iter()
        .enumerate()
        .find_map(|(ni, n)| match n {
            Node::El(e) if e.tag == "notations" => e
                .children
                .iter()
                .position(
                    |s| matches!(s, Node::El(s) if s.tag == "slur" && s.get("type") == Some(kind)),
                )
                .map(|si| (ni, si)),
            _ => None,
        })
}

fn add_tie(note: &mut El, kind: &str, notation: usize) {
    note.child_mut(notation)
        .children
        .insert(0, Node::El(El::new("tied").with_attr("type", kind)));
    let at = note.index_of("duration").map(|i| i + 1).unwrap_or(0);
    note.children
        .insert(at, Node::El(El::new("tie").with_attr("type", kind)));
}

/// homr `convert_ties`: a slur from a note to the next note of its voice
/// with the same pitch is a tie.
fn convert_ties(part: &mut El) {
    let mut previous: HashMap<(String, String), Vec<(usize, usize)>> = HashMap::new();
    let measures: Vec<usize> = part
        .children
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n, Node::El(e) if e.tag == "measure"))
        .map(|(i, _)| i)
        .collect();
    for mi in measures {
        let mut events: Vec<Vec<(usize, usize)>> = Vec::new();
        for (ni, n) in part.child(mi).children.iter().enumerate() {
            let Node::El(e) = n else { continue };
            if e.tag != "note" {
                continue;
            }
            if e.find("chord").is_some() && !events.is_empty() {
                events.last_mut().unwrap().push((mi, ni));
            } else {
                events.push(vec![(mi, ni)]);
            }
        }
        for event in events {
            let first = part.child(event[0].0).child(event[0].1);
            let key = (
                first.findtext("staff", "1").to_string(),
                first.findtext("voice", "1").to_string(),
            );
            if let Some(before) = previous.insert(key, event.clone()) {
                tie_event(part, &before, &event);
            }
        }
    }
}

fn tie_event(part: &mut El, before: &[(usize, usize)], after: &[(usize, usize)]) {
    for &(sm, sn) in before {
        let start = part.child(sm).child(sn);
        let Some(begins) = get_slur(start, "start") else {
            continue;
        };
        let Some(pitch) = note_pitch(start) else {
            continue;
        };
        for &(em, en) in after {
            let stop = part.child(em).child(en);
            if note_pitch(stop).as_deref() != Some(pitch.as_str()) {
                continue;
            }
            let Some(ends) = get_slur(stop, "stop") else {
                continue;
            };
            {
                let s = part.child_mut(sm).child_mut(sn);
                s.child_mut(begins.0).children.remove(begins.1);
                add_tie(s, "start", begins.0);
            }
            {
                let e = part.child_mut(em).child_mut(en);
                e.child_mut(ends.0).children.remove(ends.1);
                add_tie(e, "stop", ends.0);
            }
            break;
        }
    }
}

fn find_common_division(durations: &[Frac]) -> i64 {
    let gcd = |mut a: i64, mut b: i64| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    };
    let ds: Vec<i64> = durations
        .iter()
        .filter(|d| **d > Frac::ZERO)
        .map(|d| d.denom())
        .collect();
    let Some(&first) = ds.first() else { return 1 };
    ds[1..]
        .iter()
        .fold(first, |c, &d| (c * d).abs() / gcd(c, d))
}

/// homr `find_division_and_time_signature_nominators`.
fn find_division_and_time_signature_nominators(voice: &[Chord]) -> (i64, Vec<Frac>) {
    let mut durations = vec![Frac::new(1, 4)];
    let mut in_measure = Frac::ZERO;
    let mut segments: Vec<Vec<Frac>> = vec![vec![]];
    let mut has_time = false;
    for chord in voice {
        if chord
            .symbols
            .first()
            .is_some_and(|s| s.rhythm.starts_with("timeSignature"))
        {
            if in_measure > Frac::ZERO {
                segments.last_mut().unwrap().push(in_measure);
                in_measure = Frac::ZERO;
            }
            if has_time {
                segments.push(vec![]);
            }
            has_time = true;
        } else if chord.is_barline() && in_measure > Frac::ZERO {
            segments.last_mut().unwrap().push(in_measure);
            in_measure = Frac::ZERO;
        } else {
            for s in chord.symbols.iter().filter(|s| s.is_note_or_rest()) {
                let f = s.fraction();
                if f > Frac::ZERO {
                    durations.push(f);
                }
            }
            let d = chord.get_duration();
            if d > Frac::ZERO {
                in_measure = in_measure + d;
            }
        }
    }
    if in_measure > Frac::ZERO {
        segments.last_mut().unwrap().push(in_measure);
    }
    let all: Vec<Frac> = segments.iter().flatten().copied().collect();
    if all.is_empty() {
        return (
            find_common_division(&durations),
            vec![Frac::int(1); segments.len()],
        );
    }
    let mut following = median(&all);
    let mut nominators: Vec<Frac> = segments
        .iter()
        .rev()
        .map(|seg| {
            if !seg.is_empty() {
                following = median(seg);
            }
            following
        })
        .collect();
    nominators.reverse();
    (find_common_division(&durations), nominators)
}

const BEFORE_NOTES_ORDER: [&str; 3] = ["clef", "keySignature", "timeSignature"];

/// homr `_split_mixed_chord`.
fn split_mixed_chord(symbols: &[Sym]) -> Vec<Vec<Sym>> {
    let notes: Vec<Sym> = symbols
        .iter()
        .filter(|s| s.is_note_or_rest())
        .cloned()
        .collect();
    if notes.is_empty() || notes.len() == symbols.len() {
        return vec![symbols.to_vec()];
    }
    let others: Vec<&Sym> = symbols.iter().filter(|s| !s.is_note_or_rest()).collect();
    let after: Vec<Sym> = others
        .iter()
        .filter(|s| s.is_barline_or_repeat())
        .map(|s| (*s).clone())
        .collect();
    let mut before: Vec<(String, Vec<Sym>)> = Vec::new();
    for s in others.iter().filter(|s| !after.contains(s)) {
        let kind = s.rhythm.split('_').next().unwrap_or("");
        let kind = kind.split('/').next().unwrap_or("").to_string();
        match before.iter_mut().find(|(k, _)| *k == kind) {
            Some(b) => b.1.push((*s).clone()),
            None => before.push((kind, vec![(*s).clone()])),
        }
    }
    let rank = |k: &str| {
        BEFORE_NOTES_ORDER
            .iter()
            .position(|x| *x == k)
            .unwrap_or(BEFORE_NOTES_ORDER.len())
    };
    before.sort_by_key(|(k, _)| rank(k));
    let mut out: Vec<Vec<Sym>> = before.into_iter().map(|(_, v)| v).collect();
    out.push(notes);
    out.extend(after.into_iter().map(|s| vec![s]));
    out
}

/// homr `split_mixed_chords`.
fn split_mixed_chords(groups: Vec<Chord>) -> Vec<Chord> {
    let mut result: Vec<Chord> = Vec::new();
    let mut split_barline = false;
    for group in groups {
        let parts = split_mixed_chord(&group.symbols);
        if split_barline && group.is_barline() && parts.len() == 1 {
            if group.symbols[0].rhythm == "barline"
                && result
                    .last()
                    .is_some_and(|r| r.symbols[0].rhythm != "barline")
            {
                split_barline = false;
                continue;
            }
            result.pop();
        }
        let n = parts.len();
        for part in parts {
            let is_notes = part[0].is_note_or_rest();
            result.push(Chord {
                symbols: part,
                tuplet_marks: if is_notes {
                    group.tuplet_marks.clone()
                } else {
                    HashMap::new()
                },
            });
        }
        split_barline = n > 1 && result.last().is_some_and(Chord::is_barline);
    }
    result
}

fn is_tuplet_symbol(s: &Sym) -> bool {
    s.is_note_or_rest() && !s.rhythm.contains('G') && s.duration().is_tuplet()
}

/// homr `TupletParser.parse`.
fn add_tuplet_start_stop(groups: &mut [Chord]) {
    let mut start = 0;
    let mut measures = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if g.is_barline() {
            measures.push(start..i + 1);
            start = i + 1;
        }
    }
    if start < groups.len() {
        measures.push(start..groups.len());
    }
    for range in measures {
        let m = &mut groups[range];
        let positions: BTreeSet<String> = m
            .iter()
            .flat_map(|g| g.symbols.iter())
            .filter(|s| is_tuplet_symbol(s))
            .map(|s| s.position.clone())
            .collect();
        for p in positions {
            add_tuplets(m, &p);
        }
    }
}

fn duration_at(group: &Chord, position: &str) -> Option<Duration> {
    let timed: Vec<&Sym> = group
        .symbols
        .iter()
        .filter(|s| s.position == position && s.is_note_or_rest() && !s.rhythm.contains('G'))
        .collect();
    timed
        .iter()
        .find(|s| is_tuplet_symbol(s))
        .or(timed.first())
        .map(|s| s.duration())
}

fn start_times(groups: &[Chord]) -> Vec<Frac> {
    let mut clock = GroupClock::default();
    groups
        .iter()
        .map(|g| {
            if g.symbols.iter().any(Sym::is_note_or_rest) {
                clock.wait_for_voices(g);
                let t = clock.time;
                clock.advance(g);
                t
            } else {
                clock.time
            }
        })
        .collect()
}

type Run = Vec<(usize, Frac, Frac)>;

fn add_tuplets(groups: &mut [Chord], position: &str) {
    let times = start_times(groups);
    let mut run: Run = Vec::new();
    let mut format = (1, 1);
    for index in 0..groups.len() {
        let Some(d) = duration_at(&groups[index], position) else {
            continue;
        };
        let current = (d.actual_notes, d.normal_notes);
        if current != format && !run.is_empty() {
            mark_run(groups, position, &run);
            run.clear();
        }
        format = current;
        if current.0 != current.1 {
            run.push((index, times[index], d.fraction));
        }
    }
    if !run.is_empty() {
        mark_run(groups, position, &run);
    }
}

fn is_power_of_two(v: i64) -> bool {
    v > 0 && v & (v - 1) == 0
}

fn split_run(run: &Run, by_end: bool) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut tuplets = Vec::new();
    let mut pending: Vec<usize> = Vec::new();
    let mut total = Frac::ZERO;
    for &(index, start, duration) in run {
        pending.push(index);
        total = total + duration;
        let measure = if by_end { start + duration } else { total };
        if is_power_of_two(measure.denom()) {
            tuplets.push((pending[0], index));
            pending.clear();
            total = Frac::ZERO;
        }
    }
    (tuplets, pending)
}

fn mark_run(groups: &mut [Chord], position: &str, run: &Run) {
    let by_length = split_run(run, false);
    let by_grid = split_run(run, true);
    let (mut tuplets, left_over) = if by_length.1.len() <= by_grid.1.len() {
        by_length
    } else {
        by_grid
    };
    if let (Some(&f), Some(&l)) = (left_over.first(), left_over.last()) {
        tuplets.push((f, l));
    }
    for (first, last) in tuplets {
        let marks = |g: &mut Chord, m: &str| {
            g.tuplet_marks.insert(position.to_string(), m.to_string());
        };
        if first == last {
            marks(&mut groups[first], "start_stop");
        } else {
            marks(&mut groups[first], "start");
            marks(&mut groups[last], "stop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(t: &[&str]) -> Vec<Sym> {
        t.iter().map(|s| Sym::parse(s)).collect()
    }

    fn xml(v: &[Sym]) -> String {
        let mut s = String::new();
        generate_xml(&[v.to_vec()]).write(&mut s);
        s
    }

    #[test]
    fn lib_06_triplet_gets_time_modification_and_brackets() {
        let v = voice(&[
            "clef_G2 _ _ _ _ upper",
            "timeSignature/4 . . . . .",
            "note_12 C5 _ _ _ upper",
            "note_12 D5 _ _ _ upper",
            "note_12 E5 _ _ _ upper",
            "note_4 F5 _ _ _ upper",
            "barline . . . . .",
        ]);
        let s = xml(&v);
        assert_eq!(s.matches("<time-modification>").count(), 3);
        assert!(s.contains("<tuplet type=\"start\" />"));
        assert!(s.contains("<tuplet type=\"stop\" />"));
        assert!(s.contains("<divisions>3</divisions>"));
        assert!(s.contains("<beats>2</beats><beat-type>4</beat-type>"));
    }

    #[test]
    fn lib_06_slur_on_same_pitch_becomes_tie_but_not_on_other_pitch() {
        let v = voice(&[
            "note_4 C5 _ _ slurStart upper",
            "note_4 C5 _ _ slurStop upper",
            "note_4 D5 _ _ slurStart upper",
            "note_4 E5 _ _ slurStop upper",
            "barline . . . . .",
        ]);
        let s = xml(&v);
        assert_eq!(s.matches("<tie type=").count(), 2);
        assert_eq!(s.matches("<slur type=").count(), 2);
    }

    #[test]
    fn lib_06_barlines_repeats_voltas_dynamics_multirest() {
        let v = voice(&[
            "clef_F4 _ _ _ _ upper",
            "keySignature_2 . . . . .",
            "dynamic_f _ _ dynamic_piu _ upper",
            "note_2 C3 # accent_staccato _ upper",
            "note_2 D3 _ fermata _ upper",
            "repeatEnd . . . . .",
            "voltaStart . . . . .",
            "rest_1 _ _ _ _ upper",
            "voltaStop . . . . .",
            "bolddoublebarline . . . . .",
            "rest_3m _ _ _ _ upper",
            "barline . . . . .",
        ]);
        let s = xml(&v);
        assert!(s.contains("<repeat direction=\"backward\" />"));
        assert!(s.contains("<ending type=\"start\" number=\"1\" />"));
        assert!(s.contains("<bar-style>heavy-heavy</bar-style>"));
        assert!(s.contains("<f /><other-dynamics>più</other-dynamics>"));
        assert!(s.contains("<articulations><accent /><staccato /></articulations>"));
        assert!(s.contains("<fermata />"));
        assert!(s.contains("<multiple-rest>3</multiple-rest>"));
        assert!(s.contains("<alter>1</alter>"));
        // The multi-measure rest is three measures.
        assert!(s.contains("<measure number=\"5\">"));
    }

    #[test]
    fn lib_06_two_voices_on_a_staff_get_stems() {
        let v = voice(&[
            "note_4 C5 _ _ _ upper",
            "chord",
            "note_4 C4 _ _ _ upper2",
            "barline . . . . .",
        ]);
        let s = xml(&v);
        assert!(s.contains("<stem>up</stem>"));
        assert!(s.contains("<stem>down</stem>"));
        assert!(!s.contains(POSITION_ATTRIBUTE));
    }

    #[test]
    fn lib_06_mixed_chord_splits_clef_before_notes() {
        let out = split_mixed_chord(&voice(&["note_4 C4 _ _ _ upper", "clef_F4 _ _ _ _ lower"]));
        assert_eq!(out.len(), 2);
        assert!(out[0][0].rhythm.starts_with("clef"));
    }

    #[test]
    fn lib_06_round_half_even_like_python() {
        assert_eq!(round_half_even(2.5), 2);
        assert_eq!(round_half_even(3.5), 4);
        assert_eq!(round_half_even(3.4), 3);
    }
}
