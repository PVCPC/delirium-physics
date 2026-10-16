use crate::Fma;
use crate::Vd;
use crate::Vx;
use crate::prelude::*;

use crate::collision::Collision;
use crate::nmrs::{Character, ANKLE, HIP, KNEE, SPINE};
use crate::ragdoll::{Build, Ragdoll};
use crate::rigid::qrot;

#[derive(Clone, Copy, Debug, Default)]
pub struct M34 {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
    pub d: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Part {
    pub m: M34,
    pub v: Vec3,
    pub w: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Eff3 {
    pub m: M34,
    pub pos: Vec3,
    pub damping: f32,
    pub strength: f32,
    pub half_twist: f32,
    pub half_lean1: f32,
    pub half_lean2: f32,
    pub mid_lean1: f32,
    pub mid_lean2: f32,
    pub mid_twist: f32,
    pub desired: [f32; 3],
    pub actual: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Eff1 {
    pub m: M34,
    pub child: M34,
    pub pos: Vec3,
    pub damping: f32,
    pub strength: f32,
    pub half: f32,
    pub mid: f32,
    pub desired: f32,
    pub actual: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Leg {
    pub hip: Eff3,
    pub thigh: Part,
    pub knee: Eff1,
    pub shin: Part,
    pub ankle: Eff3,
    pub foot: Part,
    pub limb: M34,
}

pub fn rage_link(nm: usize, k: usize) -> Quat {
    let q = link(nm, k).normalize();
    if k == 0 { q * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2) } else { q }
}

const HOMER_PARTS: usize = 21;

pub fn link(nm: usize, k: usize) -> Quat {
    logged_links().filter(|_| nm == crate::nmrs::HOMER).or(DERIVED_LINKS[nm].get().map(|d| d.as_slice()))
        .and_then(|t| t.get(k)).copied().unwrap_or(Quat::IDENTITY)
}

pub fn logged_links() -> Option<&'static [Quat]> {
    static LINKS: std::sync::OnceLock<Vec<Quat>> = std::sync::OnceLock::new();
    let t = LINKS.get_or_init(|| {
        std::env::var("NM_LINKS").ok().and_then(|p| std::fs::read_to_string(p).ok()).map_or(vec![], |t| {
            t.lines().filter_map(|l| {
                let v: Vec<f32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.len() == 4).then(|| Quat::from_xyzw(v[0], v[1], v[2], v[3]))
            }).collect()
        })
    });
    (!t.is_empty()).then_some(t.as_slice())
}

pub static DERIVED_LINKS: [std::sync::OnceLock<Vec<Quat>>; 2] = [const { std::sync::OnceLock::new() }; 2];

fn step_clamp(b: &Body, c: Config) -> f32 {
    if b.ch.mode == 2 { c.step_clamp * 0.7 } else { c.step_clamp }
}

#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub com: Vec3,
    pub vel: Vec3,
    pub angvel: Vec3,
    pub m: M34,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Snap {
    pub com: Vec3,
    pub vel: Vec3,
    pub hvel: Vec3,
    pub angvel: Vec3,
    pub m: M34,
    pub legs: [Leg; 2],
    pub spine: Eff3,
    pub pelvis: Part,
    pub flags: u8,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Params {
    pub up: Vec3,
    pub gup: Vec3,
    pub target: [Vec3; 2],
    pub normal: [Vec3; 2],
    pub gvel: [Vec3; 2],
    pub dt: f32,
    pub pelvis_height: f32,
    pub hip_dist: f32,
    pub leg_length: f32,
    pub foot_width: f32,
    pub p148: f32,
    pub p152: f32,
    pub step_offset: f32,
    pub abort: f32,
    pub lead: f32,
    pub p168: f32,
    pub p172: f32,
    pub lean_bias: f32,
    pub step_height: f32,
    pub leg_stiffness: f32,
    pub step_clamp: f32,
    pub taper: bool,
    pub low_abort: bool,
    pub contact: [bool; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Feet {
    pub pos: [Vec3; 2],
    pub centre: Vec3,
    pub start: Vec3,
    pub prev_target: Vec3,
    pub prev_sole: Vec3,
    pub height: [f32; 2],
    pub countdown: i32,
    pub dist: [f32; 2],
    pub stepping: u32,
    pub flags: u8,
}

impl Default for Feet {
    fn default() -> Feet {
        Feet { pos: [Vec3::ZERO; 2], centre: Vec3::ZERO, start: Vec3::ZERO, prev_target: Vec3::ZERO, prev_sole: Vec3::ZERO,
               height: [0.0; 2], countdown: 1000, dist: [0.0; 2], stepping: 0, flags: 0 }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Pelvis {
    pub off: Vec3,
    pub vel: Vec3,
    pub centre: Vec3,
    pub acc: f32,
    pub p52: f32,
    pub p56: f32,
    pub yaw: [f32; 2],
    pub p68: f32,
    pub out: [f32; 2],
    pub planted: [bool; 2],
    pub fixed_vel: bool,
}

fn horizontal(v: Vec3, up: Vec3) -> Vec3 {
    v - up * up.vd(v)
}

pub(crate) fn sdot(a: Vec3, b: Vec3, o: &[u8; 3]) -> f32 {
    let [p, q, r] = o.map(|c| (c - b'x') as usize);
    a[r].fma(b[r], a[q].fma(b[q], a[p] * b[p]))
}

pub(crate) fn slen(v: Vec3, o: &[u8; 3]) -> f32 {
    sdot(v, v, o).sqrt()
}

pub(crate) fn shz(v: Vec3, up: Vec3, o: &[u8; 3]) -> Vec3 {
    up.mul_add(Vec3::splat(-sdot(v, up, o)), v)
}

fn sat(v: Vec3, up: Vec3, h: f32, o: &[u8; 3]) -> Vec3 {
    up.mul_add(Vec3::splat(h - sdot(v, up, o)), v)
}

fn rotvec_quat(r: Vec3) -> Quat {
    let n2 = sdot(r, r, b"yzx");
    if n2 <= 0.0 {
        return Quat::IDENTITY;
    }
    let len = n2.sqrt();
    let y = crate::estimate::vrefp(len);
    let axis = r * (-y).mul_add(len, 1.0).mul_add(y, y);
    let h = len * 0.5;
    let (c, sn) = ((h as f64).cos() as f32, (h as f64).sin() as f32);
    Quat::from_xyzw(axis.x * sn, axis.y * sn, axis.z * sn, c)
}

fn quat_rows(q: Quat) -> [Vec3; 3] {
    let k = std::f32::consts::SQRT_2;
    let (x, y, z, w) = (q.x * k, q.y * k, q.z * k, q.w * k);
    let (xz, xy, wz, wy, yz, wx) = (z * x, y * x, w * z, w * y, z * y, w * x);
    let (zz, yy, xx) = (z * z, y * y, x * x);
    [Vec3::new(1.0 - (zz + yy), xy + wz, xz - wy), Vec3::new(xy - wz, 1.0 - (zz + xx), wx + yz), Vec3::new(wy + xz, yz - wx, 1.0 - (yy + xx))]
}

pub(crate) fn vnorm(v: Vec3) -> Vec3 {
    let n = v.vd(v);
    let y = crate::estimate::vrsqrtefp(n);
    let y = if y == f32::INFINITY { 0.0 } else { y };
    v * (-n).mul_add(y * y, 1.0).mul_add(0.5 * y, y)
}

pub(crate) fn atan2(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

pub(crate) fn atan2f(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (e, at) = (1e-12, |v: f32| (v as f64).atan() as f32);
    if x > e {
        if y.abs() > e { at(y / x) } else { 0.0 }
    } else if x < -e {
        if y > e { PI - at(-(y / x)) } else if y < -e { at(y / x) - PI } else { PI }
    } else if y > e {
        FRAC_PI_2
    } else if y < -e {
        -FRAC_PI_2
    } else {
        0.0
    }
}

pub(crate) fn acos(x: f32) -> f32 {
    if x <= -1.0 { std::f32::consts::PI } else if x >= 1.0 { 0.0 } else { (x as f64).acos() as f32 }
}

fn separate(a: &mut Vec3, b: &mut Vec3, min: f32) {
    let d = *b - *a;
    let n = slen(d, b"xyz");
    if n >= min {
        return;
    }
    let dir = vnorm(d);
    *a = dir.mul_add(Vec3::splat((n - min) * 0.5), *a);
    *b = dir.mul_add(Vec3::splat((min - n) * 0.5), *b);
}

fn clamp_length(v: Vec3, lo: f32, hi: f32) -> Vec3 {
    let n2 = sdot(v, v, b"yxz");
    let k = if n2 > hi * hi {
        hi
    } else if n2 < lo * lo {
        lo
    } else {
        return v;
    };
    if n2 <= 0.0 {
        return Vec3::ZERO;
    }
    let (n, mut y) = (n2 as f64, crate::estimate::frsqrte(n2 as f64));
    for _ in 0..2 {
        y = (-(y * y)).mul_add(n, 1.0).mul_add(y * 0.5, y);
    }
    v * (y as f32 * k)
}

fn support(pc: Vec3, l: &Part, r: &Part, w: f32, len: f32, up: Vec3) -> f32 {
    support_at(pc, &l.m, &r.m, w, len, up).0
}

fn support_at(pc: Vec3, l: &M34, r: &M34, w: f32, len: f32, up: Vec3) -> (f32, Vec3) {
    let (sw, sl) = (Vec3::splat(w), Vec3::splat(len));
    let c0 = l.c.mul_add(-sl, l.a.mul_add(-sw, l.d));
    let c1 = r.c.mul_add(-sl, r.a.mul_add(sw, r.d));
    let c2 = r.c.mul_add(sl, r.a.mul_add(sw, r.d));
    let c3 = l.c.mul_add(sl, l.a.mul_add(-sw, l.d));
    let n = |e: Vec3| vnorm(e.vx(up));
    let (n0, n1, n2, n3) = (n(c1 - c0), n(c2 - c1), n(c3 - c2), n(c0 - c3));
    let (d3, d1) = (pc - c3, pc - c1);
    let a = sdot(n2, d3, b"zyx");
    let b = sdot(n3, d3, b"zyx");
    let c = sdot(n1, d1, b"zyx");
    let e = sdot(n0, d1, b"zyx");
    let dist = a.min(b).min(c).min(e);
    let p = up.mul_add(Vec3::splat(sdot(up, (l.d + r.d) * 0.5 - pc, b"zyx")), pc);
    let edge = if b < 0.0 && b < e && b < a { Some((c3, c0)) }
        else if c < 0.0 && c < e && c < a { Some((c2, c1)) }
        else if e < 0.0 { Some((c0, c1)) }
        else if a < 0.0 { Some((c3, c2)) }
        else { None };
    match edge.filter(|_| dist < 0.0) {
        Some((u, v)) => {
            let t = (sdot(p - u, v - u, b"zyx") / sdot(v - u, v - u, b"zyx")).clamp(0.0, 1.0);
            (dist, (v - u).mul_add(Vec3::splat(t), u))
        }
        None => (dist, p),
    }
}

fn reach(t: Vec3, up: Vec3, pelvis: &M34, f1: f32, f2: f32) -> Vec3 {
    let uc = sdot(up, pelvis.c, b"yzx");
    let fwd = vnorm(shz(pelvis.c, up, b"yzx"));
    let side = fwd.vx(up);
    let centre = side.mul_add(Vec3::splat(sdot(up, pelvis.b, b"zyx") * 0.4), fwd.mul_add(Vec3::splat(uc * 0.4), pelvis.d));
    let centre = up.mul_add(Vec3::splat(sdot(t, up, b"yxz") - sdot(centre, up, b"zyx")), centre);
    let rel = t - centre;
    let (x, y) = (sdot(rel, fwd, b"yzx") * (1.0 / (f1 * 1.5)), sdot(rel, side, b"zyx") * (1.0 / (f2 * 2.5)));
    let r = y.fma(y, x * x).sqrt();
    let s = (1.0 / (r + 1e-6)).min(1.0);
    let (x, y) = (sdot(rel, fwd, b"yzx"), sdot(rel, side, b"zyx"));
    side.mul_add(Vec3::splat(s * y), fwd.mul_add(Vec3::splat(s * x), centre))
}

fn hip_leans(hip: &Eff3, t: Vec3, mid1: f32, mid2: f32, side: f32) -> (f32, f32) {
    let d = t - hip.m.d;
    let v = vnorm(Vec3::new(hip.m.a.vd(d), hip.m.b.vd(d), hip.m.c.vd(d)));
    let th = acos(v.z);
    let k = 1.0 / (v.y.fma(v.y, v.x * v.x).sqrt() + 1e-6);
    ((v.x * k).fma(th, mid1), (v.y * k * th).fma(side, mid2))
}

fn hip_room(hip: &Eff3, l1: f32, l2: f32, l3: f32, l4: f32) -> f32 {
    let (a1, a2) = (hip.actual[0], hip.actual[1]);
    let (s1, s2) = (hip.half_lean1 * 1.5, hip.half_lean2 * 1.5);
    let (m1, m2) = (hip.mid_lean1, hip.mid_lean2);
    let (i1, i2) = (1.0 / s1, 1.0 / s2);
    let (c1, c2) = ((a1 + l3).fma(0.5, -m1) * i1, (a2 + l4).fma(0.5, -m2) * i2);
    let d1 = (l1 - m1).fma(i1, -((l3 - m1) * i1));
    let d2 = (l2 - m2).fma(i2, -((l4 - m2) * i2));
    let num = d2.fma(c2, d1 * c1);
    let den = num - (d2 + c2).fma(d2, (d1 + c1) * d1);
    let t = if den.abs() > 0.001 { 0.0 } else { num / den };
    let (p1, p2) = (t.fma(d1, c1), t.fma(d2, c2));
    let k = 1.0 / (d2.fma(d2, d1 * d1).sqrt() + 1e-6);
    let (u1, u2) = (k * d1, k * d2);
    let off = p1.fma(u2, -(p2 * u1)).abs();
    let (e1, e2) = if off >= 1.0 {
        (a1, a2)
    } else {
        let h = (-off).fma(off, 1.0).sqrt();
        (h.fma(u1, p1).fma(s1, m1), h.fma(u2, p2).fma(s2, m2))
    };
    let (g1, g2) = (l1 - l3, l2 - l4);
    let k = 1.0 / (g2.fma(g2, g1 * g1).sqrt() + 1e-6);
    (k * g2).fma(e2 - a2, k * g1 * (e1 - a1))
}

fn decide(s: &Snap, f: &mut Feet) {
    let free = f.flags & 4 == 0;
    let a = f.dist[0] > f.dist[1] && free;
    let b = f.dist[1] > f.dist[0] && free;
    if f.stepping != 0 {
        return;
    }
    let (lc, rc) = (s.flags & 0x80 != 0, s.flags & 0x20 != 0);
    let mut left = if rc { a || !lc } else { !lc && b };
    let mut right = if lc { b || !rc } else { !rc && a };
    if left || right {
        if f.countdown >= 0 {
            f.countdown -= 1;
        }
        if f.countdown <= 0 {
            (left, right) = (false, false);
        }
    }
    let (alone, far) = if left {
        f.stepping = 1;
        (rc && !lc, f.dist[0] < f.dist[1])
    } else if right {
        f.stepping = 2;
        (lc && !rc, f.dist[1] < f.dist[0])
    } else {
        return;
    };
    f.flags = (f.flags & !0x18) | (alone as u8) << 4 | (far as u8) << 3;
    f.start = f.pos[f.stepping as usize - 1];
}

pub fn foot_placement(p: &Params, s: &Snap, f: &mut Feet) {
    let up = p.up;
    f.height = [sdot(p.target[0], up, b"zyx"), sdot(p.target[1], up, b"zyx")];
    f.pos[0] = sat(s.legs[0].foot.m.d, up, f.height[0], b"zyx");
    f.pos[1] = sat(s.legs[1].foot.m.d, up, f.height[1], b"zyx");
    if s.flags & 0x80 == 0 || s.flags & 0x20 == 0 {
        let [l, r] = &mut f.pos;
        separate(l, r, p.foot_width);
    }
    f.centre = (f.pos[0] + f.pos[1]) * 0.5;
    f.dist = [0.0; 2];
    let pc = sat(s.hvel.mul_add(Vec3::splat(p.lead), s.com), up, sdot(f.centre, up, b"zyx"), b"yxz");
    let inside = support(pc, &s.legs[0].foot, &s.legs[1].foot, 0.05, 0.15, up) > p.p172;
    f.flags = (f.flags & !4) | (inside as u8) << 2;
    let (lpos, rpos) = (s.legs[0].foot.m.d, s.legs[1].foot.m.d);
    if f.flags & 4 != 0 && slen(shz(rpos - lpos, up, b"yzx"), b"zyx") > p.foot_width + 0.2 {
        f.flags &= !4;
    }
    let (a, c, so) = (s.m.a, s.m.c, p.step_offset);
    let dir_b = vnorm(shz(pc - rpos, up, b"yxz"));
    let target_a = (c * sdot(c, dir_b, b"zyx") * sdot(a, dir_b, b"zyx")).mul_add(Vec3::splat(-so), pc) - a * so;
    let dir_a = vnorm(shz(pc - lpos, up, b"yxz"));
    let target_b = a.mul_add(Vec3::splat(so), (c * sdot(c, dir_a, b"zyx") * sdot(a, dir_a, b"zyx")).mul_add(Vec3::splat(so), pc));
    f.dist = [slen(shz(target_a - lpos, up, b"yxz"), b"zyx"), slen(shz(target_b - rpos, up, b"yzx"), b"zyx")];
    decide(s, f);
    if f.stepping == 0 {
        f.flags = (f.flags & 0x3F) | (s.flags & 0x80) | (s.flags & 0x20) << 1;
        return;
    }
    let two = f.stepping == 2;
    let (i, side, mut t, contact) = if two {
        (1, -1.0, target_b, s.flags & 0x20 != 0)
    } else {
        (0, 1.0, target_a, s.flags & 0x80 != 0)
    };
    let leg = &s.legs[i];
    let stance = f.pos[1 - i];
    if if two { f.dist[0] > f.dist[1] } else { f.dist[0] < f.dist[1] } {
        f.flags |= 8;
    }
    if !contact {
        f.flags |= 16;
    }
    f.centre = s.hvel.mul_add(Vec3::splat(0.15), s.com);
    t = reach(t, up, &s.pelvis.m, p.step_clamp * leg.hip.half_lean1, p.step_clamp * leg.hip.half_lean2);
    let fp = leg.foot.m.d;
    let dir_t = vnorm(shz(t - f.start, up, b"yxz"));
    let ahead = sdot(t, dir_t, b"zyx") - sdot(fp, dir_t, b"zyx");
    let closing = sdot(s.vel, dir_t, b"zyx") - sdot(leg.foot.v, dir_t, b"zyx");
    let left_frac = slen(shz(t - fp, up, b"zyx"), b"zyx") / (slen(shz(f.start - fp, up, b"zyx"), b"zyx") + 1e-6);
    let sole = fp - leg.foot.m.c * 0.2 * 0.5;
    let passed = slen(shz(sole - f.prev_target, up, b"zyx"), b"yxz") > slen(shz(f.prev_sole - t, up, b"zyx"), b"zyx");
    let mut speed = (slen(shz(f.prev_sole - f.prev_target, up, b"zyx"), b"yxz") - slen(shz(sole - t, up, b"zyx"), b"yxz")) / p.dt;
    f.prev_target = t;
    f.prev_sole = sole;
    let t1 = up.mul_add(Vec3::splat(sdot(f.pos[i], up, b"zyx") - sdot(t, up, b"yxz")), t);
    let (r80, r84) = hip_leans(&leg.hip, t1, leg.hip.mid_lean1, leg.hip.mid_lean2, side);
    let (r88, r92) = hip_leans(&leg.hip, f.start, leg.hip.mid_lean1, leg.hip.mid_lean2, side);
    let lift = (closing.min(0.0).fma(0.1, ahead) * 0.5).clamp(-p.step_height, p.step_height);
    let (n, foot) = if two { (p.normal[1], &s.legs[1].foot) } else { (p.normal[0], &s.legs[0].foot) };
    let c2 = foot.w.vx(foot.m.c).mul_add(Vec3::splat(0.07), foot.m.c);
    let tilt = 1.5 / (sdot(n, up, b"zyx") + 0.5);
    let toe = sdot(n, c2, b"zyx");
    let room = hip_room(&leg.hip, r80, r84, r88, r92);
    let mut h = toe.abs().min(1.0).fma(0.1, lift).min(room);
    let mut g = p.target[i];
    if f.flags & 16 == 0 {
        h += 0.07;
        g = p.gup.mul_add(Vec3::splat(sdot(p.gup, f.start - g, b"zyx").max(0.0)), g);
    }
    let h = g.x.fma(up.x, g.z.fma(up.z, tilt.fma(h, g.y * up.y)));
    if toe < 0.0 || passed {
        speed = -1.0;
    }
    let t2 = sat(t1, up, h, b"yxz");
    let t3 = reach(t2 + clamp_length(shz(t2 - f.pos[i], up, b"zyx"), 0.0, 0.1), up, &s.pelvis.m,
                   p.step_clamp * leg.hip.half_lean1, p.step_clamp * leg.hip.half_lean2);
    f.pos[i] = t3;
    let mid = (t3 + stance) * 0.5;
    f.centre = up.mul_add(Vec3::splat(sdot(mid, up, b"yzx") - sdot(f.centre, up, b"yzx")), f.centre);
    f.flags = (f.flags & 0x3F) | if two { 0x80 } else { 0x40 };
    let other = if two { s.flags & 0x80 != 0 } else { s.flags & 0x20 != 0 };
    let landed = contact && speed < 0.0 && (left_frac < 0.5 || (f.flags & 8 != 0 && f.flags & 16 != 0));
    if landed || (contact && !other) {
        f.flags |= if two { 0x40 } else { 0x80 };
        f.stepping = 0;
        decide(s, f);
    }
}

fn yaw(k: f32, pelvis: &M34, w: Vec3, toward: bool, predict: bool, angvel: Vec3) -> f32 {
    let w1 = if predict { qrot(rotvec_quat(angvel * -0.2), w, false) } else { w };
    let a = atan2(-sdot(pelvis.c, w1, b"zyx"), -sdot(pelvis.b, w1, b"zyx") * k);
    let half = std::f32::consts::FRAC_PI_2;
    let mut a = if toward {
        if a >= 0.0 { a - half } else { a + half }
    } else {
        let mut a = a - half;
        if a < -std::f32::consts::PI {
            a += std::f32::consts::TAU;
        }
        if !(-2.513_274_2..=2.513_274_2).contains(&a) { 0.0 } else { a }
    };
    if predict {
        a = slen(w, b"yxz").min(1.0) * a * 0.025;
    }
    a
}

pub fn pelvis_control(p: &Params, s: &Snap, pv: &mut Pelvis) {
    let up = p.up;
    let [pl, pr] = pv.planted;
    let both = pl && pr;
    pv.off = shz(s.pelvis.m.d - s.com, up, b"zyx");
    let w = if pv.fixed_vel { pv.vel } else { s.hvel };
    let toward = !pv.fixed_vel;
    let m = &s.pelvis.m;
    let blend = |y: f32, cur: f32| ((y.clamp(-0.6, 0.6) - cur) * (slen(w, b"yxz") * 2.0).min(1.0)).fma(0.2, cur);
    if pl && !pr {
        pv.yaw[0] = blend(yaw(1.0, m, w, toward, false, s.angvel), pv.yaw[0]);
        pv.yaw[1] = pv.yaw[0] * 0.5;
    } else if pr && !pl {
        pv.yaw[1] = blend(yaw(-1.0, m, w, toward, false, s.angvel), pv.yaw[1]);
        pv.yaw[0] = pv.yaw[1] * 0.5;
    } else if pv.fixed_vel {
        pv.yaw[0] = (yaw(1.0, m, w, toward, true, s.angvel) + (pv.yaw[0] - pv.yaw[1]) * 0.5).clamp(-0.6, 0.6);
        pv.yaw[1] = -pv.yaw[0];
    } else {
        let step = p.dt * 0.5;
        let goal = [p.p152 - p.p148, -p.p152 - p.p148];
        for k in 0..2 {
            let y = pv.yaw[k];
            pv.yaw[k] = if y > goal[k] { (y - step).max(goal[k]) } else { (y + step).min(goal[k]) };
        }
    }
    let hz = shz(s.m.c, up, b"zyx");
    let hx = shz(-s.m.a, up, b"xyz");
    let d = s.com - pv.centre;
    let a = (-sdot(d, hz, b"yxz")).clamp(-0.075, 0.15);
    let b = -sdot(d, hx, b"zyx");
    let c = -sdot(s.hvel, hz, b"zyx");
    let e = -sdot(s.hvel, hx, b"yzx");
    let f4 = p.lead * 476.0;
    if both {
        pv.p52 = p.dt.fma(a, pv.p52).clamp(p.lean_bias - 0.1, p.lean_bias + 0.1);
        pv.p56 = p.dt.fma(b, pv.p56).clamp(-0.1, 0.1);
    }
    pv.out[0] = b.fma(238.0, (f4 * 0.5).fma(e, pv.p56 * 104.0)) * 0.003_472_222_2;
    if both {
        pv.out[1] = a.fma(476.0, c.fma(f4, pv.p52 * 208.0)) * -0.006_944_444_5;
    } else {
        pv.out[1] = 0.0;
        pv.acc -= (-0.2 * c).clamp(0.0, 0.5);
    }
    let g = -sdot(p.normal[1], s.legs[1].foot.m.c, b"yzx") + -sdot(p.normal[0], s.legs[0].foot.m.c, b"yzx");
    if g > 0.05 {
        pv.out[1] = ((g - 0.05) * 3.0).min(1.0).fma(-pv.out[1], pv.out[1]);
    }
}

pub(crate) fn asin(x: f32) -> f32 {
    if x <= -1.0 { -std::f32::consts::FRAC_PI_2 } else if x >= 1.0 { std::f32::consts::FRAC_PI_2 } else { (x as f64).asin() as f32 }
}

fn arc(a: Vec3, b: Vec3) -> Quat {
    let (na, mut nb) = (vnorm(a), vnorm(b));
    let d = na.y.fma(nb.y, na.z.fma(nb.z, na.x * nb.x));
    if (d - 1.0).abs() < 1e-6 {
        return Quat::IDENTITY;
    }
    let c = d + 1.0;
    if c.abs() < 1e-6 {
        let big_z = if a.x < a.y { a.y < a.z } else { a.x < a.z };
        nb = if big_z { Vec3::Y } else { Vec3::Z };
    }
    let x = na.y.fma(nb.z, -(na.z * nb.y));
    let y = na.z.fma(nb.x, -(na.x * nb.z));
    let z = na.x.fma(nb.y, -(na.y * nb.x));
    let n2 = c.fma(c, x.fma(x, z.fma(z, y * y)));
    let r = if n2 <= 0.0 {
        0.0
    } else {
        let (n, mut y) = (n2 as f64, crate::estimate::frsqrte(n2 as f64));
        for _ in 0..2 {
            y = (-(y * y)).mul_add(n, 1.0).mul_add(y * 0.5, y);
        }
        y as f32
    };
    Quat::from_xyzw(x * r, y * r, z * r, c * r)
}

fn qmul(a: Quat, b: Quat) -> Quat {
    Quat::from_xyzw(
        (-b.y).fma(a.z, b.z.fma(a.y, a.x.fma(b.w, b.x * a.w))),
        (-b.z).fma(a.x, b.y.fma(a.w, a.y.fma(b.w, a.z * b.x))),
        (-a.y).fma(b.x, b.y.fma(a.x, a.z.fma(b.w, b.z * a.w))),
        (-b.z).fma(a.z, (-b.y).fma(a.y, b.w.fma(a.w, -(a.x * b.x)))),
    )
}

fn local(m: &M34, v: Vec3) -> Vec3 {
    Vec3::new(m.a.vd(v), m.b.vd(v), m.c.vd(v))
}

fn world(m: &M34, v: Vec3) -> Vec3 {
    let col = |k: usize| Vec3::new(m.a[k], m.b[k], m.c[k]);
    Vec3::new(col(0).vd(v), col(1).vd(v), col(2).vd(v))
}

struct LegIk {
    target: Vec3,
    lead: f32,
    yaw: f32,
    side: f32,
    bend: f32,
    w: f32,
    reach: f32,
    lift: f32,
    swing: bool,
    free: bool,
}

fn lead(c: &LegIk, hip: &Eff3, pelvis: &Part, vel: Vec3) -> Vec3 {
    let r = c.target - hip.m.d;
    let k = hip.damping / (hip.strength + 0.1) * c.lead;
    let cr = r.vx(pelvis.w) * k;
    let turned = r + cr;
    let mut out = (vnorm(turned).mul_add(Vec3::splat(slen(r, b"zyx") - slen(turned, b"zyx")), cr) - (pelvis.v - vel) * k) + c.target;
    let n = vnorm(r);
    let along = sdot(n, out - hip.m.d, b"zyx");
    if along < 0.1 {
        out = n.mul_add(Vec3::splat(0.1 - along), out);
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn limb_ik(b: &Body, ch: &mut Character, j3: usize, j1: usize, root: usize, end: usize, target: Vec3, yaw: f32,
                      lead: f32, side: f32, bend: f32, vel: Vec3, swing: Option<(f32, f32)>) {
    limb_ik_w(b, ch, j3, j1, root, end, target, yaw, lead, side, bend, vel, swing, 1.0, false);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn limb_ik_w(b: &Body, ch: &mut Character, j3: usize, j1: usize, root: usize, end: usize, target: Vec3, yaw: f32,
                        lead: f32, side: f32, bend: f32, vel: Vec3, swing: Option<(f32, f32)>, w: f32, free: bool) {
    let (mut hip, mut knee) = (b.eff3(j3), b.eff1(j1));
    (hip.desired, hip.strength, hip.damping) = (ch.eff[j3].desired, ch.eff[j3].strength, ch.eff[j3].damping);
    (knee.desired, knee.strength, knee.damping) = (ch.eff[j1].desired[0], ch.eff[j1].strength, ch.eff[j1].damping);
    let (reach, lift) = swing.unwrap_or((0.0, 0.0));
    let mut c = LegIk { target, lead, yaw, side, bend, w, reach, lift, swing: swing.is_some(), free };
    leg_ik(&mut c, &b.limb(j3, j1), &mut knee, &mut hip, &b.part(end), &b.part(root), vel);
    ch.eff[j3].desired = hip.desired;
    ch.eff[j1].desired[0] = knee.desired;
}

fn leg_ik(c: &mut LegIk, limb: &M34, knee: &mut Eff1, hip: &mut Eff3, foot: &Part, pelvis: &Part, vel: Vec3) {
    let mut f25 = 0.0;
    let mut x = if c.lead > 0.0 { lead(c, hip, pelvis, vel) } else { c.target };
    if c.swing {
        let d = x - foot.m.d;
        let m = slen(d, b"zxy").min(hip.damping / hip.strength * c.reach) + 1e-4;
        let f0 = (1.0 - 0.1 / m).max(0.0);
        let f12 = if c.lift > f0 { (c.lift + f0) * 0.5 } else { c.lift };
        x = (vnorm(d) * m).mul_add(Vec3::splat(1.0 - f12), foot.m.d);
        f25 = 1.0 / (1.0001 - f12);
        c.lead *= 1.0 - f12;
    }
    let kv = world(limb, local(&knee.child, foot.m.d - knee.child.d));
    let (ld, lk) = (slen(limb.d, b"yxz"), slen(kv, b"zyx"));
    let off = asin(sdot(limb.c, kv.vx(limb.d), b"zyx") / (lk * ld));
    let mut h = local(&hip.m, x - hip.m.d);
    let hl = slen(h, b"zyx");
    if hl > lk + ld {
        h *= (lk + ld) / hl;
    }
    let arg = (-hl).fma(hl, lk.fma(lk, ld * ld)) / (lk * ld * 2.0);
    let mut bend = (std::f32::consts::PI - acos(arg)).fma(c.bend, off);
    let v = qrot(rotvec_quat(limb.c * bend), kv, false) + limb.d;
    let ys = c.yaw * c.side;
    let vn = vnorm(v);
    let [tw, mut l1, mut l2] = crate::nmrs::angles(qmul(arc(v, h), rotvec_quat(vn * ys)));
    if tw.abs() > hip.half_twist {
        let excess = tw - tw.clamp(-hip.half_twist, hip.half_twist);
        let a = crate::nmrs::angles(qmul(arc(v, h), rotvec_quat(vn * c.yaw.fma(c.side, -excess))));
        (l1, l2) = (a[1], a[2]);
    }
    let (s1, s2) = (l1 * l1, l2 * l2);
    let e = s2 / (hip.half_lean2 * hip.half_lean2) + s1 / (hip.half_lean1 * hip.half_lean1);
    if e > 1.0 && !c.free {
        bend += ((1.0 - 1.0 / e.sqrt()) * (s1 + s2).sqrt() * c.bend).clamp(-0.5, 0.5);
    }
    let (w, u) = (c.w, 1.0 - c.w);
    knee.desired = knee.desired.fma(u, w * bend);
    let want = [hip.mid_lean1 + l1, c.side.fma(l2, hip.mid_lean2), c.side.fma(tw, hip.mid_twist)];
    for k in 0..3 {
        hip.desired[k] = want[k].fma(w, hip.desired[k] * u);
    }
    if c.swing {
        let u = 1.0 - f25;
        for k in 0..3 {
            hip.desired[k] = hip.actual[k].fma(u, f25 * hip.desired[k]);
        }
        knee.desired = knee.actual.fma(u, knee.desired * f25);
    }
}

pub(crate) fn predict_knee(hip: &Eff3, knee: &Eff1, side: f32) -> Vec3 {
    let len = slen(hip.m.d - knee.m.d, b"yzx");
    let q = crate::nmrs::rotation([(hip.desired[2] - hip.mid_twist) * side, hip.desired[0] - hip.mid_lean1,
                                   (hip.desired[1] - hip.mid_lean2) * side]);
    let v = qrot(q, Vec3::new(0.0, 0.0, len), false);
    let m = &hip.m;
    crate::vm4(m.a, m.b, m.c, m.d, v)
}

pub fn balance_solve(p: &Params, f: &Feet, pv: &Pelvis, s: &mut Snap) {
    let up = p.up;
    let (lp, rp) = (f.flags & 0x80 != 0, f.flags & 0x40 != 0);
    let r = p.pelvis_height;
    let drop = |x: Vec3| {
        let e = ((slen(shz(x - f.centre, up, b"zxy"), b"zyx") - p.hip_dist) * 1.3).powi(2).min(r * r * 0.5625);
        r - (r * r - e).sqrt()
    };
    let low = (sdot(f.pos[0], up, b"yxz") - drop(f.pos[0])).min(sdot(f.pos[1], up, b"zxy") - drop(f.pos[1]));
    let mut t = f.pos;
    if lp {
        t[0] = up.mul_add(Vec3::splat(pv.out[0] * p.hip_dist), t[0]);
    }
    if rp {
        t[1] = up.mul_add(Vec3::splat(-(pv.out[0] * p.hip_dist)), t[1]);
    }
    let lean = pv.out[0].clamp(-0.2, 0.2);
    s.spine.desired[1] = lean;
    s.height = r * p.leg_length - (sdot(f.centre, up, b"yzx") - low) + if lp && rp { 0.0 } else { 0.03 };
    let mut target = t;
    let mut normal = p.normal;
    if lp || rp {
        let side = vnorm(s.pelvis.m.c.vx(up));
        let fwd = up.vx(side);
        let cs = |a: f32| ((a as f64).cos() as f32, (a as f64).sin() as f32);
        let ((cy, sy), (cz, sz)) = (cs(-pv.acc), cs(-pv.out[0]));
        let e = [Vec3::new(cz * cy, sz * cy, -sy), Vec3::new(-sz, cz, 0.0), Vec3::new(cz * sy, sz * sy, cy)];
        let q = rotvec_quat(s.pelvis.w * -0.1);
        let qm = quat_rows(q);
        let pd: [Vec3; 3] = e.map(|e| {
            let a = fwd.mul_add(Vec3::splat(e.z), side.mul_add(Vec3::splat(e.y), up * e.x));
            qm[2].mul_add(Vec3::splat(a.z), qm[1].mul_add(Vec3::splat(a.y), qm[0] * a.x))
        });
        let pdd = up.mul_add(Vec3::splat(pv.p68), f.centre) + pv.off;
        let pdm = M34 { a: pd[0], b: pd[1], c: pd[2], d: pdd };
        let pel = s.pelvis.m;
        let row = |k: usize| pel.c.mul_add(Vec3::splat(pd[2][k]), pel.b.mul_add(Vec3::splat(pd[1][k]), pel.a * pd[0][k]));
        let cm = [row(0), row(1), row(2)];
        let cd = pel.c.mul_add(Vec3::splat(-pd[2].vd(pdd)), pel.b.mul_add(Vec3::splat(-pd[1].vd(pdd)), pel.a.mul_add(Vec3::splat(-pd[0].vd(pdd)), pel.d)));
        for (i, planted) in [lp, rp].into_iter().enumerate() {
            if planted {
                let hip = &s.legs[i].hip;
                let t1 = reach(t[i], up, &pdm, p.step_clamp * hip.half_lean1, p.step_clamp * hip.half_lean2);
                target[i] = crate::vm4(cm[0], cm[1], cm[2], cd, t1);
                let col = |k: usize| Vec3::new(cm[0][k], cm[1][k], cm[2][k]);
                normal[i] = Vec3::new(col(0).vd(p.normal[i]), col(1).vd(p.normal[i]), col(2).vd(p.normal[i]));
            }
        }
    }
    let vel = s.vel;
    let pelvis = s.pelvis;
    let lfoot_v = s.legs[0].foot.v;
    for (i, planted) in [lp, rp].into_iter().enumerate() {
        let leg = &mut s.legs[i];
        let mut c = LegIk { target: target[i], lead: 0.0, yaw: pv.yaw[i], side: if i == 0 { 1.0 } else { -1.0 }, bend: -1.0,
                            w: 1.0, reach: 0.0, lift: 0.0, swing: !planted, free: false };
        if !planted {
            let ahead = target[i] - (lfoot_v - p.gvel[0]).mul_add(Vec3::splat(0.05), leg.foot.m.d);
            (c.lead, c.reach, c.lift) = (p.p168, 5.0, sdot(up, ahead, b"yxz").fma(4.0, 0.1).clamp(0.15, 0.6));
        }
        let limb = leg.limb;
        leg_ik(&mut c, &limb, &mut leg.knee, &mut leg.hip, &leg.foot, &pelvis, vel);
    }
    for (i, planted) in [lp, rp].into_iter().enumerate() {
        let side = if i == 0 { 1.0 } else { -1.0 };
        let leg = &mut s.legs[i];
        let a = if !planted {
            let sh = &leg.shin;
            let a = (-sdot(sh.w, sh.m.a, b"yzx")).fma(leg.ankle.damping / leg.ankle.strength, sdot(sh.m.c, normal[i], b"yzx"));
            let mut a = sdot(p.normal[i], vnorm(leg.foot.m.a.vx(up)), b"zyx").fma(0.8, a * 0.2);
            if sdot(leg.foot.v, s.m.c, b"yzx") < 0.0 {
                a -= 0.2;
            }
            a + acos(p.leg_length.clamp(1e-4, 0.999))
        } else {
            let k = predict_knee(&leg.hip, &leg.knee, side);
            asin(sdot(leg.knee.m.c, vnorm(k - target[i]).vx(normal[i]), b"xyz"))
        };
        leg.ankle.desired[0] = a;
    }
    let over = |h: &Eff3| {
        let (l1, l2) = ((h.desired[0] - h.mid_lean1) / h.half_lean1, (h.desired[1] - h.mid_lean2) / h.half_lean2);
        let e = l2.fma(l2, l1 * l1).sqrt();
        (e - 1.0).max(0.0) / (e + 1e-6) * (h.desired[1] - h.mid_lean2)
    };
    let (ol, or) = (over(&s.legs[0].hip), over(&s.legs[1].hip));
    if lp {
        s.legs[0].ankle.desired[0] = pv.out[1] + s.legs[0].ankle.desired[0] + lean;
    }
    s.legs[0].ankle.desired[1] = pv.yaw[0] * 0.5;
    if rp {
        s.legs[1].ankle.desired[0] += pv.out[1] - lean;
    }
    s.legs[1].ankle.desired[1] = pv.yaw[1] * 0.5;
    let (a, b) = (ol.max(-or), or.max(-ol));
    if a > 0.0 && b <= 0.0 {
        s.legs[1].hip.desired[1] += a;
        if lp {
            s.legs[0].hip.desired[1] -= a;
        }
        s.spine.desired[1] -= a;
    }
    if b > 0.0 && a <= 0.0 {
        s.legs[0].hip.desired[1] += b;
        if rp {
            s.legs[1].hip.desired[1] -= b;
        }
        s.spine.desired[1] -= b;
    }
}

fn m34(q: Quat, d: Vec3) -> M34 {
    M34 { a: q * Vec3::X, b: q * Vec3::Y, c: q * Vec3::Z, d }
}

pub fn own_rage(doll: &Ragdoll) -> Option<(Vec<crate::rigid::Link>, Vec3, Vec<M34>)> {
    let frames: Vec<Transform> = doll.frames().collect();
    if frames.len() != HOMER_PARTS {
        return None;
    }
    let links = frames.iter().enumerate().map(|(k, f)| {
        let (v, w) = doll.motion(k);
        crate::rigid::Link { r: doll.link_rows(k), d: f.translation, m: doll.part_mass(k), v, w }
    }).collect();
    let parts = frames.iter().enumerate().map(|(k, f)| m34(f.rotation * link(doll.nm, k), f.translation - doll.bound_off)).collect();
    Some((links, -doll.bound_off, parts))
}

pub struct Body<'a> {
    pub ch: &'a Character,
    pub build: &'a Build,
    pub doll: &'a Ragdoll,
    pub frames: &'a [Transform],
}

impl Body<'_> {
    fn rigid(&self, j: usize) -> Option<(&crate::rigid::JointConst, &crate::rigid::Link, &crate::rigid::Link, Vec3)> {
        let (jd, (l, place, _)) = (&self.ch.joints[j], self.doll.rigid.as_ref()?);
        Some((jd.rigid.as_ref()?, l.get(jd.parent)?, l.get(jd.child)?, *place))
    }

    pub fn read(&self, j: usize) -> [f32; 3] {
        let jd = &self.ch.joints[j];
        let tw = self.doll.prep_twist.get(jd.child).copied().flatten();
        let Some((c, p, l, _)) = self.rigid(j) else { return self.ch.angle_at(j, self.frames, tw) };
        if !jd.three {
            return [crate::rigid::hinge(c, p, l), 0.0, 0.0];
        }
        let (l1, l2) = crate::rigid::leans(c, p, l);
        let tw = tw.unwrap_or_else(|| crate::rigid::twist(c, p, l));
        let act = |k: usize, raw: f32| {
            let v = (jd.limits[k].1 + jd.limits[k].0).fma(0.5, raw);
            if jd.flip[k] { -v } else { v }
        };
        [act(0, l1), act(1, l2), act(2, tw)]
    }

    pub fn joint_pos(&self, j: usize) -> Vec3 {
        if let Some((c, _, l, place)) = self.rigid(j) {
            return crate::rigid::pos(c, l, place);
        }
        let c = self.ch.joints[j].child;
        self.frames[c].translation + self.frames[c].rotation * self.build.parts[c].anchor
    }

    pub fn part(&self, k: usize) -> Part {
        let (v, w) = self.doll.motion(k);
        if let Some(m) = self.doll.rigid.as_ref().filter(|_| k == 0).and_then(|(_, _, p)| p.get(k)) {
            return Part { m: *m, v, w };
        }
        if let Some((l, place)) = self.doll.rigid.as_ref().and_then(|(l, p, _)| Some((l.get(k)?, *p))) {
            let col = |i: usize| Vec3::new(l.r[0][i], l.r[1][i], l.r[2][i]);
            return Part { m: M34 { a: col(0), b: col(1), c: col(2), d: l.d + place }, v, w };
        }
        let link = if self.build.parts.len() == HOMER_PARTS { link(self.build.nm, k) } else { Quat::IDENTITY };
        Part { m: m34(self.frames[k].rotation * link, self.frames[k].translation), v, w }
    }

    pub fn pmat(&self, k: usize) -> Part {
        match self.doll.rigid.as_ref().and_then(|(_, _, p)| p.get(k)) {
            Some(m) => Part { m: *m, ..self.part(k) },
            None => self.part(k),
        }
    }

    pub(crate) fn eff3(&self, j: usize) -> Eff3 {
        let (jd, e) = (&self.ch.joints[j], &self.ch.eff[j]);
        let mid = |k: usize| (jd.limits[k].0 + jd.limits[k].1) * 0.5;
        let half = |k: usize| (jd.limits[k].1 - jd.limits[k].0) * 0.5;
        let flip = |k: usize, x: f32| if jd.flip[k] { -x } else { x };
        let pos = self.joint_pos(j);
        let m = self.rigid(j).map_or_else(|| m34(self.frames[jd.parent].rotation * jd.rage_parent, pos), |(c, p, l, place)| crate::rigid::frames(c, p, l, place).0);
        Eff3 { m, pos, damping: e.damping, strength: e.strength, half_twist: half(2), half_lean1: half(0),
               half_lean2: half(1), mid_lean1: flip(0, mid(0)), mid_lean2: flip(1, mid(1)), mid_twist: flip(2, mid(2)),
               desired: e.desired, actual: self.ch.actual(j, self.frames) }
    }

    pub(crate) fn eff1(&self, j: usize) -> Eff1 {
        let (jd, e) = (&self.ch.joints[j], &self.ch.eff[j]);
        let pos = self.joint_pos(j);
        let (m, child) = self.rigid(j).map_or_else(|| (m34(self.frames[jd.parent].rotation * jd.in_parent, pos), m34(self.frames[jd.child].rotation * jd.in_child, pos)),
                                                  |(c, p, l, place)| crate::rigid::frames(c, p, l, place));
        Eff1 { m, child, pos, damping: e.damping, strength: e.strength, half: (jd.limits[0].1 - jd.limits[0].0) * 0.5,
               mid: (jd.limits[0].0 + jd.limits[0].1) * 0.5, desired: e.desired[0], actual: self.ch.actual(j, self.frames)[0] }
    }

    fn limb(&self, hip: usize, knee: usize) -> M34 {
        let i = crate::nmrs::LIMBS.iter().position(|&l| l == (hip, knee));
        if let Some(m) = i.and_then(|i| self.ch.limb[i]) {
            return m;
        }
        if let (Some(k), Some((c, p, l, place))) = (self.rigid(knee), self.rigid(hip)) {
            let (k, h) = (crate::rigid::frames(k.0, k.1, k.2, k.3).0, crate::rigid::frames(c, p, l, place).1);
            let t = |v: Vec3| Vec3::new(v.vd(h.a), v.vd(h.b), v.vd(h.c));
            return M34 { a: t(k.a), b: t(k.b), c: t(k.c), d: t(k.d - h.d) };
        }
        let (h, k) = (&self.ch.joints[hip], &self.ch.joints[knee]);
        let fc = h.rage_child.inverse();
        let p = &self.build.parts;
        m34(fc * k.in_parent, fc * (p[k.child].anchor_parent - p[h.child].anchor))
    }

    pub fn at(&self, k: usize) -> Vec3 {
        match self.doll.rigid.as_ref().and_then(|(l, place, _)| Some(l.get(k)?.d + *place)) {
            Some(d) => d,
            None => self.part(k).m.d,
        }
    }

    fn zero_place(&self, k: usize) -> Option<Vec3> {
        match &self.ch.refs {
            Some(r) => r.get(k).map(|m| m[3]),
            None => crate::nmrs::zero_places(self.build).get(k).copied(),
        }
    }

    fn foot(&self, i: usize) -> Vec3 {
        self.part(self.ch.joints[ANKLE[i]].child).m.d
    }

    pub fn block(&self) -> Block {
        let refs = crate::nmrs::PARTS[self.build.nm].get().filter(|r| r.len() == self.build.parts.len() && self.build.parts.len() == HOMER_PARTS);
        if let (Some((l, place, _)), Some(r)) = (self.doll.rigid.as_ref(), refs) {
            let refs: Vec<[Vec3; 3]> = match &self.ch.refs {
                Some(g) => g.iter().map(|m| [m[0], m[1], m[2]]).collect(),
                None => r.iter().enumerate().map(|(k, m)| {
                    let m = m.map(Vec3::from);
                    if k == 0 { [m[2], m[1], -m[0]] } else { [m[0], m[1], m[2]] }
                }).collect(),
            };
            let (m, vel, angvel) = crate::rigid::block(l, &refs, *place, true);
            return Block { com: m.d, vel, angvel, m };
        }
        let (com, vel) = self.doll.com();
        let refs = crate::nmrs::PARTS[self.build.nm].get().filter(|r| r.len() == self.build.parts.len() && self.build.parts.len() == HOMER_PARTS);
        let rows = |m: &[[f32; 3]; 4]| Mat3::from_cols(Vec3::from(m[0]), Vec3::from(m[1]), Vec3::from(m[2])).transpose();
        let (mut sum, mut w, mut mass) = (Mat3::ZERO, Vec3::ZERO, 0.0);
        for (k, p) in self.build.parts.iter().enumerate() {
            let m = self.doll.part_mass(k);
            let turn = match refs {
                Some(r) => {
                    let mut re = rows(&r[k]);
                    if k == 0 {
                        re = Mat3::from_cols(Vec3::new(0.0, 0.0, -1.0), Vec3::Y, Vec3::X) * re;
                    }
                    Mat3::from_quat(self.frames[k].rotation * rage_link(self.build.nm, k)) * re.inverse()
                }
                None => Mat3::from_quat(self.frames[k].rotation * p.bind.inverse()) * Mat3::from_rotation_z(std::f32::consts::PI),
            };
            sum += turn * m;
            w += self.doll.motion(k).1 * m;
            mass += m;
        }
        let t = sum.transpose();
        let o = Mat3::from_cols(t.x_axis.normalize(), t.y_axis.normalize(), t.z_axis.normalize());
        let c = -o.row(1).normalize();
        let a = o.row(2).vx(c).normalize();
        Block { com, vel, angvel: w / mass.max(1e-6), m: M34 { a, b: c.vx(a), c, d: com } }
    }

    pub fn snap(&self, up: Vec3) -> Snap {
        let Block { com, vel, angvel, m } = self.ch.block.unwrap_or_else(|| self.block());
        let mut s = Snap { com, vel, hvel: shz(vel, up, b"yzx"), angvel, m, ..default() };
        for i in 0..2 {
            let leg = &mut s.legs[i];
            leg.hip = self.eff3(HIP[i]);
            leg.thigh = self.part(self.ch.joints[HIP[i]].child);
            leg.knee = self.eff1(KNEE[i]);
            leg.shin = self.part(self.ch.joints[KNEE[i]].child);
            leg.ankle = self.eff3(ANKLE[i]);
            leg.foot = self.part(self.ch.joints[ANKLE[i]].child);
            leg.limb = self.limb(HIP[i], KNEE[i]);
        }
        s.spine = self.eff3(SPINE[0]);
        s.pelvis = self.part(self.ch.joints[HIP[0]].parent);
        let t = |i: usize| self.doll.touch[self.ch.joints[ANKLE[i]].child];
        let (l, r) = (t(0), t(1));
        s.flags = (l & 0x20) << 2 | (l & 0x04) << 4 | (r & 0x20) | (r & 0x04) << 2;
        s
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub step_height: f32,
    pub leg_stiffness: f32,
    pub abort: f32,
    pub step_clamp: f32,
    pub max_steps: i32,
    pub taper: bool,
    pub lean_bias: f32,
    pub hip_yaw: (f32, f32),
    pub legs: (f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Dbm {
    pub p: Params,
    pub f: Feet,
    pub pv: Pelvis,
    pub acc: f32,
    pub leg_offset: f32,
    pub knee_scale: f32,
    pub lean: Lean,
    lean_vec: Vec3,
    as_force: bool,
    pub push: Vec3,
    ratio: f32,
    pub event: Option<f32>,
    pub success: bool,
    pub held: bool,
    pub last: (Vec3, Vec3),
    pub down: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum LeanMode {
    #[default]
    None,
    Dir(Vec3),
    Pos(Vec3),
    Object(Vec3, i32),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Lean {
    pub mode: LeanMode,
    pub amount: f32,
    pub as_force: bool,
}

impl Lean {
    pub fn to_position(pos: Vec3, amount: f32, as_force: bool) -> Lean {
        Lean { mode: LeanMode::Pos(pos), amount, as_force }
    }

    pub fn towards_object(instance: i32, offset: Vec3, amount: f32, as_force: bool) -> Lean {
        Lean { mode: LeanMode::Object(offset, instance), amount, as_force }
    }
}

pub const CHAR_LEGS: (f32, f32) = (0.9898098, 0.2392247);
pub const CHAR_YAW: (f32, f32) = (0.08312305, 0.044633694);
pub const MARGE_LEGS: (f32, f32) = (0.9895031, 0.14375786);
pub const MARGE_YAW: (f32, f32) = (-0.018068215, -0.055556882);
pub const CHAR_HEAD: f32 = 0.29640985;
pub const MARGE_HEAD: f32 = 0.09639438;

pub(crate) fn stance(up: Vec3, pelvis: &M34, l: &M34, r: &M34, head: &M34, zero: [Vec3; 3]) -> ((f32, f32), (f32, f32), f32) {
    let d = r.d - l.d;
    let width = slen(d - up * sdot(up, d, b"zyx"), b"zyx") * 0.75;
    let height = |p: Vec3, l: Vec3, r: Vec3| sdot(up, p - (l + r) * 0.5, b"zyx");
    let leg = (height(pelvis.d, l.d, r.d) / height(zero[0], zero[1], zero[2])).min(1.0);
    let splay = asin(sdot(up, r.c.vx(l.c), b"zyx")) * 0.25;
    let feet = vnorm(l.c + r.c);
    let turn = asin(sdot(up, feet.vx(pelvis.c), b"zyx")) * 0.5;
    ((leg, width), (splay, turn), asin(sdot(up, feet.vx(head.c), b"zyx")))
}

pub(crate) fn zero_stance(ch: &Character, build: &Build, frames: &[Transform]) -> Option<((f32, f32), (f32, f32), f32)> {
    zero_stance_rows(ch, build, &|k: usize| {
        let link = if build.parts.len() == HOMER_PARTS { link(build.nm, k) } else { Quat::IDENTITY };
        m34(frames[k].rotation * link, frames[k].translation)
    })
}

pub(crate) fn zero_stance_rows(ch: &Character, build: &Build, row: &dyn Fn(usize) -> M34) -> Option<((f32, f32), (f32, f32), f32)> {
    let (pelvis, l, r) = (ch.joints[HIP[0]].parent, ch.joints[ANKLE[0]].child, ch.joints[ANKLE[1]].child);
    let places = crate::nmrs::zero_places(build);
    let zero = |k: usize| ch.refs.as_ref().map_or(places.get(k).copied(), |r| r.get(k).map(|m| m[3]));
    Some(stance(ch.up, &row(pelvis), &row(l), &row(r), &row(12), [zero(pelvis)?, zero(l)?, zero(r)?]))
}

impl Dbm {
    pub fn new(b: &Body, c: Config) -> Dbm {
        let up = Vec3::Z;
        let (l, r) = (b.foot(0), b.foot(1));
        let pelvis = b.at(b.ch.joints[HIP[0]].parent);
        let (zero, foot) = (|k: usize| b.zero_place(k), ANKLE.map(|j| b.ch.joints[j].child));
        let cu = b.ch.up;
        let p = Params {
            up, gup: up, dt: 1.0 / 60.0,
            pelvis_height: match zero(0).zip(zero(foot[0])).zip(zero(foot[1])) {
                Some(((p, l), r)) => sdot(up, p - (l + r) * 0.5, b"zyx"),
                None => up.vd(pelvis - (l + r) * 0.5),
            },
            hip_dist: slen((b.eff3(HIP[0]).m.d - pelvis).vx(cu), b"zyx"),
            leg_length: c.legs.0, foot_width: c.legs.1, p148: c.hip_yaw.0, p152: c.hip_yaw.1,
            step_offset: c.legs.1.fma(0.3, 0.08), abort: c.abort, lead: 0.2, p168: 1.0, p172: 0.0, lean_bias: c.lean_bias,
            step_height: c.step_height, leg_stiffness: c.leg_stiffness, step_clamp: step_clamp(b, c), taper: c.taper,
            low_abort: c.taper, ..default()
        };
        let f = Feet { pos: [l, r], centre: (l + r) * 0.5, height: [up.vd(l), up.vd(r)], countdown: c.max_steps, flags: 5, ..default() };
        let (fl, fr) = (b.pmat(b.ch.joints[ANKLE[0]].child).m, b.pmat(b.ch.joints[ANKLE[1]].child).m);
        let pm = b.pmat(b.ch.joints[HIP[0]].parent).m;
        let splay = asin(sdot(cu, fr.c.vx(fl.c), b"zyx")) * 0.25;
        let turn = asin(sdot(cu, vnorm(fl.c + fr.c).vx(pm.c), b"zyx")) * 0.5;
        if std::env::var("NM_YAWINIT").is_ok() {
            for k in 0..b.ch.joints.len().min(21) { let m = b.part(k).m; let t = up.vd((fl.c + fr.c).normalize_or_zero().vx(m.c)).clamp(-1.0, 1.0).asin() * 0.5; eprintln!("YAWINIT part {k} turn {t:.4} -> [{:.4}, {:.4}]", t - splay, -t - splay); }
            eprintln!("YAWINIT splay {splay:.4} turn {turn:.4}");
        }
        let pv = Pelvis { centre: f.centre, p52: p.lean_bias, p68: 100.0, yaw: [turn - splay, -turn - splay], ..default() };
        let mut d = Dbm { p, f, pv, acc: 0.0, leg_offset: 0.0, knee_scale: 1.0, lean: Lean::default(), lean_vec: Vec3::ZERO, as_force: false, push: Vec3::ZERO, ratio: 0.0, event: None, success: false, held: false, last: (Vec3::Z, Vec3::ZERO),
            down: (0..2).any(|i| b.doll.touch[b.ch.joints[ANKLE[i]].child] & 0x20 != 0) };
        d.held = d.holding(&b.snap(up), l, r);
        d
    }

    pub fn restart_over(&mut self, old: &Dbm) {
        (self.pv.off, self.pv.out, self.lean_vec, self.as_force) = (old.pv.off, old.pv.out, old.lean_vec, old.as_force);
    }

    fn tilt(&mut self, b: &Body) {
        let z = Vec3::Z;
        (self.p.up, self.push) = (z, Vec3::ZERO);
        let now = (0..2).any(|i| b.doll.touch[b.ch.joints[ANKLE[i]].child] & 0x20 != 0);
        if !std::mem::replace(&mut self.down, now) {
            return;
        }
        let u = vnorm(z + shz(self.lean_vec, z, b"yzx"));
        if !self.as_force {
            self.p.up = u;
        }
    }

    fn lean_force(&self, mass: f32) -> Vec3 {
        if !self.down || !self.as_force {
            return Vec3::ZERO;
        }
        let z = Vec3::Z;
        let u = vnorm(z + shz(self.lean_vec, z, b"yzx"));
        let g = -z * crate::ragdoll::GRAVITY * mass;
        horizontal(g - u * u.vd(g), z)
    }

    fn lean_vector(&mut self, com: Vec3) -> Vec3 {
        let l = &mut self.lean;
        if !matches!(l.mode, LeanMode::None) {
            self.as_force = l.as_force;
        }
        let to = match l.mode {
            LeanMode::None => return Vec3::ZERO,
            LeanMode::Dir(d) => return d * l.amount,
            LeanMode::Pos(p) | LeanMode::Object(p, _) => p - com,
        };
        if to.length_squared() > 0.1 {
            to.normalize_or_zero() * l.amount
        } else {
            l.mode = LeanMode::None;
            to
        }
    }

    fn holding(&mut self, s: &Snap, l: Vec3, r: Vec3) -> bool {
        let up = self.p.up;
        let h = up.vd(s.com) - up.vd(l).min(up.vd(r));
        let off = horizontal(s.com - l, up).length().min(horizontal(s.com - r, up).length())
            .min(horizontal(s.com - (l + r) * 0.5, up).length());
        (self.event, self.success) = (None, false);
        if off > self.p.abort * h || (h < 0.5 && self.p.low_abort) || self.f.countdown == -1 {
            return false;
        }
        self.success = slen(s.vel, b"yxz") < 0.25 && slen(s.angvel, b"yxz") < 0.25;
        let x = off / (h * 0.18);
        self.ratio = if x > self.ratio { x } else { (x - self.ratio) * 0.25 + self.ratio };
        if self.ratio < 1.0 && h > 0.7 && self.f.countdown > 2 {
            self.event = Some(self.ratio);
        }
        true
    }

    fn probe(&mut self, b: &Body, world: &Collision) {
        let up = self.p.up;
        for i in 0..2 {
            let start = sat(self.f.pos[i], up, sdot(up, b.joint_pos(KNEE[i]), b"zyx"), b"zyx");
            let old = self.p.target[i];
            let hit = world.segment(start, start - up);
            self.p.contact[i] = hit.is_some();
            (self.p.target[i], self.p.normal[i]) = match hit {
                Some((at, n)) => (at + up * 0.03, n),
                None => (start - up * (self.p.leg_length * self.p.pelvis_height), up),
            };
            let y = crate::estimate::vrefp(self.p.dt);
            self.p.gvel[i] = (self.p.target[i] - old) * (-y).mul_add(self.p.dt, 1.0).mul_add(y, y);
        }
    }

    pub fn tick(&mut self, b: &Body, world: &Collision, c: Config, dt: f32) -> Option<Snap> {
        self.p.dt = dt;
        self.p.step_clamp = step_clamp(b, c);
        self.p.abort = c.abort;
        self.p.step_height = c.step_height;
        self.tilt(b);
        let mut s = b.snap(self.p.up);
        self.last = (s.m.b, s.angvel);
        if !self.holding(&s, b.foot(0), b.foot(1)) {
            return None;
        }
        self.pv.centre = self.f.centre;
        self.pv.planted = [self.f.flags & 0x80 != 0, self.f.flags & 0x40 != 0];
        self.p.leg_stiffness = c.leg_stiffness;
        self.p.leg_length = c.legs.0 + self.leg_offset;
        self.pv.acc = self.acc;
        self.lean_vec = self.lean_vector(s.com);
        self.push = self.lean_force(b.doll.mass());
        self.probe(b, world);
        if let Ok(path) = std::env::var("NM_DBMDUMP") {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(f, "{:?}|{:?}|{:?}|{:?}", self.p, s, self.f, self.pv).unwrap();
        }
        foot_placement(&self.p, &s, &mut self.f);
        pelvis_control(&self.p, &s, &mut self.pv);
        balance_solve(&self.p, &self.f, &self.pv, &mut s);
        self.phase3(&mut s);
        Some(s)
    }

    pub(crate) fn phase3(&mut self, s: &mut Snap) {
        self.pv.p68 = s.height;
        for leg in &mut s.legs {
            let x = leg.ankle.desired[0];
            leg.ankle.desired[0] = if x >= -9.5 { x.min(9.5) } else { -9.5 };
        }
    }

    pub fn activate(&self, ch: &mut Character) {
        if !self.held {
            return;
        }
        let k = self.p.leg_stiffness;
        for i in [HIP[0], KNEE[0], ANKLE[0], HIP[1], KNEE[1], ANKLE[1], SPINE[0]] {
            (ch.eff[i].strength, ch.eff[i].damping) = (k * k, 2.0 * k);
            ch.eff[i].oppose_gravity = if ANKLE.contains(&i) || KNEE.contains(&i) { 2.0 } else { 1.0 };
        }
        for i in 0..2 {
            ch.eff[KNEE[i]].muscle_stiffness = 2.0;
            ch.eff[HIP[i]].muscle_stiffness = 1.5;
        }
    }

    pub fn apply(&self, s: &Snap, ch: &mut Character) {
        for i in 0..2 {
            let leg = &s.legs[i];
            ch.eff[HIP[i]].desired = leg.hip.desired;
            ch.eff[KNEE[i]].desired[0] = leg.knee.desired;
            ch.eff[ANKLE[i]].desired = leg.ankle.desired;
        }
        ch.eff[SPINE[0]].desired = s.spine.desired;
        let foot = |i: usize| ch.joints[ANKLE[i]].child as i32;
        match self.f.stepping {
            0 => ch.support = [-2; 2],
            1 => ch.support = [-1, foot(1)],
            2 => ch.support = [foot(0), -1],
            _ => {}
        }
        let soft = self.p.dt.fma(60.0, -1.0).fma(4.0, 1.0).clamp(1.0, 5.0);
        let k = self.p.leg_stiffness * self.knee_scale;
        for i in 0..2 {
            let touching = s.flags & [0x80, 0x20][i] != 0;
            ch.eff[ANKLE[i]].muscle_stiffness = if touching { 10.0 } else { soft };
            let planted = self.f.flags & [0x80, 0x40][i] != 0;
            let knee = &mut ch.eff[KNEE[i]];
            if planted {
                let f = if self.p.taper { (-(-s.legs[i].knee.actual - 1.5)).fma(1.218_329, 1.0).clamp(0.2, 1.0) } else { 1.0 };
                (knee.strength, knee.muscle_stiffness) = ((f * k).powi(2), 1.5);
            } else {
                (knee.strength, knee.muscle_stiffness) = (k * k, 2.0);
            }
            ch.eff[ANKLE[i]].oppose_gravity = sdot(self.p.normal[i], s.legs[i].foot.m.b, b"yzx").powi(2) * 2.0;
        }
    }
}

pub fn oppose_gravity(b: &Body, attached: Option<(usize, Vec3, f32)>) -> Vec<Vec3> {
    const AHEAD: f32 = 0.1;
    const SCALE: f32 = 0.2;
    let (ch, n) = (b.ch, b.build.parts.len());
    let mut out = vec![Vec3::ZERO; n];
    let up = ch.up;
    let down: Vec<usize> = (0..2).filter_map(|i| match ch.support[i] {
        -2 => Some(ch.joints[ANKLE[i]].child).filter(|&f| b.doll.touch[f] & 0x20 != 0),
        k => usize::try_from(k).ok(),
    }).collect();
    let lo = down.iter().map(|f| f - 3).min().unwrap_or(30).min(30) as i32;
    let hi = down.iter().map(|f| f - 1).max().map_or(-30, |h| h as i32).max(-30);
    let chain = |i: usize| (lo..=hi).contains(&(i as i32));
    let pair = hi > lo + 3;
    let place = b.doll.rigid.as_ref().map_or(Vec3::ZERO, |r| r.0[0].d + r.1 - b.doll.centre(0));
    let com = b.doll.com().0 + place;
    let feet: Vec<M34> = down.iter().map(|&f| b.part(f).m).collect();
    let mut g = -up * crate::ragdoll::GRAVITY;
    if let (Some(l), Some(r)) = (feet.first(), feet.last()) {
        let sp = support_at(com, l, r, 0.05, 0.15, up).1;
        let d = (sp - com).normalize_or_zero();
        g = d * d.vd(g);
    } else {
        g = Vec3::ZERO;
    }
    let mut m: Vec<(f32, Vec3, Vec3)> = (0..n).map(|k| {
        let mk = b.doll.part_mass(k);
        (mk, (b.doll.centre(k) + place) * mk, b.doll.motion(k).0 * mk)
    }).collect();
    if let Some((k, pos, mass)) = attached.filter(|a| a.0 < n) {
        (m[k].0, m[k].1) = (m[k].0 + mass, m[k].1 + pos * mass);
    }
    let add = |m: &mut Vec<(f32, Vec3, Vec3)>, to: usize, from: usize| {
        let f = m[from];
        m[to] = (m[to].0 + f.0, m[to].1 + f.1, m[to].2 + f.2);
    };
    for i in (0..ch.joints.len()).rev().filter(|&i| !chain(i)) {
        add(&mut m, ch.joints[i].parent, ch.joints[i].child);
    }
    for i in (0..ch.joints.len()).filter(|&i| chain(i)) {
        add(&mut m, ch.joints[i].child, ch.joints[i].parent);
    }
    let ahead = |k: usize| (m[k].1 + m[k].2 * AHEAD) / m[k].0;
    let at = |i: usize, k: usize| {
        let p = b.joint_pos(i);
        let (v, w) = b.doll.motion(k);
        p + (v + w.vx(p - b.doll.centre(k) - place)) * AHEAD
    };
    for (i, (j, e)) in ch.joints.iter().zip(&ch.eff).enumerate() {
        if e.oppose_gravity == 0.0 {
            continue;
        }
        let (lever, force) = if chain(i) && pair {
            let o = if (i as i32) < lo + 3 { i + 3 } else { i - 3 };
            let (ka, kb) = (j.parent, ch.joints[o].parent);
            let half = 0.5 * (m[ka].0 + m[kb].0);
            let c = (m[ka].1 + m[ka].2 * AHEAD + m[kb].1 + m[kb].2 * AHEAD) * 0.5 / half;
            let (pa, pb) = (at(i, ka), at(o, kb));
            let d = pb - pa;
            let s = (c - pa).vd(d) / d.length_squared();
            let f = g * half;
            let dn = d.normalize_or_zero();
            (pa + d * s - c, f - dn * f.vd(dn))
        } else {
            let k = if chain(i) { j.parent } else { j.child };
            (at(i, k) - ahead(k), g * m[k].0)
        };
        let t = lever.vx(force) * if chain(i) { -1.0 } else { 1.0 };
        let max = e.oppose_gravity * e.strength * SCALE;
        let t = if j.three {
            let mut t = clamp_length(t, 0.0, max);
            if ANKLE.contains(&i) {
                let f = b.part(j.child).m;
                let x = t.vd(f.a);
                if x > 0.25 * max {
                    t -= f.a * (x - 0.25 * max);
                }
                let z = t.vd(f.c);
                t -= f.c * if z > 0.5 * max { z - 0.5 * max } else if z < -0.5 * max { z + 0.5 * max } else { 0.0 };
            }
            t
        } else {
            let axis = b.frames[j.parent].rotation * j.in_parent * Vec3::Z;
            axis * t.vd(axis).clamp(-max, max)
        };
        out[j.child] += t;
        out[j.parent] -= t;
    }
    out
}

pub fn foot_yaw(b: &Body, yaw: &mut [f32; 2], dt: f32) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; b.build.parts.len()];
    let up = b.ch.up;
    for i in 0..2 {
        let k = b.ch.joints[ANKLE[i]].child;
        let t = b.doll.touch[k];
        if t & 0x20 == 0 {
            continue;
        }
        let m = b.part(k).m;
        let upright = up.vd(m.b).max(0.0);
        let now = atan2(m.c.vd(Vec3::X), m.c.vd(up.vx(Vec3::X)));
        if t & 0x04 == 0 {
            yaw[i] = now;
        }
        let mut d = now - yaw[i];
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        if d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        let w = b.doll.motion(k).1.vd(up);
        let t = (d * 60.0 - w * 10.0) * upright / (dt * 60.0);
        out[k] = up * t;
    }
    out
}

pub fn stabilise(b: &Body, k: f32, dt: f32) -> Vec<Vec3> {
    let mut out = vec![Vec3::ZERO; b.build.parts.len()];
    let up = b.ch.up;
    let feet = [0, 1].map(|i| b.ch.joints[ANKLE[i]].child);
    if !feet.iter().any(|&f| b.doll.touch[f] & 0x28 == 0x20) {
        return out;
    }
    let (com, vel) = b.doll.com();
    let mid = (b.doll.centre(feet[0]) + b.doll.centre(feet[1])) * 0.5;
    let fv = (b.doll.motion(feet[0]).0 + b.doll.motion(feet[1]).0) * 0.5;
    let s = ((com - mid).normalize_or_zero().vd(up).max(0.0).powi(4) * k).powi(2);
    let f = (com + vel * 0.1 - mid) * s + (vel - fv) * (0.2 * s);
    let f = horizontal(f, up) * (dt * 60.0);
    out[feet[0]] += f * 0.5;
    out[feet[1]] += f * 0.5;
    out[b.ch.joints[HIP[0]].parent] -= f;
    out
}

