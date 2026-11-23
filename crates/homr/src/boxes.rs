//! Stage 3 (homr `noise_filtering`, `staff_detection.make_lines_stronger`,
//! `bounding_boxes`): remove noise from the masks and turn blobs into
//! rotated boxes and ellipses. Ported from homr 0.7.1 (AGPL-3.0, Copyright
//! the homr contributors).
pub mod contours;
pub mod geom;
pub(crate) mod morph;
mod sin_table;

use crate::{Gray, Predictions};
use contours::{find_contours, Pt, Retr};
use geom::{Ptf, RotRect};

// ------------------------------------------------------------ noise filter

const IMAGE_NOISE_LIMIT: f64 = 50.0;

fn reflect101(p: isize, n: isize) -> usize {
    if n == 1 {
        return 0;
    }
    let mut p = p;
    while p < 0 || p >= n {
        if p < 0 {
            p = -p;
        } else {
            p = 2 * n - 2 - p;
        }
    }
    p as usize
}

/// homr `estimate_noise` on a tile of a 0/255 image: mean absolute
/// Laplacian-like response (cv2.filter2D, BORDER_REFLECT_101).
fn estimate_noise(img: &Gray, x0: usize, y0: usize, w: usize, h: usize) -> f64 {
    const M: [[f64; 3]; 3] = [[1., -2., 1.], [-2., 4., -2.], [1., -2., 1.]];
    let mut sum = 0f64;
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0f64;
            for (ky, row) in M.iter().enumerate() {
                let yy = reflect101(y as isize + ky as isize - 1, h as isize);
                for (kx, k) in row.iter().enumerate() {
                    let xx = reflect101(x as isize + kx as isize - 1, w as isize);
                    acc += k * img.get(x0 + xx, y0 + yy) as f64;
                }
            }
            sum += acc.abs();
        }
    }
    sum / (h * w) as f64
}

/// homr `create_noise_grid`: a 0/255 mask of the cells to keep, or None if
/// nothing (or more than half the page) would be filtered.
pub fn noise_mask(staff255: &Gray) -> Option<Gray> {
    let (iw, ih) = (staff255.width, staff255.height);
    let (m, n) = (ih / 20, iw / 20);
    if m == 0 || n == 0 {
        return None;
    }
    let rows = ih.div_ceil(m);
    let cols = iw.div_ceil(n);
    let mut grid = vec![0u8; rows * cols];
    for (i, y1) in (0..ih).step_by(m).enumerate() {
        for (j, x1) in (0..iw).step_by(n).enumerate() {
            let h = m.min(ih - y1);
            let w = n.min(iw - x1);
            // numpy: float into a uint8 array truncates and wraps
            grid[i * cols + j] = (estimate_noise(staff255, x1, y1, w, h) as i64) as u8;
        }
    }
    let g = |i: usize, j: usize| grid[i * cols + j] as f64;
    let mut mask = Gray::new(iw, ih);
    let (mut filtered, mut total) = (0usize, 0usize);
    for (i, y1) in (0..ih).step_by(m).enumerate() {
        for (j, x1) in (0..iw).step_by(n).enumerate() {
            let mut neighbors = Vec::new();
            if i > 0 {
                neighbors.push(g(i - 1, j));
            }
            if j > 0 {
                neighbors.push(g(i, j - 1));
            }
            if i < rows - 1 {
                neighbors.push(g(i + 1, j));
            }
            if j < cols - 1 {
                neighbors.push(g(i, j + 1));
            }
            let any = neighbors.iter().any(|&v| v > IMAGE_NOISE_LIMIT);
            if g(i, j) > IMAGE_NOISE_LIMIT && any {
                filtered += 1;
            } else {
                for y in y1..(y1 + m).min(ih) {
                    for x in x1..(x1 + n).min(iw) {
                        mask.set(x, y, 255);
                    }
                }
            }
            total += 1;
        }
    }
    if filtered * 2 > total || filtered == 0 {
        None
    } else {
        Some(mask)
    }
}

fn and_mask(img: &Gray, mask: &Gray) -> Gray {
    let mut out = img.clone();
    for (o, &m) in out.data.iter_mut().zip(&mask.data) {
        if m == 0 {
            *o = 0;
        }
    }
    out
}

