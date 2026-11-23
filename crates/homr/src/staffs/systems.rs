//! homr `brace_dot_detection`, `MultiStaff` (grand staffs) and
//! `system_repair`, ported from homr 0.7.1 (AGPL-3.0, Copyright the homr
//! contributors). homr compares staffs by identity, so staffs live in an
//! arena and systems hold indices into it.
use super::model::{np_median, py_round, Staff};
use crate::boxes::{self, geom, AngledBox};
use crate::Gray;
use std::collections::{HashMap, HashSet};

const LINES: usize = 5;

/// The staffs of a page; merges add new staffs at the end.
pub struct Arena(pub Vec<Staff>);

impl Arena {
    fn merge(&mut self, a: usize, b: usize) -> usize {
        let m = self.0[a].merge(&self.0[b]);
        self.0.push(m);
        self.0.len() - 1
    }
    fn push(&mut self, s: Staff) -> usize {
        self.0.push(s);
        self.0.len() - 1
    }
    fn g(&self, i: usize) -> bool {
        self.0[i].is_grandstaff
    }
}

/// A homr `MultiStaff`: staff indices sorted by min_y, and its connections.
#[derive(Debug, Clone)]
pub struct Row {
    pub staffs: Vec<usize>,
    pub connections: Vec<AngledBox>,
}

impl Row {
    fn new(ar: &Arena, mut staffs: Vec<usize>, connections: Vec<AngledBox>) -> Row {
        staffs.sort_by(|&a, &b| ar.0[a].min_y.partial_cmp(&ar.0[b].min_y).unwrap());
        Row {
            staffs,
            connections,
        }
    }
    fn merge(&self, ar: &Arena, o: &Row) -> Row {
        let mut st: Vec<usize> = Vec::new();
        for &s in self.staffs.iter().chain(&o.staffs) {
            if !st.contains(&s) {
                st.push(s);
            }
        }
        let mut cn: Vec<AngledBox> = Vec::new();
        for c in self.connections.iter().chain(&o.connections) {
            if !cn.contains(c) {
                cn.push(c.clone());
            }
        }
        Row::new(ar, st, cn)
    }
}

// ---------------------------------------------------------------- braces

/// homr `prepare_brace_dot_image`.
pub fn prepare_brace_dot_image(symbols: &Gray, staff: &Gray) -> Gray {
    let mut d = symbols.clone();
    for (o, &s) in d.data.iter_mut().zip(&staff.data) {
        *o = o.saturating_sub(s);
    }
    let e = boxes::morph::erode(&d, &boxes::morph::ellipse(1, 5));
    boxes::morph::dilate(&e, &boxes::morph::ellipse(5, 35))
}

fn trim_to_core_span(s: &AngledBox) -> AngledBox {
    let (x, y, w, h) = geom::bounding_rect(&s.contours);
    if h <= 0 || w <= 0 {
        return s.clone();
    }
    let mut mask = Gray::new(w as usize, h as usize);
    let shifted: Vec<_> = s.contours.iter().map(|p| (p.0 - x, p.1 - y)).collect();
    geom::fill_poly(&mut mask, &[shifted], 255);
    let rows: Vec<usize> = (0..h as usize)
        .map(|r| {
            mask.data[r * w as usize..(r + 1) * w as usize]
                .iter()
                .filter(|&&v| v > 0)
                .count()
        })
        .collect();
    let max_w = *rows.iter().max().unwrap();
    if max_w == 0 {
        return s.clone();
    }
    let core: Vec<usize> = (0..rows.len())
        .filter(|&r| rows[r] as f64 >= max_w as f64 * 0.5)
        .collect();
    if core.is_empty() {
        return s.clone();
    }
    let cmin = y as i64 + core[0] as i64;
    let cmax = y as i64 + *core.last().unwrap() as i64 + 1;
    let ch = cmax - cmin;
    if ch <= 0 {
        return s.clone();
    }
    let r = geom::RotRect {
        cx: s.rect.cx,
        cy: (cmin + cmax) as f64 / 2.0,
        w: s.rect.w,
        h: ch as f64,
        angle: s.rect.angle,
    };
    AngledBox::rotated(r, s.contours.clone(), s.debug_id)
}

