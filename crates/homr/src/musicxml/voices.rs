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
        if counts.len() > 1 {
            let layouts: Vec<Vec<bool>> = rows.iter().map(|r| row_layout(systems, r)).collect();
            match assign_parts(&layouts) {
                Some(assigned) => parts = assigned,
                None => {
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
        }
    }

    /// The strips in the order homr reads them, as (system, staff).
    pub fn strip_order(&self) -> Vec<(usize, usize)> {
        self.strips.iter().map(|&(_, _, at)| at).collect()
    }

    /// The symbols of each voice (homr `parse_staffs`' result), from the
    /// decoded symbols of each strip in [`Plan::strip_order`].
    pub fn voices(&self, systems: &[System], tokens: &[Vec<Sym>]) -> Vec<Vec<Sym>> {
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
