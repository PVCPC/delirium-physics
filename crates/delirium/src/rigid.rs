use crate::Fma;
use crate::Vd;
use crate::Vx;
use crate::nmdbm::{atan2, sdot, slen, vnorm, M34};
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Link {
    pub r: [Vec3; 3],
    pub d: Vec3,
    pub m: f32,
    pub v: Vec3,
    pub w: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JointConst {
    pub pm: [Vec3; 3],
    pub pa: Vec3,
    pub ca: Vec3,
    pub cm: [Vec3; 3],
}

pub(crate) fn rows_mul(l: &[Vec3; 3], j: &[Vec3; 3]) -> [Vec3; 3] {
    l.map(|r| row_mul(r, j))
}

pub(crate) fn row_mul(r: Vec3, j: &[Vec3; 3]) -> Vec3 {
    j[2].mul_add(Vec3::splat(r.z), j[1].mul_add(Vec3::splat(r.y), j[0] * r.x))
}

pub(crate) fn ldot(l: &[Vec3; 3], v: Vec3) -> Vec3 {
    Vec3::new(l[0].vd(v), l[1].vd(v), l[2].vd(v))
}

pub(crate) fn col(m: &[Vec3; 3], k: usize) -> Vec3 {
    Vec3::new(m[0][k], m[1][k], m[2][k])
}

fn perp(v: Vec3) -> Vec3 {
    let small = !(v.x > 0.5 || v.x < -0.5 || v.y > 0.5 || v.y < -0.5);
    vnorm(if small { Vec3::new(0.0, v.z, -v.y) } else { Vec3::new(v.y, -v.x, 0.0) })
}

pub(crate) fn qrot(q: Quat, v: Vec3, inv: bool) -> Vec3 {
    let u = Vec3::new(q.x, q.y, q.z);
    let d = v.z.fma(q.z, v.x.fma(q.x, q.y * v.y)) * 2.0;
    let out = v.mul_add(Vec3::splat((q.w * q.w).fma(2.0, -1.0)), u * d);
    let c = Vec3::new(q.y.fma(v.z, -(q.z * v.y)), q.z.fma(v.x, -(v.z * q.x)), v.y.fma(q.x, -(q.y * v.x)));
    if inv { -c.mul_add(Vec3::splat(q.w * 2.0), -out) } else { c.mul_add(Vec3::splat(q.w * 2.0), out) }
}

pub(crate) fn q_axis(axis: Vec3, angle: f32) -> Quat {
    let h = (angle * 0.5) as f64;
    let s = h.sin() as f32;
    Quat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, h.cos() as f32)
}

pub(crate) fn q_scaled(v: Vec3) -> Quat {
    let n2 = v.x.fma(v.x, v.z.fma(v.z, v.y * v.y));
    if n2 <= 0.0 {
        return Quat::IDENTITY;
    }
    let len = n2.sqrt();
    q_axis(v * recip(len), len)
}

fn recip(len: f32) -> f32 {
    let y = crate::estimate::vrefp(len);
    (-y).mul_add(len, 1.0).mul_add(y, y)
}

fn turn_rows(c: f32, s: f32, a: Vec3) -> [Vec3; 3] {
    let t = 1.0 - c;
    let (xt, zyt) = (a.x * t, a.z * a.y * t);
    [Vec3::new((a.x * a.x).fma(t, c), a.y * xt - a.z * s, a.y * s + a.z * xt),
     Vec3::new(a.z * s + a.y * xt, (a.y * a.y).fma(t, c), zyt - a.x * s),
     Vec3::new(a.z * xt - a.y * s, a.x * s + zyt, (a.z * a.z).fma(t, c))]
}

pub(crate) fn integrate(r: &mut [Vec3; 3], d: &mut Vec3, v: Vec3, w: Vec3, h: f32) {
    *d = v.mul_add(Vec3::splat(h), *d);
    let w2 = w.x.fma(w.x, w.z.fma(w.z, w.y * w.y));
    if w2 > 1e-6 {
        let len = w2.sqrt();
        let a = (len * h) as f64;
        *r = rows_mul(&turn_rows(delirium_data::mathrt::cos(a) as f32, delirium_data::mathrt::sin(a) as f32, w * recip(len)), r);
    }
}

fn rsqrt2(x: f32, alt: f32) -> f32 {
    if !(x > 0.0) {
        return alt;
    }
    let x = x as f64;
    let mut y = crate::estimate::frsqrte(x);
    for _ in 0..2 {
        y = (-(y * y).mul_add(x, -1.0)).mul_add(y * 0.5, y);
    }
    y as f32
}