fn filter_tall(brace: Vec<AngledBox>, ar: &Arena, staffs: &[usize]) -> Vec<AngledBox> {
    let rough = ar.0[staffs[0]].average_unit_size;
    brace
        .into_iter()
        .filter(|s| s.rect.h > 2.0 * rough && s.rect.w < 3.0 * rough && s.rect.w >= 5.0)
        .filter(|s| {
            let mut best = staffs[0];
            let mut bd = f64::INFINITY;
            for &st in staffs {
                let d = ar.0[st].y_distance_to(s.center());
                if d < bd {
                    bd = d;
                    best = st;
                }
            }
            s.rect.h > 4.0 * ar.0[best].average_unit_size
        })
        .collect()
}

/// homr `_get_connections_between_staffs` (bar line and clef connections are
/// always empty: homr only adds notes to staffs).
fn connections(ar: &Arena, s1: usize, s2: usize, brace: &[AngledBox]) -> Vec<AngledBox> {
    let (a, b) = (&ar.0[s1], &ar.0[s2]);
    let tol = py_round(a.average_unit_size * 2.0);
    brace
        .iter()
        .filter(|s| {
            let th = s.make_box_thicker(tol);
            match (a.get_at(s.rect.cx), b.get_at(s.rect.cx)) {
                (Some(p1), Some(p2)) => {
                    th.is_overlapping(&p1.to_bounding_box())
                        && th.is_overlapping(&p2.to_bounding_box())
                }
                _ => false,
            }
        })
        .cloned()
        .collect()
}

fn score_pair(
    ar: &Arena,
    sym: &AngledBox,
    up: usize,
    lo: usize,
    above: Option<usize>,
    below: Option<usize>,
) -> f64 {
    let (u, l) = (&ar.0[up], &ar.0[lo]);
    let unit = np_median(&[u.average_unit_size, l.average_unit_size]);
    let xthr = 5.0 * unit;
    let ythr = 0.5 * sym.rect.h;
    let smin = sym.rect.cy - sym.rect.h / 2.0;
    let smax = sym.rect.cy + sym.rect.h / 2.0;
    if let Some(a) = above {
        if smin < (ar.0[a].max_y + u.min_y) / 2.0 {
            return 0.0;
        }
    }
    if let Some(b) = below {
        if smax > (l.max_y + ar.0[b].min_y) / 2.0 {
            return 0.0;
        }
    }
    let sx = sym.rect.cx;
    let xd = (u.min_x - sx).abs().min((l.min_x - sx).abs());
    let yo = smax.min(l.max_y) - smin.max(u.min_y);
    if !(xd < xthr && yo > ythr && yo > xd) {
        return 0.0;
    }
    let union = smax.max(l.max_y) - smin.min(u.min_y);
    let iou = if union > 0.0 { yo / union } else { 0.0 };
    iou - xd / xthr
}

fn create_grandstaffs(ar: &mut Arena, row: Row, brace: &[AngledBox]) -> Row {
    let n = row.staffs.len();
    if n < 2 || brace.is_empty() {
        return row;
    }
    let mut pairs: Vec<(usize, f64)> = Vec::new();
    for i in 0..n - 1 {
        let above = if i > 0 { Some(row.staffs[i - 1]) } else { None };
        let below = row.staffs.get(i + 2).copied();
        let best = brace
            .iter()
            .map(|s| score_pair(ar, s, row.staffs[i], row.staffs[i + 1], above, below))
            .fold(f64::NEG_INFINITY, f64::max);
        if best > 0.0 {
            pairs.push((i, best));
        }
    }
    // sorted(..., reverse=True) is stable: equal scores keep their order
    let mut sorted = pairs.clone();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let mut used: HashSet<usize> = HashSet::new();
    let mut sel: Vec<usize> = Vec::new();
    for (i, _) in sorted {
        if used.contains(&i) || used.contains(&(i + 1)) {
            continue;
        }
        sel.push(i);
        used.insert(i);
        used.insert(i + 1);
    }
    if sel.is_empty() {
        return row;
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if sel.iter().any(|&p| p == i || p + 1 == i) {
            out.push(ar.merge(row.staffs[i], row.staffs[i + 1]));
            i += 2;
            continue;
        }
        out.push(row.staffs[i]);
        i += 1;
    }
    Row::new(ar, out, row.connections)
}

