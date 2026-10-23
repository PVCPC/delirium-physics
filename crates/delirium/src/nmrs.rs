use crate::Fma;

pub static LOOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn loose(j: usize, l: [(f32, f32); 3]) -> [(f32, f32); 3] {
    let want = match j { 0 | 3 => [0.0, 0.0, 40.0f32], 2 | 5 => [50.0, 25.0, 0.0], _ => return l };
    if !LOOSE.load(std::sync::atomic::Ordering::Relaxed) { return l }
    std::array::from_fn(|k| {
        let (mid, half) = ((l[k].0 + l[k].1) * 0.5, ((l[k].1 - l[k].0) * 0.5).max(want[k].to_radians()));
        (mid - half, mid + half)
    })
}
use crate::prelude::*;
use delirium_data::nm_asset;

use crate::ragdoll::{Build, Muscle, Rigid};

pub const HOMER: usize = 0;
pub const MARGE: usize = 1;
pub fn asset_of(ped_type: &str) -> usize {
    if ped_type.eq_ignore_ascii_case("CIVFEMALE") { MARGE } else { HOMER }
}
pub static ASSET: [std::sync::OnceLock<Vec<nm_asset::Joint>>; 2] = [const { std::sync::OnceLock::new() }; 2];
pub static SELF: [std::sync::OnceLock<Vec<(usize, usize)>>; 2] = [const { std::sync::OnceLock::new() }; 2];
pub static PARTS: [std::sync::OnceLock<Vec<[[f32; 3]; 4]>>; 2] = [const { std::sync::OnceLock::new() }; 2];
pub static MASSES: [std::sync::OnceLock<Vec<f32>>; 2] = [const { std::sync::OnceLock::new() }; 2];

pub fn load(nm: usize, d: &[u8]) {
    set(nm, nm_asset::read(d), nm_asset::parts(d), nm_asset::masses(d), nm_asset::self_collision(d));
}

pub fn set(nm: usize, joints: Vec<nm_asset::Joint>, parts: Vec<[[f32; 3]; 4]>, masses: Vec<f32>, pairs: Vec<(usize, usize)>) {
    let _ = ASSET[nm].set(joints);
    let _ = PARTS[nm].set(parts);
    let _ = MASSES[nm].set(masses);
    let _ = SELF[nm].set(pairs);
}

pub fn cg(nm: usize) -> Vec3 {
    let Some(p) = PARTS[nm].get().and_then(|p| p.first()) else { return Vec3::ZERO };
    let h = std::f32::consts::FRAC_PI_2 as f64;
    let (c, s) = (h.cos() as f32, h.sin() as f32);
    let z = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]];
    let r: Vec<[f32; 3]> = p[..3].iter().map(|m| std::array::from_fn(|j| z[2][j].mul_add(m[2], z[1][j].mul_add(m[1], z[0][j] * m[0])))).collect();
    let t = p[3];
    Vec3::from_array(std::array::from_fn(|j| t[0] * r[0][j] + t[1] * r[1][j] + t[2] * r[2][j]))
}

pub(crate) fn ours(v: [f32; 3]) -> Vec3 {
    Vec3::new(-v[0], -v[1], v[2])
}

pub const HIP: [usize; 2] = [0, 3];
pub const KNEE: [usize; 2] = [1, 4];
pub const ANKLE: [usize; 2] = [2, 5];
pub const SPINE: [usize; 4] = [6, 7, 8, 9];
pub const NECK: [usize; 2] = [10, 11];
pub const CLAVICLE: [usize; 2] = [12, 16];
pub const SHOULDER: [usize; 2] = [13, 17];
pub const ELBOW: [usize; 2] = [14, 18];
pub const WRIST: [usize; 2] = [15, 19];
pub const LIMBS: [(usize, usize); 4] = [(13, 14), (17, 18), (0, 1), (3, 4)];

#[derive(Clone, Debug)]
pub struct JointDef {
    pub parent: usize,
    pub child: usize,
    pub three: bool,
    pub in_parent: Quat,
    pub in_child: Quat,
    pub rage_parent: Quat,
    pub rage_child: Quat,
    pub limits: [(f32, f32); 3],
    pub flip: [bool; 3],
    pub muscle_stiffness: f32,
    pub damping: f32,
    pub strength: f32,
    pub max_torque: [f32; 2],
    pub rigid: Option<crate::rigid::JointConst>,
}

