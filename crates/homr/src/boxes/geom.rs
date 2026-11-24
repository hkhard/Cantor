//! The OpenCV geometry homr uses, ported from OpenCV 4.14 (Apache-2.0,
//! Copyright the OpenCV authors): convex hull (Sklansky), minAreaRect
//! (rotating calipers), boxPoints, fitEllipse, ellipse2Poly,
//! intersectConvexConvex, pointPolygonTest, rotatedRectangleIntersection,
//! boundingRect, contourArea, fillPoly/fillConvexPoly (LINE_8),
//! getAffineTransform and warpAffine (bilinear, constant border).
//!
//! Floating point follows OpenCV's types (f32 where OpenCV uses float), so
//! results match to the last bit in most cases; compilers may fuse
//! multiply-adds differently, so allow for rare 1-ulp differences.
use super::contours::Pt;
use super::sin_table::SIN_TABLE;
use crate::Gray;

/// A point with float coordinates.
pub type Ptf = (f32, f32);

/// `cv::RotatedRect`: centre, size (width, height), angle in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotRect {
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    pub angle: f64,
}

fn sign(v: i64) -> i32 {
    (v > 0) as i32 - (v < 0) as i32
}

fn sklansky(a: &[Pt], start: isize, end: isize, nsign: i32, sign2: i32) -> Vec<isize> {
    let incr: isize = if end > start { 1 } else { -1 };
    let mut pprev = start;
    let mut pcur = pprev + incr;
    let mut pnext = pcur + incr;
    let at = |i: isize| a[i as usize];
    if start == end || at(start) == at(end) {
        return vec![start];
    }
    let mut stack: Vec<isize> = vec![0; a.len() + 3];
    let mut size: usize = 3;
    stack[0] = pprev;
    stack[1] = pcur;
    stack[2] = pnext;
    let end = end + incr;
    while pnext != end {
        let cury = at(pcur).1 as i64;
        let nexty = at(pnext).1 as i64;
        let by = nexty - cury;
        if sign(by) != nsign {
            let ax = (at(pcur).0 - at(pprev).0) as i64;
            let ay = cury - at(pprev).1 as i64;
            let bx = (at(pnext).0 - at(pcur).0) as i64;
            let convexity = ay * bx - ax * by;
            if sign(convexity) == sign2 && (ax != 0 || ay != 0) {
                pprev = pcur;
                pcur = pnext;
                pnext += incr;
                if size >= stack.len() {
                    stack.push(0);
                }
                stack[size] = pnext;
                size += 1;
            } else if pprev == start {
                pcur = pnext;
                stack[1] = pcur;
                pnext += incr;
                stack[2] = pnext;
            } else {
                stack[size - 2] = pnext;
                pcur = pprev;
                pprev = stack[size - 4];
                size -= 1;
            }
        } else {
            pnext += incr;
            stack[size - 1] = pnext;
        }
    }
    stack.truncate(size - 1);
    stack
}

/// `cv2.convexHull(points, clockwise=False, returnPoints=True)`.
pub fn convex_hull(pts: &[Pt]) -> Vec<Pt> {
    let total = pts.len();
    if total == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..total).collect();
    order.sort_by(|&i, &j| (pts[i].0, pts[i].1, i).cmp(&(pts[j].0, pts[j].1, j)));
    let s: Vec<Pt> = order.iter().map(|&i| pts[i]).collect();
    let (mut miny, mut maxy) = (0usize, 0usize);
    for i in 1..total {
        let y = s[i].1;
        if s[miny].1 > y {
            miny = i;
        }
        if s[maxy].1 < y {
            maxy = i;
        }
    }
    let mut hull: Vec<isize> = Vec::new();
    if s[0] == s[total - 1] {
        hull.push(0);
    } else {
        let last = total as isize - 1;
        let tl = sklansky(&s, 0, maxy as isize, -1, 1);
        let tr = sklansky(&s, last, maxy as isize, -1, -1);
        // clockwise == false: swap
        let (tl, tr) = (tr, tl);
        for &v in &tl[..tl.len() - 1] {
            hull.push(v);
        }
        for i in (1..tr.len()).rev() {
            hull.push(tr[i]);
        }
        let stop_idx = if tr.len() > 2 {
            tr[1]
        } else if tl.len() > 2 {
            tl[tl.len() - 2]
        } else {
            -1
        };
        let bl = sklansky(&s, 0, miny as isize, 1, -1);
        let br = sklansky(&s, last, miny as isize, 1, 1);
        let (mut blc, mut brc) = (bl.len(), br.len());
        if stop_idx >= 0 {
            let check_idx = if blc > 2 {
                bl[1]
            } else if blc + brc > 2 {
                br[2 - blc]
            } else {
                -1
            };
            if check_idx == stop_idx
                || (check_idx >= 0 && s[check_idx as usize] == s[stop_idx as usize])
            {
                blc = blc.min(2);
                brc = brc.min(2);
            }
        }
        for &v in &bl[..blc - 1] {
            hull.push(v);
        }
        for i in (1..brc).rev() {
            hull.push(br[i]);
        }
    }
    let mut hb: Vec<usize> = hull.iter().map(|&i| order[i as usize]).collect();
    let nout = hb.len();
    if nout >= 3 {
        let (mut min_idx, mut max_idx, mut lt) = (0usize, 0usize, 0usize);
        for i in 1..nout {
            let idx = hb[i];
            lt += (hb[i - 1] < idx) as usize;
            if lt > 1 && lt + 2 <= i {
                break;
            }
            if idx < hb[min_idx] {
                min_idx = i;
            }
            if idx > hb[max_idx] {
                max_idx = i;
            }
        }
        let mmdist = max_idx.abs_diff(min_idx);
        if (mmdist == 1 || mmdist == nout - 1) && (lt <= 1 || lt + 2 >= nout) {
            let ascending = (max_idx + 1) % nout == min_idx;
            let i0 = if ascending { min_idx } else { max_idx };
            if i0 > 0 {
                let mut tmp = vec![0usize; nout];
                let mut j = i0;
                let mut i = 0;
                while i < nout {
                    let curr = hb[j];
                    tmp[i] = curr;
                    let nj = if j + 1 < nout { j + 1 } else { 0 };
                    let next = hb[nj];
                    if i < nout - 1 && (ascending != (curr < next)) {
                        break;
                    }
                    j = nj;
                    i += 1;
                }
                if i == nout {
                    hb = tmp;
                }
            }
        }
    }
    hb.iter().map(|&i| pts[i]).collect()
}

fn rotating_calipers(points: &[Ptf]) -> [f32; 6] {
    let n = points.len();
    let mut minarea = f32::MAX;
    let mut buf_i = [0usize; 2];
    let mut buf = [0f32; 7];
    let mut inv = vec![0f32; n];
    let mut vect = vec![(0f32, 0f32); n];
    let (mut left, mut bottom, mut right, mut top) = (0usize, 0usize, 0usize, 0usize);
    let mut pt0 = points[0];
    let (mut left_x, mut right_x, mut top_y, mut bottom_y) = (pt0.0, pt0.0, pt0.1, pt0.1);
    for i in 0..n {
        if pt0.0 < left_x {
            left_x = pt0.0;
            left = i;
        }
        if pt0.0 > right_x {
            right_x = pt0.0;
            right = i;
        }
        if pt0.1 > top_y {
            top_y = pt0.1;
            top = i;
        }
        if pt0.1 < bottom_y {
            bottom_y = pt0.1;
            bottom = i;
        }
        let pt = points[if i + 1 < n { i + 1 } else { 0 }];
        let dx = pt.0 as f64 - pt0.0 as f64;
        let dy = pt.1 as f64 - pt0.1 as f64;
        vect[i] = (dx as f32, dy as f32);
        inv[i] = (1.0 / (dx * dx + dy * dy).sqrt()) as f32;
        pt0 = pt;
    }
    // orientation: the hull is counter-clockwise (clockwise=false) -> 1
    let mut base_a: f32;
    let mut base_b: f32;
    let mut seq = [bottom, right, top, left];
    let rot90cw = |v: (f32, f32)| (v.1, -v.0);
    let rot90ccw = |v: (f32, f32)| (-v.1, v.0);
    let rot180 = |v: (f32, f32)| (-v.0, -v.1);
    let first_is_right = |v1: (f32, f32), v2: (f32, f32)| {
        let t = rot90cw(v1);
        t.0 * v2.0 + t.1 * v2.1 < 0.0
    };
    for _ in 0..n {
        let rv = [
            vect[seq[0]],
            rot90cw(vect[seq[1]]),
            rot180(vect[seq[2]]),
            rot90ccw(vect[seq[3]]),
        ];
        let mut main = 0;
        for i in 1..4 {
            if first_is_right(rv[i], rv[main]) {
                main = i;
            }
        }
        let p = seq[main];
        let lx = vect[p].0 * inv[p];
        let ly = vect[p].1 * inv[p];
        match main {
            0 => {
                base_a = lx;
                base_b = ly;
            }
            1 => {
                base_a = ly;
                base_b = -lx;
            }
            2 => {
                base_a = -lx;
                base_b = -ly;
            }
            _ => {
                base_a = -ly;
                base_b = lx;
            }
        }
        seq[main] += 1;
        if seq[main] == n {
            seq[main] = 0;
        }
        let dx = points[seq[1]].0 - points[seq[3]].0;
        let dy = points[seq[1]].1 - points[seq[3]].1;
        let width = dx * base_a + dy * base_b;
        let dx = points[seq[2]].0 - points[seq[0]].0;
        let dy = points[seq[2]].1 - points[seq[0]].1;
        let height = -dx * base_b + dy * base_a;
        let area = width * height;
        if area <= minarea {
            minarea = area;
            buf_i[0] = seq[3];
            buf[1] = base_a;
            buf[2] = width;
            buf[3] = base_b;
            buf[4] = height;
            buf_i[1] = seq[0];
            buf[6] = area;
        }
    }
    let a1 = buf[1];
    let b1 = buf[3];
    let a2 = -buf[3];
    let b2 = buf[1];
    let c1 = a1 * points[buf_i[0]].0 + points[buf_i[0]].1 * b1;
    let c2 = a2 * points[buf_i[1]].0 + points[buf_i[1]].1 * b2;
    let idet = 1.0f32 / (a1 * b2 - a2 * b1);
    let px = (c1 * b2 - c2 * b1) * idet;
    let py = (a1 * c2 - a2 * c1) * idet;
    [px, py, a1 * buf[2], b1 * buf[2], a2 * buf[4], b2 * buf[4]]
}