/// homr `find_braces_brackets_and_grand_staff_lines`.
pub fn find_braces(ar: &mut Arena, staffs: &[usize], brace: Vec<AngledBox>) -> Vec<Row> {
    let brace: Vec<AngledBox> = brace.iter().map(trim_to_core_span).collect();
    let brace = filter_tall(brace, ar, staffs);
    let mut result: Vec<Row> = Vec::new();
    for (i, &s) in staffs.iter().enumerate() {
        let mut nb = Vec::new();
        if i > 0 {
            nb.push(staffs[i - 1]);
        }
        if i + 1 < staffs.len() {
            nb.push(staffs[i + 1]);
        }
        let mut any = false;
        for n in nb {
            let c = connections(ar, s, n, &brace);
            if !c.is_empty() {
                result.push(Row::new(ar, vec![s, n], c));
                any = true;
            }
        }
        if !any {
            result.push(Row::new(ar, vec![s], Vec::new()));
        }
    }
    // _merge_multi_staff_if_they_share_a_staff
    let mut merged: Vec<Row> = Vec::new();
    for r in result {
        let pos = merged
            .iter()
            .position(|e| e.staffs.iter().any(|s| r.staffs.contains(s)));
        match pos {
            Some(p) => {
                let e = merged.remove(p);
                let m = e.merge(ar, &r);
                merged.push(m);
            }
            None => merged.push(r),
        }
    }
    merged
        .into_iter()
        .map(|r| create_grandstaffs(ar, r, &brace))
        .collect()
}

// ---------------------------------------------------------------- repair

type Layout = Vec<bool>;

fn layout(ar: &Arena, r: &Row) -> Layout {
    r.staffs.iter().map(|&s| ar.g(s)).collect()
}

fn flatten(rows: &[Row]) -> Vec<usize> {
    rows.iter().flat_map(|r| r.staffs.iter().copied()).collect()
}

fn is_uniform(ar: &Arena, rows: &[Row]) -> bool {
    let set: HashSet<Layout> = rows.iter().map(|r| layout(ar, r)).collect();
    set.len() == 1
}

/// An insertion-ordered counter, like Python's `Counter`.
#[derive(Default, Clone)]
struct Counter {
    keys: Vec<Layout>,
    n: HashMap<Layout, usize>,
}

impl Counter {
    fn add(&mut self, k: Layout) {
        if !self.n.contains_key(&k) {
            self.keys.push(k.clone());
        }
        *self.n.entry(k).or_insert(0) += 1;
    }
    fn get(&self, k: &Layout) -> usize {
        self.n.get(k).copied().unwrap_or(0)
    }
    fn most_common(&self) -> Option<Layout> {
        let max = self.keys.iter().map(|k| self.n[k]).max()?;
        let top: Vec<&Layout> = self.keys.iter().filter(|k| self.n[*k] == max).collect();
        if top.len() > 1 {
            None
        } else {
            Some(top[0].clone())
        }
    }
}

fn vertical_gap(ar: &Arena, up: usize, lo: usize) -> Option<f64> {
    let mut pts: HashMap<i64, &super::model::Point> = HashMap::new();
    for p in &ar.0[up].grid {
        pts.insert(py_round(p.x) as i64, p);
    }
    let gaps: Vec<f64> = ar.0[lo]
        .grid
        .iter()
        .filter_map(|p| {
            pts.get(&(py_round(p.x) as i64))
                .map(|u| p.y[0] - u.y[u.y.len() - 1])
        })
        .collect();
    if gaps.is_empty() {
        None
    } else {
        Some(np_median(&gaps))
    }
}