pub fn joints(build: &Build, asset: &[nm_asset::Joint]) -> Option<Vec<JointDef>> {
    if asset.len() != 20 || build.parts.len() != 21 {
        return None;
    }
    let zero = zero_pose(build);
    asset.iter().enumerate().map(|(i, j)| {
        let child = i + 1;
        let parent = build.parts[child].parent?;
        let z = ours(j.twist_axis).normalize_or(Vec3::Z);
        let x0 = if j.three { ours(j.lean_axis) } else { z.cross(Vec3::Z) };
        let x = (x0 - z * x0.dot(z)).normalize_or(z.any_orthonormal_vector());
        let frame = Quat::from_mat3(&Mat3::from_cols(x, z.cross(x), z)).normalize();
        let (in_parent, in_child) = ((zero[parent].inverse() * frame).normalize(), (zero[child].inverse() * frame).normalize());
        let (rage_parent, rage_child) = rage_frames(j.three, &j.limits, in_parent, in_child);
        let (lp, lc) = (crate::nmdbm::rage_link(build.nm, parent).inverse(), crate::nmdbm::rage_link(build.nm, child).inverse());
        let rows = |q: Quat| { let m = Mat3::from_quat(q.normalize()); [m.row(0), m.row(1), m.row(2)] };
        let (pm, cm) = if j.three { rage_rows(&j.limits, rows(lp * in_parent), rows(lc * in_child)) } else { (rows(lp * rage_parent), rows(lc * rage_child)) };
        let rigid = crate::rigid::JointConst { pm, pa: lp * build.parts[child].anchor_parent, ca: lc * build.parts[child].anchor, cm };
        Some(JointDef {
            parent, child, three: j.three, in_parent, in_child, rage_parent, rage_child,
            limits: j.limits, flip: j.flip,
            muscle_stiffness: j.muscle_stiffness, damping: j.damping, strength: j.strength, max_torque: j.max_torque,
            rigid: Some(rigid),
        })
    }).collect()
}

pub fn asset_layout(build: &mut Build, frames: &mut [Transform]) {
    let (Some(parts), Some(asset)) = (PARTS[build.nm].get(), ASSET[build.nm].get()) else { return };
    if parts.len() != 21 || asset.len() != 20 || build.parts.len() != 21 || frames.len() != 21 {
        return;
    }
    let zero = zero_pose(build);
    let centre = |i: usize| ours(parts[i][3]);
    for (i, j) in asset.iter().enumerate() {
        let child = i + 1;
        let Some(parent) = build.parts[child].parent else { continue };
        let pivot = ours(j.pos);
        build.parts[child].anchor = zero[child].inverse() * (pivot - centre(child));
        build.parts[child].anchor_parent = zero[parent].inverse() * (pivot - centre(parent));
    }
    for c in 0..frames.len() {
        if let Some(p) = build.parts[c].parent {
            let pivot = frames[p].translation + frames[p].rotation * build.parts[c].anchor_parent;
            frames[c].translation = pivot - frames[c].rotation * build.parts[c].anchor;
        }
    }
}

pub fn derive_links(build: &Build, stand: &[Quat]) -> Option<Vec<Quat>> {
    let parts = PARTS[build.nm].get()?;
    if build.parts.len() != 21 || parts.len() != 21 || stand.len() != 21 {
        return None;
    }
    Some(parts.iter().zip(stand).map(|(m, &s)| {
        let nm = asset_frame(m, false);
        (quarter(nm.inverse() * s).inverse() * nm.inverse() * asset_frame(m, true)).normalize()
    }).collect())
}

pub(crate) fn rage_turns(limits: &[(f32, f32); 3]) -> ([f32; 4], [f32; 4]) {
    let mid = |k: usize| (limits[k].0 + limits[k].1) * 0.5;
    let tan = |v: f32| (v as f64).tan() as f32;
    let l = tan_quat(0.0, tan(mid(0) * 0.25), tan(mid(1) * 0.25));
    let half = (mid(2) * 0.5) as f64;
    let (s, c) = (half.sin() as f32, half.cos() as f32);
    ([l.y, l.x, l.z, l.w], [0.0 * s, 0.0 * s, s, c])
}

pub(crate) fn rage_rows(limits: &[(f32, f32); 3], parent: [Vec3; 3], child: [Vec3; 3]) -> ([Vec3; 3], [Vec3; 3]) {
    let (a, b) = rage_turns(limits);
    (turn_rows(parent, a), turn_rows(child, b))
}

pub(crate) fn joint_frames(p: Vec3, b: Vec3, c: Vec3, a: (&[Vec3; 3], Vec3), bb: (&[Vec3; 3], Vec3)) -> crate::rigid::JointConst {
    frames(p, c, cross(b, c), b, a, bb)
}

fn cross(b: Vec3, c: Vec3) -> Vec3 {
    Vec3::new((-c.y).fma(b.z, c.z * b.y), (-c.z).fma(b.x, c.x * b.z), (-c.x).fma(b.y, c.y * b.x))
}