/// homr `filter_predictions` followed by `make_lines_stronger(staff, (1, 2))`,
/// as `load_and_preprocess_predictions` does.
pub fn filter_predictions(pred: Predictions) -> Predictions {
    let mut staff255 = pred.staff.clone();
    for v in &mut staff255.data {
        *v = v.wrapping_mul(255);
    }
    let pred = match noise_mask(&staff255) {
        None => pred,
        Some(mask) => Predictions {
            notehead: and_mask(&pred.notehead, &mask),
            symbols: and_mask(&pred.symbols, &mask),
            staff: and_mask(&pred.staff, &mask),
            clefs_keys: and_mask(&pred.clefs_keys, &mask),
            stems_rest: and_mask(&pred.stems_rest, &mask),
        },
    };
    Predictions {
        staff: make_lines_stronger(&pred.staff, 1, 2),
        ..pred
    }
}

/// homr `make_lines_stronger`: dilate with an ellipse (w, h) and threshold to 0/1.
pub fn make_lines_stronger(img: &Gray, w: usize, h: usize) -> Gray {
    let k = morph::ellipse(w, h);
    let mut out = morph::dilate(img, &k);
    for v in &mut out.data {
        *v = (*v > 0) as u8;
    }
    out
}

// ------------------------------------------------------------ boxes

/// The shape of an [`AngledBox`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// homr `RotatedBoundingBox`.
    Rotated,
    /// homr `BoundingEllipse`.
    Ellipse,
    /// homr `BoundingBox` (axis aligned, from integer corners).
    Axis,
}

/// homr's `AngledBoundingBox` family: a rotated rectangle (or ellipse) with
/// its angle normalised to -45..45 degrees, the contour it came from and the
/// polygon used for overlap tests.
#[derive(Debug, Clone)]
pub struct AngledBox {
    pub kind: Kind,
    /// The normalised box (centre, size, angle): homr `self.box`.
    pub rect: RotRect,
    pub contours: Vec<Pt>,
    pub polygon: Vec<Pt>,
    pub debug_id: i64,
    /// For `Kind::Axis`: (x1, y1, x2, y2).
    pub axis: (i64, i64, i64, i64),
}

impl PartialEq for AngledBox {
    /// Like homr: two boxes are equal if their (centre, size, angle) are.
    fn eq(&self, o: &Self) -> bool {
        self.rect == o.rect
    }
}

fn normalise(b: RotRect) -> RotRect {
    let a = b.angle;
    let (w, h) = (b.w, b.h);
    let (w, h, a) = if a > 135.0 {
        (w, h, a - 180.0)
    } else if a < -135.0 {
        (w, h, a + 180.0)
    } else if a > 45.0 {
        (h, w, a - 90.0)
    } else if a < -45.0 {
        (h, w, a + 90.0)
    } else {
        (w, h, a)
    };
    RotRect {
        cx: b.cx,
        cy: b.cy,
        w,
        h,
        angle: a,
    }
}

fn ellipse_polygon(b: &RotRect) -> Vec<Pt> {
    geom::ellipse2poly(
        (b.cx as i64 as i32, b.cy as i64 as i32),
        ((b.w / 2.0) as i64 as i32, (b.h / 2.0) as i64 as i32),
        b.angle as i64 as i32,
    )
}

impl AngledBox {
    /// homr `RotatedBoundingBox(box, contours, debug_id)`.
    pub fn rotated(b: RotRect, contours: Vec<Pt>, debug_id: i64) -> AngledBox {
        AngledBox {
            kind: Kind::Rotated,
            polygon: geom::box_points_int(&b),
            rect: normalise(b),
            contours,
            debug_id,
            axis: (0, 0, 0, 0),
        }
    }
    /// homr `BoundingEllipse(box, contours, debug_id)`.
    pub fn ellipse(b: RotRect, contours: Vec<Pt>, debug_id: i64) -> AngledBox {
        AngledBox {
            kind: Kind::Ellipse,
            polygon: ellipse_polygon(&b),
            rect: normalise(b),
            contours,
            debug_id,
            axis: (0, 0, 0, 0),
        }
    }
    /// homr `BoundingBox((x1, y1, x2, y2), contours, debug_id)`.
    pub fn axis(x1: i64, y1: i64, x2: i64, y2: i64, contours: Vec<Pt>, debug_id: i64) -> AngledBox {
        let rect = RotRect {
            cx: (x1 + x2) as f64 / 2.0,
            cy: (y1 + y2) as f64 / 2.0,
            w: (x2 - x1) as f64,
            h: (y2 - y1) as f64,
            angle: 0.0,
        };
        AngledBox {
            kind: Kind::Axis,
            polygon: geom::box_points_int(&rect),
            rect,
            contours,
            debug_id,
            axis: (x1, y1, x2, y2),
        }
    }
    fn rebuild(&self, b: RotRect) -> AngledBox {
        match self.kind {
            Kind::Ellipse => AngledBox::ellipse(b, self.contours.clone(), self.debug_id),
            _ => AngledBox::rotated(b, self.contours.clone(), self.debug_id),
        }
    }