fn possible_grandstaffs(
    ar: &Arena,
    rows: &[Row],
    strict: bool,
    guess_lone: bool,
) -> HashSet<(usize, usize)> {
    let flat = flatten(rows);
    let mut system_of: HashMap<usize, usize> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        for &s in &r.staffs {
            system_of.insert(s, i);
        }
    }
    let mut gg = Vec::new();
    for &s in &flat {
        if ar.g(s) {
            for p in &ar.0[s].grid {
                if p.y.len() == 2 * LINES {
                    gg.push(
                        (p.y[LINES] - p.y[LINES - 1]) * (LINES - 1) as f64
                            / (p.y[LINES - 1] - p.y[0]),
                    );
                }
            }
        }
    }
    let max_gap = if gg.is_empty() {
        f64::INFINITY
    } else {
        1.5 * np_median(&gg)
    };
    let pairs: Vec<(usize, usize)> = flat.windows(2).map(|w| (w[0], w[1])).collect();
    let measured: Vec<Option<f64>> = pairs.iter().map(|&(u, l)| vertical_gap(ar, u, l)).collect();
    let gaps: Vec<f64> = measured
        .iter()
        .zip(&pairs)
        .map(|(g, &(u, l))| g.unwrap_or(ar.0[l].min_y - ar.0[u].max_y))
        .collect();
    let mut result = HashSet::new();
    for (i, &(u, l)) in pairs.iter().enumerate() {
        let unit = ar.0[u].average_unit_size.min(ar.0[l].average_unit_size);
        let gap = match measured[i] {
            Some(g) if !(ar.g(u) || ar.g(l)) && g <= max_gap * unit => g,
            _ => continue,
        };
        let nb: Vec<f64> = [i as i64 - 1, i as i64 + 1]
            .iter()
            .filter(|&&j| {
                j >= 0
                    && (j as usize) < gaps.len()
                    && !ar.g(flat[j as usize])
                    && !ar.g(flat[j as usize + 1])
            })
            .map(|&j| gaps[j as usize])
            .collect();
        let connected = system_of[&u] == system_of[&l];
        let lone = [u, l].iter().all(|s| rows[system_of[s]].staffs.len() == 1);
        let guess = !strict && (guess_lone || !lone);
        if guess || connected || nb.iter().all(|&n| gap < 0.8 * n) {
            result.insert((u, l));
        }
    }
    result
}

fn fit_to_layout(
    ar: &mut Arena,
    staffs: &[usize],
    lay: &Layout,
    gs: &HashSet<(usize, usize)>,
) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    for &isg in lay {
        if i < staffs.len() && ar.g(staffs[i]) == isg {
            out.push(staffs[i]);
            i += 1;
        } else if isg
            && i + 1 < staffs.len()
            && !ar.g(staffs[i])
            && !ar.g(staffs[i + 1])
            && gs.contains(&(staffs[i], staffs[i + 1]))
        {
            out.push(ar.merge(staffs[i], staffs[i + 1]));
            i += 2;
        } else {
            return None;
        }
    }
    if i == staffs.len() {
        Some(out)
    } else {
        None
    }
}

fn fit_neighbors(
    ar: &mut Arena,
    rows: &[Row],
    start: usize,
    lay: &Layout,
    gs: &HashSet<(usize, usize)>,
) -> Option<(Vec<usize>, usize)> {
    for end in start + 1..=rows.len() {
        let group = flatten(&rows[start..end]);
        if group.len() > 2 * lay.len() {
            return None;
        }
        if !lay.iter().any(|&b| b) && group.len() == end - start && end - start > 1 {
            continue;
        }
        if let Some(f) = fit_to_layout(ar, &group, lay, gs) {
            return Some((f, end));
        }
    }
    None
}

