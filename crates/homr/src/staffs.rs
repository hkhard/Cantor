//! Stage 4 (homr `main.detect_staffs_in_image` after the predictions:
//! `predict_symbols`, `staff_detection`, `note_detection`,
//! `bar_line_detection`, `brace_dot_detection`, `system_repair`): find
//! staffs and group them into systems. Ported from homr 0.7.1 (AGPL-3.0,
//! Copyright the homr contributors).
//!
//! Not ported: `add_notes_to_staffs`. homr only attaches notes to staffs
//! there; nothing downstream reads them (brace detection asks staffs for
//! bar lines and clefs, which homr never adds, and the transformer reads
//! the staff images), so the staffs and systems are the same without it.
pub mod detection;
pub mod model;
pub mod systems;

use crate::boxes::{self, AngledBox};
use crate::{Error, Gray, Predictions, System};
use model::np_median;

/// homr `prepare_bar_line_image`: dilate with `np.ones((5, 3))`.
pub fn prepare_bar_line_image(stems_rest: &Gray) -> Gray {
    boxes::morph::dilate(stems_rest, &boxes::morph::rect(3, 5))
}

/// The symbols homr's `predict_symbols` finds on the masks.
pub struct Symbols {
    pub noteheads: Vec<AngledBox>,
    pub staff_fragments: Vec<AngledBox>,
    pub clefs_keys: Vec<AngledBox>,
    pub stems_rest: Vec<AngledBox>,
    pub bar_lines: Vec<AngledBox>,
}

/// homr `predict_symbols`.
pub fn predict_symbols(pred: &Predictions) -> Symbols {
    Symbols {
        noteheads: boxes::create_bounding_ellipses(&pred.notehead, Some((4.0, 4.0))),
        staff_fragments: boxes::create_rotated_bounding_boxes(
            &pred.staff,
            true,
            Some((5.0, 1.0)),
            Some((10000.0, 100.0)),
        ),
        clefs_keys: boxes::create_rotated_bounding_boxes(
            &pred.clefs_keys,
            false,
            Some((20.0, 40.0)),
            Some((1000.0, 1000.0)),
        ),
        stems_rest: boxes::create_rotated_bounding_boxes(&pred.stems_rest, false, None, None),
        bar_lines: boxes::create_rotated_bounding_boxes(
            &prepare_bar_line_image(&pred.stems_rest),
            true,
            Some((1.0, 5.0)),
            None,
        ),
    }
}

/// homr `combine_noteheads_with_stems`: (notehead, stem) pairs, by notehead y.
pub fn combine_noteheads_with_stems(
    noteheads: &[AngledBox],
    stems: &[AngledBox],
) -> Vec<(AngledBox, Option<AngledBox>)> {
    let mut heads: Vec<&AngledBox> = noteheads.iter().collect();
    heads.sort_by(|a, b| a.rect.cy.partial_cmp(&b.rect.cy).unwrap());
    heads
        .into_iter()
        .map(|h| {
            let th = h.make_box_thicker(15.0);
            (
                h.clone(),
                stems.iter().find(|s| s.is_overlapping(&th)).cloned(),
            )
        })
        .collect()
}

/// homr `detect_bar_lines`.
pub fn detect_bar_lines(lines: Vec<AngledBox>, unit: f64) -> Vec<AngledBox> {
    lines
        .into_iter()
        .filter(|l| l.rect.h >= 3.0 * unit && l.rect.w <= 2.0 * unit)
        .collect()
}

/// homr `estimate_unit_size_from_clefs`.
fn unit_from_clefs(fragments: &[AngledBox], clefs: &[AngledBox]) -> Option<f64> {
    let fr = detection::Frags::new(fragments);
    let all: Vec<usize> = (0..fragments.len()).collect();
    let a = detection::find_staff_anchors(&fr, &all, clefs, true);
    if a.is_empty() {
        return None;
    }
    let u: Vec<f64> = a.iter().map(|x| x.average_unit_size).collect();
    Some(np_median(&u))
}

/// Stage 4 from filtered predictions: the systems after homr's repair.
pub fn detect(pred: &Predictions, image: &Gray) -> Result<Vec<System>, Error> {
    let _ = image;
    let sym = predict_symbols(pred);
    let fragments = detection::break_wide_fragments(sym.staff_fragments, 100.0);
    let notes = combine_noteheads_with_stems(&sym.noteheads, &sym.stems_rest);
    let unit = if notes.is_empty() {
        unit_from_clefs(&fragments, &sym.clefs_keys).ok_or(Error::NoStaffs)?
    } else {
        let h: Vec<f64> = notes.iter().map(|n| n.0.rect.h).collect();
        np_median(&h)
    };
    let heads: Vec<AngledBox> = notes.iter().map(|n| n.0.clone()).collect();
    let stems: Vec<AngledBox> = notes.iter().filter_map(|n| n.1.clone()).collect();
    let candidates: Vec<AngledBox> = sym
        .bar_lines
        .into_iter()
        .filter(|l| !l.is_overlapping_with_any(&heads) && !l.is_overlapping_with_any(&stems))
        .collect();
    let bar_lines = detect_bar_lines(candidates, unit);
    let fr = detection::Frags::new(&fragments);
    let staffs =
        detection::detect_staff(&fr, &pred.staff, &sym.clefs_keys, &bar_lines, &pred.symbols);
    if staffs.is_empty() {
        return Err(Error::NoStaffs);
    }
    let brace_img = systems::prepare_brace_dot_image(&pred.symbols, &pred.staff);
    let brace = boxes::create_rotated_bounding_boxes(&brace_img, true, None, Some((100.0, -1.0)));
    let mut arena = systems::Arena(staffs);
    let ids: Vec<usize> = (0..arena.0.len()).collect();
    let rows = systems::find_braces(&mut arena, &ids, brace);
    let rows = systems::repair_systems(&mut arena, rows);
    Ok(rows
        .iter()
        .map(|r| System {
            staffs: r.staffs.iter().map(|&s| arena.0[s].to_public()).collect(),
        })
        .collect())
}
