//! homr `staff_detection` (anchors, line fragments, raw staffs, resampling)
//! and `find_peaks`, ported from homr 0.7.1 (AGPL-3.0, Copyright the homr
//! contributors). Fragments are indices into one fragment list; homr compares
//! them by their box, so `key[i]` is the first fragment with the same box.
use super::model::{np_mean, np_median, np_std, py_round, Point, Staff};
use crate::boxes::{geom, AngledBox};
use crate::Gray;
use std::collections::{HashMap, HashSet};

const LINES: usize = 5;

fn max_line_gap(unit: f64) -> f64 {
    5.0 * unit
}

/// The fragments (staff line pieces) and their identity keys.
pub struct Frags<'a> {
    pub boxes: &'a [AngledBox],
    key: Vec<usize>,
}

impl<'a> Frags<'a> {
    pub fn new(boxes: &'a [AngledBox]) -> Frags<'a> {
        let mut seen: HashMap<[u64; 5], usize> = HashMap::new();
        let key = boxes
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let r = &b.rect;
                *seen
                    .entry([
                        r.cx.to_bits(),
                        r.cy.to_bits(),
                        r.w.to_bits(),
                        r.h.to_bits(),
                        r.angle.to_bits(),
                    ])
                    .or_insert(i)
            })
            .collect();
        Frags { boxes, key }
    }
    fn b(&self, i: usize) -> &AngledBox {
        &self.boxes[i]
    }
    fn set(&self, f: &[usize]) -> HashSet<usize> {
        f.iter().map(|&i| self.key[i]).collect()
    }
}

/// homr `StaffLineSegment`.
#[derive(Debug, Clone)]
pub struct Segment {
    pub frags: Vec<usize>,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl Segment {
    fn new(fr: &Frags, mut frags: Vec<usize>) -> Segment {
        frags.sort_by(|&a, &b| fr.b(a).rect.cx.partial_cmp(&fr.b(b).rect.cx).unwrap());
        let mut s = Segment {
            frags,
            min_x: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            min_y: f64::INFINITY,
            max_y: f64::NEG_INFINITY,
        };
        for &i in &s.frags {
            let r = &fr.b(i).rect;
            s.min_x = s.min_x.min(r.cx - r.w / 2.0);
            s.max_x = s.max_x.max(r.cx + r.w / 2.0);
            s.min_y = s.min_y.min(r.cy - r.h / 2.0);
            s.max_y = s.max_y.max(r.cy + r.h / 2.0);
        }
        s
    }
    fn merge(&self, fr: &Frags, other: &Segment) -> Segment {
        let mut lines = self.frags.clone();
        for &f in &other.frags {
            if !lines.iter().any(|&l| fr.key[l] == fr.key[f]) {
                lines.push(f);
            }
        }
        Segment::new(fr, lines)
    }
    fn get_at(&self, fr: &Frags, x: f64) -> Option<usize> {
        let tol = 10.0;
        self.frags.iter().copied().find(|&i| {
            let r = &fr.b(i).rect;
            x >= r.cx - r.w / 2.0 - tol && x <= r.cx + r.w / 2.0 + tol
        })
    }
}

/// homr `StaffAnchor`.
#[derive(Debug, Clone)]
pub struct Anchor {
    pub lines: Vec<Segment>,
    pub symbol: AngledBox,
    pub average_unit_size: f64,
    pub zone: (i64, i64),
}

impl Anchor {
    fn new(fr: &Frags, lines: Vec<Segment>, symbol: AngledBox) -> Anchor {
        let mut ys: Vec<f64> = lines
            .iter()
            .map(|l| fr.b(l.frags[0]).get_center_extrapolated(symbol.rect.cx))
            .collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let deltas: Vec<f64> = ys.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let avg = if deltas.is_empty() {
            0.0
        } else {
            np_mean(&deltas)
        };
        let max_y = lines
            .iter()
            .map(|l| l.max_y)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = lines.iter().map(|l| l.min_y).fold(f64::INFINITY, f64::min);
        Anchor {
            zone: ((min_y - 5.0 * avg) as i64, (max_y + 5.0 * avg) as i64),
            lines,
            symbol,
            average_unit_size: avg,
        }
    }
}

/// homr `RawStaff`: the lines of a staff with gaps, and the box around them.
#[derive(Debug, Clone)]
pub struct RawStaff {
    pub id: i64,
    pub lines: Vec<Segment>,
    pub anchors: Vec<Anchor>,
    pub b: AngledBox,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl RawStaff {
    fn new(fr: &Frags, id: i64, lines: Vec<Segment>, anchors: Vec<Anchor>) -> RawStaff {
        let mut pts = Vec::new();
        for l in &lines {
            for &f in &l.frags {
                pts.extend_from_slice(&fr.b(f).contours);
            }
        }
        let b = AngledBox::rotated(geom::min_area_rect(&pts), pts, id);
        let (cx, cy, w, h) = (b.rect.cx, b.rect.cy, b.rect.w, b.rect.h);
        RawStaff {
            id,
            lines,
            anchors,
            b,
            min_x: cx - w / 2.0,
            max_x: cx + w / 2.0,
            min_y: cy - h / 2.0,
            max_y: cy + h / 2.0,
        }
    }
    fn merge(&self, fr: &Frags, other: &RawStaff) -> RawStaff {
        let lines = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, l)| other.lines[i].merge(fr, l))
            .collect();
        let mut anchors = self.anchors.clone();
        anchors.extend(other.anchors.iter().cloned());
        RawStaff::new(fr, self.id, lines, anchors)
    }
}