/// `cv2.minAreaRect(points)`.
pub fn min_area_rect(pts: &[Pt]) -> RotRect {
    let hull: Vec<Ptf> = convex_hull(pts)
        .iter()
        .map(|p| (p.0 as f32, p.1 as f32))
        .collect();
    min_area_rect_hull(&hull)
}

fn min_area_rect_hull(h: &[Ptf]) -> RotRect {
    let n = h.len();
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let (mut cx, mut cy, mut w, mut hh) = (0f32, 0f32, 0f32, 0f32);
    if n > 2 {
        let o = rotating_calipers(h);
        cx = o[0] + (o[2] + o[4]) * 0.5;
        cy = o[1] + (o[3] + o[5]) * 0.5;
        w = ((o[4] as f64) * (o[4] as f64) + (o[5] as f64) * (o[5] as f64)).sqrt() as f32;
        hh = ((o[2] as f64) * (o[2] as f64) + (o[3] as f64) * (o[3] as f64)).sqrt() as f32;
        if o[2] == 0.0 && o[3] > 0.0 {
            std::mem::swap(&mut w, &mut hh);
        } else {
            angle = -(o[2] as f64).atan2(o[3] as f64);
        }
    } else if n == 2 {
        cx = (h[0].0 + h[1].0) * 0.5;
        cy = (h[0].1 + h[1].1) * 0.5;
        let dx = h[0].0 as f64 - h[1].0 as f64;
        let dy = h[0].1 as f64 - h[1].1 as f64;
        w = 0.0;
        hh = (dx * dx + dy * dy).sqrt() as f32;
        if dx == 0.0 {
            std::mem::swap(&mut w, &mut hh);
        } else if dy < 0.0 {
            angle = dy.atan2(dx);
            std::mem::swap(&mut w, &mut hh);
        } else if dy > 0.0 {
            angle = -dx.atan2(dy);
        }
    } else if n == 1 {
        cx = h[0].0;
        cy = h[0].1;
    }
    RotRect {
        cx: cx as f64,
        cy: cy as f64,
        w: w as f64,
        h: hh as f64,
        angle: (angle * 180.0 / std::f64::consts::PI) as f32 as f64,
    }
}

/// `cv2.boxPoints(box)` (float32 corners).
pub fn box_points(b: &RotRect) -> [Ptf; 4] {
    let ang = (b.angle as f32) as f64 * std::f64::consts::PI / 180.0;
    let bb = ang.cos() as f32 * 0.5;
    let a = ang.sin() as f32 * 0.5;
    let (cx, cy, w, h) = (b.cx as f32, b.cy as f32, b.w as f32, b.h as f32);
    let ah = a * h;
    let aw = a * w;
    let bh = bb * h;
    let bw = bb * w;
    [
        (cx - ah - bw, cy + bh - aw),
        (cx + ah - bw, cy - bh - aw),
        (cx + ah + bw, cy - bh + aw),
        (cx - ah + bw, cy + bh + aw),
    ]
}

/// `cv2.boxPoints(box).astype(np.int64)`: truncated towards zero.
pub fn box_points_int(b: &RotRect) -> Vec<Pt> {
    box_points(b)
        .iter()
        .map(|p| (p.0 as i32, p.1 as i32))
        .collect()
}

/// `cv2.ellipse2Poly(center, axes, angle, 0, 360, 1)` (integer version).
pub fn ellipse2poly(center: Pt, axes: Pt, angle: i32) -> Vec<Pt> {
    let mut angle = angle;
    while angle < 0 {
        angle += 360;
    }
    while angle > 360 {
        angle -= 360;
    }
    let alpha = SIN_TABLE[(450 - angle) as usize]; // cos
    let beta = SIN_TABLE[angle as usize]; // sin
    let mut out: Vec<Pt> = Vec::new();
    let mut prev = (i32::MIN, i32::MIN);
    let (arc_start, arc_end, delta) = (0, 360, 1);
    let mut i = arc_start;
    while i < arc_end + delta {
        let mut a = i.min(arc_end);
        if a < 0 {
            a += 360;
        }
        let x = axes.0 as f64 * SIN_TABLE[(450 - a) as usize] as f64;
        let y = axes.1 as f64 * SIN_TABLE[a as usize] as f64;
        let px = center.0 as f64 + x * alpha as f64 - y * beta as f64;
        let py = center.1 as f64 + x * beta as f64 + y * alpha as f64;
        let p = (round_half_even(px) as i32, round_half_even(py) as i32);
        if p != prev {
            out.push(p);
            prev = p;
        }
        i += delta;
    }
    if out.len() == 1 {
        out = vec![center, center];
    }
    out
}

/// `cvRound`: round half to even (like `lrint`).
pub fn round_half_even(v: f64) -> f64 {
    v.round_ties_even()
}

/// `cv2.contourArea(points)` (not oriented).
pub fn contour_area(pts: &[Pt]) -> f64 {
    let f: Vec<Ptf> = pts.iter().map(|p| (p.0 as f32, p.1 as f32)).collect();
    contour_area_f(&f).abs()
}

fn contour_area_f(pts: &[Ptf]) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    let mut a = 0f64;
    let mut prev = pts[pts.len() - 1];
    for &p in pts {
        a += prev.0 as f64 * p.1 as f64 - prev.1 as f64 * p.0 as f64;
        prev = p;
    }
    a * 0.5
}

