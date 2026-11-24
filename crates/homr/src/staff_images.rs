//! Stage 5 (homr `staff_parsing.prepare_staff_image`, `staff_dewarping`):
//! cut a straightened strip per staff (grand staffs: both) as the
//! transformer's input, 1280x256. Ported from homr 0.7.1 (AGPL-3.0,
//! Copyright the homr contributors).
pub mod delaunay;

use crate::boxes::contours::{find_contours, Retr};
use crate::boxes::geom::{self, Ptf};
use crate::image_ops::{self, Interp};
use crate::staffs::model::{np_mean, Point, Staff};
use crate::{Gray, System};

/// The transformer's input size (homr `Config.max_height`, `max_width`).
pub const MAX_HEIGHT: usize = 256;
pub const MAX_WIDTH: usize = 1280;

/// One strip per staff (or grand staff), in homr's reading order
/// ([`crate::musicxml::strip_order`]).
pub fn cut(image: &Gray, systems: &[System]) -> Vec<Gray> {
    let regions = Regions::new(systems);
    crate::musicxml::strip_order(systems)
        .into_iter()
        .map(|(s, i)| {
            prepare_staff_image(&Staff::from_public(&systems[s].staffs[i]), image, &regions)
        })
        .collect()
}

/// homr `StaffRegions`: (min_y, max_y) of every staff on the page.
pub struct Regions(Vec<(f64, f64)>);

impl Regions {
    pub fn new(systems: &[System]) -> Regions {
        Regions(
            systems
                .iter()
                .flat_map(|s| s.staffs.iter().map(|t| (t.min_y, t.max_y)))
                .collect(),
        )
    }
    fn above(&self, y: f64) -> f64 {
        self.0
            .iter()
            .filter(|c| c.0 < y)
            .map(|c| c.1)
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))))
            .unwrap_or(0.0)
    }
    fn below(&self, y: f64) -> f64 {
        self.0
            .iter()
            .filter(|c| c.1 > y)
            .map(|c| c.0)
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
            .unwrap_or(1e12)
    }
}

/// homr `get_tr_omr_canvas_size`: (width, height) of the content on the canvas.
pub fn canvas_size(h: f64, w: f64) -> (usize, usize) {
    let ratio = MAX_HEIGHT as f64 / MAX_WIDTH as f64;
    if h / w > ratio {
        ((w / h * MAX_HEIGHT as f64) as usize, MAX_HEIGHT)
    } else {
        (MAX_WIDTH, (h / w * MAX_WIDTH as f64) as usize)
    }
}

/// homr `crop_image_and_return_new_top`.
fn crop(img: &Gray, x1: f64, y1: f64, x2: f64, y2: f64) -> (Gray, (i64, i64)) {
    let lx = |x: f64| (x.round_ties_even() as i64).clamp(0, img.width as i64 - 1);
    let ly = |y: f64| (y.round_ties_even() as i64).clamp(0, img.height as i64 - 1);
    let (xa, xb) = (lx(x1.min(x2)), lx(x1.max(x2)));
    let (ya, yb) = (ly(y1.min(y2)), ly(y1.max(y2)));
    (sub(img, xa, ya, xb, yb), (xa, ya))
}

/// `img[y0:y1, x0:x1]` with numpy's clipping.
fn sub(img: &Gray, x0: i64, y0: i64, x1: i64, y1: i64) -> Gray {
    let x1 = x1.min(img.width as i64);
    let y1 = y1.min(img.height as i64);
    let (w, h) = ((x1 - x0).max(0) as usize, (y1 - y0).max(0) as usize);
    let mut out = Gray::new(w, h);
    for y in 0..h {
        let s = (y0 as usize + y) * img.width + x0 as usize;
        out.data[y * w..(y + 1) * w].copy_from_slice(&img.data[s..s + w]);
    }
    out
}