/// homr `connect_staff_lines`.
pub fn connect_staff_lines(fr: &Frags, lines: &[usize], unit: f64) -> Vec<Segment> {
    // sorted(..., key=bottom_left x, reverse=True) then pop(): ascending x,
    // later items first among equal keys.
    let mut order: Vec<usize> = lines.to_vec();
    order.sort_by(|&a, &b| {
        fr.b(b)
            .bottom_left()
            .0
            .partial_cmp(&fr.b(a).bottom_left().0)
            .unwrap()
    });
    let mut result: Vec<Vec<usize>> = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    let mut last_cleanup = 0f64;
    while let Some(cur) = order.pop() {
        let x = fr.b(cur).bottom_left().0;
        if x - last_cleanup > max_line_gap(unit) {
            active.retain(|&r| {
                x - fr.b(*result[r].last().unwrap()).bottom_right().0 < max_line_gap(unit)
            });
            last_cleanup = x;
        }
        if fr.b(cur).rect.w < unit / 5.0 {
            continue;
        }
        let mut connected = false;
        for &r in &active {
            let last = *result[r].last().unwrap();
            if fr.b(last).is_overlapping_extrapolated(fr.b(cur), unit) {
                result[r].push(cur);
                connected = true;
            }
        }
        if !connected {
            result.push(vec![cur]);
            active.push(result.len() - 1);
        }
    }
    result.sort_by(|a, b| fr.b(a[0]).rect.cy.partial_cmp(&fr.b(b[0]).rect.cy).unwrap());
    result.into_iter().map(|l| Segment::new(fr, l)).collect()
}

fn extrapolated_line_y(fr: &Frags, line: &Segment, x: f64) -> f64 {
    let f = line.get_at(fr, x).unwrap_or(line.frags[0]);
    fr.b(f).get_center_extrapolated(x)
}

fn are_lines_crossing(fr: &Frags, lines: &[Segment]) -> bool {
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let start = lines[i].min_x.max(lines[j].min_x);
            let stop = lines[i].max_x.min(lines[j].max_x);
            if start >= stop {
                continue;
            }
            let ds = extrapolated_line_y(fr, &lines[i], start)
                - extrapolated_line_y(fr, &lines[j], start);
            let de =
                extrapolated_line_y(fr, &lines[i], stop) - extrapolated_line_y(fr, &lines[j], stop);
            if ds == 0.0 || de == 0.0 || (ds > 0.0) != (de > 0.0) {
                return true;
            }
        }
    }
    false
}