pub(crate) fn arc(a: Vec3, b: Vec3) -> [Vec3; 3] {
    let c = a.vx(b);
    let n2 = c.y.fma(c.y, c.z.fma(c.z, c.x * c.x));
    if !(n2 >= 1e-12) {
        return [Vec3::X, Vec3::Y, Vec3::Z];
    }
    let sl = n2.sqrt();
    let r = rsqrt2(b.z.fma(b.z, b.x.fma(b.x, b.y * b.y)) * a.z.fma(a.z, a.x.fma(a.x, a.y * a.y)), 1e8);
    turn_rows(b.y.fma(a.y, a.x.fma(b.x, b.z * a.z)) * r, r * sl, c * recip(sl))
}

pub(crate) fn join(j: &JointConst, p: &Link, c: &mut Link, hinge: bool) {
    if hinge {
        let ap = ldot(&p.r, col(&j.pm, 2));
        c.r = rows_mul(&arc(ldot(&c.r, col(&j.cm, 2)), ap), &c.r);
        let along = |w: Vec3| w.x.fma(ap.x, w.z.fma(ap.z, w.y * ap.y));
        c.w = ap * (along(c.w) - along(p.w)) + p.w;
    }
    let at = ldot(&c.r, j.ca);
    let pivot = ldot(&p.r, j.pa) + p.d;
    c.d = pivot - at;
    c.v = (p.w.vx(pivot - p.d) + p.v) - c.w.vx(at);
}

pub(crate) fn resquare(r: [Vec3; 3]) -> [Vec3; 3] {
    if r.contains(&Vec3::ZERO) {
        return [Vec3::X, Vec3::Y, Vec3::Z];
    }
    let b = nrm(r[1]);
    let c = nrm(r[0].vx(b));
    [b.vx(c), b, c]
}

pub fn frames(j: &JointConst, p: &Link, c: &Link, inst: Vec3) -> (M34, M34) {
    let side = |l: &Link, m: &[Vec3; 3], at: Vec3| {
        let r = rows_mul(&l.r, m);
        M34 { a: col(&r, 0), b: col(&r, 1), c: col(&r, 2), d: (ldot(&l.r, at) + inst) + l.d }
    };
    (side(p, &j.pm, j.pa), side(c, &j.cm, j.ca))
}