/// homr `Staff.transform_coordinates` with `x -= dx; y -= dy; * scale`.
fn shift_staff(staff: &Staff, dx: f64, dy: f64, scale: f64) -> Staff {
    let grid = staff
        .grid
        .iter()
        .map(|p| {
            let xs: Vec<f64> = p.y.iter().map(|_| (p.x - dx) * scale).collect();
            let ys = p.y.iter().map(|y| (y - dy) * scale).collect();
            Point::new(np_mean(&xs), ys, p.angle)
        })
        .collect();
    let mut s = Staff::new(grid);
    s.is_grandstaff = staff.is_grandstaff;
    s
}

type Lines = Vec<Vec<(i64, i64)>>;

/// homr `calculate_span_and_optimal_points`.
fn span_points(staff: &Staff, w: usize, h: usize) -> (Lines, Lines) {
    let (mut span, mut opt) = (Vec::new(), Vec::new());
    let step = h / 6;
    if step == 0 {
        return (span, opt);
    }
    let mut first: Option<f64> = None;
    let on_image = |p: (i64, i64)| {
        let m = 10;
        !(p.0 < m || p.0 > w as i64 - m || p.1 < m || p.1 > h as i64 - m)
    };
    let mut y = 2;
    while y + 2 < h {
        let mut line = Vec::new();
        let mut x = 2;
        while x < w {
            if let Some(v) = staff.get_at(x as f64) {
                let off = v.y[2];
                let delta = match first {
                    // `if not first_y_offset`: 0.0 counts as unset
                    Some(f) if f != 0.0 => (off - f) as i64,
                    _ => {
                        first = Some(off);
                        0
                    }
                };
                let p = (x as i64, y as i64 + delta);
                if on_image(p) {
                    line.push(p);
                }
            }
            x += 80;
        }
        if line.len() > 2 {
            let avg = line.iter().map(|p| p.1 as f64).sum::<f64>() / line.len() as f64;
            opt.push(line.iter().map(|p| (p.0, avg as i64)).collect());
            span.push(line);
        }
        y += step;
    }
    (span, opt)
}

/// homr `PiecewiseAffineTransform` (estimate + warp_image).
struct Piecewise {
    src: Vec<Ptf>,
    dst: Vec<Ptf>,
    simplices: Vec<[usize; 3]>,
    mats: Vec<Option<[f64; 6]>>,
}

fn degenerate(t: &[Ptf; 3]) -> bool {
    let v1 = (t[1].0 - t[0].0, t[1].1 - t[0].1);
    let v2 = (t[2].0 - t[0].0, t[2].1 - t[0].1);
    ((v1.0 * v2.1 - v1.1 * v2.0).abs() / 2.0) < 1e-6
}

impl Piecewise {
    fn estimate(src: Vec<Ptf>, dst: Vec<Ptf>) -> Option<Piecewise> {
        if src.len() < 3 {
            return None;
        }
        let r = geom::bounding_rect_f(&src);
        let mut sd = delaunay::Subdiv::new((r.0 - 10, r.1 - 10, r.2 + 20, r.3 + 20));
        for &p in &src {
            sd.insert(p)?;
        }
        let find = |p: Ptf| -> Option<usize> {
            let mut best = 0;
            let mut bd = f32::INFINITY;
            for (i, q) in src.iter().enumerate() {
                let d = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
                if d < bd {
                    bd = d;
                    best = i;
                }
            }
            (bd < 1e-3).then_some(best)
        };
        let simplices: Vec<[usize; 3]> = sd
            .triangles()
            .iter()
            .filter_map(|t| Some([find(t[0])?, find(t[1])?, find(t[2])?]))
            .collect();
        let mats = simplices
            .iter()
            .map(|s| {
                let st = [src[s[0]], src[s[1]], src[s[2]]];
                let dt = [dst[s[0]], dst[s[1]], dst[s[2]]];
                if degenerate(&st) || degenerate(&dt) {
                    None
                } else {
                    geom::get_affine_transform(&st, &dt)
                }
            })
            .collect();
        Some(Piecewise {
            src,
            dst,
            simplices,
            mats,
        })
    }