fn are_lines_parallel(fr: &Frags, lines: &[Segment], unit: f64) -> bool {
    let all: Vec<usize> = lines.iter().flat_map(|l| l.frags.iter().copied()).collect();
    if all.is_empty() {
        return false;
    }
    let angles: Vec<f64> = all.iter().map(|&f| fr.b(f).rect.angle).collect();
    let avg = np_mean(&angles);
    !all.iter()
        .any(|&f| (fr.b(f).rect.angle - avg).abs() > 10.0 && fr.b(f).rect.w > 2.0 * unit)
}

fn begins_or_ends_on_one_staff_line(
    fr: &Frags,
    line: &AngledBox,
    lines: &[Segment],
    unit: f64,
) -> bool {
    lines.iter().any(|l| match l.get_at(fr, line.rect.cx) {
        None => false,
        Some(f) => (fr.b(f).get_center_extrapolated(line.rect.cx) - line.rect.cy).abs() < unit,
    })
}

/// homr `find_staff_anchors`.
pub fn find_staff_anchors(
    fr: &Frags,
    staff_lines: &[usize],
    symbols: &[AngledBox],
    are_clefs: bool,
) -> Vec<Anchor> {
    let mut result = Vec::new();
    for center in symbols {
        let shifts: &[f64] = if are_clefs {
            &[-10.0, 0.0, 10.0, 30.0, 60.0, 80.0]
        } else {
            &[-10.0, -5.0, 0.0, 5.0, 10.0]
        };
        for &dx in shifts {
            let symbol = if dx == 0.0 {
                center.clone()
            } else {
                center.move_to_x_horizontal_by(dx)
            };
            let unit = py_round(symbol.rect.h / (LINES - 1) as f64);
            let thick = symbol.make_box_taller(unit);
            let over: Vec<usize> = staff_lines
                .iter()
                .copied()
                .filter(|&l| fr.b(l).is_intersecting(&thick))
                .collect();
            let mut connected = connect_staff_lines(fr, &over, unit);
            if connected.len() > LINES {
                connected.retain(|l| l.max_x - l.min_x > 2.0 * unit);
            }
            if connected.len() != LINES
                || !are_lines_parallel(fr, &connected, unit)
                || are_lines_crossing(fr, &connected)
                || (!are_clefs && !begins_or_ends_on_one_staff_line(fr, &symbol, &connected, unit))
            {
                continue;
            }
            result.push(Anchor::new(fr, connected, symbol));
        }
    }
    result
}

fn staff_for_anchor(fr: &Frags, anchor: &Anchor, staffs: &[RawStaff]) -> Option<usize> {
    staffs.iter().position(|s| {
        anchor
            .lines
            .iter()
            .enumerate()
            .any(|(i, l)| fr.set(&l.frags).is_subset(&fr.set(&s.lines[i].frags)))
    })
}

/// homr `find_raw_staffs_by_connecting_line_fragments`.
pub fn find_raw_staffs(fr: &Frags, anchors: &[Anchor], fragments: &[usize]) -> Vec<RawStaff> {
    let mut staffs: Vec<RawStaff> = Vec::new();
    for (staff_id, anchor) in anchors.iter().enumerate() {
        let existing = staff_for_anchor(fr, anchor, &staffs);
        let inzone: Vec<usize> = fragments
            .iter()
            .copied()
            .filter(|&f| {
                let cy = fr.b(f).rect.cy;
                cy >= anchor.zone.0 as f64 && cy <= anchor.zone.1 as f64
            })
            .collect();
        let connected = connect_staff_lines(fr, &inzone, anchor.average_unit_size);
        let mut lines = Vec::new();
        for al in &anchor.lines {
            let req = fr.set(&al.frags);
            let m: Vec<&Segment> = connected
                .iter()
                .filter(|l| req.is_subset(&fr.set(&l.frags)))
                .collect();
            if m.len() == 1 {
                lines.push(m[0].clone());
            } else {
                lines.push(al.clone());
            }
        }
        let new = RawStaff::new(fr, staff_id as i64, lines, vec![anchor.clone()]);
        match existing {
            Some(e) => {
                let old = staffs.remove(e);
                staffs.push(old.merge(fr, &new));
            }
            None => staffs.push(new),
        }
    }
    staffs
}