fn frames(p: Vec3, c: Vec3, x: Vec3, b: Vec3, (ra, ta): (&[Vec3; 3], Vec3), (rb, tb): (&[Vec3; 3], Vec3)) -> crate::rigid::JointConst {
    let w = [Vec3::new(c.x, x.x, b.x), Vec3::new(c.y, x.y, b.y), Vec3::new(c.z, x.z, b.z)];
    use crate::Vd;
    let side = |r: &[Vec3; 3], t: Vec3| -> ([Vec3; 3], Vec3) {
        let col = |k: usize| Vec3::new(r[0][k], r[1][k], r[2][k]);
        let d = p - t;
        ([0, 1, 2].map(|i| crate::rigid::row_mul(col(i), &w)), Vec3::new(d.vd(col(0)), d.vd(col(1)), d.vd(col(2))))
    };
    let ((pm, pa), (cm, ca)) = (side(ra, ta), side(rb, tb));
    crate::rigid::JointConst { pm, pa, ca, cm }
}

pub(crate) fn joint_limits(f: [f32; 3]) -> ([f32; 6], [bool; 2]) {
    let g = |x: f32| { let v = x - f32::from_bits(0x3cf5c28f); if -v >= 0.0 { 0.0 } else { v } };
    let l = [g(f[0]), g(f[0]), g(f[1]), g(f[1]), g(f[2]), g(f[2])];
    (l, [f[0] != f[1], l[5] != f32::MAX])
}

fn across(a: Vec3) -> (Vec3, Vec3) {
    let near = !(a.x > 0.5) && !(a.x < -0.5) && !(a.y > 0.5) && !(a.y < -0.5);
    let u = crate::nmdbm::vnorm(if near { Vec3::new(0.0, a.z, -a.y) } else { Vec3::new(a.y, -a.x, 0.0) });
    (u, crate::nmdbm::vnorm(cross(a, u)))
}

#[derive(Debug)]
pub struct PhJoint {
    pub three: bool,
    pub effector: bool,
    pub jc: crate::rigid::JointConst,
    pub limits: Option<([f32; 6], [bool; 2])>,
}

pub fn ph_joint(s: &mut nm_asset::Stream, off: Vec3, a: (&[Vec3; 3], Vec3), b: (&[Vec3; 3], Vec3)) -> Option<PhJoint> {
    let three = match s.byte().ok()? { b'3' => true, b'1' => false, _ => return None };
    let eps = f32::from_bits(0x38d1b717);
    let mut v = || s.vec3().ok().map(Vec3::from);
    let p = v()? - off;
    let ax = v()?;
    let len2 = |v: Vec3| v.z.fma(v.z, v.x.fma(v.x, v.y * v.y));
    let lean = if three {
        let c = v()?;
        let d = ax - c;
        if len2(ax) < eps || len2(c) < eps || d.x.fma(d.x, d.z.fma(d.z, d.y * d.y)) < eps {
            return None;
        }
        c
    } else {
        if len2(ax) < eps {
            return None;
        }
        Vec3::ZERO
    };
    let (centred, effector) = (s.flag().ok()? != 0, s.flag().ok()? != 0);
    let n = if three { 68 } else { 32 };
    let blob = s.string(Some(n)).ok()?;
    if blob.len() != n {
        return None;
    }
    let f = |k: usize| f32::from_bits(u32::from_be_bytes([blob[k], blob[k + 1], blob[k + 2], blob[k + 3]]));
    if !centred {
        let (u, w) = across(ax);
        return Some(PhJoint { three, effector, jc: frames(p, u, w, ax, a, b), limits: None });
    }
    if !three {
        let (u, w) = across(ax);
        let c = f32::from_bits(0x3cf5c28f);
        let (lo, hi) = (c + f(8), f(12) - c);
        let (lo, hi) = if lo <= hi { (lo, hi) } else { let m = (f(8) + f(12)) * 0.5; (m, m) };
        return Some(PhJoint { three, effector, jc: frames(p, u, w, ax, a, b), limits: Some(([lo, hi, lo, hi, 0.0, 0.0], [false; 2])) });
    }
    let l = [(f(12), f(16)), (f(20), f(24)), (f(28), f(32))];
    let h = l.map(|(lo, hi)| (hi - lo) * 0.5);
    let mut jc = joint_frames(p, ax, lean, a, b);
    let (mut lim, _) = joint_limits(h);
    let (qa, qb) = rage_turns(&l);
    (jc.pm, jc.cm) = (turn_rows(jc.pm, qa), turn_rows(jc.cm, qb));
    for k in 0..3 {
        lim[2 * k + 1] = joint_limits([f(36 + 4 * k) * h[k]; 3]).0[0];
    }
    Some(PhJoint { three, effector, jc, limits: Some((lim, [true, lim[5] != f32::MAX])) })
}

