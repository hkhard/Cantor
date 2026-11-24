//! `cv::Subdiv2D` (Delaunay triangulation by incremental insertion), a port
//! of OpenCV 4.14 `imgproc/src/subdivision2d.cpp` (Apache-2.0, Copyright the
//! OpenCV authors). homr's dewarping uses its triangles, and the triangle
//! list order and the choice between equally good (cocircular) splits
//! decide which pixels each affine piece writes, so this follows OpenCV
//! step by step, free lists included.
use crate::boxes::geom::Ptf;

const NEXT_AROUND_LEFT: i32 = 0x13;
const PREV_AROUND_ORG: i32 = 0x11;
const PREV_AROUND_DST: i32 = 0x33;

#[derive(Clone, Copy, Default)]
struct QuadEdge {
    next: [i32; 4],
    pt: [i32; 4],
}

impl QuadEdge {
    fn with(e: i32) -> QuadEdge {
        QuadEdge {
            next: [e, e + 3, e + 2, e + 1],
            pt: [0; 4],
        }
    }
}

#[derive(Clone, Copy)]
struct Vertex {
    first_edge: i32,
    pt: Ptf,
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex {
            first_edge: 0,
            pt: (0.0, 0.0),
        }
    }
}

pub struct Subdiv {
    qedges: Vec<QuadEdge>,
    vtx: Vec<Vertex>,
    free_qedge: i32,
    free_point: i32,
    recent_edge: i32,
    top_left: Ptf,
    bottom_right: Ptf,
}

fn tri_area(a: Ptf, b: Ptf, c: Ptf) -> f64 {
    (b.0 as f64 - a.0 as f64) * (c.1 as f64 - a.1 as f64)
        - (b.1 as f64 - a.1 as f64) * (c.0 as f64 - a.0 as f64)
}

fn in_circle(pt: Ptf, a: Ptf, b: Ptf, c: Ptf) -> i32 {
    let eps = f32::EPSILON as f64 * 0.125;
    let sq = |p: Ptf| p.0 as f64 * p.0 as f64 + p.1 as f64 * p.1 as f64;
    let mut v = sq(a) * tri_area(b, c, pt);
    v -= sq(b) * tri_area(a, c, pt);
    v += sq(c) * tri_area(a, b, pt);
    v -= sq(pt) * tri_area(a, b, c);
    if v > eps {
        1
    } else if v < -eps {
        -1
    } else {
        0
    }
}

impl Subdiv {
    /// `Subdiv2D(Rect(x, y, w, h))`.
    pub fn new(rect: (i32, i32, i32, i32)) -> Subdiv {
        let (x, y, w, h) = rect;
        let big = 6.0f32 * w.max(h) as f32;
        let (rx, ry) = (x as f32, y as f32);
        let mut s = Subdiv {
            qedges: vec![QuadEdge::default()],
            vtx: vec![Vertex::default()],
            free_qedge: 0,
            free_point: 0,
            recent_edge: 0,
            top_left: (rx, ry),
            bottom_right: (rx + w as f32, ry + h as f32),
        };
        let pa = s.new_point((rx + big, ry));
        let pb = s.new_point((rx, ry + big));
        let pc = s.new_point((rx - big, ry - big));
        let ab = s.new_edge();
        let bc = s.new_edge();
        let ca = s.new_edge();
        s.set_edge_points(ab, pa, pb);
        s.set_edge_points(bc, pb, pc);
        s.set_edge_points(ca, pc, pa);
        s.splice(ab, ca ^ 2);
        s.splice(bc, ab ^ 2);
        s.splice(ca, bc ^ 2);
        s.recent_edge = ab;
        s
    }

    fn q(&self, e: i32) -> &QuadEdge {
        &self.qedges[(e >> 2) as usize]
    }
    fn next_edge(&self, e: i32) -> i32 {
        self.q(e).next[(e & 3) as usize]
    }
    fn rotate(e: i32, r: i32) -> i32 {
        (e & !3) + ((e + r) & 3)
    }
    fn get_edge(&self, e: i32, t: i32) -> i32 {
        let e2 = self.q(e).next[((e + t) & 3) as usize];
        (e2 & !3) + ((e2 + (t >> 4)) & 3)
    }
    fn org(&self, e: i32) -> i32 {
        self.q(e).pt[(e & 3) as usize]
    }
    fn dst(&self, e: i32) -> i32 {
        self.q(e).pt[((e + 2) & 3) as usize]
    }
    fn p(&self, v: i32) -> Ptf {
        self.vtx[v as usize].pt
    }