/// homr `remove_duplicate_staffs`.
pub fn remove_duplicate_staffs(staffs: Vec<RawStaff>) -> Vec<RawStaff> {
    let mut result: Vec<RawStaff> = Vec::new();
    for s in staffs {
        let over: Vec<usize> = (0..result.len())
            .filter(|&i| s.b.is_overlapping(&result[i].b))
            .collect();
        if over.is_empty() {
            result.push(s);
            continue;
        }
        if over.len() >= 2 {
            continue;
        }
        if result[over[0]].anchors.len() < s.anchors.len() {
            let gone = result[over[0]].b.rect;
            result.retain(|r| r.b.rect != gone);
            result.push(s);
        }
    }
    result
}

fn gap_has_ink(l: &RawStaff, r: &RawStaff, ink: &Gray) -> bool {
    let x0 = (l.max_x as i64).max(0);
    let x1 = r.min_x as i64;
    let y0 = (l.min_y.min(r.min_y) as i64).max(0);
    let y1 = l.max_y.max(r.max_y) as i64;
    if x1 <= x0 {
        return true;
    }
    let (w, h) = (ink.width as i64, ink.height as i64);
    let (xa, xb) = (x0.min(w), x1.min(w));
    let (ya, yb) = (y0.min(h), y1.clamp(0, h));
    if xb <= xa {
        return false; // mean of nothing is nan
    }
    let cols = (xa..xb)
        .filter(|&x| (ya..yb).any(|y| ink.get(x as usize, y as usize) > 0))
        .count();
    cols as f64 / (xb - xa) as f64 >= 0.5
}

fn continues(fr: &Frags, l: &RawStaff, r: &RawStaff) -> bool {
    let units: Vec<f64> = l
        .anchors
        .iter()
        .chain(&r.anchors)
        .map(|a| a.average_unit_size)
        .collect();
    let unit = np_mean(&units);
    let gap = r.min_x - l.max_x;
    if unit <= 0.0 || gap < 0.0 || gap > 15.0 * unit {
        return false;
    }
    let up = l
        .lines
        .iter()
        .zip(&r.lines)
        .filter(|(ll, rl)| {
            let ly = fr
                .b(*ll.frags.last().unwrap())
                .get_center_extrapolated(ll.max_x);
            let ry = fr.b(rl.frags[0]).get_center_extrapolated(rl.min_x);
            (ly - ry).abs() <= unit / 2.0
        })
        .count();
    up >= LINES - 1
}

/// homr `merge_staffs_in_same_row`.
pub fn merge_staffs_in_same_row(fr: &Frags, staffs: Vec<RawStaff>, ink: &Gray) -> Vec<RawStaff> {
    let mut result = staffs;
    result.sort_by(|a, b| a.min_x.partial_cmp(&b.min_x).unwrap());
    let mut i = 0;
    while i < result.len() {
        let right = (i + 1..result.len()).find(|&j| {
            continues(fr, &result[i], &result[j]) && gap_has_ink(&result[i], &result[j], ink)
        });
        match right {
            None => i += 1,
            Some(j) => {
                let merged = result[i].merge(fr, &result[j]);
                result.remove(j);
                result[i] = merged;
            }
        }
    }
    result
}

