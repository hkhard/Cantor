//! homr `triplet_restoration.py`: restores triplets printed without their "3".
//!
//! A measure whose voices do not carry on where their previous notes ended
//! (timed the way the MusicXML generator times it) is tried with three notes
//! in a row of one plain value taken as a triplet, the most helpful first, as
//! long as each leaves fewer such places. A page (one voice of it) must show
//! a note value as triplets in at least three places for it to count.
use std::collections::{BTreeSet, HashMap};

use super::fraction::Frac;
use super::symbols::{sort_token_chords_indexed, Sym};

/// A note or rest of a measure: its position, duration and index in the symbols.
type Note = (String, Frac, usize);
type Measure = Vec<Vec<Note>>;
type Indices = BTreeSet<usize>;
/// The places of the chords and the note value, then the indices and if off the beat.
type ThreesByPlace = Vec<((Vec<usize>, String), (Indices, bool))>;

const MAX_THREES_IN_A_ROW: usize = 4;
const MIN_TRIPLETS_ON_A_PAGE: usize = 3;
const MAX_MEASURE_LENGTH: i64 = 2;

/// `(note|rest)_(4|8|16|32|64)$`: the plain value, if any.
fn plain_value(rhythm: &str) -> Option<&str> {
    let (kind, value) = rhythm.split_once('_')?;
    if (kind == "note" || kind == "rest") && matches!(value, "4" | "8" | "16" | "32" | "64") {
        Some(value)
    } else {
        None
    }
}

fn as_triplet(s: &Sym) -> Sym {
    match plain_value(&s.rhythm) {
        Some(v) => {
            let mut r = s.clone();
            let kind = &s.rhythm[..4];
            r.rhythm = format!("{kind}_{}", v.parse::<i64>().unwrap() * 3 / 2);
            r
        }
        None => s.clone(),
    }
}

fn split_into_measures(symbols: &[Sym]) -> Vec<Measure> {
    let mut measures: Vec<Measure> = vec![vec![]];
    for chord in sort_token_chords_indexed(symbols) {
        let first = &symbols[chord[0]];
        if first.is_barline_or_repeat() {
            measures.push(vec![]);
        } else if first.is_note_or_rest() && !first.rhythm.ends_with('m') {
            measures.last_mut().unwrap().push(
                chord
                    .iter()
                    .filter(|&&i| symbols[i].is_note_or_rest())
                    .map(|&i| (symbols[i].position.clone(), symbols[i].fraction(), i))
                    .collect(),
            );
        }
    }
    measures
}

/// The start of every chord with the position and end of each of its notes.
fn play(measure: &Measure, triplets: &Indices) -> Vec<(Frac, Vec<(String, Frac)>)> {
    let mut clock = Frac::ZERO;
    let mut sounding: Vec<Frac> = Vec::new();
    let mut out = Vec::new();
    for chord in measure {
        let mut notes = Vec::new();
        for (voice, duration, index) in chord {
            let played = if triplets.contains(index) {
                *duration * Frac::int(2) / Frac::int(3)
            } else {
                *duration
            };
            if played > Frac::ZERO {
                notes.push((voice.clone(), clock + played));
            }
        }
        out.push((clock, notes.clone()));
        if !notes.is_empty() {
            sounding.extend(notes.iter().map(|(_, e)| *e));
            clock = sounding
                .iter()
                .copied()
                .filter(|&e| e > clock)
                .min()
                .unwrap();
            sounding.retain(|&e| e > clock);
        }
    }
    out
}

fn count_breaks(measure: &Measure, triplets: &Indices) -> (usize, Frac) {
    let mut end_of_voice: Vec<(String, Frac)> = Vec::new();
    let mut breaks = 0;
    let mut size = Frac::ZERO;
    for (start, notes) in play(measure, triplets) {
        let mut ends: Vec<(String, Frac)> = Vec::new();
        for (voice, end) in notes {
            match ends.iter_mut().find(|(v, _)| *v == voice) {
                Some(e) => e.1 = e.1.max(end),
                None => ends.push((voice, end)),
            }
        }
        for (voice, _) in &ends {
            if let Some((_, prev)) = end_of_voice.iter().find(|(v, _)| v == voice) {
                if *prev != start {
                    breaks += 1;
                    size = size
                        + if start > *prev {
                            start - *prev
                        } else {
                            *prev - start
                        };
                }
            }
        }
        for (voice, end) in ends {
            match end_of_voice.iter_mut().find(|(v, _)| *v == voice) {
                Some(e) => e.1 = end,
                None => end_of_voice.push((voice, end)),
            }
        }
    }
    if let Some(end_of_measure) = end_of_voice.iter().map(|(_, e)| *e).max() {
        for (_, end) in &end_of_voice {
            if *end != end_of_measure {
                breaks += 1;
                size = size + (end_of_measure - *end);
            }
        }
    }
    (breaks, size)
}