fn rage_frames(three: bool, limits: &[(f32, f32); 3], in_parent: Quat, in_child: Quat) -> (Quat, Quat) {
    if !three {
        return (in_parent, in_child);
    }
    let rows = |q: Quat| { let m = Mat3::from_quat(q); [m.row(0), m.row(1), m.row(2)] };
    let quat = |r: [Vec3; 3]| Quat::from_mat3(&Mat3::from_cols(r[0], r[1], r[2]).transpose()).normalize();
    let (p, c) = rage_rows(limits, rows(in_parent), rows(in_child));
    (quat(p), quat(c))
}

pub(crate) fn zero_pose(build: &Build) -> Vec<Quat> {
    let Some(parts) = PARTS[build.nm].get().filter(|p| p.len() == build.parts.len()) else { return build.parts.iter().map(|p| p.bind).collect() };
    parts.iter().enumerate().map(|(k, m)| (asset_frame(m, true) * crate::nmdbm::link(build.nm, k).inverse()).normalize()).collect()
}

pub(crate) fn zero_places(build: &Build) -> Vec<Vec3> {
    let Some(parts) = PARTS[build.nm].get() else { return vec![] };
    let mut places: Vec<Vec3> = parts.iter().map(|m| Vec3::from(m[3])).collect();
    if places.len() != build.parts.len() || build.parts.iter().all(|p| p.size == 1.0) {
        return places;
    }
    let turn = zero_pose(build);
    let mut moved = vec![Vec3::ZERO; places.len()];
    for (c, p) in build.parts.iter().enumerate() {
        if let Some(q) = p.parent {
            let grown = |s: f32| 1.0 - 1.0 / s;
            moved[c] = moved[q] + turn[q] * p.anchor_parent * grown(build.parts[q].size) - turn[c] * p.anchor * grown(p.size);
        }
        places[c] += ours(moved[c].into());
    }
    places
}

fn asset_frame(m: &[[f32; 3]; 4], rigid: bool) -> Quat {
    let r = Mat3::from_cols(Vec3::from(m[0]), Vec3::from(m[1]), Vec3::from(m[2]));
    let r = if rigid { r.transpose() } else { r };
    Quat::from_mat3(&Mat3::from_cols(ours(r.x_axis.into()), ours(r.y_axis.into()), ours(r.z_axis.into()))).normalize()
}

fn quarter(q: Quat) -> Quat {
    let m = Mat3::from_quat(q);
    let s = |v: Vec3| { let a = v.abs(); if a.x >= a.y && a.x >= a.z { Vec3::X * v.x.signum() } else if a.y >= a.z { Vec3::Y * v.y.signum() } else { Vec3::Z * v.z.signum() } };
    Quat::from_mat3(&Mat3::from_cols(s(m.x_axis), s(m.y_axis), s(m.z_axis)))
}

pub fn angles(q: Quat) -> [f32; 3] {
    let (x, y, z, w) = (q.x, q.y, q.z, q.w);
    if (x.fma(x, y * y) + z * z).sqrt() <= 0.0 {
        return [0.0; 3];
    }
    let wz = w.fma(w, z * z);
    let (tq, a, b) = if wz > 1.2e-7 {
        let s = wz.sqrt();
        let tq = if w >= 0.0 { z / (s + w) } else { z / (w - s) };
        let k = 1.0 / ((s + 1.0) * s);
        (tq, y.fma(z, -(w * x)) * k, w.fma(y, x * z) * k)
    } else {
        let r = 1.0 / (1.0 - wz).sqrt();
        (0.0, -(r * x), r * y)
    };
    let t = b.fma(b, a * a).sqrt();
    let atan = |v: f32| (v as f64).atan() as f32;
    let k = 1.0 / t * (atan(t) * 4.0);
    [atan(tq) * 4.0, k * b, k * a]
}

fn mat_quat(r: &[[f32; 3]; 3]) -> Quat {
    let tr = r[0][0] + r[1][1] + r[2][2];
    if tr > 0.0 {
        let s = (tr + 1.0).sqrt();
        let f = 0.5 / s;
        return Quat::from_xyzw((r[1][2] - r[2][1]) * f, (r[2][0] - r[0][2]) * f, (r[0][1] - r[1][0]) * f, s * 0.5);
    }
    let i = if r[1][1] > r[0][0] { 1 } else { 0 };
    let i = if r[2][2] > r[i][i] { 2 } else { i };
    let (j, k) = ((i + 1) % 3, (i + 2) % 3);
    let s = (r[i][i] - r[j][j] - r[k][k] + 1.0).sqrt();
    let mut v = [0.0; 4];
    v[i] = s * 0.5;
    if s != 0.0 {
        let f = 0.5 / s;
        v[3] = (r[j][k] - r[k][j]) * f;
        v[j] = (r[i][j] + r[j][i]) * f;
        v[k] = (r[i][k] + r[k][i]) * f;
    }
    Quat::from_xyzw(v[0], v[1], v[2], v[3])
}

