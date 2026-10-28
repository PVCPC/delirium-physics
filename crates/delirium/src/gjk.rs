use crate::prelude::*;
use crate::{Vd, Vx};

const EPS: f32 = f32::EPSILON;
#[allow(non_snake_case)]
#[inline]
fn REL2() -> f32 { delirium_data::kv!(47) }

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Box(Vec3, f32),
    Capsule(f32, f32),
    Tri([Vec3; 3], f32),
    Hull(&'static [Vec3], f32),
    Curved(&'static crate::curved::Curved, f32),
    Tapered(f32, [f32; 2], f32),
}

fn unit(d: Vec3) -> Vec3 {
    let n = (((0.0 + d.x as f64 * d.x as f64) + d.y as f64 * d.y as f64) + d.z as f64 * d.z as f64) as f32;
    let mut e = crate::estimate::vrsqrtefp(n);
    if e == f32::INFINITY {
        e = 0.0;
    }
    let t = (-(n as f64 * (e * e) as f64 - 1.0)) as f32;
    let y = (t as f64 * (0.5 * e) as f64 + e as f64) as f32;
    if n > delirium_data::kv!(96 f32) { Vec3::new(y * d.x, y * d.y, y * d.z) } else { Vec3::Y }
}

fn fma3(x: Vec3, y: f32, z: Vec3) -> Vec3 {
    Vec3::new(x.x.mul_add(y, z.x), x.y.mul_add(y, z.y), x.z.mul_add(y, z.z))
}

impl Shape {
    pub fn support(&self, d: Vec3) -> Vec3 {
        match *self {
            Shape::Box(h, _) => Vec3::select(d.cmpgt(Vec3::ZERO), h, -h),
            Shape::Capsule(half, _) => Vec3::new(0.0, if d.y < 0.0 { -half } else { half }, 0.0),
            Shape::Tri([v0, v1, v2], _) => {
                let (d0, d1, d2) = (d.vd(v0), d.vd(v1), d.vd(v2));
                let (best, bd) = if d0 - d1 > 0.0 { (v0, d0) } else { (v1, d1) };
                if bd - d2 > 0.0 { best } else { v2 }
            }
            Shape::Hull(v, _) => v.iter().fold((Vec3::ZERO, f32::NEG_INFINITY), |b, &p| { let x = d.vd(p); if x > b.1 { (p, x) } else { b } }).0,
            Shape::Curved(c, _) => c.support(d),
            Shape::Tapered(half, [top, bottom], _) => {
                let n = unit(d);
                let p1 = fma3(n, top, Vec3::new(0.0, half, 0.0));
                let p2 = fma3(n, bottom, Vec3::new(0.0, -half, 0.0));
                if d.vd(p2) > d.vd(p1) { p2 } else { p1 }
            }
        }
    }
    fn margin(&self) -> f32 {
        match *self {
            Shape::Box(_, m) | Shape::Capsule(_, m) | Shape::Tri(_, m) | Shape::Hull(_, m) | Shape::Curved(_, m) | Shape::Tapered(_, _, m) => m,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Pen {
    Tri([Vec3; 4]),
    Sphere,
}

#[allow(non_snake_case)]
#[inline]
fn SPHERE() -> &'static [[f32; 3]; 42] { static V: std::sync::OnceLock<[[f32; 3]; 42]> = std::sync::OnceLock::new(); let f = || [
    [0.0, -0.0, -1.0], [delirium_data::kv!(144), -delirium_data::kv!(145), -delirium_data::kv!(146)], [-delirium_data::kv!(147), -delirium_data::kv!(148), -delirium_data::kv!(146)],
    [-delirium_data::kv!(149), -0.0, -delirium_data::kv!(150)], [-delirium_data::kv!(147), delirium_data::kv!(148), -delirium_data::kv!(151)], [delirium_data::kv!(144), delirium_data::kv!(145), -delirium_data::kv!(146)],
    [delirium_data::kv!(147), -delirium_data::kv!(148), delirium_data::kv!(151)], [-delirium_data::kv!(144), -delirium_data::kv!(145), delirium_data::kv!(146)], [-delirium_data::kv!(144), delirium_data::kv!(145), delirium_data::kv!(146)],
    [delirium_data::kv!(147), delirium_data::kv!(148), delirium_data::kv!(146)], [delirium_data::kv!(149), 0.0, delirium_data::kv!(150)], [-0.0, 0.0, 1.0],
    [delirium_data::kv!(152), -delirium_data::kv!(153), -delirium_data::kv!(154)], [-delirium_data::kv!(155), -delirium_data::kv!(156), -delirium_data::kv!(154)], [delirium_data::kv!(157), -delirium_data::kv!(158), -delirium_data::kv!(159)],
    [delirium_data::kv!(152), delirium_data::kv!(153), -delirium_data::kv!(154)], [delirium_data::kv!(160), -0.0, -delirium_data::kv!(161)], [-delirium_data::kv!(162), -0.0, -delirium_data::kv!(163)],
    [-delirium_data::kv!(164), -delirium_data::kv!(165), -delirium_data::kv!(161)], [-delirium_data::kv!(155), delirium_data::kv!(156), -delirium_data::kv!(154)], [-delirium_data::kv!(164), delirium_data::kv!(165), -delirium_data::kv!(161)],
    [delirium_data::kv!(157), delirium_data::kv!(158), -delirium_data::kv!(159)], [delirium_data::kv!(166), delirium_data::kv!(167), 0.0], [delirium_data::kv!(166), -delirium_data::kv!(167), 0.0],
    [delirium_data::kv!(168), -delirium_data::kv!(169), 0.0], [0.0, -1.0, 0.0], [-delirium_data::kv!(168), -delirium_data::kv!(169), 0.0],
    [-delirium_data::kv!(166), -delirium_data::kv!(167), -0.0], [-delirium_data::kv!(166), delirium_data::kv!(167), -0.0], [-delirium_data::kv!(168), delirium_data::kv!(169), -0.0],
    [-0.0, 1.0, -0.0], [delirium_data::kv!(168), delirium_data::kv!(169), -0.0], [delirium_data::kv!(164), -delirium_data::kv!(165), delirium_data::kv!(161)],
    [-delirium_data::kv!(157), -delirium_data::kv!(158), delirium_data::kv!(159)], [-delirium_data::kv!(160), 0.0, delirium_data::kv!(161)], [-delirium_data::kv!(157), delirium_data::kv!(158), delirium_data::kv!(159)],
    [delirium_data::kv!(164), delirium_data::kv!(165), delirium_data::kv!(161)], [delirium_data::kv!(162), 0.0, delirium_data::kv!(163)], [delirium_data::kv!(155), -delirium_data::kv!(156), delirium_data::kv!(154)],
    [-delirium_data::kv!(152), -delirium_data::kv!(153), delirium_data::kv!(154)], [-delirium_data::kv!(152), delirium_data::kv!(153), delirium_data::kv!(154)], [delirium_data::kv!(155), delirium_data::kv!(156), delirium_data::kv!(154)],
]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

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
    let (d1, d2) = (ab.vd(-a), ac.vd(-a));
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let (d3, d4) = (ab.vd(-b), ac.vd(-b));
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 * recip(d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let (d5, d6) = (ab.vd(-c), ac.vd(-c));
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

fn tri_point(a: Vec3, b: Vec3, c: Vec3, [u, v, w]: [f32; 3]) -> Vec3 {
    match (u, v, w) {
        (_, 0.0, 0.0) => a,
        (0.0, _, 0.0) => b,
        (0.0, 0.0, _) => c,
        (0.0, _, _) => fma3(c - b, w, b),
        _ => fma3(c - a, w, fma3(b - a, v, a)),
    }
}

fn outside(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i32 {
    let n = (b - a).vx(c - a);
    let sd = n.vd(d - a);
    if sd * sd < delirium_data::kv!(47) {
        return -1;
    }
    (n.vd(-a) * sd < 0.0) as i32
}

#[derive(Default)]
struct Simplex {
    w: Vec<Vec3>,
    p: Vec<Vec3>,
    q: Vec<Vec3>,
    last: Option<Vec3>,
    v: Vec3,
    p1: Vec3,
    p2: Vec3,
}

impl Simplex {
    fn has(&self, w: Vec3) -> bool {
        let tol = self.w.iter().map(|x| x.length_squared()).fold(1.0f32, f32::max).sqrt() * delirium_data::kv!(129);
        let near = |x: Vec3| (x - w).abs().cmple(Vec3::splat(tol)).all();
        self.w.iter().any(|&x| near(x)) || self.last.is_some_and(near)
    }
    fn comb(&mut self, bary: &[f32], order: &[usize]) {
        let (mut p1, mut p2) = (self.p[order[0]] * bary[order[0]], self.q[order[0]] * bary[order[0]]);
        for &i in &order[1..] {
            (p1, p2) = (fma3(self.p[i], bary[i], p1), fma3(self.q[i], bary[i], p2));
        }
        (self.p1, self.p2, self.v) = (p1, p2, p1 - p2);
    }
    fn reduce(&mut self, used: &[bool]) {
        for i in (0..used.len()).rev() {
            if i < self.w.len() && !used[i] {
                self.w.swap_remove(i);
                self.p.swap_remove(i);
                self.q.swap_remove(i);
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
                let mut t = (-self.w[0]).vd(v);
                let mut used = [true, false];
                if t > 0.0 {
                    let vv = v.vd(v);
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
                self.p1 = fma3(self.p[1] - self.p[0], t, self.p[0]);
                self.p2 = fma3(self.q[1] - self.q[0], t, self.q[0]);
                self.v = self.p1 - self.p2;
                self.reduce(&used);
                true
            }
            3 => {
                let b = tri(self.w[0], self.w[1], self.w[2]);
                self.comb(&b, &[0, 1, 2]);
                self.reduce(&b.map(|x| x > 0.0));
                b.iter().all(|&x| x >= 0.0)
            }
            4 => {
                let [a, b, c, d] = [self.w[0], self.w[1], self.w[2], self.w[delirium_data::kv!(0 int)]];
                let o = [outside(a, b, c, d), outside(a, c, d, b), outside(a, d, b, c), outside(b, d, c, a)];
                if o.contains(&-1) {
                    return false;
                }
                if o == [0; 4] {
                    self.v = Vec3::ZERO;
                    return true;
                }
                let mut best = (f32::MAX, [0.0; 4], [0; 3]);
                for (k, idx) in [[0, 1, 2], [0, 2, delirium_data::kv!(0 int)], [0, delirium_data::kv!(0 int), 1], [1, delirium_data::kv!(0 int), 2]].into_iter().enumerate() {
                    if o[k] == 0 {
                        continue;
                    }
                    let tb = tri(self.w[idx[0]], self.w[idx[1]], self.w[idx[2]]);
                    let q = tri_point(self.w[idx[0]], self.w[idx[1]], self.w[idx[2]], tb);
                    if q.vd(q) < best.0 {
                        let mut bary = [0.0; 4];
                        idx.iter().zip(tb).for_each(|(&i, x)| bary[i] = x);
                        let mut slots = idx;
                        slots.sort();
                        best = (q.vd(q), bary, slots);
                    }
                }
                self.comb(&best.1, &best.2);
                self.reduce(&best.1.map(|x| x > 0.0));
                best.1.iter().all(|&x| x >= 0.0)
            }
            _ => false,
        }
    }
}

fn local(m: &Mat3, d: Vec3) -> Vec3 {
    Vec3::new(m.x_axis.vd(d), m.y_axis.vd(d), m.z_axis.vd(d))
}
fn world(m: &Mat3, p: Vec3, l: Vec3) -> Vec3 {
    crate::vm4(m.x_axis, m.y_axis, m.z_axis, p, l)
}

pub fn closest(a: &Shape, ra: Mat3, pa: Vec3, b: &Shape, rb: Mat3, pb: Vec3, axis: Vec3, reach: f32, pen: Option<Pen>) -> Option<Closest> {
    let (mut v, mut s, mut d2) = (axis, Simplex::default(), f32::MAX);
    let (mut check, mut degen) = (false, false);
    for it in 0.. {
        let p = world(&ra, pa, a.support(local(&ra, -v)));
        let q = world(&rb, pb, b.support(local(&rb, v)));
        let w = p - q;
        let delta = v.vd(w);
        if delta > 0.0 && delta * delta > d2 * reach * reach {
            return None;
        }
        if s.has(w) {
            (check, degen) = (true, true);
            break;
        }
        let f0 = d2 - delta;
        if f0 <= d2 * REL2() {
            (check, degen) = (true, degen || f0 <= 0.0);
            break;
        }
        s.last = Some(w);
        s.w.push(w);
        s.p.push(p);
        s.q.push(q);
        if !s.update() {
            (check, degen) = (true, true);
            break;
        }
        let old = d2;
        v = s.v;
        d2 = v.vd(v);
        if old - d2 <= EPS * old {
            check = true;
            break;
        }
        if it > delirium_data::kv!(38 int) || s.w.len() == delirium_data::kv!(1 int) || d2 <= EPS * s.w.iter().map(|x| x.length_squared()).fold(0.0, f32::max) {
            break;
        }
    }
    let (ma, mb, l2) = (a.margin(), b.margin(), v.vd(v));
    degen |= check && l2 < delirium_data::kv!(68);
    let found = (check && l2 > EPS).then(|| {
        let r = rsq(l2);
        Closest { n: (s.p1 - s.p2) * r, a: (-v).mul_add(Vec3::splat(ma * r), s.p1), b: v.mul_add(Vec3::splat(mb * r), s.p2), dist: recip(r) - ma - mb }
    });
    let Some(dirs) = pen.filter(|_| found.is_none_or(|f| degen && f.dist + ma + mb < delirium_data::kv!(48))) else { return found };
    match (found, penetration(a, ra, pa, b, rb, pb, dirs)) {
        (Some(f), Some(p)) if !(p.dist < f.dist) => Some(f),
        (f, p) => p.or(f),
    }
}

fn penetration(a: &Shape, ra: Mat3, pa: Vec3, b: &Shape, rb: Mat3, pb: Vec3, pen: Pen) -> Option<Closest> {
    let dirs: Vec<Vec3> = match pen {
        Pen::Tri(d) => {
            let [f, e0, e1, e2] = d.map(|d| Vec3::new(rb.row(0).vd(d), rb.row(1).vd(d), rb.row(2).vd(d)));
            let u = |d: Vec3| {
                let x = d.vd(d);
                let y = crate::estimate::vrsqrtefp(x);
                let y = if y == f32::INFINITY { 0.0 } else { y };
                d * (-x).mul_add(y * y, 1.0).mul_add(0.5 * y, y)
            };
            vec![f, e0, e1, e2, u(e1 + e0), u(e2 + e0), u(e2 + e1)]
        }
        Pen::Sphere => (*SPHERE()).map(Vec3::from).to_vec(),
    };
    let mut best = (f32::MAX, Vec3::ZERO);
    for d in dirs {
        let p = world(&ra, pa, a.support(local(&ra, -d)));
        let q = world(&rb, pb, b.support(local(&rb, d)));
        let delta = (q - p).vd(d);
        if !(delta >= best.0) {
            best = (delta, d);
        }
    }
    let over = if best.0 >= 0.0 || matches!(pen, Pen::Tri(..)) { best.0 } else { 0.0 };
    let (proj, n) = (over + ((a.margin() + b.margin()) + delirium_data::kv!(48)), best.1);
    let c = closest(a, ra, n.mul_add(Vec3::splat(proj), pa), b, rb, pb, Vec3::Z, f32::INFINITY, None)?;
    let pa = c.b - c.n * (proj * c.n.vd(n) - c.dist);
    let l2 = (pa - c.b).vd(pa - c.b);
    let len = l2 * rsq(l2);
    (l2 > delirium_data::kv!(170)).then(|| Closest { n: (c.b - pa) * recip(len), a: pa, b: c.b, dist: -len })
}

pub fn tapered(top: f32, bottom: f32, length: f32) -> Shape {
    let m = top.min(bottom);
    Shape::Tapered(length * 0.5, [top - m, bottom - m], m)
}