fn get_length(measure: &Measure) -> Frac {
    play(measure, &Indices::new())
        .into_iter()
        .flat_map(|(_, notes)| notes.into_iter().map(|(_, e)| e))
        .max()
        .unwrap_or(Frac::ZERO)
}

fn upsert(map: &mut ThreesByPlace, key: (Vec<usize>, String), value: (Indices, bool)) {
    match map.iter_mut().find(|(k, _)| *k == key) {
        Some(e) => e.1 = value,
        None => map.push((key, value)),
    }
}

fn find_single_threes(measure: &Measure, symbols: &[Sym]) -> ThreesByPlace {
    let mut found: ThreesByPlace = Vec::new();
    let voices: BTreeSet<&String> = measure.iter().flatten().map(|(v, _, _)| v).collect();
    for voice in voices {
        let chords: Vec<(usize, Vec<usize>)> = measure
            .iter()
            .enumerate()
            .map(|(place, chord)| {
                let notes = chord
                    .iter()
                    .filter(|(v, _, _)| v == voice)
                    .map(|(_, _, i)| *i)
                    .collect::<Vec<_>>();
                (place, notes)
            })
            .filter(|(_, notes)| !notes.is_empty())
            .collect();
        let values: Vec<Option<&str>> = chords
            .iter()
            .map(|(_, notes)| {
                let matches: Vec<Option<&str>> = notes
                    .iter()
                    .map(|&i| plain_value(&symbols[i].rhythm))
                    .collect();
                let plain: BTreeSet<&str> = matches.iter().flatten().copied().collect();
                if matches.iter().all(Option::is_some) && plain.len() == 1 {
                    plain.into_iter().next()
                } else {
                    None
                }
            })
            .collect();
        let mut start_of_run = 0;
        for (i, value) in values.iter().enumerate() {
            if i > 0 && (value.is_none() || *value != values[i - 1]) {
                start_of_run = i;
            }
            let Some(value) = value else { continue };
            if i + 3 > values.len() || values[i..i + 3].iter().any(|v| v != &Some(*value)) {
                continue;
            }
            let in_voice: Vec<usize> = chords[i..i + 3]
                .iter()
                .flat_map(|(_, n)| n.iter().copied())
                .collect();
            if !in_voice
                .iter()
                .any(|&k| symbols[k].rhythm.starts_with("note"))
            {
                continue;
            }
            let key = (
                chords[i..i + 3].iter().map(|(p, _)| *p).collect::<Vec<_>>(),
                value.to_string(),
            );
            let off = (i - start_of_run) % 3 != 0;
            match found.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => {
                    e.1 .0.extend(in_voice);
                    e.1 .1 |= off;
                }
                None => found.push((key, (in_voice.into_iter().collect(), off))),
            }
        }
    }
    found
}

fn add_following_three(
    in_a_row: &ThreesByPlace,
    single: &ThreesByPlace,
    measure: &Measure,
    symbols: &[Sym],
) -> ThreesByPlace {
    let mut longer: ThreesByPlace = Vec::new();
    for ((places, value), (indices, off_beat)) in in_a_row {
        let voices: BTreeSet<&String> = indices.iter().map(|&i| &symbols[i].position).collect();
        let last = *places.last().unwrap();
        let next_place = (last + 1..measure.len())
            .find(|&p| measure[p].iter().any(|(v, _, _)| voices.contains(v)));
        for ((next_places, next_value), (next_indices, _)) in single {
            let next_voices: BTreeSet<&String> =
                next_indices.iter().map(|&i| &symbols[i].position).collect();
            if next_value == value
                && Some(next_places[0]) == next_place
                && indices.is_disjoint(next_indices)
                && next_voices == voices
            {
                let mut p = places.clone();
                p.extend(next_places);
                upsert(
                    &mut longer,
                    (p, value.clone()),
                    (indices | next_indices, *off_beat),
                );
            }
        }
    }
    longer
}