    fn warp(&self, img: &Gray, fill: u8) -> Gray {
        let (w, h) = (img.width as i64, img.height as i64);
        let mut out = Gray::new(img.width, img.height);
        out.data.fill(fill);
        for (k, s) in self.simplices.iter().enumerate() {
            if self.mats[k].is_none() {
                continue;
            }
            let st = [self.src[s[0]], self.src[s[1]], self.src[s[2]]];
            let dt = [self.dst[s[0]], self.dst[s[1]], self.dst[s[2]]];
            let sr = geom::bounding_rect_f(&st);
            let dr = geom::bounding_rect_f(&dt);
            if sr.2 <= 0 || sr.3 <= 0 || dr.2 <= 0 || dr.3 <= 0 {
                continue;
            }
            let off = |t: &[Ptf; 3], r: (i32, i32, i32, i32)| {
                t.map(|p| (p.0 - r.0 as f32, p.1 - r.1 as f32))
            };
            let (sc, dc) = (off(&st, sr), off(&dt, dr));
            if degenerate(&sc) || degenerate(&dc) {
                continue;
            }
            let m = match geom::get_affine_transform(&sc, &dc) {
                Some(m) => m,
                None => continue,
            };
            let crop = sub(
                img,
                sr.0 as i64,
                sr.1 as i64,
                (sr.0 + sr.2) as i64,
                (sr.1 + sr.3) as i64,
            );
            if crop.data.is_empty() {
                continue;
            }
            let warped = geom::warp_affine(&crop, &m, dr.2 as usize, dr.3 as usize, fill);
            let mut mask = Gray::new(dr.2 as usize, dr.3 as usize);
            let tri: Vec<(i32, i32)> = dc.iter().map(|p| (p.0 as i32, p.1 as i32)).collect();
            geom::fill_convex_poly(&mut mask, &tri, 255);
            let (x1, y1) = (dr.0 as i64, dr.1 as i64);
            let (x2, y2) = (x1 + dr.2 as i64, y1 + dr.3 as i64);
            for y in y1.max(0)..y2.min(h) {
                for x in x1.max(0)..x2.min(w) {
                    let (mx, my) = ((x - x1) as usize, (y - y1) as usize);
                    if mask.get(mx, my) > 0 {
                        out.set(x as usize, y as usize, warped.get(mx, my));
                    }
                }
            }
        }
        out
    }
}

/// homr `dewarp_staff_image(...).dewarp(image)`.
fn dewarp(img: &Gray, staff: &Staff) -> Gray {
    let (w, h) = (img.width as i64, img.height as i64);
    let (span, opt) = span_points(staff, img.width, img.height);
    let finish = |mut lines: Lines| -> Vec<Ptf> {
        for l in lines.iter_mut() {
            let (a, b) = (l[0].1, l[l.len() - 1].1);
            l.insert(0, (0, a));
            l.push((w, b));
        }
        lines.insert(0, vec![(0, 0), (w, 0)]);
        lines.push(vec![(0, h), (w, h)]);
        lines
            .into_iter()
            .flatten()
            .map(|p| (p.0 as f32, p.1 as f32))
            .collect()
    };
    match Piecewise::estimate(finish(span), finish(opt)) {
        Some(t) => t.warp(img, 1),
        None => img.clone(),
    }
}

/// homr `remove_black_contours_at_edges_of_image`.
fn remove_black_at_edges(gray: &mut Gray, unit: f64) {
    let mut th = gray.clone();
    for v in &mut th.data {
        *v = if *v > 97 { 0 } else { 255 };
    }
    let thr = 2.0 * unit;
    for c in find_contours(&th, Retr::Tree) {
        let (x, y, w, h) = geom::bounding_rect(&c);
        if (w as f64) < thr || (h as f64) < thr {
            continue;
        }
        let edge = x == 0 || y == 0 || x + w == gray.width as i32 || y + h == gray.height as i32;
        if !edge {
            continue;
        }
        let region = sub(&th, x as i64, y as i64, (x + w) as i64, (y + h) as i64);
        let mean = region.data.iter().map(|&v| v as f64).sum::<f64>() / region.data.len() as f64;
        if mean < 127.0 {
            continue;
        }
        for yy in y..(y + h).min(gray.height as i32) {
            for xx in x..(x + w).min(gray.width as i32) {
                gray.set(xx as usize, yy as usize, 255);
            }
        }
    }
}