    pub fn center(&self) -> (f64, f64) {
        (self.rect.cx, self.rect.cy)
    }
    pub fn size(&self) -> (f64, f64) {
        (self.rect.w, self.rect.h)
    }
    pub fn angle(&self) -> f64 {
        self.rect.angle
    }
    /// homr `calculate_edges_of_rotated_rectangle`: (top_left, bottom_left,
    /// top_right, bottom_right) of the unrotated box.
    pub fn top_left(&self) -> (f64, f64) {
        (
            self.rect.cx - self.rect.w / 2.0,
            self.rect.cy - self.rect.h / 2.0,
        )
    }
    pub fn bottom_left(&self) -> (f64, f64) {
        (
            self.rect.cx - self.rect.w / 2.0,
            self.rect.cy + self.rect.h / 2.0,
        )
    }
    pub fn top_right(&self) -> (f64, f64) {
        (
            self.rect.cx + self.rect.w / 2.0,
            self.rect.cy - self.rect.h / 2.0,
        )
    }
    pub fn bottom_right(&self) -> (f64, f64) {
        (
            self.rect.cx + self.rect.w / 2.0,
            self.rect.cy + self.rect.h / 2.0,
        )
    }

    fn can_touch(&self, o: &AngledBox) -> bool {
        let m1 = self.rect.w.max(self.rect.h);
        let m2 = o.rect.w.max(o.rect.h);
        let d = ((self.rect.cx - o.rect.cx).powi(2) + (self.rect.cy - o.rect.cy).powi(2)).sqrt();
        d <= m1 + m2
    }

    /// homr `is_overlapping`.
    pub fn is_overlapping(&self, o: &AngledBox) -> bool {
        self.can_touch(o) && polygons_overlap(&self.polygon, &o.polygon)
    }
    pub fn is_overlapping_with_any(&self, others: &[AngledBox]) -> bool {
        others.iter().any(|o| self.is_overlapping(o))
    }
    /// homr `RotatedBoundingBox.is_intersecting`.
    pub fn is_intersecting(&self, o: &AngledBox) -> bool {
        self.can_touch(o) && geom::rotated_rects_intersect(&self.rect, &o.rect)
    }

    pub fn make_box_thicker(&self, t: f64) -> AngledBox {
        if t <= 0.0 && self.kind != Kind::Ellipse {
            return self.clone();
        }
        let mut b = self.rect;
        b.w += t;
        b.h += t;
        self.rebuild(b)
    }
    /// homr `make_box_taller`: always a RotatedBoundingBox.
    pub fn make_box_taller(&self, t: f64) -> AngledBox {
        let mut b = self.rect;
        b.h += t;
        AngledBox::rotated(b, self.contours.clone(), self.debug_id)
    }
    pub fn move_to_x_horizontal_by(&self, dx: f64) -> AngledBox {
        let mut b = self.rect;
        b.cx += dx;
        AngledBox::rotated(b, self.contours.clone(), self.debug_id)
    }
    pub fn get_center_extrapolated(&self, x: f64) -> f64 {
        (x - self.rect.cx) * (self.rect.angle / 180.0 * std::f64::consts::PI).tan() + self.rect.cy
    }

    /// homr `RotatedBoundingBox.is_overlapping_extrapolated`.
    pub fn is_overlapping_extrapolated(&self, other: &AngledBox, unit_size: f64) -> bool {
        let (left, right) = if self.rect.cx > other.rect.cx {
            (other, self)
        } else {
            (self, other)
        };
        let center_x = (left.rect.cx + right.rect.cx) * 0.5;
        let tolerance = unit_size / 3.0;
        let max_gap = 5.0 * unit_size;
        if center_x - left.rect.cx - (left.rect.w / 2.0).floor() > max_gap
            || right.rect.cx - center_x - (right.rect.w / 2.0).floor() > max_gap
        {
            return false;
        }
        let la = (left.rect.angle * std::f64::consts::PI / 180.0).tan();
        let ra = (right.rect.angle * std::f64::consts::PI / 180.0).tan();
        let ly = (center_x - left.rect.cx) * la + left.rect.cy;
        let ry = (center_x - right.rect.cx) * ra + right.rect.cy;
        (ly - ry).abs() <= tolerance
    }
}

