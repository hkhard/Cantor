//! Repeat/volta/D.C./D.S./Coda/Fine unfolding (PLAY-02, WP-1.5).
//!
//! Playback conventions follow common practice and MuseScore:
//! - a backward repeat plays its section `times` times (default 2, or one
//!   more than the highest volta number inside the section);
//! - a volta measure plays only on the passes its numbers name;
//! - D.C./D.S. are taken once; after the jump repeats are not taken again,
//!   "To Coda" jumps and "Fine" ends playback;
//! - jumps to missing targets are ignored, and the result is bounded.

use cantor_score::{Navigation, Score};

/// Navigation for measure `i`, merged across parts (OMR may attach a repeat
/// barline to only one part).
fn nav_at(score: &Score, i: usize) -> Navigation {
    let mut out = Navigation::default();
    for p in &score.parts {
        let Some(m) = p.measures.get(i) else { continue };
        let n = &m.nav;
        out.repeat_start |= n.repeat_start;
        out.repeat_end = match (out.repeat_end, n.repeat_end) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        if out.ending.is_empty() {
            out.ending = n.ending.clone();
        }
        out.segno = out.segno.or_else(|| n.segno.clone());
        out.coda = out.coda.or_else(|| n.coda.clone());
        out.tocoda = out.tocoda.or_else(|| n.tocoda.clone());
        out.dalsegno = out.dalsegno.or_else(|| n.dalsegno.clone());
        out.dacapo |= n.dacapo;
        out.fine |= n.fine;
    }
    out
}

/// Find the measure carrying a segno/coda `label`: exact match, else the only
/// unlabelled/any marker if there is exactly one.
fn find_target(
    navs: &[Navigation],
    label: &str,
    pick: fn(&Navigation) -> &Option<String>,
) -> Option<usize> {
    if let Some(i) = navs.iter().position(|n| pick(n).as_deref() == Some(label)) {
        return Some(i);
    }
    let all: Vec<usize> = navs
        .iter()
        .enumerate()
        .filter(|(_, n)| pick(n).is_some())
        .map(|(i, _)| i)
        .collect();
    match all.as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

/// The coda to jump to from a "To Coda" in the volta group starting at `at`.
fn volta_group_to_coda(navs: &[Navigation], at: usize) -> Option<usize> {
    let end = (at..navs.len())
        .take_while(|&k| !navs[k].ending.is_empty())
        .last()?;
    let label = (at..=end).find_map(|k| navs[k].tocoda.clone())?;
    coda_target(navs, &label, at)
}

/// Where a "To Coda" jumps: the coda sign, or, when a D.S. or D.C. sits
/// between the jump and the coda sign, the bar after that D.S./D.C. (the
/// coda music follows the D.S. bar; MuseScore plays it from there).
fn coda_target(navs: &[Navigation], label: &str, from: usize) -> Option<usize> {
    let sign = find_target(navs, label, |n| &n.coda).filter(|&t| t > from)?;
    let jump = (from..sign)
        .rev()
        .find(|&k| navs[k].dalsegno.is_some() || navs[k].dacapo);
    Some(jump.map_or(sign, |k| k + 1))
}

/// Whether a backward repeat after measure `from` (and before any forward
/// repeat sign or jump) closes a section that has no start of its own, so it
/// goes back to the same forward sign as the voltas just left.
fn returns_to_same_start(navs: &[Navigation], from: usize) -> bool {
    for n in &navs[from.min(navs.len())..] {
        if n.repeat_start || n.dacapo || n.dalsegno.is_some() {
            return false;
        }
        if n.repeat_end.is_some() {
            return true;
        }
    }
    false
}

/// Written measure indices in playback order.
pub fn unfold(score: &Score) -> Vec<usize> {
    let n = score
        .parts
        .iter()
        .map(|p| p.measures.len())
        .max()
        .unwrap_or(0);
    let navs: Vec<Navigation> = (0..n).map(|i| nav_at(score, i)).collect();
    let limit = (n * 64).min(100_000);

    let mut order = Vec::new();
    let mut i = 0usize;
    let mut section_start = 0usize;
    let mut pass = 1u32;
    let mut after_jump = false;
    let mut jumped_from: Vec<usize> = Vec::new();
    let mut arrived_by_repeat = false;

    while i < n && order.len() < limit {
        let nav = &navs[i];

        if nav.repeat_start && !arrived_by_repeat && !after_jump {
            section_start = i;
            pass = 1;
        }
        arrived_by_repeat = false;

        // Volta not for this pass: skip without playing.
        if !nav.ending.is_empty() && !nav.ending.contains(&pass) && !after_jump {
            i += 1;
            continue;
        }
        // After a jump, "To Coda" written in the first bar of a volta group
        // (MuseScore puts it there when it is printed above the volta's
        // start) means: jump before the voltas.
        if after_jump && !nav.ending.is_empty() && (i == 0 || navs[i - 1].ending.is_empty()) {
            if let Some(t) = volta_group_to_coda(&navs, i) {
                i = t;
                continue;
            }
        }
        // After a jump, play only the last volta (the "final ending").
        if after_jump && !nav.ending.is_empty() {
            let last_group_end = (i..n)
                .take_while(|&k| !navs[k].ending.is_empty())
                .last()
                .unwrap_or(i);
            let max_num = (i..=last_group_end)
                .flat_map(|k| navs[k].ending.iter().copied())
                .max();
            if Some(*nav.ending.iter().max().unwrap_or(&0)) != max_num {
                i += 1;
                continue;
            }
        }

        order.push(i);

        if after_jump && nav.fine {
            break;
        }
        if after_jump {
            if let Some(label) = &nav.tocoda {
                if let Some(t) = coda_target(&navs, label, i) {
                    i = t;
                    continue;
                }
            }
        }

        if let Some(times) = nav.repeat_end.filter(|_| !after_jump) {
            let implied = nav.ending.iter().max().map_or(0, |m| m + 1);
            let times = times.max(implied).min(64);
            if pass < times {
                pass += 1;
                i = section_start;
                arrived_by_repeat = true;
                continue;
            }
            pass = 1;
            section_start = i + 1;
        } else if !nav.ending.is_empty() && navs.get(i + 1).is_none_or(|nx| nx.ending.is_empty()) {
            // Leaving the last volta of a group. The section only ends here if
            // no later backward repeat (before the next forward sign) still
            // belongs to it: |: A |1. B :| |2. C D |1,2. E :| |3. F plays
            // A B A C D E A ... and E goes back to A (MuseScore, common
            // practice). The pass count carries on for that later repeat.
            if !returns_to_same_start(&navs, i + 1) {
                pass = 1;
                section_start = i + 1;
            }
        }

        if !jumped_from.contains(&i) {
            if nav.dacapo {
                jumped_from.push(i);
                after_jump = true;
                i = 0;
                continue;
            }
            if let Some(label) = &nav.dalsegno {
                if let Some(t) = find_target(&navs, label, |n| &n.segno) {
                    jumped_from.push(i);
                    after_jump = true;
                    i = t;
                    continue;
                }
            }
        }
        i += 1;
    }
    order
}
