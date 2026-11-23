//! homr's `Staff`/`StaffPoint` model (`model.py`) and the numpy reductions
//! it uses, with numpy's summation order so medians and means match.
use crate::boxes::AngledBox;

/// `np.sum` (pairwise summation, as numpy does for float64).
pub fn np_sum(a: &[f64]) -> f64 {
    fn pairwise(a: &[f64]) -> f64 {
        let n = a.len();
        if n < 8 {
            let mut res = 0.0;
            for &v in a {
                res += v;
            }
            res
        } else if n <= 128 {
            let mut r = [0f64; 8];
            r.copy_from_slice(&a[..8]);
            let mut i = 8;
            while i < n - (n % 8) {
                for k in 0..8 {
                    r[k] += a[i + k];
                }
                i += 8;
            }
            let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
            for &v in &a[i..] {
                res += v;
            }
            res
        } else {
            let mut n2 = n / 2;
            n2 -= n2 % 8;
            pairwise(&a[..n2]) + pairwise(&a[n2..])
        }
    }
    pairwise(a)
}

pub fn np_mean(a: &[f64]) -> f64 {
    np_sum(a) / a.len() as f64
}

pub fn np_median(a: &[f64]) -> f64 {
    let mut v = a.to_vec();
    v.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        np_mean(&[v[n / 2 - 1], v[n / 2]])
    }
}

pub fn np_std(a: &[f64]) -> f64 {
    let m = np_mean(a);
    let d: Vec<f64> = a.iter().map(|v| (v - m).abs().powi(2)).collect();
    np_mean(&d).sqrt()
}

/// Python's `round()` (half to even).
pub fn py_round(v: f64) -> f64 {
    v.round_ties_even()
}

/// homr `StaffPoint`.
#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: Vec<f64>,
    pub angle: f64,
    pub average_unit_size: f64,
}

impl Point {
    pub fn new(x: f64, y: Vec<f64>, angle: f64) -> Point {
        let d: Vec<f64> = y.windows(2).map(|w| w[1] - w[0]).collect();
        Point {
            x,
            average_unit_size: np_mean(&d),
            y,
            angle,
        }
    }
    /// homr `StaffPoint.merge`.
    pub fn merge(&self, o: &Point) -> Point {
        let mut y = self.y.clone();
        y.extend_from_slice(&o.y);
        y.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Point::new(self.x, y, (self.angle + o.angle) / 2.0)
    }
    /// homr `StaffPoint.to_bounding_box`.
    pub fn to_bounding_box(&self) -> AngledBox {
        let x = self.x as i64;
        AngledBox::axis(
            x,
            self.y[0] as i64,
            x,
            *self.y.last().unwrap() as i64,
            Vec::new(),
            -2,
        )
    }
}

/// homr `Staff` (without symbols: homr only ever adds notes to them, and
/// nothing in stages 4 and 5 reads them back).
#[derive(Debug, Clone)]
pub struct Staff {
    pub grid: Vec<Point>,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub average_unit_size: f64,
    pub is_grandstaff: bool,
}

pub const STAFF_POSITION_TOLERANCE: f64 = 50.0;

