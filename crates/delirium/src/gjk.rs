use crate::prelude::*;

const EPS: f32 = f32::EPSILON;
const REL2: f32 = 1e-6;

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box(Vec3, f32),
    Capsule(f32, f32),
    Tri([Vec3; 3], f32),
    Hull(&'static [Vec3], f32),
    Curved(&'static crate::curved::Curved, f32),
}

impl Shape {
    fn support(&self, d: Vec3) -> Vec3 {
        match *self {
            Shape::Box(h, _) => Vec3::select(d.cmpgt(Vec3::ZERO), h, -h),
            Shape::Capsule(half, _) => Vec3::new(0.0, if d.y < 0.0 { -half } else { half }, 0.0),
            Shape::Tri([v0, v1, v2], _) => {
                let (d0, d1, d2) = (d.dot(v0), d.dot(v1), d.dot(v2));
                let (best, bd) = if d0 - d1 > 0.0 { (v0, d0) } else { (v1, d1) };
                if bd - d2 > 0.0 { best } else { v2 }
            }
            Shape::Hull(v, _) => v.iter().fold((Vec3::ZERO, f32::NEG_INFINITY), |b, &p| { let x = d.dot(p); if x > b.1 { (p, x) } else { b } }).0,
            Shape::Curved(c, _) => c.support(d),
        }
    }
    fn margin(&self) -> f32 {
        match *self {
            Shape::Box(_, m) | Shape::Capsule(_, m) | Shape::Tri(_, m) | Shape::Hull(_, m) | Shape::Curved(_, m) => m,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Pen {
    Tri([Vec3; 4]),
    Sphere,
}

const SPHERE: [[f32; 3]; 42] = [
    [0.0, -0.0, -1.0], [0.723608017, -0.525725007, -0.447219014], [-0.27638799, -0.850648999, -0.447219014],
    [-0.894425988, -0.0, -0.447216004], [-0.27638799, 0.850648999, -0.447219998], [0.723608017, 0.525725007, -0.447219014],
    [0.27638799, -0.850648999, 0.447219998], [-0.723608017, -0.525725007, 0.447219014], [-0.723608017, 0.525725007, 0.447219014],
    [0.27638799, 0.850648999, 0.447219014], [0.894425988, 0.0, 0.447216004], [-0.0, 0.0, 1.0],
    [0.425323009, -0.309011012, -0.850654006], [-0.162456006, -0.499994993, -0.850654006], [0.262869, -0.809011996, -0.525738001],
    [0.425323009, 0.309011012, -0.850654006], [0.850647986, -0.0, -0.525735974], [-0.525730014, -0.0, -0.850651979],
    [-0.688189983, -0.49999699, -0.525735974], [-0.162456006, 0.499994993, -0.850654006], [-0.688189983, 0.49999699, -0.525735974],
    [0.262869, 0.809011996, -0.525738001], [0.951057971, 0.309013009, 0.0], [0.951057971, -0.309013009, 0.0],
    [0.587786019, -0.809017003, 0.0], [0.0, -1.0, 0.0], [-0.587786019, -0.809017003, 0.0],
    [-0.951057971, -0.309013009, -0.0], [-0.951057971, 0.309013009, -0.0], [-0.587786019, 0.809017003, -0.0],
    [-0.0, 1.0, -0.0], [0.587786019, 0.809017003, -0.0], [0.688189983, -0.49999699, 0.525735974],
    [-0.262869, -0.809011996, 0.525738001], [-0.850647986, 0.0, 0.525735974], [-0.262869, 0.809011996, 0.525738001],
    [0.688189983, 0.49999699, 0.525735974], [0.525730014, 0.0, 0.850651979], [0.162456006, -0.499994993, 0.850654006],
    [-0.425323009, -0.309011012, 0.850654006], [-0.425323009, 0.309011012, 0.850654006], [0.162456006, 0.499994993, 0.850654006],
];

#[derive(Clone, Copy, Debug)]
pub struct Closest {
    pub n: Vec3,
    pub a: Vec3,
    pub b: Vec3,
    pub dist: f32,
}

pub(crate) fn recip(x: f32) -> f32 {
    let r = crate::estimate::vrefp(x);
    r.mul_add((-r).mul_add(x, 1.0), r)
}

pub(crate) fn rsq(l2: f32) -> f32 {
    let y = crate::estimate::vrsqrtefp(l2);
    let y = if y == f32::INFINITY { 0.0 } else { y };
    (-l2).mul_add(y * y, 1.0).mul_add(0.5 * y, y)
}

fn tri(a: Vec3, b: Vec3, c: Vec3) -> [f32; 3] {
    let (ab, ac) = (b - a, c - a);
    let (d1, d2) = (ab.dot(-a), ac.dot(-a));
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let (d3, d4) = (ab.dot(-b), ac.dot(-b));
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 * recip(d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let (d5, d6) = (ab.dot(-c), ac.dot(-c));
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 * recip(d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        let w = (d4 - d3) * recip((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let den = recip(va + vb + vc);
    let (v, w) = (vb * den, vc * den);
    [1.0 - (v + w), v, w]
}

fn outside(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i32 {
    let n = (b - a).cross(c - a);
    let sd = n.dot(d - a);
    if sd * sd < 1e-6 {
        return -1;
    }
    (n.dot(-a) * sd < 0.0) as i32
}

#[derive(Default)]
struct Simplex {
    w: Vec<Vec3>,
    p: Vec<Vec3>,
    q: Vec<Vec3>,
    last: Option<Vec3>,
    age: Vec<u32>,
    v: Vec3,
    p1: Vec3,
    p2: Vec3,
}

impl Simplex {
    fn has(&self, w: Vec3) -> bool {
        let tol = self.w.iter().map(|x| x.length_squared()).fold(1.0f32, f32::max).sqrt() * 0.001;
        let near = |x: Vec3| (x - w).abs().cmple(Vec3::splat(tol)).all();
        self.w.iter().any(|&x| near(x)) || self.last.is_some_and(near)
    }
    fn comb(&mut self, bary: &[f32]) {
        let (mut p1, mut p2) = (Vec3::ZERO, Vec3::ZERO);
        let mut order: Vec<usize> = (0..bary.len()).collect();
        order.sort_by_key(|&i| self.age[i]);
        for i in order {
            p1 += self.p[i] * bary[i];
            p2 += self.q[i] * bary[i];
        }
        (self.p1, self.p2, self.v) = (p1, p2, p1 - p2);
    }
    fn reduce(&mut self, used: &[bool]) {
        for i in (0..used.len()).rev() {
            if i < self.w.len() && !used[i] {
                self.w.swap_remove(i);
                self.p.swap_remove(i);
                self.q.swap_remove(i);
                self.age.swap_remove(i);
            }
        }
    }
    fn update(&mut self) -> bool {
        match self.w.len() {
            1 => {
                (self.p1, self.p2) = (self.p[0], self.q[0]);
                self.v = self.p1 - self.p2;
                true
            }
            2 => {
                let v = self.w[1] - self.w[0];
                let mut t = (-self.w[0]).dot(v);
                let mut used = [true, false];
                if t > 0.0 {
                    let vv = v.dot(v);
                    if t < vv {
                        t *= recip(vv);
                        used = [true, true];
                    } else {
                        t = 1.0;
                        used = [false, true];
                    }
                } else {
                    t = 0.0;
                }
                self.p1 = self.p[0] + (self.p[1] - self.p[0]) * t;
                self.p2 = self.q[0] + (self.q[1] - self.q[0]) * t;
                self.v = self.p1 - self.p2;
                self.reduce(&used);
                true
            }
            3 => {
                let b = tri(self.w[0], self.w[1], self.w[2]);
                self.comb(&b);
                self.reduce(&b.map(|x| x > 0.0));
                b.iter().all(|&x| x >= 0.0)
            }
            4 => {
                let [a, b, c, d] = [self.w[0], self.w[1], self.w[2], self.w[3]];
                let o = [outside(a, b, c, d), outside(a, c, d, b), outside(a, d, b, c), outside(b, d, c, a)];
                if o.contains(&-1) {
                    return false;
                }
                if o == [0; 4] {
                    self.v = Vec3::ZERO;
                    return true;
                }
                let mut best = (f32::MAX, [0.0; 4]);
                for (k, idx) in [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]].into_iter().enumerate() {
                    if o[k] == 0 {
                        continue;
                    }
                    let tb = tri(self.w[idx[0]], self.w[idx[1]], self.w[idx[2]]);
                    let q = idx.iter().zip(tb).fold(Vec3::ZERO, |s, (&i, x)| s + self.w[i] * x);
                    if q.dot(q) < best.0 {
                        let mut bary = [0.0; 4];
                        idx.iter().zip(tb).for_each(|(&i, x)| bary[i] = x);
                        best = (q.dot(q), bary);
                    }
                }
                self.comb(&best.1);
                self.reduce(&best.1.map(|x| x > 0.0));
                best.1.iter().all(|&x| x >= 0.0)
            }
            _ => false,
        }
    }
}

fn local(m: &Mat3, d: Vec3) -> Vec3 {
    Vec3::new(m.x_axis.dot(d), m.y_axis.dot(d), m.z_axis.dot(d))
}
fn world(m: &Mat3, p: Vec3, l: Vec3) -> Vec3 {
    (p + m.z_axis * l.z) + (m.y_axis * l.y + m.x_axis * l.x)
}

pub fn closest(a: &Shape, ra: Mat3, pa: Vec3, b: &Shape, rb: Mat3, pb: Vec3, axis: Vec3, reach: f32, pen: Option<Pen>) -> Option<Closest> {
    let (mut v, mut s, mut d2) = (axis, Simplex::default(), f32::MAX);
    let (mut check, mut degen) = (false, false);
    for it in 0.. {
        let p = world(&ra, pa, a.support(local(&ra, -v)));
        let q = world(&rb, pb, b.support(local(&rb, v)));
        let w = p - q;
        let delta = v.dot(w);
        if delta > 0.0 && delta * delta > d2 * reach * reach {
            return None;
        }
        if s.has(w) {
            (check, degen) = (true, true);
            break;
        }
        let f0 = d2 - delta;
        if f0 <= d2 * REL2 {
            (check, degen) = (true, degen || f0 <= 0.0);
            break;
        }
        s.last = Some(w);
        s.w.push(w);
        s.p.push(p);
        s.q.push(q);
        s.age.push(it as u32);
        if !s.update() {
            (check, degen) = (true, true);
            break;
        }
        let old = d2;
        v = s.v;
        d2 = v.dot(v);
        if old - d2 <= EPS * old {
            check = true;
            break;
        }
        if it > 1000 || s.w.len() == 4 || d2 <= EPS * s.w.iter().map(|x| x.length_squared()).fold(0.0, f32::max) {
            break;
        }
    }
    let (ma, mb, l2) = (a.margin(), b.margin(), v.dot(v));
    degen |= check && l2 < 1e-4;
    let found = (check && l2 > EPS).then(|| {
        let r = rsq(l2);
        Closest { n: (s.p1 - s.p2) * r, a: s.p1 - v * (ma * r), b: s.p2 + v * (mb * r), dist: recip(r) - ma - mb }
    });
    let Some(dirs) = pen.filter(|_| found.is_none_or(|f| degen && f.dist + ma + mb < 0.01)) else { return found };
    match (found, penetration(a, ra, pa, b, rb, pb, dirs)) {
        (Some(f), Some(p)) if !(p.dist < f.dist) => Some(f),
        (f, p) => p.or(f),
    }
}

fn penetration(a: &Shape, ra: Mat3, pa: Vec3, b: &Shape, rb: Mat3, pb: Vec3, pen: Pen) -> Option<Closest> {
    let dirs: Vec<Vec3> = match pen {
        Pen::Tri([f, e0, e1, e2]) => [f, e0, e1, e2, (e1 + e0).normalize(), (e2 + e0).normalize(), (e2 + e1).normalize()].map(|d| rb * d).to_vec(),
        Pen::Sphere => SPHERE.map(Vec3::from).to_vec(),
    };
    let mut best = (f32::MAX, Vec3::ZERO);
    for d in dirs {
        let p = world(&ra, pa, a.support(local(&ra, -d)));
        let q = world(&rb, pb, b.support(local(&rb, d)));
        let delta = (q - p).dot(d);
        if !(delta >= best.0) {
            best = (delta, d);
        }
    }
    let over = if best.0 >= 0.0 || matches!(pen, Pen::Tri(..)) { best.0 } else { 0.0 };
    let (proj, n) = (over + ((a.margin() + b.margin()) + 0.01), best.1);
    let c = closest(a, ra, pa + n * proj, b, rb, pb, Vec3::Z, f32::INFINITY, None)?;
    let pa = c.b - c.n * (proj * c.n.dot(n) - c.dist);
    let l2 = (c.b - pa).length_squared();
    let len = l2 * rsq(l2);
    (l2 > 1.42e-14).then(|| Closest { n: (c.b - pa) * recip(len), a: pa, b: c.b, dist: -len })
}