pub fn pos(j: &JointConst, c: &Link, inst: Vec3) -> Vec3 {
    (ldot(&c.r, j.ca) + c.d) + inst
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Swing {
    pub pr: [Vec3; 3],
    pub s: Vec3,
    pub theta: f32,
    pub bis: Vec3,
    pub across: Vec3,
    pub k: f32,
    pub twist: f32,
}

pub(crate) fn swing3(j: &JointConst, p: &Link, c: &Link) -> Swing {
    let pr = rows_mul(&p.r, &j.pm);
    let t = ldot(&c.r, col(&j.cm, 2));
    let pz = col(&pr, 2);
    let s = pz.vx(t);
    let n2 = sdot(s, s, b"yxz");
    let u = ldot(&c.r, col(&j.cm, 0));
    if n2 < 1e-12 {
        let s = perp(pz);
        let twist = atan2(sdot(u, col(&pr, 1), b"yzx"), sdot(u, col(&pr, 0), b"yzx"));
        return Swing { pr, s, theta: 0.0, bis: pz, across: s.vx(pz), k: 1.0, twist };
    }
    let len = n2.sqrt();
    let s = s * recip(len);
    let theta = atan2(len, sdot(pz, t, b"zxy"));
    let b = pz + t;
    let h = slen(b, b"yzx");
    if h.abs() < 1e-12 {
        return Swing { pr, s, theta, bis: pz, across: s.vx(pz), k: 1.0, twist: 0.0 };
    }
    let k = 2.0 / h;
    let bis = b * (k * 0.5);
    let across = s.vx(bis);
    let (ub, uq) = (sdot(bis, u, b"zyx"), sdot(across, u, b"zyx"));
    let (cs, sn) = ((theta as f64).cos() as f32, (theta as f64).sin() as f32);
    let u = bis * (sn.fma(uq, cs * ub) - ub) + u;
    let u = across * (cs.fma(uq, -(sn * ub)) - uq) + u;
    let (x, y) = (u.vd(col(&pr, 0)), u.vd(col(&pr, 1)));
    Swing { pr, s, theta, bis, across, k, twist: if x == 0.0 && y == 0.0 { 0.0 } else { atan2(y, x) } }
}

fn swing(j: &JointConst, p: &Link, c: &Link, cos: impl Fn(Vec3, Vec3) -> f32) -> ([Vec3; 3], Vec3, Vec3, Vec3, Option<f32>) {
    let pr = rows_mul(&p.r, &j.pm);
    let t = ldot(&c.r, col(&j.cm, 2));
    let pz = col(&pr, 2);
    let s = pz.vx(t);
    let n2 = sdot(s, s, b"yxz");
    if n2 < 1e-12 {
        return (pr, pz, t, perp(pz), None);
    }
    let len = n2.sqrt();
    (pr, pz, t, s * recip(len), Some(atan2(len, cos(t, pz))))
}

pub fn leans(j: &JointConst, p: &Link, c: &Link) -> (f32, f32) {
    let (pr, _, _, s, th) = swing(j, p, c, |t, pz| sdot(t, pz, b"yzx"));
    let th = th.unwrap_or(0.0);
    (th * sdot(s, col(&pr, 1), b"yzx"), th * -sdot(s, col(&pr, 0), b"yzx"))
}

pub fn twist(j: &JointConst, p: &Link, c: &Link) -> f32 {
    swing3(j, p, c).twist
}

pub(crate) fn hinge_axis(j: &JointConst, c: &Link) -> Vec3 {
    ldot(&c.r, col(&j.cm, 2))
}

pub fn hinge(j: &JointConst, p: &Link, c: &Link) -> f32 {
    let pu = ldot(&p.r, col(&j.pm, 0));
    let cv = ldot(&c.r, col(&j.cm, 0));
    let w = hinge_axis(j, c).vx(pu);
    let (x, y) = (sdot(pu, cv, b"yzx"), sdot(w, cv, b"yzx"));
    if x == 0.0 && y == 0.0 { 0.0 } else { atan2(y, x) }
}

pub fn root(m: [Vec3; 4], n: [Vec3; 4]) -> M34 {
    let f = |v: Vec3, acc: Vec3| n[2].mul_add(Vec3::splat(v.z), n[1].mul_add(Vec3::splat(v.y), acc));
    let t = |v: Vec3| f(v, n[0] * v.x);
    M34 { a: t(m[0]), b: t(m[1]), c: t(m[2]), d: f(m[3], n[0].mul_add(Vec3::splat(m[3].x), n[3])) }
}

pub(crate) fn nrm(v: Vec3) -> Vec3 {
    v * nrm_k(v)
}

pub(crate) fn nrm_or_y(v: Vec3) -> Vec3 {
    if v.vd(v) > 1e-6 { nrm(v) } else { Vec3::Y }
}

pub(crate) fn nrm_k(v: Vec3) -> f32 {
    let n2 = v.vd(v);
    let y = crate::estimate::vrsqrtefp(n2);
    let y = if y == f32::INFINITY { 0.0 } else { y };
    (-n2).mul_add(y * y, 1.0).mul_add(0.5 * y, y)
}

pub(crate) fn inv3([a, b, c]: [Vec3; 3]) -> [Vec3; 3] {
    let c00 = b.y.fma(c.z, -(b.z * c.y));
    let c01 = b.x.fma(c.z, -(c.x * b.z));
    let c02 = b.x.fma(c.y, -(c.x * b.y));
    let det = a.z.fma(c02, a.x.fma(c00, -(a.y * c01)));
    if singular([a, b, c], det) {
        return [a, b, c];
    }
    let k = 1.0 / det;
    let (f12, f11, f13) = (a.y.fma(c.z, -(c.y * a.z)), c.y.fma(a.x, -(a.y * c.x)), c.z.fma(a.x, -(a.z * c.x)));
    let (g11, g13, g12) = (b.z.fma(a.x, -(b.x * a.z)), b.z.fma(a.y, -(b.y * a.z)), b.y.fma(a.x, -(b.x * a.y)));
    [Vec3::new(k * c00, -(f12 * k), g13 * k), Vec3::new(-(k * c01), k * f13, -(k * g11)), Vec3::new(k * c02, -(k * f11), k * g12)]
}

fn singular([a, b, c]: [Vec3; 3], det: f32) -> bool {
    det.abs() <= a.abs().max(b.abs()).max(c.abs()).max_element() * 3.6e-7
}

pub(crate) fn inv34([a, b, c, d]: [Vec3; 4]) -> Option<[Vec3; 4]> {
    let det = a.z.fma(b.x.fma(c.y, -(c.x * b.y)), a.x.fma(b.y.fma(c.z, -(b.z * c.y)), -(a.y * b.x.fma(c.z, -(c.x * b.z)))));
    if singular([a, b, c], det) {
        return None;
    }
    let [ia, ib, ic] = inv3([a, b, c]);
    let id = |k: usize| -(d.y.fma(ib[k], if k == 0 { ic[k].fma(d.z, ia[k] * d.x) } else { d.x.fma(ia[k], ic[k] * d.z) }));
    Some([ia, ib, ic, Vec3::new(id(0), id(1), id(2))])
}

pub(crate) fn to_frame(m: [Vec3; 4], p: Vec3) -> Vec3 {
    let [a, b, c, d] = inv34(m).unwrap_or(m);
    crate::vm4(a, b, c, d, p)
}

#[derive(Clone, Copy, Debug)]
pub struct PartSet {
    pub link: Link,
    pub comp: [Vec3; 4],
}

pub fn part_set(m: [Vec3; 4], origin: Vec3, inst: [Vec3; 4], first: bool) -> PartSet {
    let mut r = [0, 1, 2].map(|k| col(&[m[0], m[1], m[2]], k));
    if first {
        let a = std::f32::consts::FRAC_PI_2 as f64;
        let (c, s) = (a.cos() as f32, a.sin() as f32);
        r = rows_mul(&r, &[Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z]);
    }
    let inv = inv34(inst).unwrap_or(inst);
    PartSet { link: Link { r, d: m[3] - origin, ..Default::default() }, comp: { let c = root(m, inv); [c.a, c.b, c.c, c.d] } }
}

#[derive(Clone, Debug, Default)]
pub struct PartSetOther {
    pub obj: Vec<(u32, [Vec3; 4])>,
    pub inst: Option<[Vec3; 4]>,
    pub part: Option<[Vec3; 4]>,
}

pub fn part_set_other(kind: u32, m: [Vec3; 4], centroid: Option<Vec3>, commit: bool) -> PartSetOther {
    match kind {
        2 => PartSetOther { obj: vec![(16, m)], ..Default::default() },
        1 => {
            let mut i = m;
            if let Some(c) = centroid {
                let r = [m[0], m[1], m[2]];
                let t = Vec3::new(col(&r, 0).vd(c), col(&r, 1).vd(c), col(&r, 2).vd(c));
                i[3] = m[3] + (Vec3::ZERO - t);
            }
            PartSetOther { obj: vec![(208, m), (32, m), (96, m)], inst: Some(i), part: commit.then_some(m) }
        }
        _ => PartSetOther::default(),
    }
}

pub fn part_bound(part: u32, rd: impl Fn(u32) -> u32) -> u32 {
    let obj = rd(part + 260);
    if obj == 0 {
        return 0;
    }
    let bound = |inst: u32| rd(rd(inst + 4) + 12);
    match rd(part + 240) {
        1 => bound(rd(obj + 16)),
        2 => bound(obj),
        3 => {
            let comp = bound(rd(rd(rd(part + 264) + 100) + 16));
            rd(rd(comp + 128) + 4 * rd(part + 244))
        }
        _ => 0,
    }
}

pub fn grip_local(inst: [Vec3; 4], comp: Option<[Vec3; 4]>, p: Vec3) -> Vec3 {
    let q = ldot(&[inst[0], inst[1], inst[2]], p - inst[3]);
    comp.map_or(q, |c| ldot(&[c[0], c[1], c[2]], q - c[3]))
}

pub fn contact_pass(b165: u8, b164: u8, on: bool) -> u8 {
    if b164 == 0 && !on { b165 & 0x7F } else { b165 }
}

pub type GripSide = (u16, [Vec3; 4], Option<[Vec3; 4]>, Vec3);

pub fn grip_pair(me: GripSide, other: GripSide) -> [Vec3; 2] {
    let l = |s: GripSide| grip_local(s.1, s.2, s.3);
    if me.0 <= other.0 { [l(me), l(other)] } else { [l(other), l(me)] }
}

pub fn block(links: &[Link], refs: &[[Vec3; 3]], place: Vec3, swap: bool) -> (M34, Vec3, Vec3) {
    let (mut s, mut d, mut v, mut w, mut mass) = ([Vec3::ZERO; 3], Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 0.0);
    for (l, re) in links.iter().zip(refs) {
        let p = rows_mul(&l.r, &inv3(*re));
        let m = Vec3::splat(l.m);
        for k in 0..3 { s[k] = p[k].mul_add(m, s[k]); }
        (w, d, v) = (l.w.mul_add(m, w), l.d.mul_add(m, d), l.v.mul_add(m, v));
        mass = l.m + mass;
    }
    let r = s.map(nrm);
    let col = |k: usize| Vec3::new(r[0][k], r[1][k], r[2][k]);
    let (b, c) = if swap { (col(2), Vec3::ZERO - col(1)) } else { (col(1), col(2)) };
    let c = nrm(c);
    let a = nrm(b.vx(c));
    let k = 1.0 / mass;
    let com = d.mul_add(Vec3::splat(k), place);
    (M34 { a, b: c.vx(a), c, d: com }, v * k, w * k)
}