fn repair(ar: &mut Arena, rows: &[Row], lay: &Layout, strict: bool, guess_lone: bool) -> Vec<Row> {
    let gs = possible_grandstaffs(ar, rows, strict, guess_lone);
    let mut result = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let rl = layout(ar, &rows[i]);
        if &rl == lay {
            result.push(rows[i].clone());
            i += 1;
            continue;
        }
        let row = &rows[i].staffs;
        let repeats = row.len() / lay.len();
        if repeats > 1 && rl == lay.repeat(repeats) {
            for k in (0..row.len()).step_by(lay.len()) {
                let end = (k + lay.len()).min(row.len());
                result.push(Row::new(ar, row[k..end].to_vec(), Vec::new()));
            }
            i += 1;
            continue;
        }
        match fit_neighbors(ar, rows, i, lay, &gs) {
            None => {
                result.push(rows[i].clone());
                i += 1;
            }
            Some((f, end)) => {
                result.push(Row::new(ar, f, Vec::new()));
                i = end;
            }
        }
    }
    result
}

fn repair_score(ar: &mut Arena, rows: &[Row], lay: &Layout) -> f64 {
    let mut system_of: HashMap<usize, usize> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        for &s in &r.staffs {
            system_of.insert(s, i);
        }
    }
    let mut score = 0.0;
    let ng = lay.iter().filter(|&&b| b).count();
    for row in repair(ar, rows, lay, true, true) {
        if &layout(ar, &row) != lay {
            continue;
        }
        let set: HashSet<usize> = row.staffs.iter().copied().collect();
        let contradicts = row.staffs.iter().any(|s| match system_of.get(s) {
            None => true,
            Some(&src) => !rows[src].staffs.iter().all(|x| set.contains(x)),
        });
        score += (row.staffs.len() + ng) as f64 * if contradicts { 0.5 } else { 1.0 };
    }
    score
}

fn mixed_layouts(ar: &Arena, rows: &[Row]) -> Counter {
    let mut c = Counter::default();
    for r in rows {
        let l = layout(ar, r);
        if l.iter().collect::<HashSet<_>>().len() > 1 {
            c.add(l);
        }
    }
    for w in rows.windows(2) {
        let mut p = layout(ar, &w[0]);
        p.extend(layout(ar, &w[1]));
        if p == [false, true] || p == [true, false] {
            c.add(p);
        }
    }
    c
}

fn shown_by_mixed(ar: &Arena, rows: &[Row], lay: &Layout) -> bool {
    let m = mixed_layouts(ar, rows);
    m.most_common().as_ref() == Some(lay) && m.get(lay) > 1
}

fn common_layout(ar: &mut Arena, rows: &[Row]) -> Option<Layout> {
    let mixed = mixed_layouts(ar, rows);
    if let Some(l) = mixed.most_common() {
        if mixed.get(&l) > 1 {
            return Some(l);
        }
    }
    let mut counts = Counter::default();
    for r in rows {
        counts.add(layout(ar, r));
    }
    let mut all: Vec<Layout> = counts.keys.iter().chain(&mixed.keys).cloned().collect();
    all.sort();
    all.dedup();
    let cands: Vec<Layout> = all
        .into_iter()
        .filter(|l| !(l.len() > 1 && l.iter().all(|&b| b) && counts.get(l) < 2))
        .collect();
    let mut best: Option<(f64, usize, Layout)> = None;
    for l in cands {
        let s = repair_score(ar, rows, &l);
        let c = counts.get(&l);
        let better = match &best {
            None => true,
            Some((bs, bc, _)) => s > *bs || (s == *bs && c > *bc),
        };
        if better {
            best = Some((s, c, l));
        }
    }
    best.map(|b| b.2)
}

fn drop_single_at_end(ar: &Arena, rows: Vec<Row>, lay: &Layout) -> Vec<Row> {
    let single: Vec<usize> = (0..rows.len())
        .filter(|&i| layout(ar, &rows[i]) == [false])
        .collect();
    let only_g = !lay.contains(&false);
    if !only_g || single != [rows.len() - 1] || rows.len() < 3 {
        return rows;
    }
    let mut rows = rows;
    rows.pop();
    rows
}