fn find_threes(measure: &Measure, symbols: &[Sym]) -> Vec<(Indices, bool)> {
    let single = find_single_threes(measure, symbols);
    let mut threes: Vec<(Indices, bool)> = Vec::new();
    let set_default =
        |threes: &mut Vec<(Indices, bool)>, k: &Indices, v: bool, replace| match threes
            .iter_mut()
            .find(|(i, _)| i == k)
        {
            Some(e) if replace => e.1 = v,
            Some(_) => {}
            None => threes.push((k.clone(), v)),
        };
    for (_, (k, v)) in &single {
        set_default(&mut threes, k, *v, true);
    }
    let mut in_a_row = single.clone();
    for _ in 0..MAX_THREES_IN_A_ROW - 1 {
        in_a_row = add_following_three(&in_a_row, &single, measure, symbols);
        for (_, (k, v)) in &in_a_row {
            set_default(&mut threes, k, *v, false);
        }
    }
    threes.sort_by(|a, b| a.0.iter().cmp(b.0.iter()));
    threes
}

fn choose_triplets(measure: &Measure, symbols: &[Sym]) -> Vec<Indices> {
    let mut chosen = Vec::new();
    let mut triplets = Indices::new();
    let mut breaks = count_breaks(measure, &triplets);
    if breaks.0 == 0 || get_length(measure) > Frac::int(MAX_MEASURE_LENGTH) {
        return chosen;
    }
    let threes = find_threes(measure, symbols);
    loop {
        let best = threes
            .iter()
            .filter(|(idx, _)| idx.is_disjoint(&triplets))
            .map(|(idx, off)| {
                let with: Indices = &triplets | idx;
                (count_breaks(measure, &with), *off, idx)
            })
            // A BTreeSet orders like homr's sorted index lists; the first minimum wins.
            .min_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
        let Some((best_breaks, _, best)) = best else {
            break;
        };
        if best_breaks.0 >= breaks.0 {
            break;
        }
        chosen.push(best.clone());
        triplets = &triplets | best;
        breaks = best_breaks;
    }
    chosen
}

fn note_value(symbols: &[Sym], indices: &Indices) -> String {
    let first = &symbols[*indices.iter().next().unwrap()];
    first.rhythm.split('_').nth(1).unwrap_or("").to_string()
}

/// homr `restore_unmarked_triplets`, on the symbols of one voice of a page.
pub fn restore_unmarked_triplets(symbols: &[Sym]) -> Vec<Sym> {
    let chosen: Vec<Vec<Indices>> = split_into_measures(symbols)
        .iter()
        .map(|m| choose_triplets(m, symbols))
        .collect();
    let mut shown: HashMap<String, usize> = HashMap::new();
    for indices in chosen.iter().flatten() {
        *shown.entry(note_value(symbols, indices)).or_default() += 1;
    }
    let triplets: Indices = chosen
        .iter()
        .flatten()
        .filter(|idx| shown[&note_value(symbols, idx)] >= MIN_TRIPLETS_ON_A_PAGE)
        .flat_map(|idx| idx.iter().copied())
        .collect();
    symbols
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if triplets.contains(&i) {
                as_triplet(s)
            } else {
                s.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syms(t: &[&str]) -> Vec<Sym> {
        t.iter().map(|s| Sym::parse(s)).collect()
    }

    #[test]
    fn lib_06_triplets_against_quarters_are_restored() {
        // Upper: three eighths per beat, lower: a quarter per beat, three beats a
        // measure, three measures: the eighths must be triplets.
        let mut t: Vec<&str> = Vec::new();
        for _ in 0..3 {
            for _ in 0..3 {
                t.extend([
                    "note_8 C5 _ _ _ upper",
                    "chord",
                    "note_4 C3 _ _ _ lower",
                    "note_8 D5 _ _ _ upper",
                    "note_8 E5 _ _ _ upper",
                ]);
            }
            t.push("barline . . . . .");
        }
        let out = restore_unmarked_triplets(&syms(&t));
        assert_eq!(out[0].rhythm, "note_12");
        assert_eq!(out[2].rhythm, "note_4");
        assert_eq!(out[4].rhythm, "note_12");
    }

    #[test]
    fn lib_06_plain_eighths_stay_plain() {
        let t = [
            "note_8 C5 _ _ _ upper",
            "note_8 D5 _ _ _ upper",
            "note_8 E5 _ _ _ upper",
            "barline . . . . .",
        ];
        let out = restore_unmarked_triplets(&syms(&t));
        assert!(out.iter().all(|s| !s.rhythm.contains("12")));
    }
}
