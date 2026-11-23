//! Measure boxes on the engraved page, computed from the layout a notation
//! program wrote into the MusicXML (LIB-05). MuseScore, Finale, Dorico and
//! Sibelius exports carry page size, margins, system and page breaks
//! (`<print new-system|new-page>`), measure widths and staff distances.
//! When the PDF was engraved from the same file, these boxes sit on its
//! measures, so the play cursor works on the original PDF without OMR.
//!
//! Units are MusicXML tenths; the result is in fractions of the page
//! (origin top-left), matching the measure maps the app already uses.

use crate::dom::{self, Element};
use crate::parse::musicxml_bytes;

/// One measure's box on its page, as fractions of page width and height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    /// 1-based page.
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

const STAFF_HEIGHT: f64 = 40.0; // five lines, four spaces of 10 tenths
/// Default gap between staves of a grand staff / between parts, used when a
/// file gives none (MusicXML's usual default).
const DEFAULT_STAFF_DISTANCE: f64 = 65.0;
const DEFAULT_SYSTEM_DISTANCE: f64 = 120.0;
const DEFAULT_TOP_SYSTEM_DISTANCE: f64 = 70.0;
/// Padding around the staves so the box covers stems and note heads a bit
/// above and below the outer staff lines.
const PAD: f64 = 12.0;

fn num(e: Option<&Element>) -> Option<f64> {
    e.and_then(|e| e.text.trim().parse::<f64>().ok())
}

/// Per measure, from the first part: break kind, widths, system layout.
#[derive(Default, Clone)]
struct MeasureInfo {
    width: Option<f64>,
    new_system: bool,
    new_page: bool,
    sys_left: Option<f64>,
    sys_distance: Option<f64>,
    top_sys_distance: Option<f64>,
}