fn periodic_core(ar: &Arena, flat: &[usize]) -> Option<(usize, usize, usize)> {
    let lay: Vec<bool> = flat.iter().map(|&s| ar.g(s)).collect();
    let n = lay.len();
    let mut best: Option<(usize, usize, usize, usize)> = None;
    for period in 1..=n / 2 {
        for ft in 0..=period {
            for bt in 0..=period {
                if ft + bt > n {
                    continue;
                }
                let core = &lay[ft..n - bt];
                if core.len() < 2 * period || !core.len().is_multiple_of(period) {
                    continue;
                }
                if !core.chunks(period).all(|c| c == &core[..period]) {
                    continue;
                }
                let cand = (ft + bt, period, ft, bt);
                if best.is_none_or(|b| (cand.0, cand.1) < (b.0, b.1)) {
                    best = Some(cand);
                }
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3))
}

fn regroup_by_pattern(ar: &Arena, rows: &[Row]) -> Option<Vec<Row>> {
    let flat = flatten(rows);
    let (period, ft, bt) = periodic_core(ar, &flat)?;
    let end = flat.len() - bt;
    let pattern: HashSet<bool> = flat[ft..ft + period].iter().map(|&s| ar.g(s)).collect();
    let trimmed = ft > 0 || bt > 0;
    let nothing_grouped = rows.iter().all(|r| r.staffs.len() == 1);
    if pattern.len() != 2 || (trimmed && !nothing_grouped) {
        return None;
    }
    let mut out: Vec<Row> = flat[..ft]
        .iter()
        .map(|&s| Row::new(ar, vec![s], Vec::new()))
        .collect();
    for i in (ft..end).step_by(period) {
        out.push(Row::new(ar, flat[i..i + period].to_vec(), Vec::new()));
    }
    out.extend(
        flat[end..]
            .iter()
            .map(|&s| Row::new(ar, vec![s], Vec::new())),
    );
    Some(out)
}

fn convert_to_layout(ar: &mut Arena, row: &Row, lay: &Layout) -> Option<Row> {
    if &layout(ar, row) == lay {
        return Some(row.clone());
    }
    let mut singles = Vec::new();
    for &s in &row.staffs {
        if ar.g(s) {
            let (u, l) = ar.0[s].split()?;
            singles.push(ar.push(u));
            singles.push(ar.push(l));
        } else {
            singles.push(s);
        }
    }
    let mut it = singles.into_iter();
    let mut out = Vec::new();
    for &isg in lay {
        let mut s = it.next()?;
        if isg {
            let lo = it.next()?;
            vertical_gap(ar, s, lo)?;
            s = ar.merge(s, lo);
        }
        out.push(s);
    }
    Some(Row::new(ar, out, row.connections.clone()))
}

fn unify_grand_staffs(ar: &mut Arena, rows: Vec<Row>) -> Vec<Row> {
    let per: HashSet<usize> = rows
        .iter()
        .map(|r| r.staffs.iter().map(|&s| if ar.g(s) { 2 } else { 1 }).sum())
        .collect();
    if per.len() != 1 {
        return rows;
    }
    let mut c = Counter::default();
    for r in &rows {
        c.add(layout(ar, r));
    }
    let lay = match c.most_common() {
        None => return rows,
        Some(l) => l,
    };
    let mut out = Vec::new();
    for r in &rows {
        match convert_to_layout(ar, r, &lay) {
            None => return rows,
            Some(x) => out.push(x),
        }
    }
    out
}

/// homr `repair_systems`.
pub fn repair_systems(ar: &mut Arena, rows: Vec<Row>) -> Vec<Row> {
    let rows = unify_grand_staffs(ar, rows);
    if is_uniform(ar, &rows) && rows[0].staffs.len() > 1 {
        return rows;
    }
    let lay = common_layout(ar, &rows);
    let repaired = match &lay {
        None => rows.clone(),
        Some(l) => {
            let gl = shown_by_mixed(ar, &rows, l);
            repair(ar, &rows, l, false, gl)
        }
    };
    if !is_uniform(ar, &repaired) {
        if let Some(r) = regroup_by_pattern(ar, &rows) {
            return r;
        }
    }
    match lay {
        None => repaired,
        Some(l) => drop_single_at_end(ar, repaired, &l),
    }
}
