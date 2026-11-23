//! `cv2.findContours` with `CHAIN_APPROX_SIMPLE`, a port of OpenCV 4.14's
//! Suzuki border following (`imgproc/src/contours_new.cpp`, Apache-2.0,
//! Copyright the OpenCV authors). The contours and their points come out in
//! the same order as OpenCV's, which homr depends on (box ids, merging).
use crate::Gray;

/// A pixel position (x, y).
pub type Pt = (i32, i32);

/// Which contours to retrieve (`cv2.RETR_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retr {
    /// Only outer contours (`RETR_EXTERNAL`).
    External,
    /// All contours, holes included (`RETR_TREE`).
    Tree,
}

const DELTAS: [Pt; 8] = [
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
const RIGHT: i8 = -128; // 0x80
const NEW: i8 = 2;
const FLAGS: i8 = -2; // 0xFE
const BLACK: i8 = 1;
const LVAL: i8 = 0x7F;

struct Node {
    parent: i32,
    first_child: i32,
    prev: i32,
    next: i32,
    ctable_next: i32,
    pts: Vec<Pt>,
    // x, y, width, height
    brect: (i32, i32, i32, i32),
    origin: Pt,
    is_hole: bool,
}

impl Node {
    fn new() -> Node {
        Node {
            parent: -1,
            first_child: -1,
            prev: -1,
            next: -1,
            ctable_next: -1,
            pts: Vec::new(),
            brect: (0, 0, 0, 0),
            origin: (0, 0),
            is_hole: false,
        }
    }
}

struct Scanner {
    img: Vec<i8>,
    w: i32,
    h: i32,
    pt: Pt,
    lnbd: Pt,
    nbd: i8,
    mode: Retr,
    tree: Vec<Node>,
    ctable: [i32; 128],
}

impl Scanner {
    fn at(&self, x: i32, y: i32) -> i8 {
        self.img[(y * self.w + x) as usize]
    }
    fn delta(&self, s: i32) -> i32 {
        let d = DELTAS[(s % 8) as usize];
        d.0 + d.1 * self.w
    }
    fn simple(&self) -> bool {
        self.mode == Retr::External
    }

    fn add_child(&mut self, parent: i32, child: i32) {
        let fc = self.tree[parent as usize].first_child;
        if fc != -1 {
            self.tree[fc as usize].prev = child;
            self.tree[child as usize].next = fc;
        }
        self.tree[parent as usize].first_child = child;
        self.tree[child as usize].parent = parent;
        self.tree[child as usize].prev = -1;
    }

    fn trace(&self, start: Pt, end: Pt, is_hole: bool) -> bool {
        let stop = end.1 * self.w + end.0;
        let i0 = start.1 * self.w + start.0;
        let s_end: i32 = if is_hole { 0 } else { 4 };
        let mut s = s_end;
        let mut i1;
        loop {
            s = (s - 1) & 7;
            i1 = i0 + self.delta(s);
            if self.img[i1 as usize] != 0 || s == s_end {
                break;
            }
        }
        let mut i3 = i0;
        if s != s_end {
            loop {
                s = s.min(15);
                let mut i4 = i3;
                while s < 15 {
                    s += 1;
                    i4 = i3 + self.delta(s);
                    if self.img[i4 as usize] != 0 {
                        break;
                    }
                }
                if i3 == stop {
                    if self.img[i3 as usize] & RIGHT == 0 {
                        return true;
                    }
                    let mut t = s;
                    loop {
                        t = (t - 1) & 7;
                        let i5 = i3 + self.delta(t);
                        if self.img[i5 as usize] != 0 {
                            break;
                        }
                        if t == 0 {
                            return true;
                        }
                    }
                }
                if i4 == i0 && i3 == i1 {
                    break;
                }
                i3 = i4;
                s = (s + 4) & 7;
            }
        } else {
            return i3 == stop;
        }
        false
    }

    /// `icvFetchContourEx<schar>` with CHAIN_APPROX_SIMPLE.
    fn fetch(&mut self, start: Pt, nbd: i8, node: usize) {
        let i0 = start.1 * self.w + start.0;
        let mut pt = self.tree[node].origin;
        let is_hole = self.tree[node].is_hole;
        let mut rect = (pt.0, pt.1, pt.0, pt.1);
        let mut s_end: i32 = if is_hole { 0 } else { 4 };
        let mut s = s_end;
        let mut i1;
        loop {
            s = (s - 1) & 7;
            i1 = i0 + self.delta(s);
            if self.img[i1 as usize] != 0 || s == s_end {
                break;
            }
        }
        let mut pts = Vec::new();
        if s == s_end {
            self.img[i0 as usize] = nbd | RIGHT;
            pts.push(pt);
        } else {
            let mut i3 = i0;
            let mut prev_s = s ^ 4;
            loop {
                s_end = s;
                s = s.min(15);
                let mut i4 = i3;
                while s < 15 {
                    s += 1;
                    i4 = i3 + self.delta(s);
                    if self.img[i4 as usize] != 0 {
                        break;
                    }
                }
                s &= 7;
                if ((s - 1) as u32) < (s_end as u32) {
                    self.img[i3 as usize] = nbd | RIGHT;
                } else if self.img[i3 as usize] == BLACK {
                    self.img[i3 as usize] = nbd;
                }
                if s != prev_s {
                    pts.push(pt);
                    if pt.0 < rect.0 {
                        rect.0 = pt.0;
                    } else if pt.0 > rect.2 {
                        rect.2 = pt.0;
                    }
                    if pt.1 < rect.1 {
                        rect.1 = pt.1;
                    } else if pt.1 > rect.3 {
                        rect.3 = pt.1;
                    }
                }
                prev_s = s;
                pt.0 += DELTAS[s as usize].0;
                pt.1 += DELTAS[s as usize].1;
                if i4 == i0 && i3 == i1 {
                    break;
                }
                i3 = i4;
                s = (s + 4) & 7;
            }
        }
        rect.2 -= rect.0 - 1;
        rect.3 -= rect.1 - 1;
        let n = &mut self.tree[node];
        n.pts = pts;
        n.brect = rect;
    }

    fn make_contour(&mut self, nbd: &mut i8, is_hole: bool, x: i32, y: i32) -> usize {
        let start = (x - is_hole as i32, y);
        let idx = self.tree.len();
        let mut node = Node::new();
        node.is_hole = is_hole;
        // offset (-1, -1) for the padding
        node.origin = (start.0 - 1, start.1 - 1);
        self.tree.push(node);
        if self.simple() {
            self.fetch(start, NEW, idx);
        } else {
            let lval = *nbd;
            *nbd = (nbd.wrapping_add(1)) & LVAL;
            if *nbd == 0 {
                *nbd = BLACK | NEW;
            }
            self.fetch(start, lval, idx);
            let n = &mut self.tree[idx];
            n.brect.0 += 1;
            n.brect.1 += 1;
            n.ctable_next = self.ctable[lval as usize];
            self.ctable[lval as usize] = idx as i32;
        }
        self.tree[idx].origin = start;
        idx
    }

    fn first_bounding(&self, last_pos: Pt, y: i32, lval: i8) -> i32 {
        let end = (last_pos.0, y);
        let mut res = -1;
        let mut cur = self.ctable[lval as usize];
        while cur != -1 {
            let e = &self.tree[cur as usize];
            if last_pos.0 - e.brect.0 < e.brect.2 && last_pos.1 - e.brect.1 < e.brect.3 {
                if res != -1 {
                    let r = &self.tree[res as usize];
                    if self.trace(r.origin, end, r.is_hole) {
                        break;
                    }
                }
                res = cur;
            }
            cur = e.ctable_next;
        }
        res
    }

    fn scan(&mut self, prev: i8, p: i8, last_pos: &mut Pt, x: i32, y: i32) -> bool {
        let mut is_hole = false;
        if !(prev == 0 && p == 1) {
            if p != 0 || prev < 1 {
                return false;
            }
            if prev & FLAGS != 0 {
                last_pos.0 = x - 1;
            }
            is_hole = true;
        }
        if self.mode == Retr::External && (is_hole || self.at(last_pos.0, last_pos.1) > 0) {
            return false;
        }
        let mut main_parent;
        if self.simple() || last_pos.0 <= 0 {
            main_parent = 0;
        } else {
            let lval = self.at(last_pos.0, last_pos.1) & LVAL;
            main_parent = self.first_bounding(*last_pos, y, lval);
            if main_parent < 0 {
                main_parent = 0;
            }
            let e = &self.tree[main_parent as usize];
            if e.is_hole == is_hole {
                main_parent = if e.parent != -1 { e.parent } else { 0 };
            }
        }
        last_pos.0 = x - is_hole as i32;
        let mut nbd = self.nbd;
        let c = self.make_contour(&mut nbd, is_hole, x, y);
        if self.tree[c].parent == -1 {
            self.add_child(main_parent, c as i32);
        }
        self.pt = (x + 1, y);
        self.nbd = nbd;
        true
    }

    fn find_next(&mut self) -> bool {
        let (mut x, mut y) = self.pt;
        let width = self.w - 1;
        let height = self.h - 1;
        let mut last_pos = self.lnbd;
        let mut prev = self.at(x - 1, y);
        while y < height {
            let mut p: i8 = 0;
            while x < width {
                while x < width {
                    p = self.at(x, y);
                    if p != prev {
                        break;
                    }
                    x += 1;
                }
                if x >= width {
                    break;
                }
                if self.scan(prev, p, &mut last_pos, x, y) {
                    self.lnbd = last_pos;
                    return true;
                }
                prev = p;
                if prev & FLAGS != 0 {
                    last_pos.0 = x;
                }
                x += 1;
            }
            let _ = p;
            last_pos = (0, y + 1);
            x = 1;
            prev = 0;
            y += 1;
        }
        false
    }
}

/// `cv2.findContours(img, mode, cv2.CHAIN_APPROX_SIMPLE)[0]`: every nonzero
/// pixel is foreground.
pub fn find_contours(img: &Gray, mode: Retr) -> Vec<Vec<Pt>> {
    let w = img.width as i32 + 2;
    let h = img.height as i32 + 2;
    let mut data = vec![0i8; (w * h) as usize];
    for y in 0..img.height {
        let row = &img.data[y * img.width..(y + 1) * img.width];
        let out = &mut data[(y + 1) * w as usize + 1..];
        for (o, &v) in out.iter_mut().zip(row) {
            *o = (v > 0) as i8;
        }
    }
    let mut root = Node::new();
    root.is_hole = true;
    root.brect = (0, 0, w, h);
    let mut sc = Scanner {
        img: data,
        w,
        h,
        pt: (1, 1),
        lnbd: (0, 1),
        nbd: 2,
        mode,
        tree: vec![root],
        ctable: [-1; 128],
    };
    if h >= 3 && w >= 3 {
        while sc.find_next() {}
    }
    // Pre-order walk, first child first (contourTreeToResults).
    let mut out = Vec::with_capacity(sc.tree.len().saturating_sub(1));
    let mut stack = vec![0i32];
    while let Some(idx) = stack.pop() {
        let mut cur = sc.tree[idx as usize].first_child;
        if cur != -1 {
            while sc.tree[cur as usize].next != -1 {
                cur = sc.tree[cur as usize].next;
            }
        }
        while cur != -1 {
            stack.push(cur);
            cur = sc.tree[cur as usize].prev;
        }
        if idx != 0 {
            out.push(std::mem::take(&mut sc.tree[idx as usize].pts));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(rows: &[&str]) -> Gray {
        let mut g = Gray::new(rows[0].len(), rows.len());
        for (y, r) in rows.iter().enumerate() {
            for (x, c) in r.bytes().enumerate() {
                g.set(x, y, if c == b'#' { 255 } else { 0 });
            }
        }
        g
    }

    #[test]
    fn rectangle_matches_opencv() {
        // cv2 on a 3x4 block of ones padded by one: [[1,1],[1,3],[4,3],[4,1]]
        let g = img(&["......", ".####.", ".####.", ".####.", "......"]);
        let c = find_contours(&g, Retr::Tree);
        assert_eq!(c, vec![vec![(1, 1), (1, 3), (4, 3), (4, 1)]]);
    }

    #[test]
    fn ring_has_a_hole_and_external_skips_it() {
        let g = img(&["#####", "#...#", "#...#", "#####"]);
        let t = find_contours(&g, Retr::Tree);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], vec![(0, 0), (0, 3), (4, 3), (4, 0)]);
        assert_eq!(
            t[1],
            vec![
                (0, 1),
                (1, 0),
                (3, 0),
                (4, 1),
                (4, 2),
                (3, 3),
                (1, 3),
                (0, 2)
            ]
        );
        assert_eq!(find_contours(&g, Retr::External).len(), 1);
    }

    #[test]
    fn external_contours_come_bottom_first() {
        let g = img(&["#..", "...", "..#"]);
        let c = find_contours(&g, Retr::External);
        assert_eq!(c, vec![vec![(2, 2)], vec![(0, 0)]]);
    }
}