/// Measure boxes for every written measure of the first part, or `None`
/// when the file carries no usable layout (no page size or no widths).
pub fn layout_measure_boxes(bytes: &[u8]) -> Option<Vec<LayoutBox>> {
    let xml = musicxml_bytes(bytes).ok()?;
    let root = dom::parse(&xml).ok()?;
    if root.name != "score-partwise" {
        return None;
    }
    let defaults = root.child("defaults")?;
    let pl = defaults.child("page-layout")?;
    let page_w = num(pl.child("page-width"))?;
    let page_h = num(pl.child("page-height"))?;
    if page_w <= 0.0 || page_h <= 0.0 {
        return None;
    }
    // Odd/even margins (page 1 is odd).
    let margins = |odd: bool| -> (f64, f64, f64) {
        let m = pl
            .children("page-margins")
            .find(|m| match m.attr("type") {
                Some("odd") => odd,
                Some("even") => !odd,
                _ => true,
            })
            .or_else(|| pl.child("page-margins"));
        let g = |n: &str| num(m.and_then(|m| m.child(n))).unwrap_or(0.0);
        (g("left-margin"), g("right-margin"), g("top-margin"))
    };
    let dsys = defaults.child("system-layout");
    let mut sys_left_default = num(dsys
        .and_then(|s| s.child("system-margins"))
        .and_then(|m| m.child("left-margin")))
    .unwrap_or(0.0);
    let mut sys_dist =
        num(dsys.and_then(|s| s.child("system-distance"))).unwrap_or(DEFAULT_SYSTEM_DISTANCE);
    let mut top_sys_dist = num(dsys.and_then(|s| s.child("top-system-distance")))
        .unwrap_or(DEFAULT_TOP_SYSTEM_DISTANCE);
    let default_staff_dist = num(defaults
        .child("staff-layout")
        .and_then(|s| s.child("staff-distance")))
    .unwrap_or(DEFAULT_STAFF_DISTANCE);

    // Visible staves per part (a hidden part, e.g. print-object="no" on the
    // whole part, is rare; hidden part *names* are common and do not matter).
    let parts: Vec<&Element> = root.children("part").collect();
    let first = parts.first()?;
    let measures: Vec<&Element> = first.children("measure").collect();
    if measures.is_empty() {
        return None;
    }

    // Measure info from the first part.
    let mut infos: Vec<MeasureInfo> = Vec::with_capacity(measures.len());
    for m in &measures {
        let mut i = MeasureInfo {
            width: m.attr("width").and_then(|w| w.parse().ok()),
            ..Default::default()
        };
        if let Some(p) = m.child("print") {
            i.new_system = p.attr("new-system") == Some("yes");
            i.new_page = p.attr("new-page") == Some("yes");
            if let Some(sl) = p.child("system-layout") {
                let sm = sl.child("system-margins");
                i.sys_left = num(sm.and_then(|m| m.child("left-margin")));
                i.sys_distance = num(sl.child("system-distance"));
                i.top_sys_distance = num(sl.child("top-system-distance"));
            }
        }
        infos.push(i);
    }
    if infos.iter().any(|i| i.width.is_none()) {
        return None;
    }

    // Height of a system: every staff of every part, with the staff
    // distances in effect at that measure (a part's first staff gets its
    // distance from the part above; extra staves of a grand staff use
    // staff-layout number="2" or the default).
    let staff_counts: Vec<usize> = parts
        .iter()
        .map(|p| {
            p.children("measure")
                .flat_map(|m| m.children("attributes"))
                .find_map(|a| {
                    a.child("staves")
                        .and_then(|s| s.text.trim().parse::<usize>().ok())
                })
                .unwrap_or(1)
        })
        .collect();
    // Current staff distances per (part, staff number), updated by <print>.
    let mut dist: Vec<Vec<f64>> = staff_counts
        .iter()
        .map(|&n| vec![default_staff_dist; n])
        .collect();
    let part_measures: Vec<Vec<&Element>> = parts
        .iter()
        .map(|p| p.children("measure").collect())
        .collect();
    let system_height = |dist: &Vec<Vec<f64>>| -> f64 {
        let mut h = 0.0;
        let mut first_staff = true;
        for part in dist {
            for d in part {
                if !first_staff {
                    h += d;
                }
                h += STAFF_HEIGHT;
                first_staff = false;
            }
        }
        h
    };

    let mut out = Vec::with_capacity(infos.len());
    let mut page = 1u32;
    let mut y_sys = 0.0; // top line of the current system, tenths from page top
    let mut x = 0.0;
    let mut sys_h = 0.0;
    let (mut left, mut _right, mut top) = margins(true);
    for (k, info) in infos.iter().enumerate() {
        // Staff distances that change at this measure (any part's <print>).
        for (p, ms) in part_measures.iter().enumerate() {
            if let Some(pr) = ms.get(k).and_then(|m| m.child("print")) {
                for sl in pr.children("staff-layout") {
                    let n = sl
                        .attr("number")
                        .and_then(|n| n.parse::<usize>().ok())
                        .unwrap_or(1);
                    if let (Some(d), Some(slot)) = (
                        num(sl.child("staff-distance")),
                        dist[p].get_mut(n.saturating_sub(1)),
                    ) {
                        *slot = d;
                    }
                }
            }
        }
        let starts_system = k == 0 || info.new_system || info.new_page;
        if starts_system {
            if let Some(v) = info.sys_left {
                sys_left_default = v;
            }
            if let Some(v) = info.sys_distance {
                sys_dist = v;
            }
            if info.new_page && k > 0 {
                page += 1;
                (left, _right, top) = margins(page % 2 == 1);
            }
            let first_on_page = k == 0 || info.new_page;
            if first_on_page {
                if let Some(v) = info.top_sys_distance {
                    top_sys_dist = v;
                }
                y_sys = top + top_sys_dist;
            } else {
                y_sys += sys_h + sys_dist;
            }
            sys_h = system_height(&dist);
            x = left + sys_left_default;
        }
        let w = info.width.unwrap_or(0.0);
        let y0 = (y_sys - PAD).max(0.0);
        let y1 = (y_sys + sys_h + PAD).min(page_h);
        out.push(LayoutBox {
            page,
            x: x / page_w,
            y: y0 / page_h,
            w: w / page_w,
            h: (y1 - y0) / page_h,
        });
        x += w;
    }
    // Sanity: nothing may fall off the page (a sign of a layout we misread).
    if out.iter().any(|b| b.x + b.w > 1.02 || b.y + b.h > 1.02) {
        return None;
    }
    for b in &mut out {
        b.w = b.w.min(1.0 - b.x);
        b.h = b.h.min(1.0 - b.y);
    }
    Some(out)
}