/// homr `find_peaks.find_peaks`.
pub fn find_peaks(x: &[f64], height: f64, distance: f64, prominence: f64) -> Vec<usize> {
    let n = x.len();
    if n < 3 {
        return Vec::new();
    }
    let mut peaks = Vec::new();
    let mut i = 1;
    while i < n - 1 {
        // homr has two branches here (rise, flat start) with the same body
        if x[i] >= x[i - 1] {
            let mut j = i;
            while j < n - 1 && x[j] == x[j + 1] {
                j += 1;
            }
            if j < n - 1 && x[j] > x[j + 1] {
                peaks.push((i + j) / 2);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    peaks.retain(|&p| x[p] >= height);
    peaks.retain(|&p| {
        let mut lmin = x[p];
        for k in (0..p).rev() {
            if x[k] > x[p] {
                break;
            }
            lmin = lmin.min(x[k]);
        }
        let mut rmin = x[p];
        for &v in &x[p + 1..] {
            if v > x[p] {
                break;
            }
            rmin = rmin.min(v);
        }
        x[p] - lmin.max(rmin) >= prominence
    });
    if peaks.len() > 1 {
        // np.argsort (quicksort, not stable) then reversed: ties are rare in
        // normalised row counts; we break them by the larger index first.
        let mut by: Vec<usize> = peaks.clone();
        by.sort_by(|&a, &b| x[b].partial_cmp(&x[a]).unwrap().then(b.cmp(&a)));
        let mut keep: Vec<usize> = Vec::new();
        for p in by {
            if keep
                .iter()
                .all(|&k| (k as f64 - p as f64).abs() >= distance)
            {
                keep.push(p);
            }
        }
        keep.sort();
        peaks = keep;
    }
    peaks
}

/// homr `find_horizontal_lines` on the columns `x0..x1` of `img`.
fn find_horizontal_lines(img: &Gray, x0: usize, x1: usize, unit: f64) -> Vec<Vec<usize>> {
    let mut count = vec![0f64; img.height + 2];
    for y in 0..img.height {
        count[y + 1] = (x0..x1).filter(|&x| img.get(x, y) > 0).count() as f64;
    }
    let std = np_std(&count);
    if std == 0.0 {
        return Vec::new();
    }
    let mean = np_mean(&count);
    let norm: Vec<f64> = count.iter().map(|c| (c - mean) / std).collect();
    let centers: Vec<usize> = find_peaks(&norm, 0.0, 0.7 * unit, 1.0)
        .into_iter()
        .map(|c| c - 1)
        .collect();
    if centers.len() < LINES {
        return Vec::new();
    }
    // filter_line_peaks: only the groups are used
    let gaps: Vec<f64> = centers.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let mut sorted = gaps.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cnt = 5usize
        .max(py_round(centers.len() as f64 * 0.2) as usize)
        .min(sorted.len());
    let approx = np_mean(&sorted[..cnt]);
    let max_gap = approx * 1.5;
    let mut groups = Vec::with_capacity(centers.len());
    let mut g: i64 = -1;
    let mut prev = centers[0] as f64 - max_gap - 1.0;
    for &c in &centers {
        if c as f64 - prev > max_gap {
            g += 1;
        }
        groups.push(g);
        prev = c as f64;
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut cur_g = groups[0];
    for (i, &c) in centers.iter().enumerate() {
        if groups[i] != cur_g {
            out.push(std::mem::take(&mut cur));
            cur_g = groups[i];
        }
        cur.push(c);
    }
    out.push(cur);
    out.into_iter()
        .filter(|v| v.len() == LINES)
        .map(|mut v| {
            v.sort();
            v
        })
        .collect()
}

fn five_line_groups(img: &Gray, zones: &[(usize, usize)], unit: f64) -> Vec<AngledBox> {
    let mut result = Vec::new();
    for &(a, b) in zones {
        for g in find_horizontal_lines(img, a, b, unit) {
            let (mn, mx) = (g[0] as f64, g[LINES - 1] as f64);
            let cy = (mn + mx) / 2.0;
            let cx = a as f64 + (b - a) as f64 / 2.0;
            let r = geom::RotRect {
                cx: cx as i64 as f64,
                cy: cy as i64 as f64,
                w: (b - a) as f64,
                h: (mx - mn) as i64 as f64,
                angle: 0.0,
            };
            result.push(AngledBox::rotated(r, Vec::new(), 0));
        }
    }
    result
}

fn init_zone(clef_anchors: &[Anchor], width: usize) -> Vec<(usize, usize)> {
    let mk = |s: f64, e: f64| ((s as i64).max(0), (e as i64).min(width as i64));
    let mut ranges: Vec<(i64, i64)> = clef_anchors
        .iter()
        .map(|c| mk(c.symbol.bottom_left().0, c.symbol.top_right().0 + 10.0))
        .collect();
    ranges.sort_by_key(|r| r.0);
    let mut result: Vec<(i64, i64)> = Vec::new();
    for r in ranges {
        match result.last_mut() {
            Some(last) if r.0 < last.1 => last.1 = r.1,
            _ => result.push(r),
        }
    }
    result
        .into_iter()
        .map(|(a, b)| (a as usize, (b.max(a)) as usize))
        .collect()
}

fn predict_other_anchors_from_clefs(clef_anchors: &[Anchor], img: &Gray) -> Vec<AngledBox> {
    if clef_anchors.is_empty() {
        return Vec::new();
    }
    let units: Vec<f64> = clef_anchors.iter().map(|a| a.average_unit_size).collect();
    let avg = np_mean(&units);
    let symbols: Vec<AngledBox> = clef_anchors.iter().map(|a| a.symbol.clone()).collect();
    let zones = init_zone(clef_anchors, img.width);
    five_line_groups(img, &zones, avg)
        .into_iter()
        .filter(|r| !r.is_overlapping_with_any(&symbols))
        .collect()
}

fn filter_unusual_anchors(anchors: Vec<Anchor>) -> Vec<Anchor> {
    if anchors.is_empty() {
        return anchors;
    }
    let u: Vec<f64> = anchors.iter().map(|a| a.average_unit_size).collect();
    let (m, s) = (np_mean(&u), np_std(&u));
    anchors
        .into_iter()
        .filter(|a| (a.average_unit_size - m).abs() <= 3.0 * s)
        .collect()
}

fn main_column(ext: &[(f64, f64)]) -> (f64, f64) {
    let longest = ext
        .iter()
        .map(|(l, r)| r - l)
        .fold(f64::NEG_INFINITY, f64::max);
    let long: Vec<&(f64, f64)> = ext
        .iter()
        .filter(|(l, r)| r - l >= 0.75 * longest)
        .collect();
    let ls: Vec<f64> = long.iter().map(|e| e.0).collect();
    let rs: Vec<f64> = long.iter().map(|e| e.1).collect();
    (np_median(&ls), np_median(&rs))
}

fn share_in_column(l: f64, r: f64, c: (f64, f64)) -> f64 {
    let inside = r.min(c.1) - l.max(c.0);
    if r > l {
        inside / (r - l)
    } else {
        0.0
    }
}

fn find_staffs_without_clef(
    fr: &Frags,
    raw: &[RawStaff],
    clef_anchors: &[Anchor],
    fragments: &[usize],
    img: &Gray,
) -> Vec<RawStaff> {
    if raw.is_empty() || clef_anchors.is_empty() {
        return Vec::new();
    }
    let units: Vec<f64> = clef_anchors.iter().map(|a| a.average_unit_size).collect();
    let unit = np_mean(&units);
    let width = img.width;
    let sw = 10usize.max((2.0 * unit) as usize);
    let zones: Vec<(usize, usize)> = [0.2, 0.35, 0.5, 0.65, 0.8]
        .iter()
        .map(|f| {
            let s = (f * width as f64) as usize;
            (s, width.min(s + sw))
        })
        .collect();
    let boxes = five_line_groups(img, &zones, unit);
    let anchors = filter_unusual_anchors(find_staff_anchors(fr, fragments, &boxes, true));
    let cands = remove_duplicate_staffs(find_raw_staffs(fr, &anchors, fragments));
    let ext: Vec<(f64, f64)> = raw.iter().map(|s| (s.min_x, s.max_x)).collect();
    let col = main_column(&ext);
    let min_width = (col.1 - col.0) / 2.0;
    cands
        .into_iter()
        .filter(|c| {
            c.max_x - c.min_x >= min_width
                && share_in_column(c.min_x, c.max_x, col) >= 0.9
                && !raw.iter().any(|s| c.b.is_overlapping(&s.b))
        })
        .collect()
}

fn resample_segment(fr: &Frags, anchor: &Anchor, staff: &RawStaff, xs: &[i64]) -> Vec<Point> {
    let x = anchor.symbol.rect.cx;
    let first: Vec<usize> = anchor.lines.iter().map(|l| l.frags[0]).collect();
    let centers: Vec<f64> = first
        .iter()
        .map(|&f| fr.b(f).get_center_extrapolated(x))
        .collect();
    let angles: Vec<f64> = first.iter().map(|&f| fr.b(f).rect.angle).collect();
    let mut prev = Point::new(x, centers, np_mean(&angles));
    let unit = anchor.average_unit_size;
    let mut out = Vec::new();
    for &xi in xs {
        let x = xi as f64;
        let lines: Vec<Option<usize>> = staff.lines.iter().map(|l| l.get_at(fr, x)).collect();
        let mut ac: Vec<Option<f64>> = lines
            .iter()
            .map(|l| l.map(|f| fr.b(f).get_center_extrapolated(x)))
            .collect();
        let vals: Vec<f64> = ac.iter().flatten().copied().collect();
        if vals.is_empty() {
            continue;
        }
        for (i, w) in vals.windows(2).enumerate() {
            if w[1] - w[0] < 0.5 * unit {
                ac[i] = None;
                ac[i + 1] = None;
            }
        }
        for (i, &py) in prev.y.iter().enumerate() {
            if let Some(c) = ac[i] {
                if (c - py).abs() > 0.5 * unit {
                    ac[i] = None;
                }
            }
        }
        let n = ac.len();
        let mut pc: i64 = -1;
        for i in (0..n).chain((0..n).rev()) {
            if ac[i].is_some() {
                pc = i as i64;
            } else if pc >= 0 {
                if let Some(c) = ac[pc as usize] {
                    ac[i] = Some(c + unit * (i as i64 - pc) as f64);
                }
            }
        }
        if ac.iter().any(|c| c.is_none()) {
            continue;
        }
        let la: Vec<f64> = lines
            .iter()
            .flatten()
            .map(|&f| fr.b(f).rect.angle)
            .collect();
        prev = Point::new(x, ac.into_iter().flatten().collect(), np_mean(&la));
        out.push(prev.clone());
    }
    out
}

fn py_range(a: i64, b: i64, step: i64) -> Vec<i64> {
    let mut v = Vec::new();
    let mut x = a;
    while x < b {
        v.push(x);
        x += step;
    }
    v
}

fn resample_staff(fr: &Frags, staff: &RawStaff) -> Option<Staff> {
    let mut anchors: Vec<&Anchor> = staff.anchors.iter().collect();
    anchors.sort_by(|a, b| a.symbol.rect.cx.partial_cmp(&b.symbol.rect.cx).unwrap());
    let d = 10.0;
    let rtd = |x: f64| (py_round(x / d) as i64) * 10;
    let start = (staff.min_x / d).floor() * d;
    let stop = ((staff.max_x / d).floor() + 1.0) * d;
    let mut grid: Vec<Point> = Vec::new();
    let mut x = start;
    for (i, a) in anchors.iter().enumerate() {
        let acx = a.symbol.rect.cx;
        let to_left = py_range(rtd(x), rtd(acx), 10);
        let (to_right, right_stop) = if i + 1 < anchors.len() {
            let (s, e) = (
                acx as i64,
                ((acx + anchors[i + 1].symbol.rect.cx) / 2.0) as i64,
            );
            (py_range(s, e, 10), e)
        } else {
            let (s, e) = (rtd(acx), rtd(stop));
            (py_range(s, e, 10), e)
        };
        x = right_stop as f64;
        let rev: Vec<i64> = to_left.iter().rev().copied().collect();
        let mut left = resample_segment(fr, a, staff, &rev);
        left.reverse();
        grid.extend(left);
        grid.extend(resample_segment(fr, a, staff, &to_right));
    }
    if grid.is_empty() {
        None
    } else {
        Some(Staff::new(grid))
    }
}

fn filter_edge_of_vision(staffs: Vec<Staff>, w: usize, h: usize) -> Vec<Staff> {
    let widths: Vec<f64> = staffs.iter().map(|s| s.max_x - s.min_x).collect();
    let usual = np_mean(&widths);
    staffs
        .into_iter()
        .filter(|s| {
            if s.max_y >= h as f64 || s.min_y < 0.0 {
                return false;
            }
            let short = s.max_x - s.min_x < usual / 2.0;
            let edge = s.min_x < 0.01 * w as f64 || s.max_x > 0.99 * w as f64;
            !(edge && short)
        })
        .collect()
}

fn filter_neighbor_page(staffs: Vec<Staff>) -> Vec<Staff> {
    if staffs.is_empty() {
        return staffs;
    }
    let ext: Vec<(f64, f64)> = staffs.iter().map(|s| (s.min_x, s.max_x)).collect();
    let col = main_column(&ext);
    let min_width = (col.1 - col.0) / 2.0;
    staffs
        .into_iter()
        .filter(|s| s.max_x - s.min_x >= min_width || share_in_column(s.min_x, s.max_x, col) >= 0.5)
        .collect()
}

/// homr `detect_staff`. `image` is the staff mask, `symbols` the symbol mask.
pub fn detect_staff(
    fr: &Frags,
    image: &Gray,
    clefs_keys: &[AngledBox],
    bar_lines: &[AngledBox],
    symbols: &Gray,
) -> Vec<Staff> {
    let all: Vec<usize> = (0..fr.boxes.len()).collect();
    let clef_anchors = find_staff_anchors(fr, &all, clefs_keys, true);
    let mut anchors = clef_anchors.clone();
    let others = predict_other_anchors_from_clefs(&clef_anchors, image);
    anchors.extend(find_staff_anchors(fr, &all, &others, true));
    anchors.extend(find_staff_anchors(fr, &all, bar_lines, false));
    let anchors = filter_unusual_anchors(anchors);
    let raw = remove_duplicate_staffs(find_raw_staffs(fr, &anchors, &all));
    let mut ink = image.clone();
    for (o, &s) in ink.data.iter_mut().zip(&symbols.data) {
        *o = (*o > 0 || s > 0) as u8;
    }
    let mut raw = merge_staffs_in_same_row(fr, raw, &ink);
    let without = find_staffs_without_clef(fr, &raw, &clef_anchors, &all, image);
    raw.extend(without);
    let staffs: Vec<Staff> = raw.iter().filter_map(|s| resample_staff(fr, s)).collect();
    let staffs = filter_edge_of_vision(staffs, image.width, image.height);
    let mut staffs = filter_neighbor_page(staffs);
    staffs.sort_by(|a, b| a.min_y.partial_cmp(&b.min_y).unwrap());
    staffs
}

/// homr `break_wide_fragments`.
pub fn break_wide_fragments(frags: Vec<AngledBox>, limit: f64) -> Vec<AngledBox> {
    let mut result = Vec::new();
    for f in frags {
        let mut rem = f;
        while rem.rect.w > limit {
            let min_x = rem.contours.iter().map(|c| c.0).min().unwrap() as f64;
            let mut left: Vec<_> = rem
                .contours
                .iter()
                .copied()
                .filter(|c| (c.0 as f64) < min_x + limit)
                .collect();
            let mut right: Vec<_> = rem
                .contours
                .iter()
                .copied()
                .filter(|c| (c.0 as f64) >= min_x + limit)
                .collect();
            left.sort_by_key(|c| c.0);
            right.sort_by_key(|c| c.0);
            if left.is_empty() || right.is_empty() {
                break;
            }
            left.push(right[0]);
            right.push(*left.last().unwrap());
            let id = rem.debug_id;
            result.push(crate::boxes::rotated_box_of(left, id));
            rem = crate::boxes::rotated_box_of(right, id);
        }
        result.push(rem);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_peaks_like_homr() {
        // plateau peak in the middle, prominence and distance filters
        let x = [0.0, 1.0, 3.0, 3.0, 1.0, 0.0, 2.0, 0.5, 0.0];
        assert_eq!(find_peaks(&x, 0.0, 1.0, 1.0), vec![2, 6]);
        assert_eq!(find_peaks(&x, 0.0, 5.0, 1.0), vec![2]);
        assert_eq!(find_peaks(&x, 2.5, 1.0, 1.0), vec![2]);
    }
}