    fn splice(&mut self, a: i32, b: i32) {
        let an = self.q(a).next[(a & 3) as usize];
        let bn = self.q(b).next[(b & 3) as usize];
        let ar = Self::rotate(an, 1);
        let br = Self::rotate(bn, 1);
        self.qedges[(a >> 2) as usize].next[(a & 3) as usize] = bn;
        self.qedges[(b >> 2) as usize].next[(b & 3) as usize] = an;
        let arn = self.q(ar).next[(ar & 3) as usize];
        let brn = self.q(br).next[(br & 3) as usize];
        self.qedges[(ar >> 2) as usize].next[(ar & 3) as usize] = brn;
        self.qedges[(br >> 2) as usize].next[(br & 3) as usize] = arn;
    }

    fn set_edge_points(&mut self, e: i32, o: i32, d: i32) {
        let q = &mut self.qedges[(e >> 2) as usize];
        q.pt[(e & 3) as usize] = o;
        q.pt[((e + 2) & 3) as usize] = d;
        self.vtx[o as usize].first_edge = e;
        self.vtx[d as usize].first_edge = e ^ 2;
    }

    fn connect(&mut self, a: i32, b: i32) -> i32 {
        let e = self.new_edge();
        let al = self.get_edge(a, NEXT_AROUND_LEFT);
        self.splice(e, al);
        self.splice(e ^ 2, b);
        let (d, o) = (self.dst(a), self.org(b));
        self.set_edge_points(e, d, o);
        e
    }

    fn swap_edges(&mut self, e: i32) {
        let s = e ^ 2;
        let a = self.get_edge(e, PREV_AROUND_ORG);
        let b = self.get_edge(s, PREV_AROUND_ORG);
        self.splice(e, a);
        self.splice(s, b);
        let (da, db) = (self.dst(a), self.dst(b));
        self.set_edge_points(e, da, db);
        let al = self.get_edge(a, NEXT_AROUND_LEFT);
        self.splice(e, al);
        let bl = self.get_edge(b, NEXT_AROUND_LEFT);
        self.splice(s, bl);
    }

    fn right_of(&self, pt: Ptf, e: i32) -> i32 {
        let a = tri_area(pt, self.p(self.dst(e)), self.p(self.org(e)));
        (a > 0.0) as i32 - (a < 0.0) as i32
    }

    fn new_edge(&mut self) -> i32 {
        if self.free_qedge <= 0 {
            self.qedges.push(QuadEdge::default());
            self.free_qedge = self.qedges.len() as i32 - 1;
        }
        let e = self.free_qedge * 4;
        self.free_qedge = self.q(e).next[1];
        self.qedges[(e >> 2) as usize] = QuadEdge::with(e);
        e
    }

    fn delete_edge(&mut self, e: i32) {
        let p = self.get_edge(e, PREV_AROUND_ORG);
        self.splice(e, p);
        let s = e ^ 2;
        let p = self.get_edge(s, PREV_AROUND_ORG);
        self.splice(s, p);
        let i = (e >> 2) as usize;
        self.qedges[i].next[0] = 0;
        self.qedges[i].next[1] = self.free_qedge;
        self.free_qedge = i as i32;
    }

    fn new_point(&mut self, pt: Ptf) -> i32 {
        if self.free_point == 0 {
            self.vtx.push(Vertex::default());
            self.free_point = self.vtx.len() as i32 - 1;
        }
        let v = self.free_point;
        self.free_point = self.vtx[v as usize].first_edge;
        self.vtx[v as usize] = Vertex { first_edge: 0, pt };
        v
    }