pub fn incoming_rows(nm: usize, frames: &[Transform]) -> Vec<[Vec3; 3]> {
    frames.iter().enumerate().map(|(k, f)| {
        let m = Mat3::from_quat(f.rotation * crate::nmdbm::link(nm, k));
        [m.x_axis, m.y_axis, m.z_axis]
    }).collect()
}

pub fn rotation([tw, l1, l2]: [f32; 3]) -> Quat {
    if l2.fma(l2, tw.fma(tw, l1 * l1)).sqrt() <= 0.0 {
        return Quat::IDENTITY;
    }
    let sw = l2.fma(l2, l1 * l1).sqrt();
    let tan = |v: f32| (v as f64).tan() as f32;
    let f = 1.0 / (sw + 1e-6) * tan(sw * 0.25);
    let (a, b, tq) = (f * l2, f * l1, tan(tw * 0.25));
    let (c, e) = (2.0 / tq.fma(tq, 1.0), 2.0 / (b.fma(b, a * a) + 1.0));
    let (c1, e1) = (c - 1.0, e - 1.0);
    Quat::from_xyzw((c * b).fma(tq, -(c1 * a)) * e, (c * a).fma(tq, c1 * b) * e, e1 * c * tq, e1 * c1)
}

pub fn quat_rows(q: [f32; 4]) -> [Vec3; 3] {
    let c = f32::from_bits(0x3fb504f3);
    let (x, y, z, w) = (q[0] * c, q[1] * c, q[2] * c, q[3] * c);
    let (zx, yx, wz, wy, zy, wx) = (z * x, y * x, w * z, w * y, z * y, w * x);
    let (zz, yy, xx) = (z * z, y * y, x * x);
    [Vec3::new(1.0 - (zz + yy), yx + wz, zx - wy), Vec3::new(yx - wz, 1.0 - (zz + xx), wx + zy), Vec3::new(wy + zx, zy - wx, 1.0 - (yy + xx))]
}

pub fn turn_rows(m: [Vec3; 3], q: [f32; 4]) -> [Vec3; 3] {
    let [r0, r1, r2] = quat_rows(q);
    m.map(|r| Vec3::new(r.z.mul_add(r2.x, r.y.mul_add(r1.x, r0.x * r.x)), r.z.mul_add(r2.y, r.y.mul_add(r1.y, r0.y * r.x)), r.z.mul_add(r2.z, r.y.mul_add(r1.z, r0.z * r.x))))
}

pub fn tan_quat(tq: f32, a: f32, b: f32) -> Quat {
    let (c, e) = (2.0 / tq.fma(tq, 1.0), 2.0 / (a.fma(a, b * b) + 1.0));
    let (c1, e1) = (c - 1.0, e - 1.0);
    Quat::from_xyzw((tq * c).fma(b, -(c1 * a)) * e, (tq * c).fma(a, c1 * b) * e, e1 * tq * c, e1 * c1)
}

const MIN_DAMPING: f32 = 5.0;

