//! An exact fraction, like Python's `fractions.Fraction` which homr uses for
//! all durations (relative to a whole note).
use std::cmp::Ordering;
use std::ops::{Add, Div, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frac {
    n: i64,
    d: i64,
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Frac {
    pub const ZERO: Frac = Frac { n: 0, d: 1 };

    pub fn new(n: i64, d: i64) -> Frac {
        assert!(d != 0, "zero denominator");
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Frac {
            n: s * n / g,
            d: s * d / g,
        }
    }
    pub fn int(n: i64) -> Frac {
        Frac { n, d: 1 }
    }
    pub fn numer(self) -> i64 {
        self.n
    }
    pub fn denom(self) -> i64 {
        self.d
    }
    /// Python `int(fraction)`: truncates towards zero.
    pub fn trunc(self) -> i64 {
        self.n / self.d
    }
    pub fn is_positive(self) -> bool {
        self.n > 0
    }
}

impl Ord for Frac {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.n as i128 * o.d as i128).cmp(&(o.n as i128 * self.d as i128))
    }
}
impl PartialOrd for Frac {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Add for Frac {
    type Output = Frac;
    fn add(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d + o.n * self.d, self.d * o.d)
    }
}
impl Sub for Frac {
    type Output = Frac;
    fn sub(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d - o.n * self.d, self.d * o.d)
    }
}
impl Mul for Frac {
    type Output = Frac;
    fn mul(self, o: Frac) -> Frac {
        Frac::new(self.n * o.n, self.d * o.d)
    }
}
impl Div for Frac {
    type Output = Frac;
    fn div(self, o: Frac) -> Frac {
        Frac::new(self.n * o.d, self.d * o.n)
    }
}

/// `numpy.median` of fractions: the middle one, or the mean of the two middle ones.
pub fn median(values: &[Frac]) -> Frac {
    let mut v = values.to_vec();
    v.sort();
    let m = v.len() / 2;
    if v.len() % 2 == 1 {
        v[m]
    } else {
        (v[m - 1] + v[m]) / Frac::int(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_fraction_arithmetic_and_median() {
        assert_eq!(Frac::new(2, 4), Frac::new(1, 2));
        assert_eq!(Frac::new(1, -2), Frac::new(-1, 2));
        assert_eq!(Frac::new(1, 4) + Frac::new(1, 8), Frac::new(3, 8));
        assert_eq!(Frac::new(3, 2) * Frac::int(8), Frac::int(12));
        assert!(Frac::new(1, 3) < Frac::new(1, 2));
        assert_eq!(Frac::new(7, 2).trunc(), 3);
        // As numpy: [3/4, 3/2] -> 9/8, odd count -> middle.
        assert_eq!(median(&[Frac::new(3, 4), Frac::new(3, 2)]), Frac::new(9, 8));
        assert_eq!(
            median(&[Frac::new(3, 4), Frac::new(3, 2), Frac::int(1)]),
            Frac::int(1)
        );
    }
}