    /// `locate`: (location, edge, vertex); location 0 inside, 1 vertex, 2 on edge, -2 error.
    fn locate(&mut self, pt: Ptf) -> (i32, i32, i32) {
        let max_edges = self.qedges.len() * 4;
        let mut edge = self.recent_edge;
        let mut location = -2;
        let mut roc = self.right_of(pt, edge);
        if roc > 0 {
            edge ^= 2;
            roc = -roc;
        }
        for _ in 0..max_edges {
            let onext = self.next_edge(edge);
            let dprev = self.get_edge(edge, PREV_AROUND_DST);
            let ro = self.right_of(pt, onext);
            let rd = self.right_of(pt, dprev);
            if rd > 0 {
                if ro > 0 || (ro == 0 && roc == 0) {
                    location = 0;
                    break;
                }
                roc = ro;
                edge = onext;
            } else if ro > 0 {
                if rd == 0 && roc == 0 {
                    location = 0;
                    break;
                }
                roc = rd;
                edge = dprev;
            } else if roc == 0 && self.right_of(self.p(self.dst(onext)), edge) >= 0 {
                edge ^= 2;
            } else {
                roc = ro;
                edge = onext;
            }
        }
        self.recent_edge = edge;
        let mut vertex = 0;
        if location == 0 {
            let (o, d) = (self.p(self.org(edge)), self.p(self.dst(edge)));
            let t1 = (pt.0 - o.0).abs() as f64 + (pt.1 - o.1).abs() as f64;
            let t2 = (pt.0 - d.0).abs() as f64 + (pt.1 - d.1).abs() as f64;
            let t3 = (o.0 - d.0).abs() as f64 + (o.1 - d.1).abs() as f64;
            let feps = f32::EPSILON as f64;
            if t1 < feps {
                location = 1;
                vertex = self.org(edge);
                edge = 0;
            } else if t2 < feps {
                location = 1;
                vertex = self.dst(edge);
                edge = 0;
            } else if (t1 < t3 || t2 < t3) && tri_area(pt, o, d).abs() < feps {
                location = 2;
            }
        }
        if location == -2 {
            edge = 0;
        }
        (location, edge, vertex)
    }

    /// `insert(pt)`: returns the vertex id.
    pub fn insert(&mut self, pt: Ptf) -> Option<i32> {
        let (loc, mut cur, v) = self.locate(pt);
        match loc {
            1 => return Some(v),
            2 => {
                let deleted = cur;
                cur = self.get_edge(cur, PREV_AROUND_ORG);
                self.recent_edge = cur;
                self.delete_edge(deleted);
            }
            0 => {}
            _ => return None,
        }
        let cp = self.new_point(pt);
        let mut base = self.new_edge();
        let first = self.org(cur);
        self.set_edge_points(base, first, cp);
        self.splice(base, cur);
        loop {
            base = self.connect(cur, base ^ 2);
            cur = self.get_edge(base, PREV_AROUND_ORG);
            if self.dst(cur) == first {
                break;
            }
        }
        cur = self.get_edge(base, PREV_AROUND_ORG);
        let max_edges = self.qedges.len() * 4;
        for _ in 0..max_edges {
            let temp = self.get_edge(cur, PREV_AROUND_ORG);
            let td = self.dst(temp);
            let co = self.org(cur);
            let cd = self.dst(cur);
            if self.right_of(self.p(td), cur) > 0
                && in_circle(self.p(co), self.p(td), self.p(cd), self.p(cp)) < 0
            {
                self.swap_edges(cur);
                cur = self.get_edge(cur, PREV_AROUND_ORG);
            } else if co == first {
                break;
            } else {
                let n = self.next_edge(cur);
                cur = self.get_edge(n, 0x20); // PREV_AROUND_LEFT
            }
        }
        Some(cp)
    }

    /// `getTriangleList()`.
    pub fn triangles(&self) -> Vec<[Ptf; 3]> {
        let total = self.qedges.len() * 4;
        let mut mask = vec![false; total];
        let (x, y) = self.top_left;
        let (w, h) = (self.bottom_right.0 - x, self.bottom_right.1 - y);
        let contains = |p: Ptf| x <= p.0 && p.0 < x + w && y <= p.1 && p.1 < y + h;
        let mut out = Vec::new();
        let mut i = 4;
        while i < total {
            if !mask[i] {
                let ea = i as i32;
                let a = self.p(self.org(ea));
                if contains(a) {
                    let eb = self.get_edge(ea, NEXT_AROUND_LEFT);
                    let b = self.p(self.org(eb));
                    if contains(b) {
                        let ec = self.get_edge(eb, NEXT_AROUND_LEFT);
                        let c = self.p(self.org(ec));
                        if contains(c) {
                            mask[ea as usize] = true;
                            mask[eb as usize] = true;
                            mask[ec as usize] = true;
                            out.push([a, b, c]);
                        }
                    }
                }
            }
            i += 2;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_gives_two_triangles() {
        // cv2.Subdiv2D((−10, −10, 31, 31)) with the corners of a 10x10 square
        let mut s = Subdiv::new((-10, -10, 31, 31));
        for p in [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0), (10.0, 10.0)] {
            s.insert(p).unwrap();
        }
        let t = s.triangles();
        assert_eq!(t.len(), 2);
        let area: f64 = t
            .iter()
            .map(|t| tri_area(t[0], t[1], t[2]).abs() / 2.0)
            .sum();
        assert_eq!(area, 100.0);
    }
}