pub fn effector_angles(raw: [f32; 3], limits: &[(f32, f32); 3], flip: [bool; 3]) -> [f32; 3] {
    std::array::from_fn(|k| {
        let mid = (limits[k].0 + limits[k].1) * 0.5;
        if flip[k] { -raw[k] - mid } else { raw[k] + mid }
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoftLimit {
    pub stiffness: f32,
    pub damping: f32,
    pub angle: f32,
    pub dir: f32,
    pub velocity_scaled: bool,
    pub enabled: bool,
}

impl SoftLimit {
    pub fn set_limit(&mut self, angle: f32, dir: f32, velocity_scaled: bool) {
        (self.angle, self.dir, self.velocity_scaled) = (angle, dir.signum(), velocity_scaled);
    }

    pub fn set_limit_angle(&mut self, angle: f32) {
        self.angle = angle;
    }

    pub fn limit(&self) -> f32 {
        self.angle.abs()
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn accel(&self, at: f32, v: f32) -> f32 {
        let err = at - self.angle;
        if !self.enabled || err * self.dir <= 0.0 {
            return 0.0;
        }
        let scale = if self.velocity_scaled { 1.0 + (v * self.dir).max(0.0) } else { 1.0 };
        -(self.stiffness * err + 2.0 * self.damping * self.stiffness.max(0.0).sqrt() * v) * scale
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Effector {
    pub desired: [f32; 3],
    pub zero: [f32; 3],
    pub strength: f32,
    pub damping: f32,
    pub muscle_stiffness: f32,
    pub oppose_gravity: f32,
    pub one: bool,
    pub soft: [SoftLimit; 3],
    pub injury: f32,
}

impl Effector {
    pub(crate) fn new(j: &JointDef, now: [f32; 3]) -> Effector {
        Effector { desired: [0.0; 3], zero: now, strength: j.strength, damping: j.damping, muscle_stiffness: j.muscle_stiffness, oppose_gravity: 0.0, one: !j.three, soft: Default::default(), injury: 0.0 }
    }

    pub fn set_stiffness(&mut self, s: f32, d: f32, ms: Option<f32>) {
        self.strength = s * s;
        self.damping = 2.0 * s * d;
        if let Some(ms) = ms {
            self.muscle_stiffness = ms;
        }
    }

    pub fn set_relaxation(&mut self, j: &JointDef, x: f32) {
        self.strength = j.strength * x * x;
        self.damping = j.damping * x;
    }

    pub fn set_damping_relaxation(&mut self, j: &JointDef, x: f32) {
        self.damping = j.damping * x;
    }

    pub fn reset(&mut self, j: &JointDef) {
        (self.muscle_stiffness, self.strength, self.damping, self.oppose_gravity) = (j.muscle_stiffness, j.strength, j.damping, 0.0);
    }

    pub fn blend_to_zero(&mut self, k: f32) {
        let k = k.clamp(0.0, 1.0);
        for i in 0..3 {
            self.desired[i] = if self.one { (1.0 - k).fma(self.desired[i], self.zero[i] * k) } else { self.zero[i].fma(k, self.desired[i] * (1.0 - k)) };
        }
    }
}

#[derive(Clone)]
pub struct Character {
    pub joints: Vec<JointDef>,
    pub eff: Vec<Effector>,
    pub rng: (u32, u32),
    pub busy: [bool; 2],
    pub stabilise: Option<(f32, f32)>,
    pub foot_yaw: [f32; 2],
    pub attached: Option<(usize, Vec3, f32)>,
    pub agent: u32,
    pub inst_id: i32,
    pub mode: u32,
    pub block: Option<crate::nmdbm::Block>,
    pub read: Vec<[f32; 3]>,
    pub limb: [Option<crate::nmdbm::M34>; 4],
    pub refs: Option<Vec<[Vec3; 4]>>,
    pub support: [i32; 2],
    pub stance_yaw: (f32, f32),
    pub up: Vec3,
}

fn targets(j: &JointDef, e: &Effector) -> [f32; 3] {
    let mid = |k: usize| (j.limits[k].0 + j.limits[k].1) * 0.5;
    let half = |k: usize| (j.limits[k].1 - j.limits[k].0) * 0.5;
    let k = 20.0 / (e.strength + 1e-10) + 1.0;
    let flip = |n: usize, x: f32| if j.flip[n] { -x } else { x };
    let (mut d1, mut d2, d3) = (flip(0, e.desired[0]), flip(1, e.desired[1]), flip(2, e.desired[2]));
    let (u, v) = ((d1 - mid(0)) / half(0), (d2 - mid(1)) / half(1));
    let r2 = v.fma(v, u * u);
    if r2 > k {
        let s = k / r2.sqrt();
        (d1, d2) = ((s * u).fma(half(0), mid(0)), (s * v).fma(half(1), mid(1)));
    }
    let tw = d3.clamp(mid(2) - k * half(2), mid(2) + k * half(2));
    [d1 - mid(0), d2 - mid(1), tw - mid(2)]
}

impl Character {
    pub fn new(joints: Vec<JointDef>, frames: &[Transform]) -> Character {
        let mut c = Character { eff: vec![], joints, rng: (0, 0), busy: [false; 2], stabilise: None, foot_yaw: [0.0; 2], attached: None, agent: 0, inst_id: 0, mode: 1, block: None, read: vec![], limb: [None; 4], refs: None, support: [-2; 2], up: Vec3::Z, stance_yaw: (crate::nmdbm::CHAR_YAW.1, crate::nmdbm::CHAR_HEAD) };
        c.seed(0);
        c.eff = (0..c.joints.len()).map(|i| Effector::new(&c.joints[i], c.angle(i, frames))).collect();
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            for (i, (j, e)) in c.joints.iter().zip(&c.eff).enumerate() {
                info!("nm: joint {i} angles {:.2?} flip {:?} limits {:.2?}", e.desired, j.flip, &j.limits[..if j.three { 3 } else { 1 }]);
            }
        });
        c
    }

    pub fn seed(&mut self, s: u32) {
        let s = (s << 3) & 0xFFF8 & 0xFFFF_BEEF;
        self.rng = (s + (s == 0) as u32, s.rotate_left(16) ^ s);
    }

    pub fn random(&mut self) -> f32 {
        (self.random_bits() & 0x7F_FFFF) as f32 / 8_388_608.0
    }

    pub fn random_bits(&mut self) -> u32 {
        let n = self.rng.0 as u64 * 0x5cdc_faa7 + self.rng.1 as u64;
        self.rng = (n as u32, (n >> 32) as u32);
        self.rng.0 & 0x7FFF_FFFF
    }

    pub fn actual(&self, i: usize, frames: &[Transform]) -> [f32; 3] {
        self.read.get(i).copied().unwrap_or_else(|| self.angle(i, frames))
    }

    pub fn angle(&self, i: usize, frames: &[Transform]) -> [f32; 3] {
        self.angle_at(i, frames, None)
    }

    pub fn angle_at(&self, i: usize, frames: &[Transform], twist: Option<f32>) -> [f32; 3] {
        let j = &self.joints[i];
        let rel = (frames[j.parent].rotation * j.rage_parent).inverse() * (frames[j.child].rotation * j.rage_child);
        if j.three {
            let [tw, l1, l2] = angles(rel);
            effector_angles([l1, l2, twist.unwrap_or(tw)], &j.limits, j.flip)
        } else {
            let r = if rel.w < 0.0 { -rel } else { rel };
            [2.0 * r.z.atan2(r.w), 0.0, 0.0]
        }
    }

    pub fn incoming_quat(&self, i: usize, m: &[[Vec3; 3]]) -> Quat {
        let j = &self.joints[i];
        let k = j.rigid.expect("joint constants");
        let t = |r: &[Vec3; 3]| [0, 1, 2].map(|c| crate::rigid::col(r, c));
        let mut tp = t(&m[j.parent]);
        if j.parent == 0 {
            let a = std::f32::consts::FRAC_PI_2 as f64;
            let (c, s) = (a.cos() as f32, a.sin() as f32);
            tp = crate::rigid::rows_mul(&tp, &[Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z]);
        }
        let p = t(&crate::rigid::rows_mul(&tp, &k.pm));
        let c = t(&crate::rigid::rows_mul(&t(&m[j.child]), &k.cm));
        let r = [0, 1, 2].map(|a| [0, 1, 2].map(|b| c[a].x.mul_add(p[b].x, c[a].y.mul_add(p[b].y, c[a].z * p[b].z))));
        mat_quat(&r)
    }

    pub fn incoming_angle(&self, i: usize, m: &[[Vec3; 3]]) -> [f32; 3] {
        let j = &self.joints[i];
        let [tw, l1, l2] = angles(self.incoming_quat(i, m));
        if j.three { effector_angles([l1, l2, tw], &j.limits, j.flip) } else { [tw, 0.0, 0.0] }
    }

    pub fn pose(&self, build: &Build, root: Transform, angles: &[[f32; 3]]) -> Vec<Transform> {
        let mut out = vec![root; build.parts.len()];
        for (c, p) in build.parts.iter().enumerate() {
            let (Some(par), Some(i)) = (p.parent, self.joints.iter().position(|j| j.child == c)) else { continue };
            let (j, a) = (&self.joints[i], angles[i]);
            let rel = if j.three {
                let mid = |k: usize| (j.limits[k].0 + j.limits[k].1) * 0.5;
                let flip = |n: usize, x: f32| if j.flip[n] { -x } else { x };
                rotation([flip(2, a[2]) - mid(2), flip(0, a[0]) - mid(0), flip(1, a[1]) - mid(1)])
            } else {
                rotation([a[0], 0.0, 0.0])
            };
            let q = (out[par].rotation * j.rage_parent * rel * j.rage_child.inverse()).normalize();
            let pivot = out[par].translation + out[par].rotation * build.parts[c].anchor_parent;
            out[c] = Transform { translation: pivot - q * build.parts[c].anchor, rotation: q, ..root };
        }
        out
    }

    pub fn mask(mask: &str) -> Vec<usize> {
        let b = mask.as_bytes();
        let (p, s) = (b.first().copied().unwrap_or(b'f'), b.get(1).copied().unwrap_or(b'b'));
        let has = |set: &[u8]| set.contains(&s);
        let mut out = vec![];
        match p {
            b'f' if s == b'b' => out.extend(0..20),
            b'u' => {
                if s == b'w' { out.extend(WRIST); }
                if s == b'c' { out.extend(CLAVICLE); }
                if has(b"abtl") { out.extend([CLAVICLE[0], ELBOW[0], SHOULDER[0], WRIST[0]]); }
                if has(b"abtr") { out.extend([CLAVICLE[1], ELBOW[1], SHOULDER[1], WRIST[1]]); }
                if has(b"sbtk") { out.extend(SPINE); }
                if has(b"nbk") { out.extend(NECK); }
            }
            b'l' => {
                if has(b"lb") { out.extend([KNEE[0], HIP[0], ANKLE[0]]); }
                if has(b"rb") { out.extend([KNEE[1], HIP[1], ANKLE[1]]); }
            }
            _ => {}
        }
        out
    }

    pub fn lock(&self, mask: &str) -> Option<Vec<Effector>> {
        (mask != "fb").then(|| self.eff.clone())
    }

    pub fn unlock(&mut self, mask: &str, keep: Option<Vec<Effector>>) {
        let open = Self::mask(mask);
        for (i, e) in keep.into_iter().flatten().enumerate() {
            if !open.contains(&i) {
                self.eff[i] = e;
            }
        }
    }

    pub fn unlocked(flags: u8) -> u8 {
        flags & !0x20
    }

    pub fn limits(&self, j: usize) -> [f32; 6] {
        let jd = &self.joints[j];
        let l = loose(j, jd.limits);
        let f = |k: usize| if jd.flip[k] { (-l[k].1, -l[k].0) } else { l[k] };
        let ((a, b), (c, d), (e, g)) = (f(0), f(1), f(2));
        [a, b, c, d, e, g]
    }

    pub fn set_relative(&mut self, j: usize, k: usize, x: f32) {
        let l = self.limits(j);
        let (lo, hi) = (l[2 * k], l[2 * k + 1]);
        self.eff[j].desired[k] = ((hi - lo) * 0.5).fma(x, (hi + lo) * 0.5);
    }

    pub fn set_stiffness(&mut self, s: f32, d: f32, mask: &str, ms: Option<f32>) {
        for i in Self::mask(mask) {
            self.eff[i].set_stiffness(s, d, ms);
        }
    }

    pub fn muscles(&mut self, root: Muscle) -> Vec<Muscle> {
        let mut m = vec![root; self.joints.len() + 1];
        for (i, (j, e)) in self.joints.iter().zip(self.eff.iter_mut()).enumerate() {
            let mid = |k: usize| (j.limits[k].0 + j.limits[k].1) * 0.5;
            let half = |k: usize| (j.limits[k].1 - j.limits[k].0) * 0.5;
            let rel = if j.three {
                let [l1, l2, tw] = targets(j, e);
                rotation([tw, l1, l2])
            } else {
                let d = e.desired[0].clamp((-half(0)).fma(1.6, mid(0)), half(0).fma(1.6, mid(0)));
                e.desired[0] = d;
                rotation([d, 0.0, 0.0])
            };
            let s = e.strength.max(0.0).sqrt();
            e.damping = e.damping.max(MIN_DAMPING);
            m[i + 1] = Muscle { target: j.rage_parent * rel * j.rage_child.inverse(), stiffness: s,
                                damping: if s > 0.0 { e.damping / (2.0 * s) } else { 0.0 } };
        }
        m
    }

    pub fn rigid(&self, dt: f32, k: f32, most: f32) -> Vec<Rigid> {
        let mut out = vec![Rigid::default(); self.joints.len() + 1];
        for (i, (j, e)) in self.joints.iter().zip(&self.eff).enumerate() {
            let well = 1.0 - e.injury;
            let (strength, damping) = (e.strength * well * k, (e.damping * well * k).max(MIN_DAMPING));
            let x = damping.fma(8.0, strength) * e.muscle_stiffness * dt * 0.5;
            let blend = (6.0 * dt).max(x / (x + 1.0)).clamp(0.0, 1.0);
            let target = if j.three { targets(j, e) } else { [e.desired[0], 0.0, 0.0] };
            let soft = std::array::from_fn(|k| {
                let (l, f) = (e.soft[k], j.three && j.flip[k]);
                let mid = if j.three { (j.limits[k].0 + j.limits[k].1) * 0.5 } else { 0.0 };
                SoftLimit { angle: if f { -l.angle } else { l.angle } - mid, dir: if f { -l.dir } else { l.dir }, ..l }
            });
            out[i + 1] = Rigid { frame: j.rage_parent, child: j.rage_child, limits: loose(i, j.limits), max: j.max_torque.map(|m| m * k * most), blend, hinge: !j.three, desired: e.desired[0], jc: j.rigid,
                                target, gain: [strength, damping], soft };
        }
        out
    }
}