/// `cv2.boundingRect(points)` for integer points: (x, y, width, height).
pub fn bounding_rect(pts: &[Pt]) -> (i32, i32, i32, i32) {
    if pts.is_empty() {
        return (0, 0, 0, 0);
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in pts {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// `cv2.boundingRect` for float points.
pub fn bounding_rect_f(pts: &[Ptf]) -> (i32, i32, i32, i32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    let (fx0, fy0) = (x0.floor() as i32, y0.floor() as i32);
    (
        fx0,
        fy0,
        x1.floor() as i32 + 1 - fx0,
        y1.floor() as i32 + 1 - fy0,
    )
}

/// `cv2.pointPolygonTest(contour, pt, False)`: 1 inside, 0 on the edge, -1 outside.
pub fn point_polygon_test(cnt: &[Ptf], pt: Ptf) -> f64 {
    let total = cnt.len();
    if total == 0 {
        return -1.0;
    }
    let mut counter = 0;
    let mut v = cnt[total - 1];
    for &c in cnt {
        let v0 = v;
        v = c;
        if (v0.1 <= pt.1 && v.1 <= pt.1)
            || (v0.1 > pt.1 && v.1 > pt.1)
            || (v0.0 < pt.0 && v.0 < pt.0)
        {
            if pt.1 == v.1
                && (pt.0 == v.0
                    || (pt.1 == v0.1
                        && ((v0.0 <= pt.0 && pt.0 <= v.0) || (v.0 <= pt.0 && pt.0 <= v0.0))))
            {
                return 0.0;
            }
            continue;
        }
        let mut dist = (pt.1 as f64 - v0.1 as f64) * (v.0 as f64 - v0.0 as f64)
            - (pt.0 as f64 - v0.0 as f64) * (v.1 as f64 - v0.1 as f64);
        if dist == 0.0 {
            return 0.0;
        }
        if v.1 < v0.1 {
            dist = -dist;
        }
        counter += (dist > 0.0) as i32;
    }
    if counter % 2 == 0 {
        -1.0
    } else {
        1.0
    }
}

fn area_sign(a: Ptf, b: Ptf, c: Ptf) -> i32 {
    let area2 = (b.0 - a.0) as f64 * (c.1 - a.1) as f64 - (c.0 - a.0) as f64 * (b.1 - a.1) as f64;
    if area2 > 1e-5 {
        1
    } else if area2 < -1e-5 {
        -1
    } else {
        0
    }
}

fn between(a: Ptf, b: Ptf, c: Ptf) -> bool {
    if a.0 != b.0 {
        (a.0 <= c.0 && c.0 <= b.0) || (a.0 >= c.0 && c.0 >= b.0)
    } else {
        (a.1 <= c.1 && c.1 <= b.1) || (a.1 >= c.1 && c.1 >= b.1)
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Seg {
    None,
    Single,
    Overlap,
    Endpoint,
}

fn parallel_int(a: Ptf, b: Ptf, c: Ptf, d: Ptf, p: &mut Ptf, q: &mut Ptf) -> Seg {
    if area_sign(a, b, c) != 0 {
        return Seg::None;
    }
    if between(a, b, c) && between(a, b, d) {
        *p = c;
        *q = d;
    } else if between(c, d, a) && between(c, d, b) {
        *p = a;
        *q = b;
    } else if between(a, b, c) && between(c, d, b) {
        *p = c;
        *q = b;
    } else if between(a, b, c) && between(c, d, a) {
        *p = c;
        *q = a;
    } else if between(a, b, d) && between(c, d, b) {
        *p = d;
        *q = b;
    } else if between(a, b, d) && between(c, d, a) {
        *p = d;
        *q = a;
    } else {
        return Seg::None;
    }
    Seg::Overlap
}

fn intersect_segments(a: Ptf, b: Ptf, c: Ptf, d: Ptf, p: &mut Ptf, q: &mut Ptf) -> Seg {
    let (ax, ay, bx, by) = (a.0 as f64, a.1 as f64, b.0 as f64, b.1 as f64);
    let (cx, cy, dx, dy) = (c.0 as f64, c.1 as f64, d.0 as f64, d.1 as f64);
    let denom = (ax - bx) * (dy - cy) - (ay - by) * (dx - cx);
    if denom == 0.0 {
        return parallel_int(a, b, c, d, p, q);
    }
    let num = (dy - ay) * (ax - cx) + (ax - dx) * (ay - cy);
    let s = num / denom;
    let num = (by - ay) * (ax - cx) + (cy - ay) * (bx - ax);
    let t = num / denom;
    *p = ((ax + s * (bx - ax)) as f32, (ay + s * (by - ay)) as f32);
    *q = *p;
    if !(0.0..=1.0).contains(&s) || !(0.0..=1.0).contains(&t) {
        Seg::None
    } else if s == 0.0 || s == 1.0 || t == 0.0 || t == 1.0 {
        Seg::Endpoint
    } else {
        Seg::Single
    }
}

#[derive(PartialEq, Clone, Copy)]
enum InFlag {
    P,
    Q,
    Unknown,
}

fn push_new(res: &mut Vec<Ptf>, p: Ptf) {
    if *res.last().unwrap() != p {
        res.push(p);
    }
}

/// Returns (area, number of points) or None if it did not converge.
fn intersect_cc_inner(p: &[Ptf], q: &[Ptf], result_size: usize) -> Option<(f32, Vec<Ptf>)> {
    let (n, m) = (p.len(), q.len());
    let (mut a, mut b) = (0usize, 0usize);
    let origin = (0f32, 0f32);
    let mut inflag = InFlag::Unknown;
    let (mut aa, mut ba) = (0usize, 0usize);
    let mut first = true;
    let mut res: Vec<Ptf> = vec![(f32::MAX, f32::MAX)];
    loop {
        let a1 = (a + n - 1) % n;
        let b1 = (b + m - 1) % m;
        let av = (p[a].0 - p[a1].0, p[a].1 - p[a1].1);
        let bv = (q[b].0 - q[b1].0, q[b].1 - q[b1].1);
        let cross = area_sign(origin, av, bv);
        let a_hb = area_sign(q[b1], q[b], p[a]);
        let b_ha = area_sign(p[a1], p[a], q[b]);
        let mut ip = (0f32, 0f32);
        let mut iq = (0f32, 0f32);
        let code = intersect_segments(p[a1], p[a], q[b1], q[b], &mut ip, &mut iq);
        if code == Seg::Single || code == Seg::Endpoint {
            if inflag == InFlag::Unknown && first {
                aa = 0;
                ba = 0;
                first = false;
                res.push(ip);
            }
            push_new(&mut res, ip);
            inflag = if a_hb > 0 {
                InFlag::P
            } else if b_ha > 0 {
                InFlag::Q
            } else {
                inflag
            };
        }
        let dot = av.0 as f64 * bv.0 as f64 + av.1 as f64 * bv.1 as f64;
        if code == Seg::Overlap && dot < 0.0 {
            push_new(&mut res, ip);
            push_new(&mut res, iq);
            return finish_cc(res, result_size);
        }
        let adv_a = |a: &mut usize, aa: &mut usize, inside: bool, res: &mut Vec<Ptf>| {
            if inside {
                push_new(res, p[*a]);
            }
            *aa += 1;
            *a = (*a + 1) % n;
        };
        let adv_b = |b: &mut usize, ba: &mut usize, inside: bool, res: &mut Vec<Ptf>| {
            if inside {
                push_new(res, q[*b]);
            }
            *ba += 1;
            *b = (*b + 1) % m;
        };
        if cross == 0 && a_hb < 0 && b_ha < 0 {
            return finish_cc(res, result_size);
        } else if cross == 0 && a_hb == 0 && b_ha == 0 {
            if inflag == InFlag::P {
                adv_b(&mut b, &mut ba, inflag == InFlag::Q, &mut res);
            } else {
                adv_a(&mut a, &mut aa, inflag == InFlag::P, &mut res);
            }
        } else if cross >= 0 {
            if b_ha > 0 {
                adv_a(&mut a, &mut aa, inflag == InFlag::P, &mut res);
            } else {
                adv_b(&mut b, &mut ba, inflag == InFlag::Q, &mut res);
            }
        } else if a_hb > 0 {
            adv_b(&mut b, &mut ba, inflag == InFlag::Q, &mut res);
        } else {
            adv_a(&mut a, &mut aa, inflag == InFlag::P, &mut res);
        }
        if !((aa < n || ba < m) && aa < 2 * n && ba < 2 * m && res.len() <= result_size) {
            break;
        }
    }
    finish_cc(res, result_size)
}

fn finish_cc(mut res: Vec<Ptf>, result_size: usize) -> Option<(f32, Vec<Ptf>)> {
    let nr = res.len();
    if nr > result_size {
        return None;
    }
    let mut area = 0f64;
    let mut prev = res[nr - 1];
    for i in 1..nr {
        res[i - 1] = res[i];
        area += prev.0 as f64 * res[i].1 as f64 - prev.1 as f64 * res[i].0 as f64;
        prev = res[i];
    }
    let area = (area * 0.5) as f32;
    let mut nr = nr;
    if nr > 1 && res[nr - 2] == res[0] {
        nr -= 1;
    }
    res.truncate(nr - 1);
    Some((area, res))
}

fn orient_ccw(pts: &[Ptf]) -> Vec<Ptf> {
    let len = pts.len();
    let mut v = pts.to_vec();
    let d0 = (pts[0].0 - pts[len - 1].0, pts[0].1 - pts[len - 1].1);
    for i in 1..len {
        let di = (pts[i].0 - pts[i - 1].0, pts[i].1 - pts[i - 1].1);
        let s = d0.0 as f64 * di.1 as f64 - d0.1 as f64 * di.0 as f64;
        if s != 0.0 {
            if s < 0.0 {
                v.reverse();
            }
            break;
        }
    }
    v
}

/// `cv2.intersectConvexConvex(p1, p2)[0]` (handleNested=True): the area,
/// -1 if the algorithm did not converge.
pub fn intersect_convex_convex(p1: &[Ptf], p2: &[Ptf]) -> f32 {
    let (n, m) = (p1.len(), p2.len());
    if n < 2 || m < 2 {
        return 0.0;
    }
    let f1 = orient_ccw(p1);
    let f2 = orient_ccw(p2);
    let (area, pts) = match intersect_cc_inner(&f1, &f2, n + m + 1) {
        None => return -1.0,
        Some(r) => r,
    };
    if pts.is_empty() {
        if f2.iter().all(|&v| point_polygon_test(&f1, v) >= 0.0) {
            return contour_area_f(&f2).abs() as f32;
        }
        if f1.iter().all(|&v| point_polygon_test(&f2, v) >= 0.0) {
            return contour_area_f(&f1).abs() as f32;
        }
        return 0.0;
    }
    area.abs()
}

/// `cv2.rotatedRectangleIntersection(r1, r2)[0] != INTERSECT_NONE`.
pub fn rotated_rects_intersect(r1: &RotRect, r2: &RotRect) -> bool {
    let (w1, h1, w2, h2) = (r1.w as f32, r1.h as f32, r2.w as f32, r2.h as f32);
    if w1 <= 0.0 || h1 <= 0.0 || w2 <= 0.0 || h2 <= 0.0 {
        return false;
    }
    let acx = (r1.cx as f32 + r2.cx as f32) / 2.0;
    let acy = (r1.cy as f32 + r2.cy as f32) / 2.0;
    let mut s1 = *r1;
    let mut s2 = *r2;
    s1.cx = (r1.cx as f32 - acx) as f64;
    s1.cy = (r1.cy as f32 - acy) as f64;
    s2.cx = (r2.cx as f32 - acx) as f64;
    s2.cy = (r2.cy as f32 - acy) as f64;
    let p1 = box_points(&s1);
    let p2 = box_points(&s2);
    let same_eps = 1e-6f32 * (w1 * h1).max(w2 * h2);
    if (0..4)
        .all(|i| (p1[i].0 - p2[i].0).abs() <= same_eps && (p1[i].1 - p2[i].1).abs() <= same_eps)
    {
        return true;
    }
    let mut v1 = [(0f32, 0f32); 4];
    let mut v2 = [(0f32, 0f32); 4];
    for i in 0..4 {
        v1[i] = (p1[(i + 1) % 4].0 - p1[i].0, p1[(i + 1) % 4].1 - p1[i].1);
        v2[i] = (p2[(i + 1) % 4].0 - p2[i].0, p2[(i + 1) % 4].1 - p2[i].1);
    }
    for i in 0..4 {
        for j in 0..4 {
            let x21 = p2[j].0 - p1[i].0;
            let y21 = p2[j].1 - p1[i].1;
            let (vx1, vy1, vx2, vy2) = (v1[i].0, v1[i].1, v2[j].0, v2[j].1);
            let det = vx2 * vy1 - vx1 * vy2;
            if (det.abs() as f64) < 1e-12 {
                continue;
            }
            let inv = 1.0f32 / det;
            let t1 = (vx2 * y21 - vy2 * x21) * inv;
            let t2 = (vx1 * y21 - vy1 * x21) * inv;
            if !t1.is_finite() || !t2.is_finite() {
                continue;
            }
            if (0.0..=1.0).contains(&t1) && (0.0..=1.0).contains(&t2) {
                return true;
            }
        }
    }
    let positive = |lv: (f32, f32), lp: Ptf, pt: Ptf| lv.1 * (lp.0 - pt.0) >= lv.0 * (lp.1 - pt.1);
    for (pa, va, pb) in [(&p2, &v2, &p1), (&p1, &v1, &p2)] {
        for pt in pb.iter() {
            let pos = (0..4).filter(|&j| positive(va[j], pa[j], *pt)).count();
            if pos == 4 || pos == 0 {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------- drawing

const XY_SHIFT: i64 = 16;
const XY_ONE: i64 = 1 << XY_SHIFT;

fn clip_line(w: i64, h: i64, p1: &mut (i64, i64), p2: &mut (i64, i64)) -> bool {
    let (right, bottom) = (w - 1, h - 1);
    if w <= 0 || h <= 0 {
        return false;
    }
    let code = |p: (i64, i64)| {
        (p.0 < 0) as i32
            + (p.0 > right) as i32 * 2
            + (p.1 < 0) as i32 * 4
            + (p.1 > bottom) as i32 * 8
    };
    let mut c1 = code(*p1);
    let mut c2 = code(*p2);
    if (c1 & c2) == 0 && (c1 | c2) != 0 {
        if c1 & 12 != 0 {
            let a = if c1 < 8 { 0 } else { bottom };
            p1.0 += ((a - p1.1) as f64 * (p2.0 - p1.0) as f64 / (p2.1 - p1.1) as f64) as i64;
            p1.1 = a;
            c1 = (p1.0 < 0) as i32 + (p1.0 > right) as i32 * 2;
        }
        if c2 & 12 != 0 {
            let a = if c2 < 8 { 0 } else { bottom };
            p2.0 += ((a - p2.1) as f64 * (p2.0 - p1.0) as f64 / (p2.1 - p1.1) as f64) as i64;
            p2.1 = a;
            c2 = (p2.0 < 0) as i32 + (p2.0 > right) as i32 * 2;
        }
        if (c1 & c2) == 0 && (c1 | c2) != 0 {
            if c1 != 0 {
                let a = if c1 == 1 { 0 } else { right };
                p1.1 += ((a - p1.0) as f64 * (p2.1 - p1.1) as f64 / (p2.0 - p1.0) as f64) as i64;
                p1.0 = a;
                c1 = 0;
            }
            if c2 != 0 {
                let a = if c2 == 1 { 0 } else { right };
                p2.1 += ((a - p2.0) as f64 * (p2.1 - p1.1) as f64 / (p2.0 - p1.0) as f64) as i64;
                p2.0 = a;
                c2 = 0;
            }
        }
    }
    (c1 | c2) == 0
}

/// `Line(img, pt1, pt2, color, 8)`: an 8-connected line, left to right.
fn draw_line(img: &mut Gray, p1: (i64, i64), p2: (i64, i64), v: u8) {
    let (w, h) = (img.width as i64, img.height as i64);
    let mut a = p1;
    let mut b = p2;
    if (a.0 < 0 || a.0 >= w || b.0 < 0 || b.0 >= w || a.1 < 0 || a.1 >= h || b.1 < 0 || b.1 >= h)
        && !clip_line(w, h, &mut a, &mut b)
    {
        return;
    }
    let (mut dx_s, mut dy_s) = (1i64, 1i64);
    let mut dx = b.0 - a.0;
    let mut dy = b.1 - a.1;
    if dx < 0 {
        dx = -dx;
        dy = -dy;
        a = b;
    }
    if dy < 0 {
        dy = -dy;
        dy_s = -1;
    }
    let vert = dy > dx;
    if vert {
        std::mem::swap(&mut dx, &mut dy);
        std::mem::swap(&mut dx_s, &mut dy_s);
    }
    let mut err = dx - (dy + dy);
    let plus_delta = dx + dx;
    let minus_delta = -(dy + dy);
    // x and y steps (point mode of LineIterator)
    let (mut minus_x, mut plus_x, mut minus_y, mut plus_y) = (dx_s, 0i64, 0i64, dy_s);
    if vert {
        std::mem::swap(&mut plus_y, &mut plus_x);
        std::mem::swap(&mut minus_y, &mut minus_x);
    }
    let count = dx + 1;
    let mut p = a;
    for _ in 0..count {
        img.set(p.0 as usize, p.1 as usize, v);
        let mask = if err < 0 { -1i64 } else { 0 };
        err += minus_delta + (plus_delta & mask);
        p.0 += minus_x + (plus_x & mask);
        p.1 += minus_y + (plus_y & mask);
    }
}

fn hline(img: &mut Gray, y: usize, x1: i64, x2: i64, v: u8) {
    let row = &mut img.data[y * img.width..(y + 1) * img.width];
    for px in &mut row[x1 as usize..=x2 as usize] {
        *px = v;
    }
}

/// `cv2.fillConvexPoly(img, pts, v)` (LINE_8, shift 0).
pub fn fill_convex_poly(img: &mut Gray, pts: &[Pt], val: u8) {
    let npts = pts.len();
    if npts == 0 {
        return;
    }
    let v: Vec<(i64, i64)> = pts.iter().map(|p| (p.0 as i64, p.1 as i64)).collect();
    let (wd, ht) = (img.width as i64, img.height as i64);
    let delta1 = XY_ONE >> 1;
    let delta2 = XY_ONE >> 1;
    let mut p0 = v[npts - 1];
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (v[0].0, v[0].0, v[0].1, v[0].1);
    let mut imin = 0usize;
    for (i, &p) in v.iter().enumerate() {
        if p.1 < ymin {
            ymin = p.1;
            imin = i;
        }
        ymax = ymax.max(p.1);
        xmax = xmax.max(p.0);
        xmin = xmin.min(p.0);
        draw_line(img, p0, p, val);
        p0 = p;
    }
    if npts < 3 || xmax < 0 || ymax < 0 || xmin >= wd || ymin >= ht {
        return;
    }
    let ymax = ymax.min(ht - 1);
    #[derive(Clone, Copy)]
    struct E {
        idx: usize,
        di: usize,
        x: i64,
        dx: i64,
        ye: i64,
    }
    let mut edge = [
        E {
            idx: imin,
            di: 1,
            x: -XY_ONE,
            dx: 0,
            ye: ymin,
        },
        E {
            idx: imin,
            di: npts - 1,
            x: -XY_ONE,
            dx: 0,
            ye: ymin,
        },
    ];
    let mut edges = npts as i64;
    let mut y = ymin;
    loop {
        for e in edge.iter_mut() {
            if y >= e.ye {
                let mut idx0 = e.idx;
                let mut idx = idx0 + e.di;
                if idx >= npts {
                    idx -= npts;
                }
                loop {
                    let go = edges > 0;
                    edges -= 1;
                    if !go {
                        break;
                    }
                    let ty = v[idx].1;
                    if ty > y {
                        let xs = v[idx0].0 << XY_SHIFT;
                        let xe = v[idx].0 << XY_SHIFT;
                        e.ye = ty;
                        e.dx = ((xe - xs) * 2 + (ty - y)) / (2 * (ty - y));
                        e.x = xs;
                        e.idx = idx;
                        break;
                    }
                    idx0 = idx;
                    idx += e.di;
                    if idx >= npts {
                        idx -= npts;
                    }
                }
            }
        }
        if edges < 0 {
            break;
        }
        if y >= 0 {
            let (l, r) = if edge[0].x > edge[1].x {
                (1, 0)
            } else {
                (0, 1)
            };
            let mut x1 = (edge[l].x + delta1) >> XY_SHIFT;
            let mut x2 = (edge[r].x + delta2) >> XY_SHIFT;
            if x2 >= 0 && x1 < wd {
                x1 = x1.max(0);
                x2 = x2.min(wd - 1);
                if x1 <= x2 {
                    hline(img, y as usize, x1, x2, val);
                }
            }
        }
        edge[0].x += edge[0].dx;
        edge[1].x += edge[1].dx;
        y += 1;
        if y > ymax {
            break;
        }
    }
}

#[derive(Clone, Copy)]
struct PolyEdge {
    y0: i64,
    y1: i64,
    x: i64,
    dx: i64,
    next: usize, // 0 = none; edges are 1-based in the list
}

/// `cv2.fillPoly(img, polys, v)` and `cv2.drawContours(..., thickness=FILLED)`
/// (LINE_8, shift 0).
pub fn fill_poly(img: &mut Gray, polys: &[Vec<Pt>], val: u8) {
    let (wd, ht) = (img.width as i64, img.height as i64);
    let mut edges: Vec<PolyEdge> = Vec::new();
    for poly in polys {
        let count = poly.len();
        if count == 0 {
            continue;
        }
        let v: Vec<(i64, i64)> = poly.iter().map(|p| (p.0 as i64, p.1 as i64)).collect();
        let mut pt0 = v[count - 1];
        pt0.0 <<= XY_SHIFT;
        for &raw in &v {
            let pt1 = (raw.0 << XY_SHIFT, raw.1);
            let mut t0 = (pt0.0 + (XY_ONE >> 1)) >> XY_SHIFT;
            let mut t1 = (pt1.0 + (XY_ONE >> 1)) >> XY_SHIFT;
            let (mut a, mut b) = ((t0, pt0.1), (t1, pt1.1));
            draw_line(img, a, b, val);
            let (mut pt0c, mut pt1c) = (pt0, pt1);
            if a.0 < 0
                || a.0 >= wd
                || b.0 < 0
                || b.0 >= wd
                || a.1 < 0
                || a.1 >= ht
                || b.1 < 0
                || b.1 >= ht
            {
                clip_line(wd, ht, &mut a, &mut b);
                if a.1 != b.1 {
                    pt0c.1 = a.1;
                    pt1c.1 = b.1;
                }
            }
            t0 = a.0;
            t1 = b.0;
            pt0c.0 = t0 << XY_SHIFT;
            pt1c.0 = t1 << XY_SHIFT;
            if pt0.1 != pt1.1 {
                let dx = (pt1c.0 - pt0c.0) / (pt1c.1 - pt0c.1);
                let e = if pt0.1 < pt1.1 {
                    PolyEdge {
                        y0: pt0.1,
                        y1: pt1.1,
                        x: pt0c.0 + (pt0.1 - pt0c.1) * dx,
                        dx,
                        next: 0,
                    }
                } else {
                    PolyEdge {
                        y0: pt1.1,
                        y1: pt0.1,
                        x: pt1c.0 + (pt1.1 - pt1c.1) * dx,
                        dx,
                        next: 0,
                    }
                };
                edges.push(e);
            }
            pt0 = pt1;
        }
    }
    fill_edge_collection(img, edges, val);
}

fn fill_edge_collection(img: &mut Gray, mut edges: Vec<PolyEdge>, val: u8) {
    let total = edges.len();
    if total < 2 {
        return;
    }
    let (wd, ht) = (img.width as i64, img.height as i64);
    let (mut y_max, mut y_min) = (i64::MIN, i64::MAX);
    let (mut x_max, mut x_min) = (i64::MIN, i64::MAX);
    for e in &edges {
        let x1 = e.x + (e.y1 - e.y0) * e.dx;
        y_min = y_min.min(e.y0);
        y_max = y_max.max(e.y1);
        x_min = x_min.min(e.x).min(x1);
        x_max = x_max.max(e.x).max(x1);
    }
    if y_max < 0 || y_min >= ht || x_max < 0 || x_min >= (wd << XY_SHIFT) {
        return;
    }
    edges.sort_by(|a, b| a.y0.cmp(&b.y0).then(a.x.cmp(&b.x)).then(a.dx.cmp(&b.dx)));
    // Node 0 is `tmp` (list head); edges are nodes 1..=total; node total+1 is the sentinel.
    let mut nodes: Vec<PolyEdge> = Vec::with_capacity(total + 2);
    nodes.push(PolyEdge {
        y0: i64::MAX,
        y1: 0,
        x: 0,
        dx: 0,
        next: 0,
    });
    nodes.extend(edges);
    nodes.push(PolyEdge {
        y0: i64::MAX,
        y1: 0,
        x: 0,
        dx: 0,
        next: 0,
    });
    const NONE: usize = usize::MAX;
    for n in nodes.iter_mut() {
        n.next = NONE;
    }
    let delta = XY_ONE - 1;
    let mut i = 0usize;
    let mut e = 1usize;
    let y_max = y_max.min(ht);
    let mut y = nodes[e].y0;
    while y < y_max {
        let mut draw = false;
        let clipline = y < 0;
        let mut prelast = 0usize;
        let mut last = nodes[0].next;
        while last != NONE || nodes[e].y0 == y {
            if last != NONE && nodes[last].y1 == y {
                nodes[prelast].next = nodes[last].next;
                last = nodes[last].next;
                continue;
            }
            let keep_prelast = prelast;
            if last != NONE && (nodes[e].y0 > y || nodes[last].x < nodes[e].x) {
                prelast = last;
                last = nodes[last].next;
            } else if i < total {
                nodes[prelast].next = e;
                nodes[e].next = last;
                prelast = e;
                i += 1;
                e = i + 1;
            } else {
                break;
            }
            if draw {
                if !clipline {
                    let (x1, x2) = if nodes[keep_prelast].x > nodes[prelast].x {
                        (
                            (nodes[prelast].x + delta) >> XY_SHIFT,
                            nodes[keep_prelast].x >> XY_SHIFT,
                        )
                    } else {
                        (
                            (nodes[keep_prelast].x + delta) >> XY_SHIFT,
                            nodes[prelast].x >> XY_SHIFT,
                        )
                    };
                    if x1 < wd && x2 >= 0 {
                        let (x1, x2) = (x1.max(0), x2.min(wd - 1));
                        if x1 <= x2 {
                            hline(img, y as usize, x1, x2, val);
                        }
                    }
                }
                let d = nodes[keep_prelast].dx;
                nodes[keep_prelast].x += d;
                let d = nodes[prelast].dx;
                nodes[prelast].x += d;
            }
            draw = !draw;
        }
        // bubble sort the active list by x
        let mut keep_prelast = NONE;
        loop {
            let mut prelast = 0usize;
            let mut last = nodes[0].next;
            let mut last_exchange = NONE;
            while last != keep_prelast && last != NONE && nodes[last].next != NONE {
                let te = nodes[last].next;
                if nodes[last].x > nodes[te].x {
                    nodes[prelast].next = te;
                    nodes[last].next = nodes[te].next;
                    nodes[te].next = last;
                    prelast = te;
                    last_exchange = prelast;
                } else {
                    prelast = last;
                    last = te;
                }
            }
            if last_exchange == NONE {
                break;
            }
            keep_prelast = last_exchange;
            if keep_prelast == nodes[0].next || keep_prelast == 0 {
                break;
            }
        }
        y += 1;
    }
}

// ---------------------------------------------------------------- affine

/// Solve `a x = b` (n x n) by Gaussian elimination with partial pivoting.
pub fn solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for k in 0..n {
        let mut piv = k;
        for i in k + 1..n {
            if a[i * n + k].abs() > a[piv * n + k].abs() {
                piv = i;
            }
        }
        if a[piv * n + k].abs() < f64::EPSILON * 100.0 {
            return None;
        }
        if piv != k {
            for j in 0..n {
                a.swap(k * n + j, piv * n + j);
            }
            b.swap(k, piv);
        }
        let d = -1.0 / a[k * n + k];
        for i in k + 1..n {
            let alpha = a[i * n + k] * d;
            for j in k + 1..n {
                a[i * n + j] += alpha * a[k * n + j];
            }
            b[i] += alpha * b[k];
        }
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s -= a[i * n + j] * b[j];
        }
        b[i] = s / a[i * n + i];
    }
    Some(b)
}

/// `cv2.getAffineTransform(src, dst)`: the 2x3 matrix, row-major.
pub fn get_affine_transform(src: &[Ptf; 3], dst: &[Ptf; 3]) -> Option<[f64; 6]> {
    let mut a = vec![0f64; 36];
    let mut b = vec![0f64; 6];
    for i in 0..3 {
        let j = i * 12;
        let k = i * 12 + 6;
        a[j] = src[i].0 as f64;
        a[k + 3] = src[i].0 as f64;
        a[j + 1] = src[i].1 as f64;
        a[k + 4] = src[i].1 as f64;
        a[j + 2] = 1.0;
        a[k + 5] = 1.0;
        b[i * 2] = dst[i].0 as f64;
        b[i * 2 + 1] = dst[i].1 as f64;
    }
    let x = solve(a, b, 6)?;
    Some([x[0], x[1], x[2], x[3], x[4], x[5]])
}

/// `cv2.invertAffineTransform(m)`.
pub fn invert_affine(m: &[f64; 6]) -> [f64; 6] {
    let d = m[0] * m[4] - m[1] * m[3];
    let d = if d != 0.0 { 1.0 / d } else { 0.0 };
    let (a11, a22, a12, a21) = (m[4] * d, m[0] * d, -m[1] * d, -m[3] * d);
    let b1 = -a11 * m[2] - a12 * m[5];
    let b2 = -a21 * m[2] - a22 * m[5];
    [a11, a12, b1, a21, a22, b2]
}

fn sat_int(v: f64) -> i32 {
    let r = v.round_ties_even();
    r.clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

/// `cv2.warpAffine(src, m, (w, h), flags=INTER_LINEAR,
/// borderMode=BORDER_CONSTANT, borderValue=border)`.
pub fn warp_affine(src: &Gray, m: &[f64; 6], w: usize, h: usize, border: u8) -> Gray {
    let mi = invert_affine(m);
    let mut dst = Gray::new(w, h);
    const AB_BITS: i32 = 10;
    const AB_SCALE: f64 = 1024.0;
    const INTER_BITS: i32 = 5;
    const TAB: i32 = 32;
    let round_delta = (AB_SCALE as i32) / TAB / 2;
    let adelta: Vec<i32> = (0..w)
        .map(|x| sat_int(mi[0] * x as f64 * AB_SCALE))
        .collect();
    let bdelta: Vec<i32> = (0..w)
        .map(|x| sat_int(mi[3] * x as f64 * AB_SCALE))
        .collect();
    let (sw, sh) = (src.width as i32, src.height as i32);
    let px = |x: i32, y: i32| -> i32 {
        if x < 0 || y < 0 || x >= sw || y >= sh {
            border as i32
        } else {
            src.data[(y * sw + x) as usize] as i32
        }
    };
    for y in 0..h {
        let x0 = sat_int((mi[1] * y as f64 + mi[2]) * AB_SCALE) + round_delta;
        let y0 = sat_int((mi[4] * y as f64 + mi[5]) * AB_SCALE) + round_delta;
        for x in 0..w {
            let xx = (x0 + adelta[x]) >> (AB_BITS - INTER_BITS);
            let yy = (y0 + bdelta[x]) >> (AB_BITS - INTER_BITS);
            let sx = (xx >> INTER_BITS).clamp(i16::MIN as i32, i16::MAX as i32);
            let sy = (yy >> INTER_BITS).clamp(i16::MIN as i32, i16::MAX as i32);
            let fx = xx & (TAB - 1);
            let fy = yy & (TAB - 1);
            let v = if sx >= sw || sx + 1 < 0 || sy >= sh || sy + 1 < 0 {
                border as i32
            } else {
                // BilinearTab_i: products of (1 - i/32, i/32), scaled by 2^15
                let wx1 = fx * 1024;
                let wx0 = 32768 - wx1;
                let wy1 = fy;
                let wy0 = 32 - fy;
                let w00 = wy0 * wx0 / 32;
                let w01 = wy0 * wx1 / 32;
                let w10 = wy1 * wx0 / 32;
                let w11 = wy1 * wx1 / 32;
                let s = px(sx, sy) * w00
                    + px(sx + 1, sy) * w01
                    + px(sx, sy + 1) * w10
                    + px(sx + 1, sy + 1) * w11;
                ((s + (1 << 14)) >> 15).clamp(0, 255)
            };
            dst.data[y * w + x] = v as u8;
        }
    }
    dst
}

// ---------------------------------------------------------------- ellipse

/// Least squares `a x = b` (rows x cols, row-major) by one-sided Jacobi SVD,
/// like OpenCV's `SVDecomp` + `SVBackSubst`. Returns the solution and the
/// singular values (descending).
pub fn svd_solve(a: &[f64], rows: usize, cols: usize, b: &[f64]) -> (Vec<f64>, Vec<f64>) {
    // Work on columns of A: U = A V, with V accumulated.
    let mut u: Vec<f64> = a.to_vec();
    let mut v = vec![0f64; cols * cols];
    for i in 0..cols {
        v[i * cols + i] = 1.0;
    }
    for _sweep in 0..60 {
        let mut changed = false;
        for p in 0..cols {
            for q in p + 1..cols {
                let (mut alpha, mut beta, mut gamma) = (0f64, 0f64, 0f64);
                for r in 0..rows {
                    let up = u[r * cols + p];
                    let uq = u[r * cols + q];
                    alpha += up * up;
                    beta += uq * uq;
                    gamma += up * uq;
                }
                if gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() || gamma == 0.0 {
                    continue;
                }
                changed = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let t = if zeta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for r in 0..rows {
                    let up = u[r * cols + p];
                    let uq = u[r * cols + q];
                    u[r * cols + p] = c * up - s * uq;
                    u[r * cols + q] = s * up + c * uq;
                }
                for r in 0..cols {
                    let vp = v[r * cols + p];
                    let vq = v[r * cols + q];
                    v[r * cols + p] = c * vp - s * vq;
                    v[r * cols + q] = s * vp + c * vq;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut sv: Vec<f64> = (0..cols)
        .map(|j| {
            (0..rows)
                .map(|r| u[r * cols + j].powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .collect();
    let wmax = sv.iter().cloned().fold(0.0, f64::max);
    let thresh = wmax * f64::EPSILON * rows.max(cols) as f64;
    let mut x = vec![0f64; cols];
    for j in 0..cols {
        if sv[j] <= thresh {
            continue;
        }
        // (u_j / s_j) . b / s_j
        let dot: f64 = (0..rows).map(|r| u[r * cols + j] * b[r]).sum::<f64>() / (sv[j] * sv[j]);
        for i in 0..cols {
            x[i] += v[i * cols + j] * dot;
        }
    }
    sv.sort_by(|a, b| b.partial_cmp(a).unwrap());
    (x, sv)
}

/// A small deterministic RNG for the jitter OpenCV applies to degenerate
/// point sets (OpenCV uses its global `theRNG()`).
struct Jitter(u64);

impl Jitter {
    fn uniform(&mut self, eps: f32) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let u = (self.0 >> 40) as f32 / (1u64 << 24) as f32;
        -eps + 2.0 * eps * u
    }
}

/// `cv2.fitEllipse(points)` (at least 5 points).
pub fn fit_ellipse(pts: &[Pt]) -> RotRect {
    if pts.len() == 5 {
        if let Some(r) = fit_ellipse_direct(pts) {
            return r;
        }
    }
    fit_ellipse_no_direct(pts)
}

fn fit_ellipse_no_direct(pts: &[Pt]) -> RotRect {
    let n = pts.len();
    let pf: Vec<Ptf> = pts.iter().map(|p| (p.0 as f32, p.1 as f32)).collect();
    let mut c = (0f32, 0f32);
    for p in &pf {
        c.0 += p.0;
        c.1 += p.1;
    }
    c.0 /= n as f32;
    c.1 /= n as f32;
    let mut s = 0f64;
    for p in &pf {
        s += ((p.0 - c.0).abs() + (p.1 - c.1).abs()) as f64;
    }
    let scale = 100.0
        / if s > f32::EPSILON as f64 {
            s
        } else {
            f32::EPSILON as f64
        };
    let b = vec![10000.0f64; n];
    let build = |pf: &[Ptf]| {
        let centred: Vec<(f64, f64)> = pf
            .iter()
            .map(|p| (((p.0 - c.0) as f64) * scale, ((p.1 - c.1) as f64) * scale))
            .collect();
        let mut a = vec![0f64; n * 5];
        for (i, &(px, py)) in centred.iter().enumerate() {
            a[i * 5] = -px * px;
            a[i * 5 + 1] = -py * py;
            a[i * 5 + 2] = -px * py;
            a[i * 5 + 3] = px;
            a[i * 5 + 4] = py;
        }
        (centred, a)
    };
    let (mut centred, a) = build(&pf);
    let (mut gfp, w) = svd_solve(&a, n, 5, &b);
    if w[0] * f32::EPSILON as f64 > w[4] {
        // Singular (e.g. points on a line): OpenCV jitters the points with its
        // global RNG, so the result is not reproducible; any jitter gives a
        // degenerate ellipse, which is what callers filter on.
        let eps = (s / (n as f64 * 2.0) * 1e-3) as f32;
        let mut rng = Jitter(0x1234_5678);
        let jit: Vec<Ptf> = pf
            .iter()
            .map(|p| (p.0 + rng.uniform(eps), p.1 + rng.uniform(eps)))
            .collect();
        let (c2, a2) = build(&jit);
        centred = c2;
        gfp = svd_solve(&a2, n, 5, &b).0;
    }
    let a2 = [2.0 * gfp[0], gfp[2], gfp[2], 2.0 * gfp[1]];
    let (rp01, _) = svd_solve(&a2, 2, 2, &[gfp[3], gfp[4]]);
    let mut a3 = vec![0f64; n * 3];
    for (i, &(px, py)) in centred.iter().enumerate() {
        a3[i * 3] = (px - rp01[0]) * (px - rp01[0]);
        a3[i * 3 + 1] = (py - rp01[1]) * (py - rp01[1]);
        a3[i * 3 + 2] = (px - rp01[0]) * (py - rp01[1]);
    }
    let (g, _) = svd_solve(&a3, n, 3, &vec![1.0; n]);
    let min_eps = 1e-8;
    let rp4 = -0.5 * g[2].atan2(g[1] - g[0]);
    let t = if g[2].abs() > min_eps {
        g[2] / (-2.0 * rp4).sin()
    } else {
        g[1] - g[0]
    };
    let mut rp2 = (g[0] + g[1] - t).abs();
    if rp2 > min_eps {
        rp2 = (2.0 / rp2).sqrt();
    }
    let mut rp3 = (g[0] + g[1] + t).abs();
    if rp3 > min_eps {
        rp3 = (2.0 / rp3).sqrt();
    }
    let cx = (rp01[0] / scale) as f32 + c.0;
    let cy = (rp01[1] / scale) as f32 + c.1;
    let mut w = (rp2 * 2.0 / scale) as f32;
    let mut h = (rp3 * 2.0 / scale) as f32;
    let mut angle = 0f32;
    if w > h {
        std::mem::swap(&mut w, &mut h);
        angle = (90.0 + rp4 * 180.0 / std::f64::consts::PI) as f32;
    }
    if angle < -180.0 {
        angle += 360.0;
    }
    if angle > 360.0 {
        angle -= 360.0;
    }
    RotRect {
        cx: cx as f64,
        cy: cy as f64,
        w: w as f64,
        h: h as f64,
        angle: angle as f64,
    }
}

/// Real eigenvectors of a 3x3 matrix (rows), by eigenvalue; None if complex.
fn eigvecs3(m: &[[f64; 3]; 3]) -> Option<Vec<[f64; 3]>> {
    // characteristic polynomial: l^3 - tr l^2 + c1 l - det
    let tr = m[0][0] + m[1][1] + m[2][2];
    let c1 = m[0][0] * m[1][1] - m[0][1] * m[1][0] + m[0][0] * m[2][2] - m[0][2] * m[2][0]
        + m[1][1] * m[2][2]
        - m[1][2] * m[2][1];
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    // depressed cubic
    let a = -tr;
    let b = c1;
    let c = -det;
    let p = b - a * a / 3.0;
    let q = 2.0 * a * a * a / 27.0 - a * b / 3.0 + c;
    let disc = q * q / 4.0 + p * p * p / 27.0;
    let mut roots = Vec::new();
    if disc > 1e-12 * (q * q).max(1e-300) {
        return None;
    }
    if p.abs() < 1e-300 {
        roots.push(-a / 3.0);
    } else {
        let r = (-p / 3.0).sqrt();
        let arg = (-q / (2.0 * r * r * r)).clamp(-1.0, 1.0);
        let phi = arg.acos();
        for k in 0..3 {
            roots.push(
                2.0 * r * ((phi - 2.0 * std::f64::consts::PI * k as f64) / 3.0).cos() - a / 3.0,
            );
        }
    }
    let mut out = Vec::new();
    for l in roots {
        let r0 = [m[0][0] - l, m[0][1], m[0][2]];
        let r1 = [m[1][0], m[1][1] - l, m[1][2]];
        let r2 = [m[2][0], m[2][1], m[2][2] - l];
        let cross = |u: [f64; 3], v: [f64; 3]| {
            [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ]
        };
        let cands = [cross(r0, r1), cross(r0, r2), cross(r1, r2)];
        let best = cands
            .iter()
            .max_by(|x, y| {
                let nx = x.iter().map(|v| v * v).sum::<f64>();
                let ny = y.iter().map(|v| v * v).sum::<f64>();
                nx.partial_cmp(&ny).unwrap()
            })
            .copied()?;
        out.push(best);
    }
    Some(out)
}

fn fit_ellipse_direct(pts: &[Pt]) -> Option<RotRect> {
    let n = pts.len() as f64;
    let mut c = (0f64, 0f64);
    for p in pts {
        c.0 += p.0 as f64;
        c.1 += p.1 as f64;
    }
    c.0 /= n;
    c.1 /= n;
    let mut s = 0f64;
    for p in pts {
        s += (p.0 as f64 - c.0).abs() + (p.1 as f64 - c.1).abs();
    }
    let scale = 100.0
        / if s > f32::EPSILON as f64 {
            s
        } else {
            f32::EPSILON as f64
        };
    let rows: Vec<[f64; 6]> = pts
        .iter()
        .map(|p| {
            let px = (p.0 as f64 - c.0) * scale;
            let py = (p.1 as f64 - c.1) * scale;
            [px * px, px * py, py * py, px, py, 1.0]
        })
        .collect();
    let mut dm = [[0f64; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            dm[i][j] = rows.iter().map(|r| r[i] * r[j]).sum::<f64>() / n;
        }
    }
    let d = |i: usize, j: usize| dm[i][j];
    let mut tm = [[0f64; 3]; 3];
    #[allow(clippy::needless_range_loop)] // OpenCV's TM formulas, column by column
    for k in 0..3 {
        tm[0][k] =
            d(k, 5) * d(3, 5) * d(4, 4) - d(k, 5) * d(3, 4) * d(4, 5) - d(k, 4) * d(3, 5) * d(5, 4)
                + d(k, 3) * d(4, 5) * d(5, 4)
                + d(k, 4) * d(3, 4) * d(5, 5)
                - d(k, 3) * d(4, 4) * d(5, 5);
        tm[1][k] = d(k, 5) * d(3, 3) * d(4, 5) - d(k, 5) * d(3, 5) * d(4, 3)
            + d(k, 4) * d(3, 5) * d(5, 3)
            - d(k, 3) * d(4, 5) * d(5, 3)
            - d(k, 4) * d(3, 3) * d(5, 5)
            + d(k, 3) * d(4, 3) * d(5, 5);
        tm[2][k] =
            d(k, 5) * d(3, 4) * d(4, 3) - d(k, 5) * d(3, 3) * d(4, 4) - d(k, 4) * d(3, 4) * d(5, 3)
                + d(k, 3) * d(4, 4) * d(5, 3)
                + d(k, 4) * d(3, 3) * d(5, 4)
                - d(k, 3) * d(4, 3) * d(5, 4);
    }
    let ts =
        -(d(3, 5) * d(4, 4) * d(5, 3)) + d(3, 4) * d(4, 5) * d(5, 3) + d(3, 5) * d(4, 3) * d(5, 4)
            - d(3, 3) * d(4, 5) * d(5, 4)
            - d(3, 4) * d(4, 3) * d(5, 5)
            + d(3, 3) * d(4, 4) * d(5, 5);
    if ts.abs() < f64::EPSILON {
        return None;
    }
    let mut m = [[0f64; 3]; 3];
    for k in 0..3 {
        m[0][k] =
            (d(2, k) + (d(2, 3) * tm[0][k] + d(2, 4) * tm[1][k] + d(2, 5) * tm[2][k]) / ts) / 2.0;
        m[1][k] = -d(1, k) - (d(1, 3) * tm[0][k] + d(1, 4) * tm[1][k] + d(1, 5) * tm[2][k]) / ts;
        m[2][k] =
            (d(0, k) + (d(0, 3) * tm[0][k] + d(0, 4) * tm[1][k] + d(0, 5) * tm[2][k]) / ts) / 2.0;
    }
    let evec = eigvecs3(&m)?;
    let cond: Vec<f64> = evec
        .iter()
        .map(|e| 4.0 * e[0] * e[2] - e[1] * e[1])
        .collect();
    let (i, ci) = cond
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, c)| (i, *c))?;
    let e = evec[i];
    let vnorm2 = e[0] * e[0] + e[1] * e[1] + e[2] * e[2];
    if ci <= 1e-6 * vnorm2 {
        return None;
    }
    let mut norm = vnorm2.sqrt();
    let sg = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
    if sg(e[0]) * sg(e[1]) * sg(e[2]) <= 0.0 {
        norm = -norm;
    }
    let pv = [e[0] / norm, e[1] / norm, e[2] / norm];
    let q0 = (tm[0][0] * pv[0] + tm[0][1] * pv[1] + tm[0][2] * pv[2]) / ts;
    let q1 = (tm[1][0] * pv[0] + tm[1][1] * pv[1] + tm[1][2] * pv[2]) / ts;
    let q2 = (tm[2][0] * pv[0] + tm[2][1] * pv[1] + tm[2][2] * pv[2]) / ts;
    let u1 = pv[2] * q0 * q0 - pv[1] * q0 * q1 + pv[0] * q1 * q1 + pv[1] * pv[1] * q2;
    let u2 = pv[0] * pv[2] * q2;
    let l1 = (pv[1] * pv[1] + (pv[0] - pv[2]) * (pv[0] - pv[2])).sqrt();
    let l2 = pv[0] + pv[2];
    let l3 = pv[1] * pv[1] - 4.0 * pv[0] * pv[2];
    let p1 = 2.0 * pv[2] * q0 - pv[1] * q1;
    let p2 = 2.0 * pv[0] * q1 - pv[1] * q0;
    let x0 = p1 / l3 / scale + c.0;
    let y0 = p2 / l3 / scale + c.1;
    let a = 2f64.sqrt() * ((u1 - 4.0 * u2) / ((l1 - l2) * l3)).sqrt() / scale;
    let b = 2f64.sqrt() * (-((u1 - 4.0 * u2) / ((l1 + l2) * l3))).sqrt() / scale;
    let theta = if pv[1] == 0.0 {
        if pv[0] < pv[2] {
            0.0
        } else {
            std::f64::consts::FRAC_PI_2
        }
    } else {
        std::f64::consts::FRAC_PI_2 + 0.5 * pv[1].atan2(pv[0] - pv[2])
    };
    let mut w = (2.0 * a) as f32;
    let mut h = (2.0 * b) as f32;
    let angle = if w > h {
        std::mem::swap(&mut w, &mut h);
        ((90.0 + theta * 180.0 / std::f64::consts::PI) % 180.0) as f32
    } else {
        ((theta * 180.0 / std::f64::consts::PI) % 180.0) as f32
    };
    Some(RotRect {
        cx: x0 as f32 as f64,
        cy: y0 as f32 as f64,
        w: w as f64,
        h: h as f64,
        angle: angle as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_area_rect_matches_opencv() {
        // cv2.minAreaRect([[0,0],[4,1],[3,5]]) = ((2.75, 1.75), (2.9154758, 5.8309517), -30.963755)
        let r = min_area_rect(&[(0, 0), (4, 1), (3, 5)]);
        assert!(
            (r.cx - 2.75).abs() < 1e-6 && (r.cy - 1.75).abs() < 1e-6,
            "{r:?}"
        );
        assert!((r.w - 2.915_475_845).abs() < 1e-5 && (r.h - 5.830_951_69).abs() < 1e-5);
        assert!((r.angle + 30.963_754_65).abs() < 1e-5);
    }

    #[test]
    fn hull_of_a_square_with_inner_point() {
        let h = convex_hull(&[(0, 0), (2, 1), (4, 0), (4, 4), (0, 4)]);
        assert_eq!(h.len(), 4);
        assert!(!h.contains(&(2, 1)));
    }

    #[test]
    fn fit_ellipse_matches_opencv() {
        // A 10x3 ellipse rotated by 10 degrees, rounded to pixels.
        let mut pts = Vec::new();
        for i in 0..40 {
            let t = i as f64 * std::f64::consts::TAU / 40.0;
            let r = 10f64.to_radians();
            let (x, y) = (10.0 * t.cos(), 3.0 * t.sin());
            pts.push((
                (50.0 + x * r.cos() - y * r.sin()).round() as i32,
                (50.0 + x * r.sin() + y * r.cos()).round() as i32,
            ));
        }
        // cv2: ((50.0, 50.0), (5.8767376, 20.234249), 99.214058)
        let e = fit_ellipse(&pts);
        assert!(
            (e.cx - 50.0).abs() < 1e-3 && (e.cy - 50.0).abs() < 1e-3,
            "{e:?}"
        );
        assert!((e.w - 5.876_737_6).abs() < 1e-3 && (e.h - 20.234_249).abs() < 1e-3);
        assert!((e.angle - 99.214_058).abs() < 1e-3);
    }

    #[test]
    fn polygon_tests_match_opencv() {
        let sq: Vec<Ptf> = vec![(0., 0.), (10., 0.), (10., 10.), (0., 10.)];
        assert_eq!(point_polygon_test(&sq, (10.0, 5.0)), 0.0);
        assert_eq!(point_polygon_test(&sq, (10.5, 5.0)), -1.0);
        assert_eq!(point_polygon_test(&sq, (5.0, 5.0)), 1.0);
        let far: Vec<Ptf> = sq.iter().map(|p| (p.0 + 20.0, p.1 + 20.0)).collect();
        assert_eq!(intersect_convex_convex(&sq, &far), 0.0);
        let half: Vec<Ptf> = sq.iter().map(|p| (p.0 + 5.0, p.1)).collect();
        assert!((intersect_convex_convex(&sq, &half) - 50.0).abs() < 1e-4);
        let inner: Vec<Ptf> = vec![(2., 2.), (4., 2.), (4., 4.), (2., 4.)];
        assert!((intersect_convex_convex(&sq, &inner) - 4.0).abs() < 1e-4);
    }

    #[test]
    fn fill_convex_poly_triangle() {
        let mut g = Gray::new(6, 6);
        fill_convex_poly(&mut g, &[(0, 0), (5, 0), (0, 5)], 255);
        // row y has x = 0..=5-y filled
        for y in 0..6 {
            for x in 0..6 {
                assert_eq!(g.get(x, y) == 255, x + y <= 5, "{x},{y}");
            }
        }
    }

    #[test]
    fn fill_poly_square() {
        let mut g = Gray::new(5, 5);
        fill_poly(&mut g, &[vec![(1, 1), (1, 3), (3, 3), (3, 1)]], 9);
        assert_eq!(g.data.iter().filter(|&&v| v == 9).count(), 9);
    }

    #[test]
    fn warp_affine_identity_and_shift() {
        let mut g = Gray::new(4, 3);
        for (i, v) in g.data.iter_mut().enumerate() {
            *v = (i * 10) as u8;
        }
        let id = warp_affine(&g, &[1., 0., 0., 0., 1., 0.], 4, 3, 7);
        assert_eq!(id, g);
        let sh = warp_affine(&g, &[1., 0., 1., 0., 1., 0.], 4, 3, 7);
        assert_eq!(sh.get(0, 0), 7);
        assert_eq!(sh.get(1, 0), g.get(0, 0));
    }

    #[test]
    fn ellipse2poly_circle() {
        let p = ellipse2poly((10, 10), (3, 3), 0);
        assert_eq!(p[0], (13, 10));
        assert!(p.contains(&(10, 13)) && p.contains(&(7, 10)) && p.contains(&(10, 7)));
    }
}