/// homr `center_image_on_canvas`.
fn center_on_canvas(img: &Gray, size: (usize, usize)) -> Gray {
    let r = image_ops::resize(img, size.0, size.1, Interp::Linear);
    let mut out = Gray::new(MAX_WIDTH, MAX_HEIGHT);
    out.data.fill(255);
    let yo = (MAX_HEIGHT - r.height) / 2;
    for y in 0..r.height.min(MAX_HEIGHT) {
        let w = r.width.min(MAX_WIDTH);
        out.data[(yo + y) * MAX_WIDTH..(yo + y) * MAX_WIDTH + w]
            .copy_from_slice(&r.data[y * r.width..y * r.width + w]);
    }
    out
}

/// homr `prepare_staff_image`: the strip of `staff` on the page `image`.
pub fn prepare_staff_image(staff: &Staff, image: &Gray, regions: &Regions) -> Gray {
    let u = staff.average_unit_size;
    let rx0 = (staff.min_x - 2.0 * u) as i64 as f64;
    let ry0 = ((staff.min_y - 4.0 * u).max(regions.above(staff.min_y))) as i64 as f64;
    let rx1 = (staff.max_x + 2.0 * u) as i64 as f64;
    let ry1 = ((staff.max_y + 4.0 * u).min(regions.below(staff.max_y))) as i64 as f64;
    let dims = canvas_size(ry1 - ry0, rx1 - rx0);
    let sf = dims.1 as f64 / (ry1 - ry0);
    let page = image_ops::resize(
        image,
        (image.width as f64 * sf) as usize,
        (image.height as f64 * sf) as usize,
        Interp::Linear,
    );
    let r = [rx0, ry0, rx1, ry1].map(|v| (v * sf).round_ties_even());
    let (img, tl) = crop(&page, r[0] - 10.0, r[1] - 50.0, r[2] + 10.0, r[3] + 50.0);
    let r2 = [
        r[0] - tl.0 as f64,
        r[1] - tl.1 as f64,
        r[2] - tl.0 as f64,
        r[3] - tl.1 as f64,
    ];
    let staff = shift_staff(staff, tl.0 as f64 / sf, tl.1 as f64 / sf, sf);
    let img = dewarp(&img, &staff);
    let (mut img, _) = crop(&img, r2[0], r2[1], r2[2], r2[3]);
    remove_black_at_edges(&mut img, staff.average_unit_size);
    center_on_canvas(&img, dims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_size_like_homr() {
        assert_eq!(canvas_size(100.0, 1000.0), (1280, 128));
        assert_eq!(canvas_size(300.0, 600.0), (512, 256));
    }

    #[test]
    fn regions_above_and_below() {
        let r = Regions(vec![(0.0, 10.0), (20.0, 30.0), (40.0, 50.0)]);
        assert_eq!(r.above(20.0), 10.0);
        assert_eq!(r.above(0.0), 0.0);
        assert_eq!(r.below(30.0), 40.0);
        assert_eq!(r.below(50.0), 1e12);
    }

    #[test]
    fn flat_staff_dewarps_to_itself_inside() {
        // A straight staff: the source and destination points agree, so the
        // warp is the identity wherever a triangle covers the image.
        let grid: Vec<Point> = (0..30)
            .map(|i| Point::new(i as f64 * 10.0, vec![40.0, 50.0, 60.0, 70.0, 80.0], 0.0))
            .collect();
        let staff = Staff::new(grid);
        let mut img = Gray::new(300, 120);
        for (i, v) in img.data.iter_mut().enumerate() {
            *v = (i % 251) as u8;
        }
        let out = dewarp(&img, &staff);
        let same = img
            .data
            .iter()
            .zip(&out.data)
            .filter(|(a, b)| a == b)
            .count();
        assert!(same as f64 > 0.95 * img.data.len() as f64, "{same}");
    }
}
