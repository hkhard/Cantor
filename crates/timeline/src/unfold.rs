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

/// D.S. al Coda from `ds` back to `segno`: true when the score has a coda
/// but every "To Coda" between the segno and the D.S. sits in a volta that
/// isn't the final one, so it can't be reached after the jump (only the final
/// volta plays then). Taking such a D.S. would replay the section and never
/// reach the coda; MuseScore doesn't take it.
fn coda_unreachable(navs: &[Navigation], segno: usize, ds: usize) -> bool {
    if !navs.iter().any(|n| n.coda.is_some()) {
        return false;
    }
    let to_codas: Vec<usize> = (segno..=ds.min(navs.len() - 1))
        .filter(|&k| navs[k].tocoda.is_some())
        .collect();
    !to_codas.is_empty()
        && to_codas.iter().all(|&k| {
            let nums = &navs[k].ending;
            if nums.is_empty() {
                return false;
            }
            // The final volta of the group k belongs to.
            let start = (0..=k)
                .rev()
                .take_while(|&j| !navs[j].ending.is_empty())
                .last()
                .unwrap_or(k);
            let end = (k..navs.len())
                .take_while(|&j| !navs[j].ending.is_empty())
                .last()
                .unwrap_or(k);
            let max = (start..=end)
                .flat_map(|j| navs[j].ending.iter().copied())
                .max();
            nums.iter().max().copied() != max
        })
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
                if let Some(t) = find_target(&navs, label, |n| &n.coda).filter(|&t| t > i) {
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
                if let Some(t) = find_target(&navs, label, |n| &n.segno)
                    .filter(|&t| !coda_unreachable(&navs, t, i))
                {
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