impl Staff {
    pub fn new(grid: Vec<Point>) -> Staff {
        let min_y = grid
            .iter()
            .map(|p| p.y.iter().cloned().fold(f64::INFINITY, f64::min))
            .fold(f64::INFINITY, f64::min);
        let max_y = grid
            .iter()
            .map(|p| p.y.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
            .fold(f64::NEG_INFINITY, f64::max);
        let units: Vec<f64> = grid.iter().map(|p| p.average_unit_size).collect();
        Staff {
            min_x: grid[0].x,
            max_x: grid[grid.len() - 1].x,
            min_y,
            max_y,
            average_unit_size: np_median(&units),
            grid,
            is_grandstaff: false,
        }
    }

    pub fn get_at(&self, x: f64) -> Option<&Point> {
        let mut best = &self.grid[0];
        for p in &self.grid[1..] {
            if (p.x - x).abs() < (best.x - x).abs() {
                best = p;
            }
        }
        if (best.x - x).abs() > STAFF_POSITION_TOLERANCE {
            None
        } else {
            Some(best)
        }
    }

    pub fn y_distance_to(&self, pt: (f64, f64)) -> f64 {
        match self.get_at(pt.0) {
            None => 1e10,
            Some(p) => {
                p.y.iter()
                    .map(|y| (y - pt.1).abs())
                    .fold(f64::INFINITY, f64::min)
            }
        }
    }

    /// homr `Staff.merge`: a grand staff from two staffs.
    pub fn merge(&self, other: &Staff) -> Staff {
        let key = |p: &Point| py_round(p.x) as i64;
        let mut a: std::collections::BTreeMap<i64, &Point> = Default::default();
        for p in &self.grid {
            a.insert(key(p), p);
        }
        let mut b: std::collections::BTreeMap<i64, &Point> = Default::default();
        for p in &other.grid {
            b.insert(key(p), p);
        }
        let grid: Vec<Point> = a
            .iter()
            .filter_map(|(k, p)| b.get(k).map(|q| p.merge(q)))
            .collect();
        let mut s = Staff::new(grid);
        s.is_grandstaff = true;
        s
    }

    /// homr `Staff.split`: the inverse of `merge`.
    pub fn split(&self) -> Option<(Staff, Staff)> {
        let grid: Vec<&Point> = self.grid.iter().filter(|p| p.y.len() == 10).collect();
        if grid.is_empty() {
            return None;
        }
        let upper = Staff::new(
            grid.iter()
                .map(|p| Point::new(p.x, p.y[..5].to_vec(), p.angle))
                .collect(),
        );
        let lower = Staff::new(
            grid.iter()
                .map(|p| Point::new(p.x, p.y[5..].to_vec(), p.angle))
                .collect(),
        );
        Some((upper, lower))
    }

    pub fn to_public(&self) -> crate::Staff {
        crate::Staff {
            grandstaff: self.is_grandstaff,
            min_x: self.min_x,
            max_x: self.max_x,
            min_y: self.min_y,
            max_y: self.max_y,
            grid: self
                .grid
                .iter()
                .map(|p| crate::StaffPoint {
                    x: p.x,
                    y: p.y.clone(),
                    angle: p.angle,
                })
                .collect(),
        }
    }

    pub fn from_public(s: &crate::Staff) -> Staff {
        let mut out = Staff::new(
            s.grid
                .iter()
                .map(|p| Point::new(p.x, p.y.clone(), p.angle))
                .collect(),
        );
        out.is_grandstaff = s.grandstaff;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numpy_reductions() {
        assert_eq!(np_median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(np_median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!((np_std(&[1.0, 2.0, 3.0, 4.0]) - 1.118_033_988_749_895).abs() < 1e-15);
        let v: Vec<f64> = (0..300).map(|i| i as f64 * 0.1).collect();
        assert!((np_sum(&v) - 4485.0).abs() < 1e-9);
        assert_eq!(py_round(2.5), 2.0);
        assert_eq!(py_round(3.5), 4.0);
    }

    #[test]
    fn merge_and_split_a_grand_staff() {
        let up = Staff::new(vec![
            Point::new(0.0, vec![0., 1., 2., 3., 4.], 0.0),
            Point::new(10.0, vec![0., 1., 2., 3., 4.], 0.0),
        ]);
        let lo = Staff::new(vec![Point::new(10.0, vec![10., 11., 12., 13., 14.], 2.0)]);
        let g = up.merge(&lo);
        assert!(g.is_grandstaff);
        assert_eq!(g.grid.len(), 1);
        assert_eq!(g.grid[0].y.len(), 10);
        assert_eq!(g.grid[0].angle, 1.0);
        let (u, l) = g.split().unwrap();
        assert_eq!(u.grid[0].y, vec![0., 1., 2., 3., 4.]);
        assert_eq!(l.min_y, 10.0);
    }
}
