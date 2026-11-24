//! homr `staff_parsing.parse_staffs` (without the image work): which strip
//! is read in which order, and how the symbols of the strips become one
//! symbol list per voice (homr's "ResultStaff": a part of the score).
//!
//! homr reads the strips voice by voice: voice 0 of every system, then
//! voice 1 of every system, and so on (a grand staff is one voice). When the
//! systems of a page have different numbers of staffs, the staffs are
//! matched to the parts of the largest system by their grand-staff flags,
//! and a part hidden in a system gets one measure rest per measure.
use std::collections::HashMap;

use super::symbols::{is_lower_position, Sym};
use crate::System;

/// The reading plan of a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The rows (systems) as homr reads them: (system, staff) of the input.
    /// A row is a whole system, or one staff if the layout changes on the page.
    rows: Vec<Vec<(usize, usize)>>,
    /// The voices (parts) shown by each row's staffs.
    parts: Vec<Vec<usize>>,
    /// The grand-staff flag of each voice.
    layout: Vec<bool>,
    /// The strips in reading order: row, voice and (system, staff).
    strips: Vec<(usize, usize, (usize, usize))>,
    /// ChoirBird: the systems couldn't be matched by layout, but the largest
    /// system has this many single staffs; [`Plan::voices`] then assigns the
    /// staffs to its parts by their clefs (see [`by_clefs`]).
    clef_parts: Option<usize>,
}

fn row_layout(systems: &[System], row: &[(usize, usize)]) -> Vec<bool> {
    row.iter()
        .map(|&(s, i)| systems[s].staffs[i].grandstaff)
        .collect()
}

fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn go(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            go(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(0, n, k, &mut Vec::new(), &mut out);
    out
}

/// homr `_slots_in_layout`.
fn slots_in_layout(row: &[bool], layout: &[bool]) -> Option<Vec<usize>> {
    let matches: Vec<Vec<usize>> = combinations(layout.len(), row.len())
        .into_iter()
        .filter(|slots| slots.iter().zip(row).all(|(&s, &g)| layout[s] == g))
        .collect();
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
}

/// homr `_assign_parts`.
fn assign_parts(layouts: &[Vec<bool>]) -> Option<Vec<Vec<usize>>> {
    let largest = layouts.iter().max_by_key(|l| l.len())?;
    if layouts
        .iter()
        .any(|l| l.len() == largest.len() && l != largest)
    {
        return None;
    }
    layouts
        .iter()
        .map(|l| slots_in_layout(l, largest))
        .collect()
}

impl Plan {
    pub fn new(systems: &[System]) -> Plan {
        let mut rows: Vec<Vec<(usize, usize)>> = systems
            .iter()
            .enumerate()
            .map(|(s, sys)| (0..sys.staffs.len()).map(|i| (s, i)).collect())
            .collect();
        rows.retain(|r| !r.is_empty());
        let mut parts: Vec<Vec<usize>> = rows.iter().map(|r| (0..r.len()).collect()).collect();
        let counts: std::collections::BTreeSet<usize> = rows.iter().map(Vec::len).collect();
        let mut clef_parts = None;
        if counts.len() > 1 {
            let layouts: Vec<Vec<bool>> = rows.iter().map(|r| row_layout(systems, r)).collect();
            match assign_parts(&layouts) {
                Some(assigned) => parts = assigned,
                None => {
                    clef_parts = clef_part_count(systems);
                    rows = rows.into_iter().flatten().map(|x| vec![x]).collect();
                    parts = vec![vec![0]; rows.len()];
                }
            }
        }
        let number_of_voices = rows.iter().map(Vec::len).max().unwrap_or(0);
        let layout = rows
            .iter()
            .find(|r| r.len() == number_of_voices)
            .map(|r| row_layout(systems, r))
            .unwrap_or_default();
        let mut strips = Vec::new();
        for voice in 0..number_of_voices {
            for (row, slots) in parts.iter().enumerate() {
                if let Some(k) = slots.iter().position(|&v| v == voice) {
                    strips.push((row, voice, rows[row][k]));
                }
            }
        }
        Plan {
            rows,
            parts,
            layout,
            strips,
            clef_parts,
        }
    }

    /// The number of parts the page is read into.
    #[cfg(test)]
    fn number_of_parts(&self) -> usize {
        self.clef_parts.unwrap_or(self.layout.len())
    }

    /// The strips in the order homr reads them, as (system, staff).
    pub fn strip_order(&self) -> Vec<(usize, usize)> {
        self.strips.iter().map(|&(_, _, at)| at).collect()
    }

    /// The symbols of each voice (homr `parse_staffs`' result), from the
    /// decoded symbols of each strip in [`Plan::strip_order`].
    pub fn voices(&self, systems: &[System], tokens: &[Vec<Sym>]) -> Vec<Vec<Sym>> {
        if let Some(n) = self.clef_parts {
            let by_staff: HashMap<(usize, usize), &Vec<Sym>> = self
                .strips
                .iter()
                .zip(tokens)
                .map(|(&(_, _, at), t)| (at, t))
                .collect();
            if let Some(v) = by_clefs(systems, &by_staff, n) {
                return v;
            }
        }
        let mut results: HashMap<(usize, usize), Vec<Sym>> = HashMap::new();
        for (&(row, voice, (s, i)), symbols) in self.strips.iter().zip(tokens) {
            // homr `predict_best`: a single staff has no lower staff.
            let symbols: Vec<Sym> = if systems[s].staffs[i].grandstaff {
                symbols.clone()
            } else {
                symbols
                    .iter()
                    .filter(|x| !is_lower_position(&x.position))
                    .cloned()
                    .collect()
            };
            if !symbols.is_empty() {
                results.insert((row, voice), symbols);
            }
        }
        (0..self.layout.len())
            .map(|voice| {
                let mut out = Vec::new();
                for (row, slots) in self.parts.iter().enumerate() {
                    let mut symbols = results.get(&(row, voice)).cloned();
                    let shown = slots.iter().find_map(|&v| results.get(&(row, v)));
                    if !slots.contains(&voice) {
                        if let Some(shown) = shown {
                            symbols = Some(measure_rests_like(shown, self.layout[voice]));
                        }
                    }
                    let Some(symbols) = symbols else { continue };
                    out.extend(symbols);
                    out.push(Sym::new("newline"));
                }
                super::clean_up_voice(&out)
            })
            .collect()
    }
}

/// ChoirBird: the largest system's number of staffs, when the page's
/// systems differ and the largest is made of single staffs only (a choir
/// going from one solo line to S, T, B, and on to T and B as a grand staff).
fn clef_part_count(systems: &[System]) -> Option<usize> {
    let largest = systems.iter().max_by_key(|s| units(s))?;
    let n = units(largest);
    let single_only = largest.staffs.iter().all(|s| !s.grandstaff);
    // Unique largest system size, else nothing says which parts there are.
    let same = systems.iter().filter(|s| units(s) == n).count();
    let same_layout = systems
        .iter()
        .filter(|s| units(s) == n)
        .all(|s| s.staffs.len() == largest.staffs.len());
    (n > 1 && single_only && (same == 1 || same_layout)).then_some(n)
}

/// Staves of a system, a grand staff counting two.
fn units(s: &System) -> usize {
    s.staffs
        .iter()
        .map(|x| if x.grandstaff { 2 } else { 1 })
        .sum()
}

/// The clef family (G, F, C) of a strip's first clef at `position`.
fn clef_of(symbols: &[Sym], position: &str) -> Option<char> {
    symbols
        .iter()
        .find(|s| s.rhythm.starts_with("clef_") && s.position == position)
        .and_then(|s| s.rhythm.chars().nth(5))
}

/// One staff's symbols out of a grand staff strip: the `upper` or `lower`
/// voices, as upper-staff symbols. Chords keep only that staff's notes;
/// symbols without a staff (bar lines, key and time signatures) go to both.
fn half_of(symbols: &[Sym], lower: bool) -> Vec<Sym> {
    let want = if lower { "lower" } else { "upper" };
    let mut out: Vec<Sym> = Vec::new();
    for chord in super::symbols::group_into_chords(symbols) {
        let keep: Vec<Sym> = chord
            .into_iter()
            .filter(|s| {
                !(s.position.starts_with("upper") || s.position.starts_with("lower"))
                    || s.position.starts_with(want)
            })
            .map(|s| s.to_upper_position())
            .collect();
        for (i, s) in keep.into_iter().enumerate() {
            if i > 0 {
                out.push(Sym::new("chord"));
            }
            out.push(s);
        }
    }
    out
}

/// ChoirBird: the parts of a page whose systems differ in size. Each staff
/// (each half of a grand staff) goes to the part of the largest system whose
/// clef matches, in order from the top; among several matches, neighbouring
/// parts and then the topmost win. Parts a system doesn't show get one
/// measure rest per measure. None if a system doesn't fit.
fn by_clefs(
    systems: &[System],
    tokens: &HashMap<(usize, usize), &Vec<Sym>>,
    n: usize,
) -> Option<Vec<Vec<Sym>>> {
    // Every system as staves: (symbols, clef).
    let staves: Vec<Vec<(Vec<Sym>, Option<char>)>> = systems
        .iter()
        .enumerate()
        .map(|(si, sys)| {
            let mut v = Vec::new();
            for (i, st) in sys.staffs.iter().enumerate() {
                let t = tokens.get(&(si, i)).map(|t| t.as_slice()).unwrap_or(&[]);
                if st.grandstaff {
                    v.push((half_of(t, false), clef_of(t, "upper")));
                    v.push((half_of(t, true), clef_of(t, "lower")));
                } else {
                    let upper: Vec<Sym> = t
                        .iter()
                        .filter(|x| !is_lower_position(&x.position))
                        .cloned()
                        .collect();
                    let clef = clef_of(&upper, "upper");
                    v.push((upper, clef));
                }
            }
            v
        })
        .collect();
    let largest = staves.iter().position(|v| v.len() == n)?;
    let part_clefs: Vec<Option<char>> = staves[largest].iter().map(|x| x.1).collect();
    let mut parts: Vec<Vec<Sym>> = vec![Vec::new(); n];
    for sys in &staves {
        if sys.len() > n {
            return None;
        }
        let fits = |slots: &[usize]| {
            slots
                .iter()
                .zip(sys)
                .all(|(&p, st)| match (part_clefs[p], st.1) {
                    (Some(a), Some(b)) => a == b,
                    _ => true,
                })
        };
        let mut matches: Vec<Vec<usize>> = combinations(n, sys.len())
            .into_iter()
            .filter(|c| fits(c))
            .collect();
        // Neighbouring parts first (a grand staff is two neighbours), then the topmost.
        matches.sort_by_key(|c| (c.last().unwrap() - c[0], c.clone()));
        let slots = matches.into_iter().next()?;
        let shown = &sys[0].0;
        for (p, part) in parts.iter_mut().enumerate() {
            match slots.iter().position(|&x| x == p) {
                Some(k) => part.extend(sys[k].0.iter().cloned()),
                None => part.extend(measure_rests_like(shown, false)),
            }
            part.push(Sym::new("newline"));
        }
    }
    Some(parts.iter().map(|p| super::clean_up_voice(p)).collect())
}

/// homr `_measure_rests_like`: rests for a staff hidden in a system.
fn measure_rests_like(symbols: &[Sym], is_grandstaff: bool) -> Vec<Sym> {
    let positions: &[&str] = if is_grandstaff {
        &["upper", "lower"]
    } else {
        &["upper"]
    };
    let mut out = Vec::new();
    let mut in_measure = false;
    for s in symbols {
        if s.is_note_or_rest() && !in_measure {
            for (i, p) in positions.iter().enumerate() {
                if i > 0 {
                    out.push(Sym::new("chord"));
                }
                out.push(Sym::with_position("measureRest", p));
            }
            in_measure = true;
        } else if s.is_barline_or_repeat() {
            out.push(Sym::new(&s.rhythm));
            in_measure = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Staff;

    fn staff(grandstaff: bool) -> Staff {
        Staff {
            grandstaff,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 0.0,
            max_y: 0.0,
            grid: vec![],
        }
    }

    fn system(flags: &[bool]) -> System {
        System {
            staffs: flags.iter().map(|&g| staff(g)).collect(),
        }
    }

    #[test]
    fn lib_06_strips_are_read_voice_by_voice() {
        let systems = [system(&[false, true]), system(&[false, true])];
        let plan = Plan::new(&systems);
        assert_eq!(plan.strip_order(), vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
    }

    #[test]
    fn lib_06_hidden_staff_gets_measure_rests() {
        // The second system hides the voice, only the piano is shown.
        let systems = [system(&[false, true]), system(&[true])];
        let plan = Plan::new(&systems);
        assert_eq!(plan.strip_order(), vec![(0, 0), (0, 1), (1, 0)]);
        let t = |s: &str| s.split('|').map(Sym::parse).collect::<Vec<_>>();
        let tokens = vec![
            t("clef_G2 _ _ _ _ upper|note_1 C5 _ _ _ upper|barline"),
            t("clef_G2 _ _ _ _ upper|note_1 C4 _ _ _ upper|barline"),
            t("note_1 D4 _ _ _ upper|barline|note_1 E4 _ _ _ upper|barline"),
        ];
        let voices = plan.voices(&systems, &tokens);
        let rests = voices[0]
            .iter()
            .filter(|s| s.rhythm == "measureRest")
            .count();
        assert_eq!(rests, 2);
    }

    #[test]
    fn lib_06_changing_layout_reads_every_staff_alone() {
        // A grand staff doesn't fit into two single staffs.
        let systems = [system(&[false, false]), system(&[true])];
        assert_eq!(
            Plan::new(&systems).strip_order(),
            vec![(0, 0), (0, 1), (1, 0)]
        );
        let systems = [system(&[false, false]), system(&[false])];
        // [false] fits [false, false] twice: ambiguous, so staffs are read alone.
        assert_eq!(
            Plan::new(&systems).strip_order(),
            vec![(0, 0), (0, 1), (1, 0)]
        );
    }
}

#[cfg(test)]
mod lib_10_layout_change {
    use super::*;
    use crate::Staff;

    fn staff(grandstaff: bool) -> Staff {
        Staff {
            grandstaff,
            min_x: 0.0,
            max_x: 0.0,
            min_y: 0.0,
            max_y: 0.0,
            grid: vec![],
        }
    }
    fn system(flags: &[bool]) -> System {
        System {
            staffs: flags.iter().map(|&g| staff(g)).collect(),
        }
    }

    fn t(s: &str) -> Vec<Sym> {
        s.split('|').map(Sym::parse).collect()
    }

    #[test]
    fn lib_10_solo_then_three_staffs_then_two_gives_three_parts() {
        // Abide with me, page 1: S alone for three systems, then S, T, B, then
        // T and B on a grand staff. homr 0.7.1 read every staff as one line of
        // a single part, so the tenor and bass ended up in the soprano's part.
        let systems = [
            system(&[false]),
            system(&[false]),
            system(&[false]),
            system(&[false, false, false]),
            system(&[true]),
        ];
        let plan = Plan::new(&systems);
        // Three parts: the soprano and the two staffs that join it.
        assert_eq!(plan.number_of_parts(), 3, "{plan:?}");
        // The grand staff is T (upper) and B (lower): the soprano is hidden.
        let bar = "|barline";
        let solo = t(&format!("clef_G2 _ _ _ _ upper|note_1 B4 _ _ _ upper{bar}"));
        let tokens: Vec<Vec<Sym>> = plan
            .strip_order()
            .iter()
            .map(|&(s, i)| match (s, i) {
                (3, 1) => t(&format!("clef_G2 _ _ _ _ upper|note_1 C4 _ _ _ upper{bar}")),
                (3, 2) => t(&format!("clef_F4 _ _ _ _ upper|note_1 C3 _ _ _ upper{bar}")),
                (4, 0) => t(&format!(
                    "clef_G2 _ _ _ _ upper|chord|clef_F4 _ _ _ _ lower|note_1 D4 _ _ _ upper|chord|note_1 D3 _ _ _ lower{bar}"
                )),
                _ => solo.clone(),
            })
            .collect();
        let voices = plan.voices(&systems, &tokens);
        assert_eq!(voices.len(), 3);
        let pitches = |v: &[Sym]| {
            v.iter()
                .filter(|s| s.rhythm.starts_with("note"))
                .map(|s| s.pitch.clone())
                .collect::<Vec<_>>()
        };
        // Soprano: four systems of B4 and a measure rest on the last.
        assert_eq!(pitches(&voices[0]), ["B4", "B4", "B4", "B4"]);
        assert!(voices[0].iter().any(|s| s.rhythm == "measureRest"));
        // Tenor and bass: rests while the soprano sings alone, then their notes.
        assert_eq!(pitches(&voices[1]), ["C4", "D4"]);
        assert_eq!(pitches(&voices[2]), ["C3", "D3"]);
        assert_eq!(
            voices[1]
                .iter()
                .filter(|s| s.rhythm == "measureRest")
                .count(),
            3
        );
    }
}