fn to_f(p: &[Pt]) -> Vec<Ptf> {
    p.iter().map(|q| (q.0 as f32, q.1 as f32)).collect()
}

/// homr `do_polygons_overlap`.
pub fn polygons_overlap(p1: &[Pt], p2: &[Pt]) -> bool {
    let f1 = to_f(p1);
    let f2 = to_f(p2);
    if geom::intersect_convex_convex(&f1, &f2) > 0.0 {
        return true;
    }
    f1.iter().any(|&p| geom::point_polygon_test(&f2, p) >= 0.0)
        || f2.iter().any(|&p| geom::point_polygon_test(&f1, p) >= 0.0)
}

fn valid(b: &RotRect) -> bool {
    !b.w.is_nan() && !b.h.is_nan() && b.w > 0.0 && b.h > 0.0
}

/// homr `create_rotated_bounding_boxes`. `max_size` components <= 0 are ignored.
pub fn create_rotated_bounding_boxes(
    img: &Gray,
    skip_merging: bool,
    min_size: Option<(f64, f64)>,
    max_size: Option<(f64, f64)>,
) -> Vec<AngledBox> {
    let mut boxes = Vec::new();
    for (i, c) in find_contours(img, Retr::Tree).into_iter().enumerate() {
        let fit = geom::min_area_rect(&c);
        if !valid(&fit) {
            continue;
        }
        let b = AngledBox::rotated(fit, c, i as i64);
        let (w, h) = b.size();
        if let Some((mw, mh)) = min_size {
            if w < mw || h < mh {
                continue;
            }
        }
        if let Some((xw, xh)) = max_size {
            if (xw > 0.0 && w > xw) || (xh > 0.0 && h > xh) {
                continue;
            }
        }
        boxes.push(b);
    }
    if skip_merging {
        return boxes;
    }
    merge_overlapping(&boxes)
        .into_iter()
        .map(|g| {
            let all: Vec<Pt> = g.iter().flat_map(|b| b.contours.iter().copied()).collect();
            AngledBox::rotated(geom::min_area_rect(&all), all, 0)
        })
        .collect()
}

/// homr `create_bounding_ellipses` (with merging).
pub fn create_bounding_ellipses(img: &Gray, min_size: Option<(f64, f64)>) -> Vec<AngledBox> {
    let mut boxes = Vec::new();
    for (i, c) in find_contours(img, Retr::Tree).into_iter().enumerate() {
        if c.len() < 5 {
            continue;
        }
        let fit = geom::fit_ellipse(&c);
        if !valid(&fit) {
            continue;
        }
        let b = AngledBox::ellipse(fit, c, i as i64);
        let (w, h) = b.size();
        if let Some((mw, mh)) = min_size {
            if w < mw || h < mh {
                continue;
            }
        }
        boxes.push(b);
    }
    merge_overlapping(&boxes)
        .into_iter()
        .map(|g| {
            let all: Vec<Pt> = g.iter().flat_map(|b| b.contours.iter().copied()).collect();
            AngledBox::ellipse(geom::min_area_rect(&all), all, 0)
        })
        .collect()
}

/// homr `merge_overlaying_bounding_boxes` (union-find, groups in the order
/// of their first box).
pub fn merge_overlapping(boxes: &[AngledBox]) -> Vec<Vec<&AngledBox>> {
    let n = boxes.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank = vec![0u32; n];
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut c = x;
        while p[c] != r {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    for i in 0..n {
        for j in i + 1..n {
            if boxes[i].is_overlapping(&boxes[j]) {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    if rank[ri] > rank[rj] {
                        parent[rj] = ri;
                    } else if rank[ri] < rank[rj] {
                        parent[ri] = rj;
                    } else {
                        parent[rj] = ri;
                        rank[ri] += 1;
                    }
                }
            }
        }
    }
    let mut order: Vec<usize> = Vec::new();
    let mut groups: std::collections::HashMap<usize, Vec<&AngledBox>> = Default::default();
    for (i, b) in boxes.iter().enumerate() {
        let r = find(&mut parent, i);
        groups
            .entry(r)
            .or_insert_with(|| {
                order.push(r);
                Vec::new()
            })
            .push(b);
    }
    order
        .into_iter()
        .map(|r| groups.remove(&r).unwrap())
        .collect()
}

/// homr `create_rotated_bounding_box(contour)`.
pub fn rotated_box_of(contour: Vec<Pt>, debug_id: i64) -> AngledBox {
    AngledBox::rotated(geom::min_area_rect(&contour), contour, debug_id)
}
