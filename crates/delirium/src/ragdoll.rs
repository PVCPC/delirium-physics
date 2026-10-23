use crate::{Fma, Vd, Vx};
use bevy_math::{DMat3, DVec3};
use crate::prelude::*;

use crate::artic::{self, Artic, V6, v6};
use crate::collision::Collision;
use crate::gjk;
use crate::nmdbm::sdot;

pub const GRAVITY: f32 = 9.8;
const MARGIN: f32 = 0.04;
const WORLD_MARGIN: f32 = 0.005;
fn part_friction(p: &PartDef) -> f32 {
    if p.name.ends_with("Foot") { 1.0 } else if p.name.ends_with("Hand") { 2.25 } else { 0.75 }
}
const PART_ELASTICITY: f32 = 0.4;
const ELASTICITY: f32 = 0.1;
pub static SURFACES: std::sync::RwLock<Vec<[f32; 2]>> = std::sync::RwLock::new(Vec::new());
const SLEEP_V2: f32 = 0.01;
const SLEEP_W2: f32 = 0.005;
const SLEEP_MEAN2: f32 = 0.2;
const SLEEP_STOP: u32 = 15;
const SLEEP_SLEEP: u32 = 20;

#[derive(Clone, Copy, Debug)]
pub enum Limit {
    Cone { swing: f32, twist: f32 },
    Hinge { axis: Vec3, lo: f32, hi: f32, twist: f32 },
    Bone { pf: Quat, cf: Quat, lean: [f32; 2], twist: f32 },
    BoneHinge { pf: Quat, cf: Quat, lo: f32, hi: f32 },
    Locked,
}

impl Limit {
    pub fn from_bone(flags: u32, lo: [f32; 3], hi: [f32; 3], parent: Quat, child: Quat) -> Limit {
        let free: Vec<usize> = (0..3).filter(|&k| flags & (0x10 << k) != 0 && flags & (0x2 << k) != 0 && lo[k] != hi[k]).collect();
        let e = [Vec3::X, Vec3::Y, Vec3::Z];
        let pi = std::f32::consts::PI;
        match free.as_slice() {
            [] => Limit::Locked,
            &[a] => {
                let f = Quat::from_rotation_arc(Vec3::Z, e[a]);
                let (lo, hi) = if lo[a] > pi || hi[a] > pi { (lo[a] - 2.0 * pi, hi[a] - 2.0 * pi) } else { (lo[a], hi[a]) };
                Limit::BoneHinge { pf: parent * f, cf: child * f, lo, hi }
            }
            _ => {
                let t = if free.len() == 3 { 2 } else { (0..3).find(|k| !free.contains(k)).unwrap() };
                let (x, y) = ((t + 1) % 3, (t + 2) % 3);
                let f = Quat::from_mat3(&Mat3::from_cols(e[x], e[y], e[t]));
                let half = |k: usize| { let h = (hi[k] - lo[k]) * 0.5; if h >= pi { 0.9 * pi } else { h } };
                let twist = if free.len() == 3 { if (hi[2] - lo[2]) * 0.5 >= pi { f32::MAX } else { ((hi[2] - lo[2]) * 0.5).max(0.01) } } else { 0.01 };
                let c = |k: usize| if free.contains(&k) { (lo[k] + hi[k]) * 0.5 } else { 0.0 };
                let centre = Quat::from_rotation_z(c(2)) * Quat::from_rotation_y(c(1)) * Quat::from_rotation_x(c(0));
                Limit::Bone { pf: parent * centre * f, cf: child * f, lean: [half(y), half(x)], twist }
            }
        }
    }
}

#[derive(Clone)]
pub struct Build {
    pub parts: Vec<PartDef>,
    pub nm: usize,
}

#[derive(Clone)]
pub struct PartDef {
    pub bone: usize,
    pub parent: Option<usize>,
    pub mass: f32,
    pub radius: f32,
    pub half: f32,
    pub offset: Transform,
    pub rest: Quat,
    pub anchor_parent: Vec3,
    pub anchor: Vec3,
    pub limit: Limit,
    pub lower: bool,
    pub reach: bool,
    pub name: String,
    pub bind: Quat,
    pub boxed: Option<Vec3>,
    pub inertia: Vec3,
    pub size: f32,
    pub heft: f32,
    pub grown: f32,
}

impl PartDef {
    fn moments(&self) -> Vec3 {
        if self.inertia.min_element() > 0.0 {
            return self.inertia;
        }
        let (m, r, l) = (self.mass.max(0.1), self.radius, 2.0 * (self.half + self.radius));
        match self.boxed {
            Some(e) => Vec3::new(e.y * e.y + e.z * e.z, e.x * e.x + e.z * e.z, e.x * e.x + e.y * e.y) * (m / 3.0),
            None => Vec3::new(m * (3.0 * r * r + l * l) / 12.0, 0.5 * m * r * r, m * (3.0 * r * r + l * l) / 12.0),
        }
    }
}

impl Build {
    pub fn undump(text: &str) -> (Build, Vec<Transform>) {
        let mut parts = vec![];
        let mut frames = vec![];
        let mut nm = crate::nmrs::HOMER;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            if let Some(k) = line.strip_prefix("nm ") {
                nm = k.trim().parse().unwrap();
                continue;
            }
            let w: Vec<&str> = line.split_whitespace().collect();
            let f = |i: usize| w[i].parse::<f32>().unwrap();
            let v = |i: usize| Vec3::new(f(i), f(i + 1), f(i + 2));
            let q = |i: usize| Quat::from_xyzw(f(i), f(i + 1), f(i + 2), f(i + 3));
            let limit = match w.iter().position(|&t| t == "j") {
                Some(k) if w[k + 1] == "b" => Limit::Bone { pf: q(k + 2), cf: q(k + 6), lean: [f(k + 10), f(k + 11)], twist: f(k + 12) },
                Some(k) if w[k + 1] == "h" => Limit::BoneHinge { pf: q(k + 2), cf: q(k + 6), lo: f(k + 10), hi: f(k + 11) },
                Some(_) => Limit::Locked,
                None if w[22] == "c" => Limit::Cone { swing: f(23), twist: f(24) },
                None => Limit::Hinge { axis: v(23), lo: f(26), hi: f(27), twist: f(28) },
            };
            parts.push(PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: parts.len(), parent: w[1].parse::<i32>().ok().filter(|&j| j >= 0).map(|j| j as usize), mass: f(2), radius: f(3), half: f(4),
                                 offset: Transform::from_translation(v(5)).with_rotation(q(8)), rest: q(12), anchor_parent: v(16), anchor: v(19),
                                 limit, lower: w[29] == "1", reach: w[30] == "1", name: w[0].to_string(),
                                 bind: w.iter().position(|&t| t == "B").map_or(q(34), |k| q(k + 1)), boxed: (w.get(38) == Some(&"b")).then(|| v(39)),
                                 inertia: w.iter().position(|&t| t == "i").map_or(Vec3::ZERO, |k| v(k + 1)) });
            frames.push(Transform::from_translation(v(31)).with_rotation(q(34)));
        }
        let mut build = Build { parts, nm };
        crate::nmrs::asset_layout(&mut build, &mut frames);
        (build, frames)
    }

    pub fn dump(&self, frames: &[Transform]) -> String {
        let v = |x: Vec3| format!("{} {} {}", x.x, x.y, x.z);
        let q = |x: Quat| format!("{} {} {} {}", x.x, x.y, x.z, x.w);
        self.parts.iter().zip(frames).map(|(p, f)| {
            let limit = match p.limit {
                Limit::Cone { swing, twist } => format!("c {swing} {twist} 0 0 0 0"),
                Limit::Hinge { axis, lo, hi, twist } => format!("h {} {lo} {hi} {twist}", v(axis)),
                _ => "c 0 0 0 0 0 0".into(),
            };
            let bone = match p.limit {
                Limit::Bone { pf, cf, lean, twist } => format!(" j b {} {} {} {} {twist}", q(pf), q(cf), lean[0], lean[1]),
                Limit::BoneHinge { pf, cf, lo, hi } => format!(" j h {} {} {lo} {hi} 0", q(pf), q(cf)),
                Limit::Locked => " j l 0 0 0 1 0 0 0 1 0 0 0".into(),
                _ => String::new(),
            };
            format!("{} {} {} {} {} {} {} {} {} {} {} {} {} {} {}{} i {}", p.name, p.parent.map_or(-1, |j| j as i32), p.mass, p.radius, p.half,
                    v(p.offset.translation), q(p.offset.rotation), q(p.rest), v(p.anchor_parent), v(p.anchor), limit,
                    p.lower as u8, p.reach as u8, v(f.translation), q(f.rotation), p.boxed.map_or(String::new(), |e| format!(" b {}", v(e))), v(p.inertia)) + &bone
                    + &format!(" B {}\n", q(p.bind))
        }).collect::<String>() + &if self.nm == crate::nmrs::HOMER { String::new() } else { format!("nm {}\n", self.nm) }
    }

    pub fn fit(&self, frames: &[Transform], joints: &[Vec3]) -> (Build, Vec<Transform>) {
        let n = self.parts.len();
        let pivot = |k: usize| match self.parts[k].parent {
            Some(_) => frames[k].translation + frames[k].rotation * self.parts[k].anchor,
            None => frames[k].translation,
        };
        let basis = |side: Vec3, up: Vec3| {
            let x = side.normalize_or(Vec3::X);
            let z = (up - x * up.dot(x)).normalize_or(Vec3::Z);
            Quat::from_mat3(&Mat3::from_cols(x, z.cross(x), z)).normalize()
        };
        let (mut size, mut turn) = (vec![1.0f32; n], vec![Quat::IDENTITY; n]);
        for k in 0..n {
            let kids: Vec<usize> = (0..n).filter(|&c| self.parts[c].parent == Some(k)).collect();
            let (s0, t0) = self.parts[k].parent.map_or((1.0, Quat::IDENTITY), |p| (size[p], turn[p]));
            let named = |tag: &str| kids.iter().copied().find(|&c| self.parts[c].name.contains(tag));
            let middle = kids.iter().copied().find(|&c| !self.parts[c].name.contains("_L_") && !self.parts[c].name.contains("_R_"));
            (size[k], turn[k]) = match kids[..] {
                [] => (s0, t0),
                [c] => {
                    let (ours, theirs) = (pivot(c) - pivot(k), joints[c] - joints[k]);
                    let arc = Quat::from_rotation_arc((t0 * ours).normalize_or(Vec3::Z), theirs.normalize_or(Vec3::Z));
                    (theirs.length() / ours.length().max(1e-4), (arc * t0).normalize())
                }
                _ => {
                    let spread = |at: &dyn Fn(usize) -> Vec3| kids.iter().map(|&c| (at(c) - at(k)).length()).sum::<f32>();
                    let s = spread(&|c| joints[c]) / spread(&pivot).max(1e-4);
                    let t = match (named("_L_"), named("_R_"), middle) {
                        (Some(l), Some(r), Some(m)) => {
                            let b = |at: &dyn Fn(usize) -> Vec3| basis(at(l) - at(r), at(m) - (at(l) + at(r)) * 0.5);
                            (b(&|c| joints[c]) * b(&pivot).inverse()).normalize()
                        }
                        _ => t0,
                    };
                    (s, t)
                }
            };
        }
        let mut build = self.clone();
        for (k, p) in build.parts.iter_mut().enumerate() {
            let s = size[k];
            (p.size, p.radius, p.half, p.anchor) = (p.size * s, p.radius * s, p.half * s, p.anchor * s);
            p.offset.translation *= s;
            (p.boxed, p.inertia) = (p.boxed.map(|e| e * s), p.inertia * s * s);
            p.anchor_parent *= p.parent.map_or(1.0, |q| size[q]);
        }
        let mut out: Vec<Transform> = frames.iter().zip(&turn).map(|(f, t)| Transform::from_rotation((*t * f.rotation).normalize())).collect();
        out[0].translation = joints[0];
        for c in 1..n {
            if let Some(p) = build.parts[c].parent {
                let at = out[p].translation + out[p].rotation * build.parts[c].anchor_parent;
                out[c].translation = at - out[c].rotation * build.parts[c].anchor;
            }
        }
        (build, out)
    }

    pub fn scaled(&self, frames: &[Transform], len: &[f32], m: f32) -> (Build, Vec<Transform>) {
        let pivot = |k: usize| self.parts[k].parent.map_or(frames[k].translation, |_| frames[k].translation + frames[k].rotation * self.parts[k].anchor);
        let mut joints = vec![frames[0].translation; self.parts.len()];
        for (c, p) in self.parts.iter().enumerate() {
            if let Some(q) = p.parent {
                joints[c] = joints[q] + (pivot(c) - pivot(q)) * len[q];
            }
        }
        let (mut b, f) = self.fit(frames, &joints);
        let w: f32 = self.parts.iter().map(|p| p.mass).sum();
        let v = self.parts.iter().zip(&b.parts).map(|(a, p)| a.mass * p.size.powi(3)).sum::<f32>() / w;
        let g = (m / v).sqrt();
        let legs = b.parts.iter().find(|p| p.lower && p.parent == Some(0)).map_or(1.0, |p| p.size);
        for p in &mut b.parts {
            (p.heft, p.grown, p.radius) = (p.heft * g * g, p.grown * legs, p.radius * g);
        }
        (b, f)
    }

    pub fn new(parts: &[delirium_data::ragdoll::Part], bind: &[Transform], bones: &[delirium_data::skeleton::Bone], nm: usize) -> Build {
        let linked = parts.len() == 21 && crate::nmrs::PARTS[nm].get().is_some_and(|p| p.len() == parts.len());
        let frames: Vec<Transform> = parts.iter().enumerate().map(|(i, p)| {
            let [x, y, z, t] = p.frame;
            let m = Mat3::from_cols(Vec3::from(x), Vec3::from(y), Vec3::from(z));
            let link = if linked { crate::nmdbm::link(nm, i).inverse() } else { Quat::IDENTITY };
            Transform { translation: Vec3::from(t), rotation: (Quat::from_mat3(&m).normalize() * link).normalize(), scale: Vec3::ONE }
        }).collect();
        let pelvis = 0;
        let defs = parts.iter().enumerate().map(|(i, p)| {
            let parent = p.parent.or((i != pelvis).then_some(pelvis));
            let f = frames[i];
            let joint = bind[p.bone].translation;
            let (rest, anchor_parent) = parent.map_or((Quat::IDENTITY, Vec3::ZERO), |q| {
                let pf = frames[q];
                (f.rotation.inverse() * pf.rotation, pf.rotation.inverse() * (joint - pf.translation))
            });
            let bone = &bones[p.bone];
            let name = bone.name.as_str();
            let limit = parent.map_or(Limit::Locked, |q| Limit::from_bone(bone.flags, bone.rot_min, bone.rot_max,
                frames[q].rotation.inverse() * bind[p.bone].rotation, f.rotation.inverse() * bind[p.bone].rotation));
            let (radius, half) = if p.kind == 3 { (p.extent.iter().copied().fold(f32::MAX, f32::min), 0.0) } else { (p.radius, p.half) };
            PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: p.bone, parent, mass: p.mass, radius, half,
                      offset: Transform::from_matrix(bind[p.bone].to_matrix().inverse() * f.to_matrix()),
                      rest, anchor_parent, anchor: f.rotation.inverse() * (joint - f.translation), limit,
                      lower: i == pelvis || ["Thigh", "Calf", "Foot"].iter().any(|l| name.ends_with(l)),
                      reach: name.ends_with("UpperArm") || name.ends_with("Forearm"), name: name.to_string(), bind: f.rotation,
                      boxed: (p.kind == 3).then(|| Vec3::from(p.extent)), inertia: Vec3::from(p.inertia) }
        }).collect();
        Build { parts: defs, nm }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Muscle {
    pub target: Quat,
    pub stiffness: f32,
    pub damping: f32,
}

impl Muscle {
}

#[derive(Clone, Default)]
pub struct Kept {
    pub muscles: Vec<Muscle>,
    pub lift: f32,
    pub accel: Vec3,
    pub rigid: Vec<Rigid>,
    pub torque: Vec<Vec3>,
    pub force: Vec<Vec3>,
}

impl Kept {
    pub fn drive(&self) -> Drive<'_> {
        Drive { muscles: &self.muscles, lift: self.lift, accel: self.accel, rigid: &self.rigid, torque: &self.torque, force: &self.force }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Drive<'a> {
    pub muscles: &'a [Muscle],
    pub lift: f32,
    pub accel: Vec3,
    pub rigid: &'a [Rigid],
    pub torque: &'a [Vec3],
    pub force: &'a [Vec3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rigid {
    pub frame: Quat,
    pub child: Quat,
    pub limits: [(f32, f32); 3],
    pub max: [f32; 2],
    pub blend: f32,
    pub hinge: bool,
    pub desired: f32,
    pub jc: Option<crate::rigid::JointConst>,
    pub target: [f32; 3],
    pub gain: [f32; 2],
    pub soft: [crate::nmrs::SoftLimit; 3],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Obstacle {
    pub id: i32,
    pub at: Vec3,
    pub rot: Quat,
    pub centre: Vec3,
    pub half: Vec3,
    pub vel: Vec3,
    pub spin: Vec3,
    pub com: Vec3,
    pub parts: &'static [ObPart],
    pub last: Option<(Vec3, Quat)>,
}

#[derive(Clone, Copy, Debug)]
pub struct ObPart {
    pub shape: gjk::Shape,
    pub m: Mat3,
    pub at: Vec3,
    pub lo: Vec3,
    pub hi: Vec3,
    pub mesh: Option<&'static crate::sphere::Mesh>,
}

impl Obstacle {
    pub fn sphere(&self, c: Vec3, r: f32) -> Option<crate::sphere::Hit> {
        if self.parts.iter().all(|p| p.mesh.is_none()) {
            return None;
        }
        let (inst, ms): ([Vec3; 4], Vec<[Vec3; 4]>) = ([self.rot * Vec3::X, self.rot * Vec3::Y, self.rot * Vec3::Z, self.at],
                                                       self.parts.iter().map(|p| [p.m.x_axis, p.m.y_axis, p.m.z_axis, p.at]).collect());
        let parts: Vec<_> = self.parts.iter().zip(&ms).map(|(p, m)| (m, p.mesh)).collect();
        crate::sphere::probe(&inst, &parts, c, r)
    }

    pub fn find(obstacles: &[Obstacle], id: i32) -> Option<&Obstacle> {
        if id < 0 { None } else { obstacles.iter().find(|o| o.id == id) }
    }

    pub fn component(&self, k: usize) -> (Vec3, Mat3) {
        self.parts.get(k).map_or((self.at, Mat3::from_quat(self.rot)), |c| (c.at + self.at, c.m.transpose()))
    }

    pub fn to_world(&self, p: Vec3) -> Vec3 {
        self.at + self.rot * p
    }

    pub fn vel_at(&self, p: Vec3) -> Vec3 {
        let (r, w) = (p - self.com, self.spin);
        (-r.yzx()).mul_add(w.zxy(), r.zxy() * w.yzx()) + self.vel
    }
}
const CAR_FRICTION: f32 = 0.2;

pub type Capsule = (Vec3, Vec3, f32, Vec3, bool);

pub fn nearest_point(a: Vec3, b: Vec3, x: Vec3) -> (Vec3, f32) {
    let d = b - a;
    let t = if d.length_squared() > 1e-9 { ((x - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t, t)
}

fn nearest(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (a1 - a0, b1 - b0, a0 - b0);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (c, b) = (d1.dot(r), d1.dot(d2));
    let den = a * e - b * b;
    let mut s = if den > 1e-9 { ((b * f - c * e) / den).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = if e > 1e-9 { (b * s + f) / e } else { 0.0 };
    if t < 0.0 {
        (t, s) = (0.0, if a > 1e-9 { (-c / a).clamp(0.0, 1.0) } else { 0.0 });
    } else if t > 1.0 {
        (t, s) = (1.0, if a > 1e-9 { ((b - c) / a).clamp(0.0, 1.0) } else { 0.0 });
    }
    (a0 + d1 * s, b0 + d2 * t)
}

#[derive(Clone)]
struct Body {
    x: Vec3,
    r: [Vec3; 3],
    q: Quat,
    v: Vec3,
    w: Vec3,
    m: f32,
    i: Vec3,
    l: Quat,
    att: [Vec3; 3],
}

impl Body {
    fn set_q(&mut self, q: Quat) {
        let m = Mat3::from_quat(q * self.l);
        (self.q, self.r) = (q, [m.row(0), m.row(1), m.row(2)]);
    }

    fn set_r(&mut self, r: [Vec3; 3]) {
        self.r = r;
        self.q = (Quat::from_mat3(&Mat3::from_cols(r[0], r[1], r[2]).transpose()) * self.l.inverse()).normalize();
    }

    fn link(&self) -> crate::rigid::Link {
        crate::rigid::Link { r: self.r, d: self.x, m: self.m, v: self.v, w: self.w }
    }

    fn moved(&mut self, v: Vec3, w: Vec3, h: f32) {
        crate::rigid::integrate(&mut self.r, &mut self.x, v, w, h);
        self.set_r(self.r);
    }

    fn bound(&self) -> [Vec3; 3] {
        crate::rigid::rows_mul(&self.att, &[0, 1, 2].map(|k| crate::rigid::col(&self.r, k)))
    }

    fn cap(&self) -> Quat {
        Quat::from_mat3(&turn(&self.bound())).normalize()
    }

    fn tensor(&self) -> DMat3 {
        let r = DMat3::from_cols(self.r[0].as_dvec3(), self.r[1].as_dvec3(), self.r[2].as_dvec3()).transpose();
        r * DMat3::from_diagonal(self.i.as_dvec3()) * r.transpose()
    }
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Ball { lean: [f32; 2], twist: f32 },
    Hinge { lo: f32, hi: f32 },
}

#[derive(Clone, Copy, Debug)]
struct Joint {
    parent: usize,
    pf: Quat,
    cf: Quat,
    kind: Kind,
    stiff: f32,
    mode: u32,
    jc: crate::rigid::JointConst,
}

#[derive(Clone, Copy, Debug)]
pub struct JointInfo {
    pub parent: usize,
    pub hinge: bool,
    pub in_parent: Quat,
    pub in_child: Quat,
    pub limits: [f32; 3],
    pub anchor_parent: Vec3,
    pub anchor_child: Vec3,
}

pub fn joint_info(build: &Build, k: usize, rigid: Option<&Rigid>) -> Option<JointInfo> {
    let p = build.parts.get(k)?;
    let j = Joint::new(p, p.parent?, rigid);
    let (hinge, limits) = match j.kind {
        Kind::Ball { lean, twist } => (false, [lean[0], lean[1], twist]),
        Kind::Hinge { lo, hi } => (true, [lo, hi, 0.0]),
    };
    Some(JointInfo { parent: j.parent, hinge, in_parent: j.pf, in_child: j.cf, limits, anchor_parent: p.anchor_parent, anchor_child: p.anchor })
}

impl Joint {
    fn new(p: &PartDef, parent: usize, rigid: Option<&Rigid>) -> Joint {
        let (pf, cf, kind, stiff) = if let Some(r) = rigid.filter(|r| r.limits != [(0.0, 0.0); 3]) {
            let half = |k: usize| (r.limits[k].1 - r.limits[k].0) * 0.5;
            let kind = if r.hinge { Kind::Hinge { lo: r.limits[0].0, hi: r.limits[0].1 } } else { Kind::Ball { lean: [half(0), half(1)], twist: half(2) } };
            (r.frame, r.child, kind, r.blend)
        } else {
            let stiff = rigid.map_or(0.0, |r| r.blend);
            match p.limit {
                Limit::Bone { pf, cf, lean, twist } => (pf, cf, Kind::Ball { lean, twist }, stiff),
                Limit::BoneHinge { pf, cf, lo, hi } => (pf, cf, Kind::Hinge { lo, hi }, stiff),
                Limit::Locked => (p.rest.inverse(), Quat::IDENTITY, Kind::Ball { lean: [0.0; 2], twist: 0.0 }, LOCKED),
                Limit::Cone { swing, twist } => {
                    let z = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
                    (p.rest.inverse() * z, z, Kind::Ball { lean: [swing; 2], twist }, stiff)
                }
                Limit::Hinge { axis, lo, hi, .. } => {
                    let z = Quat::from_rotation_arc(Vec3::Z, axis.normalize());
                    (p.rest.inverse() * z, z, Kind::Hinge { lo, hi }, stiff)
                }
            }
        };
        Joint { parent, pf, cf, kind, stiff, mode: if rigid.is_some() { 3 } else { 0 }, jc: Default::default() }
    }
}

const LOCKED: f32 = 1.0;

fn pair(joints: &[Option<Joint>], k: usize, t: DVec3, lt: &mut [V6]) {
    for c in 0..3 {
        lt[k][3 + c] += t[c];
        if let Some(j) = joints[k] {
            lt[j.parent][3 + c] -= t[c];
        }
    }
}

fn answer(artic: &Artic, k: usize, t: DVec3) -> DVec3 {
    artic.answer(k, t.as_vec3()).as_dvec3()
}

struct Row {
    j: Vec<f64>,
    mj: Vec<f64>,
    eff: f64,
    target: f64,
    lambda: f64,
    kind: RowKind,
    at: Spot,
}

#[derive(Clone, Copy)]
struct Spot {
    at: [usize; 2],
    cut: usize,
}

impl Spot {
    const OWN: Spot = Spot { at: [0, 0], cut: usize::MAX };

    fn of(self, i: usize) -> usize {
        if i < self.cut { self.at[0] + i } else { self.at[1] + i - self.cut }
    }

    fn runs(self, len: usize) -> [(std::ops::Range<usize>, std::ops::Range<usize>); 2] {
        let c = self.cut.min(len);
        [(self.at[0]..self.at[0] + c, 0..c), (self.at[1]..self.at[1] + len - c, c..len)]
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    Push,
    Equal,
    Friction { of: usize, mu: f64 },
}

impl Row {
    fn new(artic: &Artic, j: Vec<f64>, target: f64, kind: RowKind) -> Row {
        let mj = artic.solve_row(&j);
        Row::with(j, mj, target, kind)
    }

    fn with(j: Vec<f64>, mj: Vec<f64>, target: f64, kind: RowKind) -> Row {
        let eff = j.iter().zip(&mj).map(|(a, b)| a * b).sum::<f64>().max(1e-12);
        Row { j, mj, eff, target, lambda: 0.0, kind, at: Spot::OWN }
    }
}

fn frame(n: Vec3) -> [Vec3; 3] {
    let t = if n.x.abs() >= n.y.abs() { Vec3::new(-n.z, 0.0, n.x) } else { Vec3::new(0.0, n.z, -n.y) }.normalize();
    [n, t, n.cross(t)]
}

#[derive(Clone)]
struct Block {
    j: [Vec<f64>; 3],
    mj: [Vec<f64>; 3],
    kinv: DMat3,
    axis: DVec3,
    target: DVec3,
    lambda: DVec3,
    mu: f64,
    held: Option<(usize, usize)>,
    scale: DVec3,
    ob: Option<(i32, usize, Vec3, Vec3)>,
    at: Spot,
}

impl Block {
    fn new(artic: &Artic, j: [Vec<f64>; 3], target: DVec3, mu: f64, held: Option<(usize, usize)>, warm: Vec3, self_contact: bool) -> Block {
        let mj = artic.solve_rows([&j[0][..], &j[1], &j[2]]);
        let on: [Vec<bool>; 3] = std::array::from_fn(|a| if self_contact { vec![] } else { j[a].chunks(6).map(|x| x.iter().any(|&v| v != 0.0)).collect() });
        let ans: [Vec<V6>; 3] = std::array::from_fn(|b| on[0].iter().enumerate().map(|(c, _)| if on[0][c] || on[1][c] || on[2][c] {
            artic.own(c, std::array::from_fn(|i| j[b][6 * c + i]))
        } else { [0.0; 6] }).collect());
        let own = |a: usize, b: usize| -> f64 {
            (0..on[a].len()).filter(|&c| on[a][c] || on[b][c]).map(|c| j[a][6 * c..6 * c + 6].iter().zip(ans[b][c]).map(|(x, y)| x * y).sum::<f64>()).sum()
        };
        let k = DMat3::from_cols_array_2d(&std::array::from_fn(|a| std::array::from_fn(|b| {
            if self_contact { j[b].iter().zip(&mj[a]).map(|(j, m)| j * m).sum() } else { own(a, b) }
        })));
        Block::with(j, mj, k, target, mu, held, warm)
    }

    fn with(j: [Vec<f64>; 3], mj: [Vec<f64>; 3], k: DMat3, target: DVec3, mu: f64, held: Option<(usize, usize)>, warm: Vec3) -> Block {
        Block { j, mj, kinv: ldl_inverse(k), axis: k.row(0) / k.x_axis.x, target, lambda: warm.as_dvec3(), mu, held, scale: DVec3::ONE, ob: None, at: Spot::OWN }
    }

    fn apply(&self, d: DVec3, qd: &mut [f64]) {
        for (q, e) in self.at.runs(self.mj[0].len()) {
            for (((q, x), y), z) in qd[q].iter_mut().zip(&self.mj[0][e.clone()]).zip(&self.mj[1][e.clone()]).zip(&self.mj[2][e]) {
                *q += x * d.x + y * d.y + z * d.z;
            }
        }
    }

    fn solve(&mut self, qd: &mut [f64], nz: &[(usize, usize)]) -> f64 {
        let v = DVec3::from_array(std::array::from_fn(|a| nz.iter().map(|&(i, g)| self.j[a][i] * qd[g]).sum()));
        let mut l = self.lambda - self.kinv * (v - self.target);
        let an = self.axis.dot(l);
        let tt = l.y * l.y + l.z * l.z;
        let g = DVec3::new(-tt.sqrt() * self.mu, l.y, l.z);
        let way = DVec3::new(l.x - an, l.y, l.z);
        let over = g.dot(l);
        if over > 0.0 {
            l -= way * (over / (g.dot(way) + 1e-8));
        }
        if an < 0.0 {
            l = DVec3::ZERO;
        }
        let d = l - self.lambda;
        self.lambda = l;
        self.apply(d, qd);
        d.length_squared()
    }
}

fn ldl_inverse(k: DMat3) -> DMat3 {
    let (a00, a10, a20, a11, a21, a22) = (k.x_axis.x, k.x_axis.y, k.x_axis.z, k.y_axis.y, k.y_axis.z, k.z_axis.z);
    let eps = (a11 + a22 + a00) * 9.999_999_747_378_752e-6;
    let d0 = a00.max(eps);
    let i0 = 1.0 / d0;
    let (l10, l20) = (a10 * i0, a20 * i0);
    let b11 = (-a10).mul_add(l10, a11);
    let b21 = (-a10).mul_add(l20, a21);
    let b22 = (-a20).mul_add(l20, a22);
    let i1 = 1.0 / b11.max(eps);
    let l21 = i1 * b21;
    let m10 = i1 * l10;
    let c22 = (-l21).mul_add(b21, b22);
    let n10 = m10 * l10;
    let p = l21.mul_add(l10, -l20);
    let v22 = 1.0 / c22.max(eps);
    let v12 = -(v22 * l21);
    let v02 = p * v22;
    let v11 = (-v12).mul_add(l21, i1);
    let v01 = -(v02 * l21) - m10;
    let v00 = v02.mul_add(p, n10) + i0;
    DMat3::from_cols(DVec3::new(v00, v01, v02), DVec3::new(v01, v11, v12), DVec3::new(v02, v12, v22))
}

fn impulses(rows: &mut [Row], blocks: &mut [Block], qd: &mut [f64]) {
    let apply = |row: &Row, d: f64, qd: &mut [f64]| {
        for (q, e) in row.at.runs(row.mj.len()) {
            for (q, m) in qd[q].iter_mut().zip(&row.mj[e]) {
                *q += m * d;
            }
        }
    };
    for b in blocks.iter() {
        b.apply(b.lambda, qd);
    }
    let mut flat = Vec::with_capacity(8 * (blocks.len() + rows.len()));
    let mut nz = |j: &[&Vec<f64>], at: Spot| {
        let from = flat.len();
        for c in (0..j[0].len() / 6).filter(|&c| j.iter().fold(0, |a, j| j[6 * c..6 * c + 6].iter().fold(a, |a, x| a | x.to_bits() << 1)) != 0) {
            flat.extend((6 * c..6 * c + 6).filter(|&i| j.iter().any(|j| j[i] != 0.0)).map(|i| (i, at.of(i))));
        }
        from..flat.len()
    };
    let (bnz, rnz): (Vec<_>, Vec<_>) = (blocks.iter().map(|b| nz(&[&b.j[0], &b.j[1], &b.j[2]], b.at)).collect(), rows.iter().map(|r| nz(&[&r.j], r.at)).collect());
    for _ in 0..SWEEPS {
        let mut moved: f64 = blocks.iter_mut().zip(&bnz).map(|(b, nz)| b.solve(qd, &flat[nz.clone()])).sum();
        for r in 0..rows.len() {
            let lo = if matches!(rows[r].kind, RowKind::Push) { 0.0 } else { f64::MIN };
            let row = &mut rows[r];
            let v: f64 = flat[rnz[r].clone()].iter().map(|&(i, g)| row.j[i] * qd[g]).sum();
            let was = row.lambda;
            row.lambda = (was + (row.target - v) / row.eff).max(lo);
            let d = row.lambda - was;
            moved += d * d;
            apply(row, d, qd);
            if let RowKind::Friction { of, mu } = rows[r].kind {
                if r == of + 2 {
                    let (a, b) = (rows[of + 1].lambda, rows[of + 2].lambda);
                    let (len, most) = ((a * a + b * b).sqrt(), mu * rows[of].lambda);
                    if len > most {
                        let k = most / len;
                        for (t, l) in [(of + 1, a), (of + 2, b)] {
                            rows[t].lambda = l * k;
                            apply(&rows[t], l * k - l, qd);
                        }
                    }
                }
            }
        }
        if moved == 0.0 {
            break;
        }
    }
}

const ALLOW: f32 = 0.007;
macro_rules! env_on {
    ($n:literal) => {{ static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new(); *ON.get_or_init(|| std::env::var($n).is_ok()) }};
}
fn elastic(e: f64) -> DVec3 {
    if env_on!("NM_NOSCALE") { return DVec3::ONE; }
    DVec3::splat(1.0 + e)
}
const BOX_MARGIN: f32 = 0.01;

#[derive(Clone, Copy)]
struct Held { local: Vec3, ground: Vec3, gap: f32, age: u16, m: u8, imp: Vec3, pimp: Vec3, n: Vec3, inst: u32, off: bool }
fn hold(held: &mut Vec<Held>, new: Held) {
    if new.gap >= 0.02 {
        return;
    }
    let mut near = (None, 0.0004);
    for (k, r) in held.iter().enumerate() {
        let d = r.local.distance_squared(new.local);
        if d < near.1 && r.age != 0 {
            near = (Some(k), d);
        }
    }
    match near.0 {
        Some(k) => held[k] = Held { imp: held[k].imp, pimp: held[k].pimp, ..new },
        None if held.len() < 6 => held.push(new),
        None => if let Some(k) = replaced(held, new.local) { held[k] = new },
    }
}

fn replaced(kept: &[Held], new: Vec3) -> Option<usize> {
    let mut p: Vec<Vec3> = kept.iter().map(|r| r.local).chain([new]).collect();
    let mut idx: Vec<usize> = (0..p.len()).collect();
    let (len, last) = (p.len(), kept.len());
    let a = p[1] - p[0];
    let mut n = a.cross(p[2] - p[0]);
    for k in 3..last {
        if n.length_squared() >= 1e-6 {
            break;
        }
        n = a.cross(p[k] - p[0]);
    }
    let d = n.cross(a);
    let (mut m, mut bar) = (0, Vec3::splat(1e30));
    for (i, q) in p.iter().enumerate() {
        if d.dot(*q) < bar.max_element() {
            (m, bar) = (i, *q);
        }
    }
    p.swap(0, m);
    idx.swap(0, m);
    for _ in 0..last {
        let mut swapped = false;
        for i in 1..len - 1 {
            if n.dot((p[i] - p[0]).cross(p[i + 1] - p[0])) < 0.0 {
                p.swap(i, i + 1);
                idx.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
    let (mut at, mut least) = (0, 1e30);
    for k in 0..len {
        let (j, l) = ((k + 1) % len, (k + 2) % len);
        let s = n.dot((p[j] - p[k]).cross(p[l] - p[k]));
        if idx[j] == last {
            if s < 0.0 {
                return None;
            }
        } else if s < least {
            (at, least) = (j, s);
        }
    }
    Some(idx[at])
}

fn swept(now: (Vec3, Mat3), last: (Vec3, Mat3), half: Vec3) -> (Vec3, Vec3) {
    let r = now.1.transpose() * last.1;
    let t = now.1.transpose() * (last.0 - now.0);
    let h = Mat3::from_cols(r.x_axis.abs(), r.y_axis.abs(), r.z_axis.abs()) * half;
    let (lo, hi) = ((-half).min(t - h), half.max(t + h));
    ((lo + hi) * 0.5, (hi - lo) * 0.5)
}

fn swept_box(p: &PartDef, f: (Vec3, Mat3), s: (Vec3, Mat3)) -> (Vec3, Vec3) {
    let (o, half) = swept(f, s, bound_half(p));
    (f.0 + f.1 * o, half)
}

fn boxes_meet(ca: Vec3, a: Mat3, ha: Vec3, cb: Vec3, b: Mat3, hb: Vec3) -> bool {
    let ax = [a.x_axis, a.y_axis, a.z_axis];
    let bx = [b.x_axis, b.y_axis, b.z_axis];
    let d = cb - ca;
    let reach = |u: Vec3, ax: &[Vec3; 3], h: Vec3| (0..3).map(|i| ax[i].dot(u).abs() * h[i]).sum::<f32>();
    let crosses = ax.into_iter().flat_map(|u| bx.map(|v| u.cross(v))).filter(|c| c.length_squared() > 1e-10);
    ax.into_iter().chain(bx).chain(crosses).all(|u| d.dot(u).abs() <= reach(u, &ax, ha) + reach(u, &bx, hb))
}

fn box_meets_tri(c: Vec3, m: Mat3, e: Vec3, t: [Vec3; 3]) -> bool {
    let v = t.map(|p| p - c);
    let apart = |a: Vec3| {
        let p = v.map(|x| x.dot(a));
        let r = (m.transpose() * a).abs().dot(e);
        p[0].min(p[1]).min(p[2]) > r || p[0].max(p[1]).max(p[2]) < -r
    };
    ![(v[1] - v[0]).cross(v[2] - v[0]), m.x_axis, m.y_axis, m.z_axis].into_iter().any(apart)
}

fn part_shape(p: &PartDef) -> gjk::Shape {
    match p.boxed {
        Some(e) => gjk::Shape::Box(e - Vec3::splat(BOX_MARGIN), BOX_MARGIN),
        None => gjk::Shape::Capsule(p.half, p.radius),
    }
}

fn bound_half(p: &PartDef) -> Vec3 {
    p.boxed.unwrap_or(Vec3::new(p.radius, p.half + p.radius, p.radius))
}

const PUSH_KEEP: f32 = 0.007;
fn out(gap: f32) -> f32 {
    -gap - (-gap * 0.5).clamp(0.0, PUSH_KEEP)
}

impl Ragdoll {
    fn push_block(b: &Block, over: f32, h: f32, last: Vec3) -> Block {
        Block { target: DVec3::new((out(over) / h) as f64, 0.0, 0.0), lambda: last.as_dvec3(), held: None, scale: DVec3::ONE, ..b.clone() }
    }
}

const SWEEPS: usize = 8;
const MAX_SPEED: f32 = 80.0;
const MAX_SPIN: f32 = std::f32::consts::TAU;
pub(crate) const MAX_SPIN_NM: f32 = 3.0 * std::f32::consts::TAU;
pub(crate) const DAMPING: f32 = 0.1;
const LIMIT_MARGIN: f32 = 0.03;

const LIMIT_GAIN: f32 = 4.0;

fn hinge_limits(lo: f32, hi: f32) -> (f32, f32) {
    let (l, h) = (LIMIT_MARGIN + lo, hi - LIMIT_MARGIN);
    if l > h { ((lo + hi) * 0.5, (lo + hi) * 0.5) } else { (l, h) }
}

fn lean_limit(st: &crate::rigid::Swing, lean: [f32; 2]) -> Option<f32> {
    let [l0, l1] = lean.map(|l| (l - LIMIT_MARGIN).max(0.0));
    let (l0, l1) = (l0.max(1e-4), l1.max(1e-4));
    let (c1, c0) = (sdot(st.s, crate::rigid::col(&st.pr, 1), b"yxz") / l0, sdot(st.s, crate::rigid::col(&st.pr, 0), b"yxz") / l1);
    let e = c0.fma(c0, c1 * c1) * st.theta * st.theta;
    (!(e < 1.0)).then(|| st.theta / e.sqrt())
}

fn twist_limit(st: &crate::rigid::Swing, twist: f32) -> Option<f32> {
    let l = (twist - LIMIT_MARGIN).max(0.0);
    if !(st.twist - l < 0.0) { Some(l) } else if !(l + st.twist > 0.0) { Some(-l) } else { None }
}
pub const SUBSTEPS: usize = 2;
const GHOST: f32 = 0.2;

pub struct Ragdoll {
    pub nm: usize,
    bodies: Vec<Body>,
    order: Vec<usize>,
    pub mode: u8,
    still: [u32; 2],
    pub sleep_tol: [f32; 3],
    pub asleep: bool,
    pub age: f32,
    pub fresh: bool,
    pub touch: Vec<u8>,
    pub contact_off: u32,
    pub hit: Vec<Option<(Vec3, Vec3, i32)>>,
    pub contact: crate::bump::ContactRecord<i32>,
    pub ob_hits: Vec<(i32, Vec3, Vec3)>,
    held: Vec<Vec<Held>>,
    pub stage_vel: Vec<(Vec3, Vec3)>,
    pub stage_lt: Vec<V6>,
    pub stage_lim: Vec<Vec3>,
    stiff: Vec<f32>,
    pins: Vec<Option<Transform>>,
    cons: Vec<(Option<Vec3>, Option<Quat>)>,
    pub grips: Vec<(usize, Vec3, Vec3, f32, f32)>,
    pub snapped: Vec<usize>,
    pub gone: u32,
    grip_rows: Vec<(usize, usize, f32)>,
    pub pairs: Vec<(usize, usize)>,
    seen: Vec<(Vec3, Mat3)>,
    coll: Vec<(Vec3, [Vec3; 3])>,
    held_pairs: Vec<Vec<Held>>,
    held_car: Vec<(usize, i32, usize, Vec<Held>)>,
    pub prep_twist: Vec<Option<f32>>,
    pub rigid: Option<(Vec<crate::rigid::Link>, Vec3, Vec<crate::nmdbm::M34>)>,
    pub inertia: Option<Vec<Vec3>>,
    pub inst: Option<[Vec3; 4]>,
    pending: Vec<(usize, Vec3, Vec3, Vec3)>,
    pub out: Vec<(Vec3, Vec3)>,
    pub joint_torque: Vec<Vec3>,
    pub armature: Vec<f32>,
    o: Vec3,
    pub bound_off: Vec3,
    rejuv: u32,
    mwc: (u32, u32),
}

fn turn(r: &[Vec3; 3]) -> Mat3 {
    Mat3::from_cols(r[0], r[1], r[2])
}

pub fn orthonormalise(r: &mut [Vec3; 3]) {
    let norm = |v: Vec3| {
        let l = v.z.mul_add(v.z, v.x.mul_add(v.x, v.y * v.y));
        if l > 0.0 { v * (1.0 / l.sqrt()) } else { Vec3::X }
    };
    let (a, b) = (norm(r[0]), r[1]);
    let d = -a.x.mul_add(b.x, b.y.mul_add(a.y, b.z * a.z));
    let b = norm(Vec3::new(a.x.mul_add(d, b.x), a.y.mul_add(d, b.y), d.mul_add(a.z, b.z)));
    r[2] = Vec3::new(a.y.mul_add(b.z, -(b.y * a.z)), b.x.mul_add(a.z, -(b.z * a.x)), b.y.mul_add(a.x, -(a.y * b.x)));
    (r[0], r[1]) = (a, b);
}

pub fn start_motion(a: &[Vec3; 4], b: &[Vec3; 4], c: Vec3, inv_dt: f32, scale: f32, max_v: f32, max_w: f32, v_ref: Vec3) -> (Vec3, Vec3) {
    let d4 = |r: Vec3, p: f32| (((c.x as f64 * r.x as f64) + (c.z as f64 * r.z as f64)) + ((c.y as f64 * r.y as f64) + p as f64)) as f32;
    let dp = b[3] - a[3];
    let mv = Vec3::new(d4(b[0] - a[0], dp.x), d4(b[1] - a[1], dp.y), d4(b[2] - a[2], dp.z));
    let col = |m: &[Vec3; 4], i: usize| Vec3::new(m[0][i], m[1][i], m[2][i]);
    let row = |i: usize| Vec3::new(col(b, i).vd(col(a, 0)), col(b, i).vd(col(a, 1)), col(b, i).vd(col(a, 2)));
    let (axis, angle) = crate::nmhf::axis_angle(crate::nmhf::quat_of(&Mat3::from_cols(row(0), row(1), row(2))));
    let v = mv * inv_dt * scale - v_ref;
    (clamp_len(v, max_v) + v_ref, clamp_len(axis * angle * -inv_dt * scale, max_w))
}

fn clamp_len(v: Vec3, max: f32) -> Vec3 {
    let l2 = v.z.mul_add(v.z, v.x.mul_add(v.x, v.y * v.y));
    if l2 <= max * max {
        return v;
    }
    let (x, mut y) = (l2 as f64, crate::estimate::frsqrte(l2 as f64));
    for _ in 0..2 {
        y = (-(y * y)).mul_add(x, 1.0).mul_add(y * 0.5, y);
    }
    v * ((y as f32) * max)
}

fn short(q: Quat) -> Quat {
    if q.w < 0.0 { -q } else { q }
}

impl Ragdoll {
    pub fn new(build: &Build, frames: &[Transform], v: Vec3) -> Ragdoll {
        let o = frames.first().map_or(Vec3::ZERO, |f| f.translation);
        let nm = crate::nmrs::MASSES[build.nm].get().filter(|m| m.len() == build.parts.len());
        let bodies = build.parts.iter().zip(frames).enumerate().map(|(k, (p, f))| {
            let m = p.mass.max(0.1);
            let s = nm.map_or(1.0, |n| n[k] / m);
            let l = nm.map_or(Quat::IDENTITY, |_| crate::nmdbm::rage_link(build.nm, k));
            let z = p.size;
            let att = if nm.is_some() && k == 0 { Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2) } else { Mat3::IDENTITY };
            let att = [att.x_axis, att.y_axis, att.z_axis];
            let i = p.moments();
            let i = if nm.is_some() && k == 0 { Vec3::new(i.y, i.x, i.z) } else { i };
            let mut b = Body { x: f.translation - o, r: [Vec3::ZERO; 3], q: f.rotation, v, w: Vec3::ZERO, m: m * s * z.powi(3) * p.heft, i: i * s * z.powi(3) * p.heft, l, att };
            b.set_q(f.rotation);
            b
        }).collect::<Vec<Body>>();
        let n = bodies.len();
        let mut order: Vec<usize> = (0..n).filter(|&k| build.parts[k].parent.is_none()).collect();
        let mut at = 0;
        while at < order.len() {
            let k = order[at];
            at += 1;
            order.extend((0..n).filter(|&c| build.parts[c].parent == Some(k)));
        }
        Ragdoll { nm: build.nm, pins: vec![None; n], cons: vec![(None, None); n], grips: vec![], snapped: vec![], gone: 0, grip_rows: vec![], touch: vec![0; n], contact_off: 0, hit: vec![None; n], contact: crate::bump::ContactRecord::new(), ob_hits: vec![], held: vec![vec![]; n], stage_vel: vec![], stage_lt: vec![], stage_lim: vec![], stiff: vec![], bodies, order, mode: 1, still: [SLEEP_STOP - 2, SLEEP_SLEEP - 2],
                  sleep_tol: [SLEEP_V2, SLEEP_W2, SLEEP_MEAN2], asleep: false, age: 0.0, fresh: false, pending: vec![], pairs: vec![], seen: vec![], coll: vec![], held_pairs: vec![], held_car: vec![], prep_twist: vec![], rigid: None, inertia: None, inst: None, out: vec![(Vec3::ZERO, Vec3::ZERO); n], joint_torque: vec![Vec3::ZERO; n], armature: vec![0.0; n], o, bound_off: Vec3::ZERO,
                  rejuv: 0, mwc: (1, 0) }
    }

    pub fn set_body(&mut self, k: usize, x: Vec3, q: Quat, v: Vec3, w: Vec3) {
        let b = &mut self.bodies[k];
        (b.x, b.v, b.w) = (x - self.o, v, w);
        b.set_q(q.normalize());
        self.wake();
    }

    pub fn pin(&mut self, k: usize, to: Transform) {
        self.pins[k] = Some(Transform { translation: to.translation - self.o, ..to });
    }

    pub fn constrain(&mut self, k: usize, at: Option<Vec3>, turn: Option<Quat>) {
        self.cons[k] = (at, turn);
        if at.is_some() || turn.is_some() {
            self.wake();
        }
    }

    pub fn restiffen(&mut self) {
        self.stiff.clear();
    }

    pub fn set_stiff(&mut self, stiff: Vec<f32>) {
        self.stiff = stiff;
    }

    pub fn unpin(&mut self) {
        self.pins.iter_mut().for_each(|p| *p = None);
        self.age = 0.0;
    }

    pub fn com(&self) -> (Vec3, Vec3) {
        let (mut m, mut x, mut v) = (0.0, Vec3::ZERO, Vec3::ZERO);
        for b in &self.bodies {
            (m, x, v) = (m + b.m, x + b.x * b.m, v + b.v * b.m);
        }
        (x / m + self.o, v / m)
    }

    pub fn part_mass(&self, k: usize) -> f32 {
        self.bodies[k].m
    }

    pub fn mass(&self) -> f32 {
        self.bodies.iter().map(|b| b.m).sum()
    }

    pub fn kinetic(&self) -> f32 {
        let n4 = self.bodies.len() / 4 * 4;
        let (mut e, mut mass) = (0.0f32, 0.0f32);
        let links = self.rigid.as_ref().map(|r| &r.0);
        for (k, b) in self.bodies.iter().enumerate() {
            let (m, v, w) = match links.and_then(|l| l.get(k)) {
                Some(l) => (l.m, l.v, l.w),
                None => (b.m, b.v, b.w),
            };
            let sq = |v: Vec3| if k < n4 { v.z.fma(v.z, v.x.fma(v.x, v.y * v.y)) } else { v.x.fma(v.x, v.y.fma(v.y, v.z * v.z)) };
            e = (sq(v) * m).fma(0.5, e);
            let i = self.inertia.as_ref().and_then(|i| i.get(k)).copied().unwrap_or(b.i);
            e = (sq(w) * ((i.z + i.y + i.x) * 0.3333)).fma(0.5, e);
            mass += m;
        }
        e / mass
    }

    pub fn collided(&self, parts: &[usize]) -> bool {
        parts.iter().any(|&k| self.touch[k] & 0x28 == 0x20)
    }

    pub fn mobility(&self, build: &Build, rigid: &[Rigid], k: usize) -> [Vec3; 3] {
        let joints: Vec<Option<Joint>> = build.parts.iter().enumerate().map(|(i, p)| p.parent.map(|j| {
            let j = Joint::new(p, j, rigid.get(i));
            Joint { jc: self.jconst(build, i, &j, rigid.get(i)), ..j }
        })).collect();
        let artic = self.artic(&joints);
        [DVec3::X, DVec3::Y, DVec3::Z].map(|e| artic::parts(artic.own(k, v6(DVec3::ZERO, e))).1.as_vec3())
    }

    pub fn vel(&self, k: usize) -> (Vec3, Vec3) { (self.bodies[k].v, self.bodies[k].w) }

    pub fn tensor(&self, k: usize) -> DMat3 {
        self.bodies[k].tensor()
    }

    pub fn inertia(&self, k: usize, axis: Vec3) -> f32 {
        let b = &self.bodies[k];
        let a = (b.q * b.l).inverse() * axis;
        1.0 / a.dot(b.i.recip() * a).max(1e-9)
    }

    pub fn link_rows(&self, k: usize) -> [Vec3; 3] {
        self.bodies[k].r
    }

    pub fn centre(&self, k: usize) -> Vec3 {
        self.bodies[k].x + self.o
    }

    pub fn held_world(&self, i: usize) -> Vec<(Vec3, Vec3, f32, u16)> {
        let (b, m) = (&self.bodies[i], self.turns()[i]);
        self.held[i].iter().map(|r| (b.x - self.bound_off + self.o + m * r.local, r.ground, r.gap, r.age)).collect()
    }

    fn inst(&self) -> [Vec3; 3] {
        let r = &self.bodies[self.order[0]].r;
        let c = crate::rigid::nrm(crate::rigid::col(r, 2));
        let a = crate::rigid::nrm(crate::rigid::col(r, 1).vx(c));
        [a, c.vx(a), c]
    }

    fn comp(&self, inst: &[Vec3; 3]) -> Vec<(Vec3, [Vec3; 3])> {
        let xr = self.bodies[self.order[0]].x;
        self.bodies.iter().map(|b| (crate::rigid::ldot(inst, b.x - xr), b.bound().map(|w| crate::rigid::ldot(inst, w)))).collect()
    }

    fn turns(&self) -> Vec<Mat3> {
        let inst = self.inst();
        self.comp(&inst).iter().map(|(_, c)| turn(&crate::rigid::rows_mul(c, &inst))).collect()
    }

    pub fn motion(&self, k: usize) -> (Vec3, Vec3) {
        (self.bodies[k].v, self.bodies[k].w)
    }

    pub fn spin(&mut self, k: usize, w: Vec3) {
        let b = &self.bodies[k];
        let ang = (b.tensor() * w.as_dvec3()).as_vec3();
        self.pending.push((k, b.x, Vec3::ZERO, ang));
        self.wake();
    }

    pub fn frames(&self) -> impl Iterator<Item = Transform> + '_ {
        self.bodies.iter().map(|b| Transform::from_translation(b.x + self.o).with_rotation(b.q))
    }

    pub fn turned(&self, turn: Quat) -> impl Iterator<Item = Quat> + '_ {
        self.bodies.iter().map(move |b| (Quat::from_mat3(&Mat3::from_quat(turn * b.q * b.l)) * b.l.inverse()).normalize())
    }

    pub fn capsules(&self) -> impl Iterator<Item = Transform> + '_ {
        self.bodies.iter().map(|b| Transform::from_translation(b.x + self.o).with_rotation(b.cap()))
    }

    pub fn sever(&mut self, build: &Build, k: usize) -> Option<(Build, Ragdoll, Vec<usize>)> {
        if build.parts.get(k)?.parent.is_none() || self.gone >> k & 1 != 0 {
            return None;
        }
        let under = |mut at: Option<usize>| { while let Some(a) = at { if a == k { return true; } at = build.parts[a].parent; } false };
        let below: Vec<usize> = self.order.iter().copied().filter(|&c| under(Some(c))).collect();
        let parts = below.iter().map(|&c| PartDef { parent: build.parts[c].parent.and_then(|q| below.iter().position(|&b| b == q)), ..build.parts[c].clone() }).collect();
        let sub = Build { parts, nm: build.nm };
        let frames: Vec<Transform> = self.frames().collect();
        let mut doll = Ragdoll::new(&sub, &below.iter().map(|&c| frames[c]).collect::<Vec<_>>(), Vec3::ZERO);
        for (i, &c) in below.iter().enumerate() {
            let b = &self.bodies[c];
            doll.bodies[i] = Body { x: b.x + self.o - doll.o, ..b.clone() };
        }
        self.ghost(&below);
        doll.wake();
        Some((sub, doll, below))
    }

    pub fn ghost(&mut self, parts: &[usize]) {
        let was = self.gone;
        for &c in parts {
            if self.gone >> c & 1 != 0 {
                continue;
            }
            let b = &mut self.bodies[c];
            (b.m, b.i) = (b.m * GHOST, b.i * GHOST);
            self.held[c].clear();
            self.gone |= 1 << c;
        }
        if self.gone != was {
            let gone = self.gone;
            self.grips.retain(|g| gone >> g.0 & 1 == 0);
            self.wake();
        }
    }

    pub fn nearest(&self, build: &Build, at: Vec3) -> usize {
        let at = at - self.o;
        self.bodies.iter().zip(&build.parts).enumerate().map(|(k, (b, p))| {
            let axis = b.cap() * Vec3::Y;
            let s = (at - b.x).dot(axis).clamp(-p.half, p.half);
            (k, (b.x + axis * s).distance(at) - p.radius)
        }).min_by(|a, b| a.1.total_cmp(&b.1)).map_or(0, |(k, _)| k)
    }

    pub fn probe(&self, build: &Build, a: Vec3, b: Vec3) -> Option<(usize, Vec3, Vec3)> {
        use crate::ragstart::{ProbePart, ProbeShape};
        let r = self.inst();
        let inst = [r[0], r[1], r[2], self.bodies[self.order[0]].x - self.bound_off + self.o];
        let parts: Vec<ProbePart> = self.comp(&r).into_iter().zip(&build.parts).map(|((d, m), p)| ProbePart {
            m: [m[0], m[1], m[2], d],
            cen: Vec3::ZERO,
            shape: p.boxed.map_or(ProbeShape::Capsule { len: 2.0 * p.half, radius: p.radius }, |e| ProbeShape::Box(2.0 * e)),
        }).collect();
        crate::ragstart::probe_parts(&inst, &parts, a, b).map(|h| (h.k, h.at, h.n))
    }

    pub fn push(&mut self, build: &Build, at: Vec3, impulse: Vec3) {
        let k = self.nearest(build, at);
        self.impulse(k, at, impulse);
    }

    pub fn impulse(&mut self, k: usize, at: Vec3, impulse: Vec3) {
        self.pending.push((k, at - self.o, impulse, Vec3::ZERO));
        self.wake();
    }

    pub fn equalized_impulse(&mut self, k: usize, impulse: Vec3, at: Option<Vec3>, equalize: f32) {
        let Some(b) = self.bodies.get(k) else { return };
        let mean = self.mass() / self.bodies.len() as f32;
        let at = at.unwrap_or(b.x + self.o);
        let s = 1.0 - equalize + equalize * b.m / mean;
        self.impulse(k, at, impulse * s);
    }

    pub fn root(&self) -> Vec3 {
        self.bodies[0].x + self.o
    }

    pub fn share(&self, k: usize) -> f32 {
        let mean = self.mass() / self.bodies.len() as f32;
        0.4 + 0.6 * (self.bodies[k].m / mean).clamp(0.2, 1.5)
    }

    pub fn kick(&mut self, v: Vec3) {
        for b in &mut self.bodies {
            b.v += v;
        }
        self.wake();
    }

    fn wake(&mut self) {
        (self.mode, self.asleep) = (1, false);
    }

    pub fn awake(&self) -> bool {
        self.mode == 1
    }

    fn sleep(&mut self) {
        let root = &self.bodies[0];
        let mean = self.bodies.iter().map(|b| b.v.length_squared() + b.w.length_squared()).sum::<f32>() / self.bodies.len() as f32;
        let [v2, w2, mean2] = self.sleep_tol;
        if root.v.length_squared() <= v2 && root.w.length_squared() <= w2 && mean <= mean2 {
            if self.mode == 1 {
                self.still[0] += 1;
                if self.still[0] >= SLEEP_STOP {
                    self.mode = 0;
                }
            }
            if self.mode != 1 {
                self.still[1] += 1;
                if self.still[1] >= SLEEP_SLEEP {
                    self.mode = 2;
                }
            }
        } else if self.mode == 1 {
            self.still = [0, 0];
        } else {
            self.mode = 1;
        }
    }

    fn artic(&self, joints: &[Option<Joint>]) -> Artic {
        let xr = self.bodies[self.order[0]].x;
        let links: Vec<artic::Link> = self.bodies.iter().map(|b| artic::Link { r: b.r, x: b.x - xr, m: b.m, i: b.i }).collect();
        let js: Vec<Option<artic::Joint>> = joints.iter().enumerate().map(|(k, j)| j.map(|j| {
            let axis = match j.kind {
                Kind::Ball { .. } => None,
                Kind::Hinge { .. } => Some(crate::rigid::hinge_axis(&j.jc, &self.bodies[k].link())),
            };
            artic::Joint { parent: j.parent, ca: j.jc.ca, axis, stiff: j.stiff, mode: j.mode }
        })).collect();
        Artic::new(&links, &js, &self.order)
    }

    fn muscles(&self, artic: &Artic, joints: &[Option<Joint>], drive: &Drive, h: f32, lt: &mut [V6], out: &mut [Vec3]) {
        let b = &self.bodies;
        for (i, m) in drive.muscles.iter().enumerate().filter(|&(i, m)| m.stiffness > 0.0 && self.gone >> i & 1 == 0) {
            let parent = joints[i].map(|j| j.parent);
            let (pq, pw) = parent.map_or((Quat::IDENTITY, Vec3::ZERO), |j| (b[j].q, b[j].w));
            let err = short((pq * m.target * b[i].q.inverse()).normalize()).to_scaled_axis();
            let rel = b[i].w - pw;
            let kn = |t: Vec3| { let u = t.normalize_or_zero().as_dvec3(); u.dot(answer(artic, i, u)) as f32 };
            if let Some(r) = drive.rigid.get(i) {
                let Some(j) = joints[i] else { continue };
                let (pl, cl) = (b[j.parent].link(), b[i].link());
                let [s, d] = r.gain;
                let most = |m: f32| m / (1.0 - r.blend).max(1e-3);
                let speed = |a: Vec3, o: &[u8; 3]| sdot(cl.w, a, o) - sdot(pl.w, a, o);
                let held = |acc: f32, v: f32, at: f32, to: f32| if acc.abs() > 1e-6 && !((acc.fma(h, v).fma(h, at) - to) * (at - to) >= 0.0) { 0.0 } else { acc };
                if !r.hinge {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    let (cs, sn) = (sdot(st.s, crate::rigid::col(&st.pr, 1), b"zyx"), -sdot(st.s, crate::rigid::col(&st.pr, 0), b"yzx"));
                    let (q, ls, ts) = (speed(st.across, b"zyx") / st.k, speed(st.s, b"zyx"), speed(st.bis, b"zyx") * st.k);
                    let at = [st.theta * cs, st.theta * sn, st.twist];
                    let v = [ls.fma(cs, q * sn), ls.fma(sn, -(q * cs)), ts];
                    let mut e = std::array::from_fn::<f32, 3, _>(|k| r.target[k] - at[k]);
                    if matches!(j.kind, Kind::Ball { twist, .. } if twist - LIMIT_MARGIN >= f32::MAX) {
                        use std::f32::consts::{PI, TAU};
                        e[2] = if e[2] > PI { e[2] - TAU } else if e[2] < -PI { e[2] + TAU } else { e[2] };
                    }
                    let acc: [f32; 3] = std::array::from_fn(|k| held(e[k] * s + (0.0 - v[k]) * d, v[k], at[k], r.target[k]));
                    let (sw, qw, tw) = (acc[0].fma(cs, acc[1] * sn), acc[0].fma(sn, -(acc[1] * cs)) * st.k, acc[2] / st.k);
                    let world = st.bis.mul_add(Vec3::splat(tw), st.across.mul_add(Vec3::splat(qw), st.s * sw));
                    let cols = [DVec3::X, DVec3::Y, DVec3::Z].map(|e| answer(artic, i, e));
                    let t = (DMat3::from_cols(cols[0], cols[1], cols[2]).inverse() * world.as_dvec3()).as_vec3();
                    let (ml, mt) = (most(r.max[0]), most(r.max[1]));
                    let (ts, ta) = (sdot(t, st.s, b"yzx"), sdot(t, st.across, b"yzx"));
                    let (t0, t1, t2) = (ts.fma(cs, ta * sn).clamp(-ml, ml), ts.fma(sn, -(ta * cs)).clamp(-ml, ml), sdot(t, st.bis, b"yzx").clamp(-mt, mt));
                    let t = st.bis.mul_add(Vec3::splat(t2), st.across.mul_add(Vec3::splat(t0.fma(sn, -(t1 * cs))), st.s * t0.fma(cs, t1 * sn)));
                    pair(joints, i, t.as_dvec3(), lt);
                    out[i] += t;
                    continue;
                }
                let n = crate::rigid::hinge_axis(&j.jc, &cl);
                let (at, w) = (crate::rigid::hinge(&j.jc, &pl, &cl), speed(n, b"yzx"));
                let acc = held((0.0 - w) * d + (r.target[0] - at) * s, w, at, r.target[0]);
                let k = n.as_dvec3().dot(answer(artic, i, n.as_dvec3())) as f32;
                let mt = most(r.max[0]);
                let t = if k > 1e-12 { n * (acc / k).clamp(-mt, mt) } else { Vec3::ZERO };
                pair(joints, i, t.as_dvec3(), lt);
                out[i] += t;
            } else if drive.rigid.is_empty() {
                let (s, d) = (m.stiffness * m.stiffness, 2.0 * m.damping * m.stiffness);
                let a = (err * s - rel * (d + h * s)) / (1.0 + h * d + h * h * s);
                let k = kn(a);
                if k > 1e-9 {
                    pair(joints, i, (a / k).as_dvec3(), lt);
                    out[i] += a / k;
                }
            }
        }
    }

    fn soft_limits(&self, artic: &Artic, joints: &[Option<Joint>], drive: &Drive, lt: &mut [V6], out: &mut [Vec3]) {
        let b = &self.bodies;
        for (i, r) in drive.rigid.iter().enumerate().filter(|(_, r)| r.soft.iter().any(|l| l.enabled)) {
            let Some(j) = joints.get(i).copied().flatten() else { continue };
            let (pl, cl) = (b[j.parent].link(), b[i].link());
            let speed = |a: Vec3, o: &[u8; 3]| sdot(cl.w, a, o) - sdot(pl.w, a, o);
            let t = if r.hinge {
                let n = crate::rigid::hinge_axis(&j.jc, &cl);
                let acc = r.soft[0].accel(crate::rigid::hinge(&j.jc, &pl, &cl), speed(n, b"yzx"));
                let k = n.as_dvec3().dot(answer(artic, i, n.as_dvec3())) as f32;
                if k > 1e-12 { n * (acc / k) } else { Vec3::ZERO }
            } else {
                let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                let (cs, sn) = (sdot(st.s, crate::rigid::col(&st.pr, 1), b"zyx"), -sdot(st.s, crate::rigid::col(&st.pr, 0), b"yzx"));
                let (q, ls, ts) = (speed(st.across, b"zyx") / st.k, speed(st.s, b"zyx"), speed(st.bis, b"zyx") * st.k);
                let at = [st.theta * cs, st.theta * sn, st.twist];
                let v = [ls.fma(cs, q * sn), ls.fma(sn, -(q * cs)), ts];
                let acc: [f32; 3] = std::array::from_fn(|k| r.soft[k].accel(at[k], v[k]));
                let (sw, qw, tw) = (acc[0].fma(cs, acc[1] * sn), acc[0].fma(sn, -(acc[1] * cs)) * st.k, acc[2] / st.k);
                let world = st.bis.mul_add(Vec3::splat(tw), st.across.mul_add(Vec3::splat(qw), st.s * sw));
                let cols = [DVec3::X, DVec3::Y, DVec3::Z].map(|e| answer(artic, i, e));
                (DMat3::from_cols(cols[0], cols[1], cols[2]).inverse() * world.as_dvec3()).as_vec3()
            };
            if t != Vec3::ZERO {
                pair(joints, i, t.as_dvec3(), lt);
                out[i] += t;
            }
        }
    }

    fn limits(&self, artic: &Artic, joints: &[Option<Joint>], h: f32, lt: &mut [V6]) {
        let b = &self.bodies;
        for (k, j) in joints.iter().enumerate() {
            let Some(j) = *j else { continue };
            let (pl, cl) = (b[j.parent].link(), b[k].link());
            let speed = |a: Vec3| sdot(cl.w, a, b"yzx") - sdot(pl.w, a, b"yzx");
            let mut past: Vec<(Vec3, f32)> = vec![];
            match j.kind {
                Kind::Hinge { lo, hi } => {
                    let (lo, hi) = hinge_limits(lo, hi);
                    let (a, n) = (crate::rigid::hinge(&j.jc, &pl, &cl), crate::rigid::hinge_axis(&j.jc, &cl));
                    let (oh, ol) = (a - hi, a - lo);
                    if !(oh > 0.0) && ol >= 0.0 {
                        continue;
                    }
                    let over = if oh >= 0.0 { oh } else { ol };
                    let w = speed(n);
                    let f = if w * over >= 0.0 {
                        over
                    } else {
                        let sd = w * h;
                        let f = sd + over;
                        if sd * f >= 0.0 {
                            continue;
                        }
                        f
                    };
                    let r = n.as_dvec3().dot(answer(artic, k, n.as_dvec3()));
                    let t = -((if r != 0.0 { 1.0 / r } else { 1e8 }) as f32 * LIMIT_GAIN * f);
                    pair(joints, k, (n * t).as_dvec3(), lt);
                    continue;
                }
                Kind::Ball { lean, twist } => {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    if let Some(lim) = lean_limit(&st, lean) {
                        let (over, ls) = (st.theta - lim, speed(st.s));
                        let f = (if ls >= 0.0 { 0.0 } else { ls }).fma(h, over);
                        if f > 0.0 || ls > 0.0 {
                            past.extend([(st.s, -(LIMIT_GAIN * f)), (st.across, -speed(st.across))]);
                        }
                    }
                    if let Some(lim) = twist_limit(&st, twist) {
                        let (mut over, mut ts, mut axis) = (st.twist - lim, speed(st.bis) * st.k, st.bis);
                        if lim < 0.0 {
                            (over, ts, axis) = (-over, -ts, -axis);
                        }
                        let f = (if ts >= 0.0 { 0.0 } else { ts }).fma(h, if over >= 0.0 { over } else { 0.0 });
                        if f > 0.0 || ts >= 0.0 {
                            past.push((axis, (if st.k != 0.0 { 1.0 / st.k } else { 1e8 }) * -(LIMIT_GAIN * f)));
                        }
                    }
                }
            }
            let resp: Vec<DVec3> = past.iter().map(|&(u, _)| answer(artic, k, u.as_dvec3())).collect();
            let m = |i: usize, c: usize| past[i].0.as_dvec3().dot(resp[c]);
            let w: Vec<f64> = past.iter().map(|&(_, w)| w as f64).collect();
            let rec = |d: f64| if d != 0.0 { 1.0 / d } else { 1e8 };
            let x: Vec<f64> = match past.len() {
                1 => vec![w[0] * rec(m(0, 0))],
                2 => {
                    let r = rec(m(1, 1) * m(0, 0) - m(0, 1) * m(1, 0));
                    vec![(m(1, 1) * w[0] - m(0, 1) * w[1]) * r, (w[1] * m(0, 0) - m(1, 0) * w[0]) * r]
                }
                3 => {
                    let mm = DMat3::from_cols(DVec3::new(m(0, 0), m(1, 0), m(2, 0)), DVec3::new(m(0, 1), m(1, 1), m(2, 1)), DVec3::new(m(0, 2), m(1, 2), m(2, 2)));
                    (mm.inverse() * DVec3::new(w[0], w[1], w[2])).to_array().to_vec()
                }
                _ => continue,
            };
            let t: DVec3 = past.iter().zip(&x).map(|(&(u, _), &x)| u.as_dvec3() * x).sum();
            if t != DVec3::ZERO {
                pair(joints, k, t, lt);
            }
        }
    }

    fn limit_rows(&mut self, artic: &Artic, joints: &[Option<Joint>], h: f32, qd0: &[f64]) -> (Vec<Row>, Vec<Row>) {
        self.prep_twist = vec![None; joints.len()];
        let mut touched: Vec<bool> = self.held.iter().map(|h| !h.is_empty()).collect();
        touched.resize(joints.len(), false);
        for (&(a, c), _) in self.pairs.iter().zip(&self.held_pairs).filter(|(_, h)| !h.is_empty()) {
            (touched[a], touched[c]) = (true, true);
        }
        let b = &self.bodies;
        let (mut rows, mut push) = (vec![], vec![]);
        let mut found = vec![];
        let n = 6 * b.len();
        for (k, j) in joints.iter().enumerate() {
            let Some(j) = *j else { continue };
            let (pl, cl) = (b[j.parent].link(), b[k].link());
            let mut dofs: Vec<(Vec3, f32, f32)> = vec![];
            match j.kind {
                Kind::Hinge { lo, hi } => {
                    let (lo, hi) = hinge_limits(lo, hi);
                    let (a, n) = (crate::rigid::hinge(&j.jc, &pl, &cl), crate::rigid::hinge_axis(&j.jc, &cl));
                    let side = if !(a - hi < 0.0) { 1 } else if a - lo <= 0.0 { -1 } else if touched[k] { if hi - a < a - lo { 1 } else { -1 } } else { 0 };
                    match side {
                        1 => dofs.push((n, a - hi, 1.0)),
                        -1 => dofs.push((-n, lo - a, 1.0)),
                        _ => {}
                    }
                }
                Kind::Ball { lean, twist } => {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    self.prep_twist[k] = Some(st.twist);
                    if let Some(lim) = lean_limit(&st, lean) {
                        dofs.push((st.s, st.theta - lim, 1.0));
                        let w = sdot(cl.w, st.across, b"zyx") - sdot(pl.w, st.across, b"zyx");
                        dofs.push((if w < 0.0 { -st.across } else { st.across }, 0.0, 1.0));
                    }
                    if let Some(lim) = twist_limit(&st, twist) {
                        dofs.push(if lim > 0.0 { (st.bis, st.twist - lim, st.k) } else { (-st.bis, -st.twist + lim, st.k) });
                    }
                }
            }
            for (u, o, c) in dofs {
                let mut jr = vec![0.0; n];
                for i in 0..3 {
                    jr[6 * k + 3 + i] = -u[i] as f64;
                    jr[6 * j.parent + 3 + i] += u[i] as f64;
                }
                let b0 = -jr.iter().zip(qd0).map(|(x, y)| x * y).sum::<f64>();
                let to = (c as f64 - 1.0) * b0 - ((LIMIT_MARGIN - o).max(0.0) / h) as f64;
                found.push((jr, to, o, c));
            }
        }
        for g in found.chunks(4) {
            let mj = artic.solve_rows(std::array::from_fn::<_, 4, _>(|i| &g[i.min(g.len() - 1)].0[..]));
            for ((jr, to, o, c), mj) in g.iter().cloned().zip(mj) {
                let mut r = Row::with(jr, mj, to, RowKind::Push);
                r.eff *= c as f64;
                push.push(Row { j: r.j.clone(), mj: r.mj.clone(), target: ((o - LIMIT_MARGIN) / h) as f64, ..r });
                rows.push(r);
            }
        }
        (rows, push)
    }

    #[allow(clippy::too_many_arguments)]
    fn contacts(&mut self, build: &Build, world: &Collision, artic: &Artic, h: f32, pose: &[Option<(Vec3, Quat)>], obstacles: &[Obstacle],
                others: &[Capsule], free: &[f64]) -> (Vec<Row>, Vec<Block>, Vec<Block>) {
        let (mut rows, mut blocks, mut push_blocks) = (vec![], vec![], vec![]);
        let fv = |j: &[f64]| j.iter().zip(free).map(|(a, b)| a * b).sum::<f64>();
        let (o, turns) = (self.o, self.turns());
        let d = |v: Vec3| v.as_dvec3();
        let (n_links, xr) = (self.bodies.len(), self.bodies[self.order[0]].x.as_dvec3());
        let lrow = |k: usize, at: DVec3, n: DVec3, t: DVec3| -> Vec<f64> {
            let mut r = vec![0.0; 6 * n_links];
            r[6 * k..6 * k + 6].copy_from_slice(&v6(n, (at - xr).cross(n) + t));
            r
        };
        for (k, p) in pose.iter().enumerate() {
            let Some((x, q)) = *p else { continue };
            let b = &self.bodies[k];
            let (v, w) = ((x - b.x) / h, short(q * b.q.inverse()).to_scaled_axis() / h);
            for e in [Vec3::X, Vec3::Y, Vec3::Z] {
                for (j, to) in [(lrow(k, d(b.x), d(e), DVec3::ZERO), v.dot(e)), (lrow(k, d(b.x), DVec3::ZERO, d(e)), w.dot(e))] {
                    let to = to as f64 - fv(&j);
                    rows.push(Row::new(artic, j, to, RowKind::Equal));
                }
            }
        }
        for (k, &(at, turn)) in self.cons.iter().enumerate().filter(|&(k, _)| pose[k].is_none()) {
            let b = &self.bodies[k];
            for e in [Vec3::X, Vec3::Y, Vec3::Z] {
                if let Some(at) = at {
                    let j = lrow(k, d(b.x), d(e), DVec3::ZERO);
                    let to = ((at - o - b.x) / h).dot(e) as f64 - fv(&j);
                    rows.push(Row::new(artic, j, to, RowKind::Equal));
                }
                if let Some(q) = turn {
                    let j = lrow(k, d(b.x), DVec3::ZERO, d(e));
                    let to = (short(q * b.q.inverse()).to_scaled_axis() / h).dot(e) as f64 - fv(&j);
                    rows.push(Row::new(artic, j, to, RowKind::Equal));
                }
            }
        }
        self.grip_rows.clear();
        for &(k, local, to, len, strength) in &self.grips {
            let b = &self.bodies[k];
            let at = b.x + b.q * local;
            let gap = (to - o) - at;
            let dist = gap.length();
            if dist > len - 0.01 && dist > 1e-6 {
                self.grip_rows.push((rows.len(), k, strength));
                let j = lrow(k, d(at), d(gap / dist), DVec3::ZERO);
                let to = ((dist - len) / h) as f64 - fv(&j);
                rows.push(Row::new(artic, j, to, RowKind::Push));
            }
        }
        let mut hits: Vec<(usize, Vec3, Vec3, f32, f32, Vec3, f32, Option<(usize, usize)>)> = vec![];
        let fresh = self.fresh;
        self.held_car.retain(|h| !h.3.is_empty() && !fresh && pose[h.0].is_none() && obstacles.iter().any(|o| o.id == h.1 && !o.parts.is_empty()));
        self.held_pairs.resize(self.pairs.len(), vec![]);
        let car_base = self.held.len() + self.held_pairs.len();
        let mut hit_ob: Vec<(usize, i32, Vec3, Vec3)> = vec![];
        for (_, held) in self.held.iter_mut().enumerate().filter(|&(i, _)| pose[i].is_some() || self.fresh) {
            held.clear();
        }
        for (i, p) in build.parts.iter().enumerate().filter(|&(i, _)| pose[i].is_none() && !self.fresh) {
            let (b, tm) = (&self.bodies[i], turns[i]);
            let x = b.x - self.bound_off;
            let mut acc = (Vec3::ZERO, Vec3::ZERO, 0.0, -1);
            let mut add = |at: Vec3, n: Vec3, depth: f32, inst: i32| {
                let w = depth.max(0.01);
                acc = (acc.0 + at * w, acc.1 + n * w, acc.2 + w, inst);
            };
            let held = &mut self.held[i];
            let shape = match p.boxed {
                Some(e) => gjk::Shape::Box(e - Vec3::splat(BOX_MARGIN), BOX_MARGIN),
                None => gjk::Shape::Capsule(p.half, p.radius),
            };
            let last = self.seen.get(i).map_or(x, |s| s.0 - self.bound_off);
            let fast = (tm.transpose() * (x - last)).abs().cmpgt(bound_half(p)).any();
            let leaving = |local: Vec3, face: Vec3| self.seen.get(i).is_some_and(|&(lx, lm)| ((b.x + tm * local) - (lx + lm * local)).dot(face) > 0.001);
            let last_m = self.seen.get(i).map_or(tm, |s| s.1);
            let (sc, sh) = swept((x, tm), (last, last_m), bound_half(p));
            let (sm, sh) = (tm, sh + Vec3::splat(WORLD_MARGIN));
            let sc = x + o + sm * sc;
            let found = {
                let reach = Vec3::splat(bound_half(p).length() + 0.1);
                let new = |q: gjk::Closest, at: Vec3| {
                    let local = tm.transpose() * (q.a - at);
                    let ground = q.b - q.n * WORLD_MARGIN + o;
                    Held { local, ground, gap: (x + o + tm * local - ground).dot(q.n) - WORLD_MARGIN, age: 0, m: 0, imp: Vec3::ZERO, pimp: Vec3::ZERO, n: q.n, inst: 0, off: false }
                };
                let frame = |m: Mat3, at: Vec3| crate::bvh::Frame { rows: [m.x_axis, m.y_axis, m.z_axis], t: at + o };
                let mut found: Vec<(Held, crate::collision::Poly)> = world.tris_in_part(x.min(last) + o - reach, x.max(last) + o + reach,
                    &frame(tm, x), &frame(last_m, last), -bound_half(p), bound_half(p)).into_iter().filter_map(|(t, m, dirs, mask, inst, poly)| {
                    if !box_meets_tri(sc, sm, sh, t) {
                        return None;
                    }
                    let (tri, back) = (gjk::Shape::Tri(t.map(|v| v - t[0]), WORLD_MARGIN), t[0] - o);
                    let query = |at: Vec3| gjk::closest(&shape, tm, at + o - t[0], &tri, Mat3::IDENTITY, Vec3::ZERO, Vec3::X, f32::INFINITY, Some(gjk::Pen::Tri(dirs)))
                        .map(|q| gjk::Closest { a: q.a + back, b: q.b + back, ..q });
                    let h = if fast {
                        let (t, q) = Self::advance(|t| query(last + (x - last) * t), last - x)?;
                        new(q, last + (x - last) * t)
                    } else {
                        let q = query(x).filter(|q| q.dist < 0.02)?;
                        new(q, x)
                    };
                    let across_ok = |at: Vec3, n: Vec3| (0..3).filter(|k| mask & 1 << k != 0).all(|k| {
                        let (p, e) = (t[k], t[(k + 1) % 3] - t[k]);
                        let (s, ee) = ((at - p).dot(e), e.dot(e));
                        let c = if !(s > 1e-12) { p } else if ee - s > 1e-12 { p + e * (s / ee) } else { p + e };
                        if (c - at).length_squared() > 1.0816 * WORLD_MARGIN * WORLD_MARGIN {
                            return true;
                        }
                        let u = e.normalize();
                        let across = if u.dot(n) == 0.0 { n } else { (n - u * u.dot(n)).normalize() };
                        dirs[k + 1].dot(across) >= dirs[0].dot(dirs[k + 1]).min(1.0 - 1e-6) - 0.001
                    });
                    if leaving(h.local, dirs[0]) || !across_ok(h.ground + h.n * WORLD_MARGIN, h.n) {
                        return None;
                    }
                    Some((Held { m, inst, ..h }, poly))
                }).collect();
                for k in 0..found.len() {
                    let poly = found[k].1;
                    if found[..k].iter().any(|f| f.1.first == poly.first) {
                        continue;
                    }
                    let mut hits: Vec<(Vec3, Vec3)> = found[k..].iter().filter(|f| f.1.first == poly.first).map(|f| (f.0.ground + f.0.n * WORLD_MARGIN, f.0.n)).collect();
                    poly.clamp(&mut hits, WORLD_MARGIN);
                    for (f, (at, n)) in found[k..].iter_mut().filter(|f| f.1.first == poly.first).zip(hits) {
                        (f.0.ground, f.0.n) = (at - n * WORLD_MARGIN, n);
                    }
                }
                found.into_iter().map(|f| f.0).collect::<Vec<Held>>()
            };
            #[cfg(feature = "portal")]
            let found: Vec<Held> = found.into_iter().filter(|h| !world.in_hole(h.ground)).collect();
            #[cfg(feature = "portal")]
            held.retain(|h| !world.in_hole(h.ground));
            let mut groups: Vec<(u32, Vec<Held>)> = vec![];
            for r in held.drain(..) {
                match groups.iter_mut().find(|g| g.0 == r.inst) {
                    Some(g) => g.1.push(r),
                    None => groups.push((r.inst, vec![r])),
                }
            }
            for new in found {
                let g = match groups.iter().position(|g| g.0 == new.inst) {
                    Some(g) => g,
                    None => { groups.push((new.inst, vec![])); groups.len() - 1 }
                };
                hold(&mut groups[g].1, new);
            }
            for (_, held) in groups.iter_mut() {
                for r in held.iter_mut() {
                    if r.age != 0 {
                        r.gap = (x + o + tm * r.local - r.ground).dot(r.n) - WORLD_MARGIN;
                    }
                    r.age += if r.age == 1 { 2 } else { 1 };
                }
                let slid = |r: &Held| { let d = x + o + tm * r.local - r.ground; (d - r.n * d.dot(r.n)).length_squared() };
                let (mut k, mut worst) = (0, (None, 0.0));
                while k < held.len() {
                    if held[k].gap >= MARGIN {
                        held.swap_remove(k);
                        continue;
                    }
                    let d = slid(&held[k]);
                    if held[k].age > 4 && d > worst.1 && d > 0.0004 {
                        worst = (Some(k), d);
                    }
                    k += 1;
                }
                if let Some(k) = worst.0 {
                    held.swap_remove(k);
                }
            }
            *held = groups.into_iter().flat_map(|g| g.1).collect();
            for (k, r) in held.iter().enumerate() {
                let at = x + tm * r.local;
                let row = SURFACES.read().ok().and_then(|s| s.get(r.m as usize).copied()).unwrap_or([1.0, ELASTICITY]);
                self.touch[i] |= 0x20;
                add(at + o, r.n, -r.gap, -1);
                hits.push((i, at, r.n, r.gap, (part_friction(p) * row[0]).max(0.0), Vec3::ZERO, (PART_ELASTICITY * row[1]).min(1.0), Some((i, k))));
            }
            let a_last = (last, self.seen.get(i).map_or(tm, |s| s.1));
            for ob in obstacles.iter().filter(|ob| !ob.parts.is_empty()) {
                let (rot, lrot) = (Mat3::from_quat(ob.rot), Mat3::from_quat(ob.last.map_or(ob.rot, |l| l.1)));
                let lat = ob.last.map_or(ob.at, |l| l.0);
                let (sc, sh) = swept((x, tm), a_last, bound_half(p));
                for (c, cp) in ob.parts.iter().enumerate() {
                    if matches!(cp.shape, gjk::Shape::Hull(v, _) if v.is_empty()) {
                        continue;
                    }
                    let (cq, cx) = (rot * cp.m, ob.at - o + rot * cp.at);
                    let (cent, half) = ((cp.lo + cp.hi) * 0.5, (cp.hi - cp.lo) * 0.5);
                    let ac = cq.transpose() * (x + tm * sc - cx) - cent;
                    let r = cq.transpose() * tm;
                    let ah = Mat3::from_cols(r.x_axis.abs(), r.y_axis.abs(), r.z_axis.abs()) * sh;
                    let meet = ac.abs().cmple(ah + half + Vec3::splat(2e-6)).all();
                    let slot = self.held_car.iter().position(|h| (h.0, h.1, h.2) == (i, ob.id, c));
                    if !meet {
                        if let Some(s) = slot { self.held_car[s].3.clear(); }
                        continue;
                    }
                    let s = slot.unwrap_or_else(|| { self.held_car.push((i, ob.id, c, vec![])); self.held_car.len() - 1 });
                    let side_c = (cp.shape, half, (cx, cq), (lat - o + lrot * cp.at, lrot * cp.m));
                    Self::manifold(&mut self.held_car[s].3, (shape, bound_half(p), (x, tm), a_last), side_c);
                    for (k, r) in self.held_car[s].3.iter().enumerate() {
                        let (at, on) = (x + tm * r.local, cx + cq * r.ground);
                        self.touch[i] |= 0x20;
                        add(at + o, r.n, -r.gap, ob.id);
                        hit_ob.push((hits.len(), ob.id, at + o, r.n));
                        hits.push((i, at, r.n, r.gap, part_friction(p) * CAR_FRICTION, ob.vel_at(on + o), PART_ELASTICITY * ELASTICITY, Some((car_base + s, k))));
                    }
                }
            }
            for ob in obstacles.iter().filter(|ob| ob.parts.is_empty()) {
                let car = gjk::Shape::Box(ob.half, 0.0);
                let Some(q) = gjk::closest(&shape, tm, x, &car, Mat3::from_quat(ob.rot), ob.centre - o, Vec3::Z, f32::MAX.sqrt(), Some(gjk::Pen::Sphere)) else { continue };
                if q.dist >= MARGIN {
                    continue;
                }
                self.touch[i] |= 0x20;
                add(q.a + o, q.n, -q.dist, ob.id);
                hit_ob.push((hits.len(), ob.id, q.a + o, q.n));
                hits.push((i, q.a, q.n, q.dist, part_friction(p) * CAR_FRICTION, ob.vel_at(q.b + o), PART_ELASTICITY * ELASTICITY, None));
            }
            if acc.2 > 0.0 {
                self.hit[i] = Some((acc.0 / acc.2, (acc.1 / acc.2).normalize_or_zero(), acc.3));
            }
            for &(o0, o1, or, ov, doll) in others {
                let axis = tm * Vec3::Y * p.half;
                let (here, there) = nearest(b.x - axis, b.x + axis, o0 - o, o1 - o);
                let (gap, room) = (here - there, p.radius + or);
                let dist = gap.length();
                if dist < room && dist > 1e-5 {
                    hits.push((i, here, gap / dist, (dist - room) * if doll { 0.5 } else { 1.0 }, part_friction(p) * 0.75, ov, PART_ELASTICITY * PART_ELASTICITY, None));
                }
            }
        }
        let boxes: Vec<(Vec3, Vec3)> = (0..n_links).map(|k| { let f = (self.bodies[k].x, turns[k]); swept_box(&build.parts[k], f, self.seen.get(k).copied().unwrap_or(f)) }).collect();
        for (pi, &(a, c)) in self.pairs.clone().iter().enumerate() {
            let (fa, fc) = ((self.bodies[a].x, turns[a]), (self.bodies[c].x, turns[c]));
            let ((xa, qa), (xc, qc)) = (fa, fc);
            let seen = |k: usize, f: (Vec3, Mat3)| self.seen.get(k).copied().unwrap_or(f);
            let (sa, sc) = (seen(a, fa), seen(c, fc));
            Self::pair_manifold(&mut self.held_pairs[pi], (&build.parts[a], fa, sa), (&build.parts[c], fc, sc), [boxes[a], boxes[c]]);
            let held = &mut self.held_pairs[pi];
            let ends = |r: &Held| (xa + qa * r.local, xc + qc * r.ground);
            let off = (self.contact_off >> a | self.contact_off >> c) & 1 != 0;
            held.iter_mut().for_each(|r| r.off |= off);
            for (x, r) in held.iter().enumerate().filter(|(_, r)| !r.off) {
                let (at_a, at_c) = ends(r);
                let (n, over) = (r.n, r.gap + ALLOW);
                let pair = |dir: Vec3| -> Vec<f64> {
                    lrow(a, d(at_a), d(dir), DVec3::ZERO).iter().zip(lrow(c, d(at_c), d(dir), DVec3::ZERO)).map(|(x, y)| x - y).collect()
                };
                let e = (PART_ELASTICITY * PART_ELASTICITY) as f64;
                let j = frame(n).map(pair);
                let to = DVec3::new((-over.max(0.0) / h) as f64 - fv(&j[0]) / (1.0 + e), -fv(&j[1]), -fv(&j[2]));
                blocks.push(Block { scale: elastic(e), ..Block::new(artic, j, to, (part_friction(&build.parts[a]) * part_friction(&build.parts[c])) as f64, Some((n_links + pi, x)), r.imp, true) });
                push_blocks.push(Self::push_block(blocks.last().unwrap(), over, h, r.pimp));
            }
        }
        let n_self = blocks.len();
        #[allow(unused_mut)]
        let mut gwarm = vec![];
        for (k, held) in self.held.iter_mut().enumerate() {
            let on = self.contact_off >> k & 1 == 0;
            held.iter_mut().for_each(|r| r.off = crate::rigid::contact_pass(if r.off { 0 } else { 0x80 }, 0, on) < 0x80);
        }
        let on = |&(_, h): &(usize, &(usize, Vec3, Vec3, f32, f32, Vec3, f32, Option<(usize, usize)>))| self.contact_off >> h.0 & 1 == 0 && h.7.map_or(true, |r| !self.rec(r).off);
        let hits: Vec<_> = hits.iter().enumerate().filter(on).map(|(i, h)| (i, *h)).collect();
        for (i, (k, at, n, gap, mu, v, e, held)) in hits {
            let gap = gap + ALLOW;
            let ts = frame(n);
            let j = ts.map(|t| lrow(k, d(at), d(t), DVec3::ZERO));
            let to = DVec3::new((-gap.max(0.0) / h + v.dot(n)) as f64 - fv(&j[0]) / (1.0 + e as f64), v.dot(ts[1]) as f64 - fv(&j[1]), v.dot(ts[2]) as f64 - fv(&j[2]));
            let warm = held.map_or(gwarm.get(i).copied().unwrap_or(Vec3::ZERO), |r| self.rec(r).imp);
            let ob = hit_ob.iter().find(|x| x.0 == i).map(|&(_, id, at, n)| (id, k, at, n));
            blocks.push(Block { scale: elastic(e as f64), ob, ..Block::new(artic, j, to, mu as f64, held, warm, false) });
            push_blocks.push(Self::push_block(blocks.last().unwrap(), gap, h, held.map_or(Vec3::ZERO, |r| self.rec(r).pimp)));
        }
        for v in [&mut blocks, &mut push_blocks] {
            v.rotate_left(n_self);
            island_sort(v);
        }
        (rows, blocks, push_blocks)
    }

    fn rec(&self, (k, x): (usize, usize)) -> &Held {
        let (g, pp) = (self.held.len(), self.held_pairs.len());
        if k < g { &self.held[k][x] } else if k < g + pp { &self.held_pairs[k - g][x] } else { &self.held_car[k - g - pp].3[x] }
    }

    fn rec_mut(&mut self, (k, x): (usize, usize)) -> &mut Held {
        let (g, pp) = (self.held.len(), self.held_pairs.len());
        if k < g { &mut self.held[k][x] } else if k < g + pp { &mut self.held_pairs[k - g][x] } else { &mut self.held_car[k - g - pp].3[x] }
    }

    fn pair_manifold(held: &mut Vec<Held>, (a, fa, sa): (&PartDef, (Vec3, Mat3), (Vec3, Mat3)), (c, fc, sc): (&PartDef, (Vec3, Mat3), (Vec3, Mat3)),
                     [(ca, ha), (cc, hc)]: [(Vec3, Vec3); 2]) {
        let ((_, qa), (_, qc)) = (fa, fc);
        let meet = boxes_meet(ca, qa, ha, cc, qc, hc);
        if !meet {
            held.clear();
            return;
        }
        Self::manifold(held, (part_shape(a), bound_half(a), fa, sa), (part_shape(c), bound_half(c), fc, sc));
    }

    fn manifold(held: &mut Vec<Held>, (a, ha, fa, sa): (gjk::Shape, Vec3, (Vec3, Mat3), (Vec3, Mat3)), (c, hc, fc, sc): (gjk::Shape, Vec3, (Vec3, Mat3), (Vec3, Mat3))) {
        let ((xa, qa), (xc, qc)) = (fa, fc);
        let (la, lc) = (sa.0, sc.0);
        let moved = (xa - la) - (xc - lc);
        let fast = (qa.transpose() * moved).abs().cmpgt(ha).any() || (qc.transpose() * moved).abs().cmpgt(hc).any();
        let found = if held.is_empty() && fast {
            Self::cast_shapes((&a, &c), (la, xa, qa), (lc, xc, qc)).map(|(at_a, at_c, n)| {
                let (pa, pc) = (xa + qa * at_a, xc + qc * at_c);
                (pa, pc, n, (pa - pc).dot(n))
            })
        } else {
            Self::shape_contacts((&a, &c), fa, fc, MARGIN, Vec3::Z).into_iter().min_by(|x, y| x.3.total_cmp(&y.3))
        };
        if let Some((at_a, at_c, n, gap)) = found {
            hold(held, Held { local: qa.transpose() * (at_a - xa), ground: qc.transpose() * (at_c - xc), gap, age: 0, m: 0, imp: Vec3::ZERO, pimp: Vec3::ZERO, n, inst: 0, off: false });
        }
        let ends = |r: &Held| (xa + qa * r.local, xc + qc * r.ground);
        for r in held.iter_mut() {
            if r.age != 0 {
                let (pa, pc) = ends(r);
                r.gap = (pa - pc).dot(r.n);
            }
            r.age += if r.age == 1 { 2 } else { 1 };
        }
        let slid = |r: &Held| { let (pa, pc) = ends(r); let d = pc - pa; (d - r.n * d.dot(r.n)).length_squared() };
        let (mut k, mut worst) = (0, (None, 0.0));
        while k < held.len() {
            if held[k].gap >= MARGIN {
                held.swap_remove(k);
                continue;
            }
            let d = slid(&held[k]);
            if held[k].age > 4 && d > worst.1 && d > 0.0004 {
                worst = (Some(k), d);
            }
            k += 1;
        }
        if let Some(k) = worst.0 {
            held.swap_remove(k);
        }
    }

    fn cast_shapes((a, c): (&gjk::Shape, &gjk::Shape), (la, xa, qa): (Vec3, Vec3, Mat3), (lc, xc, qc): (Vec3, Vec3, Mat3)) -> Option<(Vec3, Vec3, Vec3)> {
        let at = |t: f32| (la + (xa - la) * t, lc + (xc - lc) * t);
        let axis = std::cell::Cell::new(Vec3::Z);
        let query = |t: f32| { let (pa, pc) = at(t); Self::shape_contacts((a, c), (pa, qa), (pc, qc), f32::INFINITY, axis.get()).into_iter().min_by(|x, y| x.3.total_cmp(&y.3))
            .map(|(a, b, n, dist)| { axis.set(n); gjk::Closest { a, b, n, dist } }) };
        let (t, q) = Self::advance(query, (xc - lc) - (xa - la))?;
        let (pa, pc) = at(t);
        Some((qa.transpose() * (q.a - pa), qc.transpose() * (q.b - pc), q.n))
    }

    fn advance(query: impl Fn(f32) -> Option<gjk::Closest>, r: Vec3) -> Option<(f32, gjk::Closest)> {
        let (mut t, mut prev, mut q) = (0.0f32, 0.0f32, query(0.0)?);
        let mut step = q.dist;
        if step > 0.001 {
            for it in 1.. {
                let nr = q.n.dot(r);
                let dt = if nr != 0.0 { step / nr } else { f32::MAX };
                t += dt;
                if it > 12 || dt < 0.0 || t > 1.0 {
                    return None;
                }
                if t <= prev {
                    break;
                }
                prev = t;
                q = query(t)?;
                if q.dist <= 0.021 {
                    break;
                }
                step = q.dist - 0.02;
            }
        }
        Some((t, q))
    }
    fn shape_contacts((a, c): (&gjk::Shape, &gjk::Shape), ba: (Vec3, Mat3), bc: (Vec3, Mat3), reach: f32, axis: Vec3) -> Vec<(Vec3, Vec3, Vec3, f32)> {
        gjk::closest(a, ba.1, ba.0, c, bc.1, bc.0, axis, f32::MAX.sqrt(), Some(gjk::Pen::Sphere))
            .filter(|q| q.dist < reach).map(|q| (q.a, q.b, q.n, q.dist)).into_iter().collect()
    }

    fn velocities(&self, artic: &Artic, lt: &[V6], h: f32, forces: bool) -> Vec<(Vec3, Vec3)> {
        let root = self.order[0];
        let xr = self.bodies[root].x;
        let x: Vec<Vec3> = self.bodies.iter().map(|b| b.x - xr).collect();
        let (v, w): (Vec<Vec3>, Vec<Vec3>) = self.bodies.iter().map(|b| (b.v, b.w)).unzip();
        let acc: Vec<artic::P2> = lt.iter().zip(&x).map(|(&a, &x)| {
            let (f, t) = artic::parts(a);
            (f.as_vec3(), (t + x.as_dvec3().cross(f)).as_vec3())
        }).collect();
        let mut imp: Vec<artic::P2> = vec![(Vec3::ZERO, Vec3::ZERO); x.len()];
        for &(k, at, i, a) in self.pending.iter().filter(|_| !forces) {
            let a = a.as_dvec3() + (at - xr).as_dvec3().cross(i.as_dvec3());
            imp[k] = (imp[k].0 + i, imp[k].1 + a.as_vec3());
        }
        artic.velocities(root, &x, &v, &w, &acc, &imp, h, forces)
    }

    fn rejoin(&mut self, joints: &[Option<Joint>], pose: &[Option<(Vec3, Quat)>]) {
        for &k in &self.order {
            let Some(j) = joints[k] else {
                if let Some((x, q)) = pose[k] {
                    self.bodies[k].x = x;
                    self.bodies[k].set_q(q);
                }
                continue;
            };
            let p = self.bodies[j.parent].link();
            let b = &mut self.bodies[k];
            let mut c = b.link();
            crate::rigid::join(&j.jc, &p, &mut c, matches!(j.kind, Kind::Hinge { .. }));
            (b.x, b.v, b.w) = (c.d, c.v, c.w);
            b.set_r(c.r);
        }
    }

    fn jconst(&self, build: &Build, k: usize, j: &Joint, rigid: Option<&Rigid>) -> crate::rigid::JointConst {
        rigid.and_then(|r| r.jc).unwrap_or_else(|| {
            let (lp, lc, part) = (self.bodies[j.parent].l.inverse(), self.bodies[k].l.inverse(), &build.parts[k]);
            let rows = |q: Quat| { let m = Mat3::from_quat(q); [m.row(0), m.row(1), m.row(2)] };
            crate::rigid::JointConst { pm: rows(lp * j.pf), pa: lp * part.anchor_parent, ca: lc * part.anchor, cm: rows(lc * j.cf) }
        })
    }

    fn recentre(&mut self, pose: &mut [Option<(Vec3, Quat)>]) -> Vec3 {
        let d0 = self.bodies[self.order[0]].x;
        let back = Vec3::ZERO - d0;
        self.o += d0;
        self.bodies.iter_mut().for_each(|b| b.x += back);
        self.pins.iter_mut().flatten().for_each(|p| p.translation += back);
        self.pending.iter_mut().for_each(|p| p.1 += back);
        pose.iter_mut().flatten().for_each(|p| p.0 += back);
        back
    }

    fn rejuvenate(&mut self) {
        self.rejuv += 1;
        if self.rejuv <= 32 {
            return;
        }
        self.bodies.iter_mut().for_each(|b| b.set_r(crate::rigid::resquare(b.r)));
        let x = self.mwc.0 as u64 * 0x5CDC_FAA7 + self.mwc.1 as u64;
        self.mwc = (x as u32, (x >> 32) as u32);
        self.rejuv = (x as u32 & 0x7FFF_FFFF) % 16;
    }

    pub fn step(&mut self, build: &Build, world: &Collision, dt: f32, drive: Drive, obstacles: &[Obstacle], others: &[Capsule]) {
        let Some(st) = self.begin(build, world, dt, &drive, obstacles, others) else { return };
        self.finish(st, drive.lift);
    }

    fn finish(&mut self, mut st: Stage, lift: f32) {
        impulses(&mut st.rows, &mut st.blocks, &mut st.qd0.clone());
        if !self.middle(&mut st) {
            return;
        }
        let mut qp = vec![0.0; st.qd0.len()];
        impulses(&mut st.push, &mut st.push_blocks, &mut qp);
        self.end(st, &qp, lift);
    }

    fn begin(&mut self, build: &Build, world: &Collision, dt: f32, drive: &Drive, obstacles: &[Obstacle], others: &[Capsule]) -> Option<Stage> {
        self.snapped.clear();
        self.rigid = None;
        self.contact_off |= self.gone;
        if self.gone != 0 {
            let g = self.gone;
            self.grips.retain(|x| g >> x.0 & 1 == 0);
            for (k, (p, c)) in self.pins.iter_mut().zip(&mut self.cons).enumerate().filter(|(k, _)| g >> k & 1 != 0) {
                let _ = k;
                (*p, *c) = (None, (None, None));
            }
        }
        let moving = obstacles.iter().any(|o| o.vel.length() > 0.5 || o.spin.length() > 0.5);
        if self.asleep && !moving && self.pending.is_empty() {
            self.out.fill((Vec3::ZERO, Vec3::ZERO));
            return None;
        }
        if moving {
            self.wake();
        }
        let d = self.bodies[0].x;
        if d.length_squared() > 100.0 * 100.0 {
            self.o += d;
            self.bodies.iter_mut().for_each(|b| b.x -= d);
            self.pins.iter_mut().flatten().for_each(|p| p.translation -= d);
            self.pending.iter_mut().for_each(|p| p.1 -= d);
        }
        self.age += dt;
        for t in &mut self.touch {
            *t = (*t & !0x24) | ((*t >> 3) & 0x04);
        }
        self.hit.iter_mut().for_each(|h| *h = None);
        let joints: Vec<Option<Joint>> = build.parts.iter().enumerate().map(|(i, p)| p.parent.map(|j| {
            let j = Joint::new(p, j, drive.rigid.get(i));
            Joint { jc: self.jconst(build, i, &j, drive.rigid.get(i)), ..j }
        })).collect();
        let subs = 1;
        let h = dt / subs as f32;
        let spin = if drive.rigid.is_empty() { MAX_SPIN } else { MAX_SPIN_NM };
        let fall = Vec3::Z * -GRAVITY * (1.0 - drive.lift) + drive.accel;
        let (start, o0) = (self.bodies.iter().map(|b| (b.x, b.q)).collect::<Vec<_>>(), self.o);
        let along = 1.0 / subs as f32;
        let mut pose: Vec<Option<(Vec3, Quat)>> = self.pins.iter().zip(&start).map(|(p, &(x, q))| {
            p.map(|to| (x.lerp(to.translation, along), q.slerp(to.rotation, along)))
        }).collect();
        let was: Vec<Option<Joint>> = joints.iter().zip(&self.stiff).map(|(j, &stiff)| j.map(|j| Joint { stiff, ..j })).collect();
        let artic = self.artic(if was.len() == joints.len() { &was } else { &joints });
        self.stiff = joints.iter().map(|j| j.map_or(0.0, |j| j.stiff)).collect();
        let mut lt: Vec<V6> = self.bodies.iter().enumerate().map(|(k, b)| {
            v6((fall * b.m + drive.force.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO)).as_dvec3(), drive.torque.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO).as_dvec3())
        }).collect();
        let mut jt = vec![Vec3::ZERO; self.bodies.len()];
        self.muscles(&artic, &joints, &drive, h, &mut lt, &mut jt);
        self.soft_limits(&artic, &joints, &drive, &mut lt, &mut jt);
        for (k, b) in self.bodies.iter().enumerate() {
            let f = (fall + Vec3::Z * GRAVITY) * b.m + drive.force.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO);
            self.out[k] = (f, drive.torque.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO) + jt[k]);
        }
        self.joint_torque = jt.clone();
        for (k, j) in joints.iter().enumerate() {
            if let Some(j) = j {
                self.out[j.parent].1 -= jt[k];
            }
        }
        let inst = self.inst();
        if self.coll.len() != self.bodies.len() {
            self.coll = self.comp(&inst);
        }
        let xr = self.bodies[self.order[0]].x;
        let mut now: Vec<(Vec3, Mat3)> = self.coll.iter().map(|(d, c)| (xr + crate::rigid::row_mul(*d, &inst), turn(&crate::rigid::rows_mul(c, &inst)))).collect();
        let d = self.bodies[self.order[0]].v * h;
        self.pending.iter_mut().for_each(|p| p.1 += d);
        for b in &mut self.bodies {
            let (v, w) = (b.v, b.w);
            b.moved(v, w, h);
        }
        self.rejoin(&joints, &pose);
        let back = self.recentre(&mut pose);
        now.iter_mut().for_each(|f| f.0 += back);
        self.rejuvenate();
        let artic = self.artic(&joints);
        let before: Vec<V6> = lt.clone();
        self.limits(&artic, &joints, h, &mut lt);
        self.stage_lim = lt.iter().zip(&before).map(|(a, b)| (artic::parts(*a).1 - artic::parts(*b).1).as_vec3()).collect();
        {
            let xr = self.bodies[self.order[0]].x;
            self.stage_lt = lt.iter().zip(&self.bodies).map(|(x, b)| { let (f, t) = artic::parts(*x); v6(f, t + (b.x - xr).as_dvec3().cross(f)) }).collect();
        }
        let dvw: Vec<(Vec3, Vec3)> = self.velocities(&artic, &lt, h, true).iter().zip(&self.bodies).map(|(&(v, w), b)| (v - b.v, w - b.w)).collect();
        if env_on!("NM_DVW") { eprintln!("dvw {}", dvw.iter().zip(&self.bodies).enumerate().map(|(k, ((v, w), b))| format!("{k}:{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.4},{:.4},{:.4}", v.x, v.y, v.z, w.x, w.y, w.z, b.x.x, b.x.y, b.x.z)).collect::<Vec<_>>().join(" ")); }
        let free_vel: Vec<(Vec3, Vec3)> = self.bodies.iter().zip(&dvw).map(|(b, d)| (b.v + d.0, b.w + d.1)).collect();
        self.armature = artic.arm.clone();
        let x0 = self.bodies[self.order[0]].x.as_dvec3();
        let qd0 = spatial(&self.bodies, x0);
        let free = spatial_of(&dvw, &self.bodies, x0);
        self.seen = now;
        self.coll = self.comp(&self.inst());
        let (mut rows, blocks, push_blocks) = self.contacts(build, world, &artic, h, &pose, obstacles, others, &free);
        let (lr, push) = self.limit_rows(&artic, &joints, h, &qd0);
        rows.extend(lr);
        Some(Stage { joints, pose, start, o0, artic, lt, x0, qd0, rows, blocks, push, push_blocks, h, spin, free_vel, damp: 0.0, free })
    }

    fn middle(&mut self, st: &mut Stage) -> bool {
        let mass = self.mass();
        self.ob_hits.clear();
        for b in &st.blocks {
            let Some((id, k, at, n)) = b.ob else { continue };
            let ts = frame(n);
            let l = b.lambda.as_vec3();
            let impulse = ts[0] * l.x + ts[1] * l.y + ts[2] * l.z;
            let a = (b.lambda * b.scale).as_vec3();
            self.ob_hits.push((id, -(ts[0] * a.x + ts[1] * a.y + ts[2] * a.z), at));
            let c = crate::bump::Contact { other: Some(id), other_ragdoll: true, impulse, normal: [n.x, n.y, n.z, 0.0], point: [at.x, at.y, at.z, 0.0],
                                           component: k as i32, extra: [0; 2] };
            self.contact.add(crate::bump::Gate { ragdoll: true, flagged: false, damages: false }, &c, mass, 0);
        }
        for b in &st.blocks {
            match b.held {
                Some(r) => self.rec_mut(r).imp = b.lambda.as_vec3(),
                None => {}
            }
        }
        for &(r, k, strength) in &self.grip_rows {
            if strength >= 0.0 && st.rows[r].lambda.abs() > strength as f64 {
                self.snapped.push(k);
            }
        }
        let mut gi = vec![0.0; st.qd0.len()];
        for r in &st.rows {
            gi.iter_mut().zip(&r.j).for_each(|(g, j)| *g += j * r.lambda);
        }
        for b in &st.blocks {
            for (j, l) in b.j.iter().zip((b.lambda * b.scale).to_array()) {
                gi.iter_mut().zip(j).for_each(|(g, j)| *g += j * l);
            }
        }
        for (k, g) in gi.chunks(6).enumerate().filter(|(_, g)| g.iter().any(|&x| x != 0.0)) {
            self.pending.push((k, st.x0.as_vec3(), DVec3::new(g[0], g[1], g[2]).as_vec3(), DVec3::new(g[3], g[4], g[5]).as_vec3()));
        }
        let vw = self.velocities(&st.artic, &st.lt, st.h, false);
        self.pending.clear();
        for (b, (v, w)) in self.bodies.iter_mut().zip(vw) {
            (b.v, b.w) = (v, w);
        }
        self.stage_vel = self.bodies.iter().map(|b| (b.v, b.w)).collect();
        if env_on!("NM_SVFREE") { self.stage_vel = st.free_vel.clone(); }
        if env_on!("NM_VELONLY") { return false; }
        let damp = (-DAMPING).fma(st.h, 1.0);
        for b in &mut self.bodies {
            b.v = b.v.clamp_length_max(MAX_SPEED) * damp;
            b.w = b.w.clamp_length_max(st.spin) * damp;
        }
        st.damp = damp;
        true
    }

    fn end(&mut self, mut st: Stage, qp: &[f64], lift: f32) {
        for (p, b) in st.push_blocks.iter().zip(&st.blocks) {
            match b.held {
                Some(r) => self.rec_mut(r).pimp = p.lambda.as_vec3(),
                None => {}
            }
        }
        for (k, b) in self.bodies.iter_mut().enumerate() {
            let (pv, pw) = artic::parts(std::array::from_fn(|c| qp[6 * k + c]));
            let (v, w) = ((pv + pw.cross(b.x.as_dvec3() - st.x0)).as_vec3(), pw.as_vec3());
            b.moved(v.clamp_length_max(17.0 / st.h) * st.damp, w.clamp_length_max(1.414 / st.h) * st.damp, st.h);
        }
        let root = &mut self.bodies[self.order[0]];
        let (p, w) = (root.x, root.w);
        root.v = (root.v + Vec3::new(p.z * w.y, p.x * w.z, p.y * w.x)) - Vec3::new(p.y * w.z, p.z * w.x, p.x * w.y);
        self.rejoin(&st.joints, &st.pose);
        self.recentre(&mut st.pose);
        if self.bodies.iter().any(|b| !(b.x.is_finite() && b.r.iter().all(|r| r.is_finite()) && b.v.is_finite() && b.w.is_finite())) {
            let back = st.o0 - self.o;
            for (b, &(x, q)) in self.bodies.iter_mut().zip(&st.start) {
                (b.x, b.v, b.w) = (x + back, Vec3::ZERO, Vec3::ZERO);
                b.set_q(q);
            }
        }
        self.sleep();
        self.asleep = self.mode == 2 && lift == 0.0 && self.pins.iter().all(Option::is_none) && self.grips.is_empty()
            && self.cons.iter().all(|c| c.0.is_none() && c.1.is_none());
    }
}

struct Stage {
    joints: Vec<Option<Joint>>,
    pose: Vec<Option<(Vec3, Quat)>>,
    start: Vec<(Vec3, Quat)>,
    o0: Vec3,
    artic: Artic,
    lt: Vec<V6>,
    x0: DVec3,
    qd0: Vec<f64>,
    rows: Vec<Row>,
    blocks: Vec<Block>,
    push: Vec<Row>,
    push_blocks: Vec<Block>,
    h: f32,
    spin: f32,
    free_vel: Vec<(Vec3, Vec3)>,
    damp: f32,
    free: Vec<f64>,
}

#[derive(Default)]
pub struct Group {
    held: std::collections::HashMap<(usize, usize, usize, usize), Vec<Held>>,
}

pub struct Member<'a> {
    pub doll: &'a mut Ragdoll,
    pub id: usize,
    pub build: &'a Build,
    pub drive: Drive<'a>,
    pub obstacles: &'a [Obstacle],
    pub others: &'a [Capsule],
}

#[derive(Clone, Copy)]
enum Src {
    Own(usize, usize),
    Cross((usize, usize, usize, usize), usize),
}

#[derive(Clone)]
struct Entry {
    node: usize,
    a: usize,
    b: usize,
    at: usize,
    src: Src,
}

impl Ragdoll {
    pub fn step_group(ms: &mut [Member], world: &Collision, dt: f32, group: &mut Group) {
        let n = ms.len();
        let mut st: Vec<Option<Stage>> = (0..n).map(|_| None).collect();
        Self::group_wake(ms);
        par_each(&mut ms.iter_mut().zip(st.iter_mut()).collect::<Vec<_>>(), |(m, s)| **s = m.doll.begin(m.build, world, dt, &m.drive, m.obstacles, m.others));
        Self::group_solve(ms, st, group);
    }

    fn group_boxes(ms: &[Member]) -> Vec<(Vec3, Vec3)> {
        let reach = |m: &Member| -> (Vec3, Vec3) {
            let d = &m.doll;
            let (lo, hi) = d.bodies.iter().zip(&m.build.parts).fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(lo, hi), (b, p)| {
                let r = Vec3::splat(bound_half(p).length() + MARGIN);
                (lo.min(b.x - r), hi.max(b.x + r))
            });
            (lo + d.o, hi + d.o)
        };
        ms.iter().map(reach).collect()
    }

    fn group_wake(ms: &mut [Member]) {
        let n = ms.len();
        let boxes = Self::group_boxes(ms);
        let meet = |i: usize, k: usize| boxes[i].0.cmple(boxes[k].1).all() && boxes[k].0.cmple(boxes[i].1).all();
        for i in 0..n {
            for k in 0..n {
                if i != k && ms[i].doll.asleep && !ms[k].doll.asleep && meet(i, k) {
                    ms[i].doll.wake();
                }
            }
        }
    }

    fn group_solve(ms: &mut [Member], mut st: Vec<Option<Stage>>, group: &mut Group) {
        let n = ms.len();
        let boxes = Self::group_boxes(ms);
        let meet = |i: usize, k: usize| boxes[i].0.cmple(boxes[k].1).all() && boxes[k].0.cmple(boxes[i].1).all();
        let mut joined: Vec<usize> = (0..n).collect();
        fn root(j: &mut [usize], mut i: usize) -> usize {
            while j[i] != i {
                i = j[i];
            }
            i
        }
        let frame_of = |d: &Ragdoll, t: &[Mat3], a: usize| {
            let f = (d.bodies[a].x + d.o, t[a]);
            (f, d.seen.get(a).map_or(f, |s| (s.0 + d.o, s.1)))
        };
        let parts: Vec<Vec<((Vec3, Mat3), (Vec3, Mat3), (Vec3, Vec3))>> = (0..n).map(|i| if st[i].is_none() { vec![] } else {
            let d = &*ms[i].doll;
            let t = d.turns();
            (0..d.bodies.len()).map(|a| { let (f, s) = frame_of(d, &t, a); (f, s, swept_box(&ms[i].build.parts[a], f, s)) }).collect()
        }).collect();
        struct Job { i: usize, k: usize, cross: Vec<(usize, usize, Block, Block, Src)>, held: Vec<((usize, usize, usize, usize), Vec<Held>)>, joined: bool }
        let mut jobs = vec![];
        for i in 0..n {
            for k in i + 1..n {
                let (i, k) = if ms[i].id < ms[k].id { (i, k) } else { (k, i) };
                if st[i].is_some() && st[k].is_some() && meet(i, k) {
                    jobs.push(Job { i, k, cross: vec![], held: vec![], joined: false });
                }
            }
        }
        let kept = std::mem::take(&mut group.held);
        {
            let (ms, st, parts, kept) = (&*ms, &st, &parts, &kept);
            par_each(&mut jobs, |job| {
                let (i, k) = (job.i, job.k);
                let (Some(si), Some(sk)) = (&st[i], &st[k]) else { return };
                let (di, dk) = (&*ms[i].doll, &*ms[k].doll);
                for (a, &(fa, sa, ba)) in parts[i].iter().enumerate() {
                    for (c, &(fc, sc, bc)) in parts[k].iter().enumerate() {
                        if ba.0.distance_squared(bc.0) > (ba.1.length() + bc.1.length()).powi(2) {
                            continue;
                        }
                        let key = (ms[i].id, a, ms[k].id, c);
                        let mut h = kept.get(&key).cloned().unwrap_or_default();
                        let (pa, pc) = (&ms[i].build.parts[a], &ms[k].build.parts[c]);
                        Self::pair_manifold(&mut h, (pa, fa, sa), (pc, fc, sc), [ba, bc]);
                        if h.is_empty() {
                            continue;
                        }
                        let off = (di.contact_off >> a | dk.contact_off >> c) & 1 != 0;
                        h.iter_mut().for_each(|r| r.off |= off);
                        for (x, r) in h.iter().enumerate().filter(|(_, r)| !r.off) {
                            let (at_a, at_c) = (fa.0 + fa.1 * r.local, fc.0 + fc.1 * r.ground);
                            let (nn, over) = (r.n, r.gap + ALLOW);
                            let rows = |d: &Ragdoll, s: &Stage, k: usize, at: Vec3, sign: f64| frame(nn).map(|t| {
                                let mut j = point_row(d.bodies.len(), s.x0, k, (at - d.o).as_dvec3(), t.as_dvec3());
                                j.iter_mut().for_each(|x| *x *= sign);
                                let imp: Vec<V6> = j.chunks(6).map(|c| std::array::from_fn(|i| c[i])).collect();
                                let mj: Vec<f64> = s.artic.solve(&imp).into_iter().flatten().collect();
                                (j, mj)
                            });
                            let (ra, rc) = (rows(di, si, a, at_a, 1.0), rows(dk, sk, c, at_c, -1.0));
                            let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(x, y)| x * y).sum::<f64>();
                            let k3 = DMat3::from_cols_array_2d(&std::array::from_fn(|p| std::array::from_fn(|q| dot(&ra[q].0, &ra[p].1) + dot(&rc[q].0, &rc[p].1))));
                            let fv = |t: usize| dot(&ra[t].0, &si.free) + dot(&rc[t].0, &sk.free);
                            let e = (PART_ELASTICITY * PART_ELASTICITY) as f64;
                            let to = DVec3::new((-over.max(0.0) / si.h) as f64 - fv(0) / (1.0 + e), -fv(1), -fv(2));
                            let both = |t: usize, m: bool| -> Vec<f64> { let (x, y) = if m { (&ra[t].1, &rc[t].1) } else { (&ra[t].0, &rc[t].0) }; x.iter().chain(y).copied().collect() };
                            let b = Block { scale: elastic(e), ..Block::with(std::array::from_fn(|t| both(t, false)), std::array::from_fn(|t| both(t, true)), k3, to,
                                (part_friction(pa) * part_friction(pc)) as f64, None, r.imp) };
                            let pb = Self::push_block(&b, over, si.h, r.pimp);
                            job.cross.push((i, k, b, pb, Src::Cross(key, x)));
                        }
                        job.joined |= h.iter().any(|r| !r.off);
                        job.held.push((key, h));
                    }
                }
            });
        }
        let mut held = std::collections::HashMap::new();
        let mut cross: Vec<Option<(usize, usize, Block, Block, Src)>> = vec![];
        for job in jobs {
            cross.extend(job.cross.into_iter().map(Some));
            held.extend(job.held);
            if job.joined {
                let (ri, rk) = (root(&mut joined, job.i), root(&mut joined, job.k));
                joined[ri.max(rk)] = ri.min(rk);
            }
        }
        group.held = held;
        let tops: Vec<usize> = (0..n).map(|i| root(&mut joined, i)).collect();
        let alone: Vec<bool> = (0..n).map(|i| st[i].is_some() && (0..n).filter(|&k| st[k].is_some() && tops[k] == tops[i]).count() == 1).collect();
        for (m, s) in ms.iter_mut().zip(st.iter_mut()).zip(&alone).filter(|x| *x.1).map(|x| x.0) {
            m.doll.finish(s.take().unwrap(), m.drive.lift);
        }
        for top in 0..n {
            let mut is: Vec<usize> = (0..n).filter(|&i| st[i].is_some() && tops[i] == top).collect();
            if is.is_empty() {
                continue;
            }
            is.sort_by_key(|&i| ms[i].id);
            let mut off = vec![0; n];
            let mut total = 0;
            for &i in &is {
                off[i] = total;
                total += st[i].as_ref().unwrap().qd0.len();
            }
            let alone_at = |at: usize| Spot { at: [at, 0], cut: usize::MAX };
            let lay_row = |r: Row, at: Spot, base: isize| Row { kind: match r.kind { RowKind::Friction { of, mu } => RowKind::Friction { of: of.wrapping_add_signed(base), mu }, k => k }, at, ..r };
            let mut firsts = vec![(0, 0); n];
            let mut counts = vec![(0, 0); n];
            let (mut rows, mut push): (Vec<Row>, Vec<Row>) = (vec![], vec![]);
            let (mut bank, mut push_bank): (Vec<Option<Block>>, Vec<Option<Block>>) = (vec![], vec![]);
            let mut list: Vec<Entry> = vec![];
            let mut nodes: Vec<(Option<usize>, Option<usize>)> = vec![];
            for &i in &is {
                let s = st[i].as_mut().unwrap();
                let (rb, pbase) = (rows.len(), push.len());
                (firsts[i], counts[i]) = ((rb, pbase), (s.rows.len(), s.push.len()));
                rows.extend(std::mem::take(&mut s.rows).into_iter().map(|r| lay_row(r, alone_at(off[i]), rb as isize)));
                push.extend(std::mem::take(&mut s.push).into_iter().map(|r| lay_row(r, alone_at(off[i]), pbase as isize)));
                let s = &*s;
                let mut idx: Vec<usize> = (0..s.blocks.len()).collect();
                island_sort(&mut idx);
                let mut pre = vec![0; idx.len()];
                for (p, &o) in idx.iter().enumerate() {
                    pre[o] = p;
                }
                let n_links = ms[i].doll.bodies.len();
                let is_self = |p: usize| s.blocks[p].held.is_some_and(|(k, _)| k >= n_links);
                let (ground, own) = (nodes.len(), ms[i].id + 1);
                if pre.iter().any(|&p| !is_self(p)) {
                    nodes.push((Some(own), None));
                }
                let own_self = nodes.len();
                if pre.iter().any(|&p| is_self(p)) {
                    nodes.push((Some(own), Some(own)));
                }
                let base = bank.len();
                list.extend(pre.iter().map(|&p| Entry { node: if is_self(p) { own_self } else { ground }, a: own, b: if is_self(p) { own } else { 0 },
                    at: base + p, src: Src::Own(i, p) }));
                let s = st[i].as_mut().unwrap();
                bank.extend(std::mem::take(&mut s.blocks).into_iter().map(|b| Some(Block { at: alone_at(off[i]), ..b })));
                push_bank.extend(std::mem::take(&mut s.push_blocks).into_iter().map(|b| Some(Block { at: alone_at(off[i]), ..b })));
            }
            let mut pairs_across: Vec<((usize, usize), usize)> = vec![];
            for (i, k, b, pb, from) in cross.iter_mut().filter(|c| c.as_ref().is_some_and(|c| is.contains(&c.0))).map(|c| c.take().unwrap()) {
                let (i, k) = (&i, &k);
                let spot = Spot { at: [off[*i], off[*k]], cut: st[*i].as_ref().unwrap().qd0.len() };
                let node = match pairs_across.iter().find(|x| x.0 == (*i, *k)) {
                    Some(x) => x.1,
                    None => {
                        nodes.push((Some(ms[*i].id + 1), Some(ms[*k].id + 1)));
                        pairs_across.push(((*i, *k), nodes.len() - 1));
                        nodes.len() - 1
                    }
                };
                list.push(Entry { node, a: ms[*i].id + 1, b: ms[*k].id + 1, at: bank.len(), src: from });
                bank.push(Some(Block { at: spot, ..b }));
                push_bank.push(Some(Block { at: spot, ..pb }));
            }
            let at: Vec<usize> = island_order(&nodes).iter().enumerate().fold(vec![0; nodes.len()], |mut at, (r, &x)| { at[x] = r; at });
            list.sort_by_key(|x| at[x.node]);
            let mut pushed = list.clone();
            sgi_sort(&mut list, |x| x.a);
            sgi_sort(&mut pushed, |x| x.b);
            let mut blocks: Vec<Block> = list.iter().map(|x| bank[x.at].take().unwrap()).collect();
            let mut push_blocks: Vec<Block> = pushed.iter().map(|x| push_bank[x.at].take().unwrap()).collect();
            let mut qd: Vec<f64> = is.iter().flat_map(|&i| st[i].as_ref().unwrap().qd0.clone()).collect();
            impulses(&mut rows, &mut blocks, &mut qd);
            let part = |b: &Block, first: bool| {
                let side = |j: &Vec<f64>| if first { j[..b.at.cut].to_vec() } else { j[b.at.cut..].to_vec() };
                Block { j: b.j.each_ref().map(side), mj: b.mj.each_ref().map(side), held: None, at: Spot::OWN, ..b.clone() }
            };
            let mut own: Vec<Vec<Option<Block>>> = (0..n).map(|_| vec![]).collect();
            let mut theirs: Vec<Vec<Block>> = (0..n).map(|_| vec![]).collect();
            for (x, b) in list.iter().zip(blocks) {
                match x.src {
                    Src::Own(i, p) => {
                        if own[i].len() <= p {
                            own[i].resize_with(p + 1, || None);
                        }
                        own[i][p] = Some(Block { at: Spot::OWN, ..b });
                    }
                    Src::Cross(key, at) => {
                        group.held.get_mut(&key).unwrap()[at].imp = b.lambda.as_vec3();
                        let (i, k) = is.iter().fold((usize::MAX, usize::MAX), |acc, &m| if ms[m].id == key.0 { (m, acc.1) } else if ms[m].id == key.2 { (acc.0, m) } else { acc });
                        for (m, first) in [(i, true), (k, false)] {
                            theirs[m].push(part(&b, first));
                        }
                    }
                }
            }
            let mut rows = rows.into_iter();
            for &i in &is {
                let s = st[i].as_mut().unwrap();
                s.rows = rows.by_ref().take(counts[i].0).map(|r| lay_row(r, Spot::OWN, -(firsts[i].0 as isize))).collect();
                s.blocks = std::mem::take(&mut own[i]).into_iter().map(|b| b.unwrap()).chain(std::mem::take(&mut theirs[i])).collect();
            }
            let mut go = vec![false; n];
            for &i in &is {
                go[i] = ms[i].doll.middle(st[i].as_mut().unwrap());
            }
            let mut qp = vec![0.0; total];
            let push_was: Vec<f64> = push.iter().map(|r| r.lambda).collect();
            impulses(&mut push, &mut push_blocks, &mut qp);
            let mut own: Vec<Vec<Option<Block>>> = (0..n).map(|_| vec![]).collect();
            for (x, b) in pushed.iter().zip(push_blocks) {
                match x.src {
                    Src::Own(i, p) => {
                        if own[i].len() <= p {
                            own[i].resize_with(p + 1, || None);
                        }
                        own[i][p] = Some(Block { at: Spot::OWN, ..b });
                    }
                    Src::Cross(key, at) => group.held.get_mut(&key).unwrap()[at].pimp = b.lambda.as_vec3(),
                }
            }
            let mut push = push.into_iter();
            for &i in &is {
                let s = st[i].as_mut().unwrap();
                let was = &push_was[firsts[i].1..];
                s.push = push.by_ref().take(counts[i].1).zip(was).map(|(r, &lambda)| Row { lambda, ..lay_row(r, Spot::OWN, -(firsts[i].1 as isize)) }).collect();
                s.push_blocks = std::mem::take(&mut own[i]).into_iter().map(|b| b.unwrap()).collect();
            }
            for &i in &is {
                let s = st[i].take().unwrap();
                if go[i] {
                    let len = s.qd0.len();
                    ms[i].doll.end(s, &qp[off[i]..off[i] + len], ms[i].drive.lift);
                }
            }
        }
    }
}

fn point_row(n_links: usize, xr: DVec3, k: usize, at: DVec3, n: DVec3) -> Vec<f64> {
    let mut r = vec![0.0; 6 * n_links];
    r[6 * k..6 * k + 6].copy_from_slice(&v6(n, (at - xr).cross(n)));
    r
}

fn spatial(bodies: &[Body], x0: DVec3) -> Vec<f64> {
    spatial_of(&bodies.iter().map(|b| (b.v, b.w)).collect::<Vec<_>>(), bodies, x0)
}

fn spatial_of(vw: &[(Vec3, Vec3)], bodies: &[Body], x0: DVec3) -> Vec<f64> {
    vw.iter().zip(bodies).flat_map(|(&(v, w), b)| {
        let w = w.as_dvec3();
        v6(v.as_dvec3() - w.cross(b.x.as_dvec3() - x0), w)
    }).collect()
}

pub fn pose_frames(parents: &[Option<usize>], pose: &[(Vec3, Quat)], root: Transform) -> Vec<(Vec3, Quat)> {
    let mut out: Vec<(Vec3, Quat)> = Vec::with_capacity(pose.len());
    for (i, &(t, r)) in pose.iter().enumerate() {
        let (pt, pr) = parents[i].map_or((root.translation, root.rotation), |p| out[p]);
        out.push((pt + pr * t, pr * r));
    }
    out
}

pub struct Rig {
    pub build: Build,
    part_of: Vec<Option<usize>>,
    parents: Vec<Option<usize>>,
}

impl Rig {
    pub fn new(parts: &[delirium_data::ragdoll::Part], parents: &[Option<usize>], bind: &[(Vec3, Quat)], bones: &[delirium_data::skeleton::Bone], nm: usize) -> Option<Rig> {
        if parts.is_empty() {
            return None;
        }
        let model: Vec<Transform> = pose_frames(parents, bind, Transform::IDENTITY).into_iter()
            .map(|(t, r)| Transform::from_translation(t).with_rotation(r)).collect();
        let build = Build::new(parts, &model, bones, nm);
        let part_of = (0..bind.len()).map(|b| build.parts.iter().position(|d| d.bone == b)).collect();
        Some(Rig { build, part_of, parents: parents.to_vec() })
    }

    pub fn place(&self, pose: &[(Vec3, Quat)], body: Transform) -> Vec<Transform> {
        let bones = pose_frames(&self.parents, pose, body);
        self.build.parts.iter().map(|d| {
            let (bt, br) = bones[d.bone];
            Transform::from_translation(bt + br * d.offset.translation).with_rotation(br * d.offset.rotation)
        }).collect()
    }

    pub fn start(&self, pose: &[(Vec3, Quat)], body: Transform, prev: Option<(&[(Vec3, Quat)], Transform, f32)>, v_ref: Vec3) -> Ragdoll {
        let now = self.place(pose, body);
        let mut frames = now.clone();
        for c in 0..frames.len() {
            if let Some(p) = self.build.parts[c].parent {
                let pivot = frames[p].translation + frames[p].rotation * self.build.parts[c].anchor_parent;
                frames[c].translation = pivot - frames[c].rotation * self.build.parts[c].anchor;
            }
        }
        let mut doll = Ragdoll::new(&self.build, &frames, Vec3::ZERO);
        if let Some((pp, pb, dt)) = prev.filter(|p| p.2 > 0.0) {
            let rows = |t: &Transform| { let m = Mat3::from_quat(t.rotation); [m.x_axis, m.y_axis, m.z_axis, t.translation] };
            for ((b, c), p) in doll.bodies.iter_mut().zip(&now).zip(self.place(pp, pb)) {
                (_, b.v, b.w) = crate::ragstart::start_part(rows(c), Some(&rows(&p)), Vec3::ZERO, dt, [0.5, 3.0, 2.0], v_ref);
            }
        }
        let off = -crate::rigid::row_mul(crate::nmrs::cg(doll.nm), &doll.inst());
        doll.bodies.iter_mut().for_each(|b| b.x += off);
        doll
    }

    pub fn stand(&mut self, idle: &[(Vec3, Quat)]) {
        let mut frames = self.place(idle, Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)));
        for (p, f) in self.build.parts.iter_mut().zip(&frames) {
            p.bind = f.rotation;
        }
        crate::nmrs::asset_layout(&mut self.build, &mut frames);
    }

    pub fn derive_links(&self, idle: &[(Vec3, Quat)]) -> bool {
        let nm = self.build.nm;
        if (nm == crate::nmrs::HOMER && crate::nmdbm::logged_links().is_some()) || crate::nmdbm::DERIVED_LINKS[nm].get().is_some() {
            return false;
        }
        let stand: Vec<Quat> = self.place(idle, Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))).iter().map(|f| f.rotation).collect();
        crate::nmrs::derive_links(&self.build, &stand).is_some_and(|l| crate::nmdbm::DERIVED_LINKS[nm].set(l).is_ok())
    }

    pub fn dump(&self, pose: &[(Vec3, Quat)], body: Transform) -> String {
        self.build.dump(&self.place(pose, body))
    }

    pub fn rise(&self, doll: &Ragdoll, firsts: &[Vec<(Vec3, Quat)>]) -> (usize, f32, Vec3) {
        let now: Vec<Transform> = doll.frames().collect();
        let spine = self.build.parts.iter().position(|d| d.parent == Some(0) && !d.lower).unwrap_or(0);
        let yaw = |q: Quat| { let a = q * Vec3::Y; a.y.atan2(a.x) };
        firsts.iter().enumerate().map(|(k, pose)| {
            let first = self.place(pose, Transform::IDENTITY);
            let h = yaw(now[spine].rotation) - yaw(first[spine].rotation);
            let turn = Quat::from_rotation_z(h);
            ((turn * first[spine].rotation).angle_between(now[spine].rotation), k, h, now[0].translation - turn * first[0].translation)
        }).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, k, h, at)| (k, h, at)).expect("a get-up clip")
    }

    pub fn settle(&self, heading: f32, end: &[(Vec3, Quat)], end_turn: Quat, next: &[(Vec3, Quat)]) -> (Vec3, f32) {
        let (a, _, _) = end_turn.to_euler(EulerRot::ZXY);
        let pelvis = |pose: &[(Vec3, Quat)]| self.place(pose, Transform::IDENTITY)[0].translation;
        let moved = Quat::from_rotation_z(heading) * pelvis(end) - Quat::from_rotation_z(heading + a) * pelvis(next);
        (moved.with_z(0.0), a)
    }

    pub fn bones(&self, doll: &Ragdoll, pose: &[(Vec3, Quat)], facing: Quat) -> (Transform, Vec<(Vec3, Quat)>) {
        let frames: Vec<Transform> = doll.frames().collect();
        let bone_at = |k: usize| {
            let (f, o) = (frames[k], self.build.parts[k].offset);
            let r = f.rotation * o.rotation.inverse();
            (f.translation - r * o.translation, r)
        };
        let (pt, pr) = bone_at(0);
        let (lt, lr) = pose[self.build.parts[0].bone];
        let root = (pt - (pr * lr.inverse()) * lt, pr * lr.inverse());
        let mut placed: Vec<(Vec3, Quat)> = Vec::with_capacity(pose.len());
        for (b, &(lt, lr)) in pose.iter().enumerate() {
            placed.push(match (self.part_of[b], self.parents[b]) {
                (Some(k), _) => bone_at(k),
                (None, Some(q)) => (placed[q].0 + placed[q].1 * lt, placed[q].1 * lr),
                (None, None) => root,
            });
        }
        let body = Transform::from_translation(frames[0].translation).with_rotation(facing);
        let local = (0..pose.len()).map(|b| {
            let (pt, pr) = self.parents[b].map_or((body.translation, body.rotation), |q| placed[q]);
            (pr.inverse() * (placed[b].0 - pt), pr.inverse() * placed[b].1)
        }).collect();
        (body, local)
    }
}

pub(crate) fn island_order(nodes: &[(Option<usize>, Option<usize>)]) -> Vec<usize> {
    struct Forest {
        parent: Vec<Option<usize>>,
        rank: Vec<u32>,
        kids: Vec<Vec<usize>>,
        roots: Vec<usize>,
    }
    impl Forest {
        fn hang(&mut self, x: usize, p: usize) {
            if self.parent[x] == Some(p) {
                return;
            }
            match self.parent[x] {
                Some(q) => self.kids[q].retain(|&c| c != x),
                None => self.roots.retain(|&c| c != x),
            }
            self.parent[x] = Some(p);
            self.kids[p].push(x);
        }
        fn find(&mut self, x: usize) -> usize {
            let mut r = x;
            while let Some(p) = self.parent[r] {
                r = p;
            }
            let mut c = x;
            while c != r {
                let next = self.parent[c].unwrap();
                self.hang(c, r);
                c = next;
            }
            r
        }
        fn join(&mut self, x: usize, y: usize) -> usize {
            if x == y {
                x
            } else if self.rank[x] > self.rank[y] {
                self.hang(y, x);
                x
            } else if self.rank[x] < self.rank[y] {
                self.hang(x, y);
                y
            } else {
                self.hang(y, x);
                self.rank[x] += 1;
                x
            }
        }
        fn walk(&self, x: usize, out: &mut Vec<usize>) {
            out.push(x);
            for &k in &self.kids[x] {
                self.walk(k, out);
            }
        }
    }
    let n = nodes.len();
    let mut f = Forest { parent: vec![None; n], rank: vec![0; n], kids: vec![vec![]; n], roots: vec![] };
    let mut last = std::collections::HashMap::new();
    for (m, &(a, b)) in nodes.iter().enumerate() {
        f.roots.push(m);
        let mut cur = m;
        if let Some(&l) = a.and_then(|a| last.get(&a)) {
            let x = f.find(l);
            cur = f.join(x, cur);
        }
        if let Some(b) = b.filter(|&b| Some(b) != a) {
            if let Some(&l) = last.get(&b) {
                let x = f.find(l);
                cur = f.join(x, cur);
            }
            last.insert(b, cur);
        }
        if let Some(a) = a {
            last.insert(a, cur);
        }
    }
    let mut out = vec![];
    for &r in &f.roots.clone() {
        f.walk(r, &mut out);
    }
    out
}

fn island_sort<T>(v: &mut [T]) {
    sgi_sort(v, |_| 0);
}

fn sgi_sort<T>(v: &mut [T], key: impl Fn(&T) -> usize + Copy) {
    fn split<T>(mut v: &mut [T], key: impl Fn(&T) -> usize + Copy) {
        while v.len() > 16 {
            let (a, b, c) = (key(&v[0]), key(&v[v.len() / 2]), key(&v[v.len() - 1]));
            let pivot = if a < b { if b < c { b } else if a < c { c } else { a } } else if a < c { a } else if b < c { c } else { b };
            let (mut f, mut l) = (0, v.len());
            loop {
                while key(&v[f]) < pivot {
                    f += 1;
                }
                l -= 1;
                while pivot < key(&v[l]) {
                    l -= 1;
                }
                if f >= l {
                    break;
                }
                v.swap(f, l);
                f += 1;
            }
            let (lo, hi) = std::mem::take(&mut v).split_at_mut(f);
            split(hi, key);
            v = lo;
        }
    }
    split(v, key);
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && key(&v[j]) < key(&v[j - 1]) {
            v.swap(j, j - 1);
            j -= 1;
        }
    }
}

#[test]
fn island_sort_as_the_game() {
    let sorted = |n: usize| { let mut v: Vec<usize> = (0..n).collect(); island_sort(&mut v); v };
    assert_eq!(sorted(16), (0..16).collect::<Vec<_>>());
    assert_eq!(sorted(31), (0..31).rev().collect::<Vec<_>>());
    assert_eq!(sorted(40), (20..40).chain(0..20).collect::<Vec<_>>());
}

#[test]
fn island_order_as_the_game() {
    assert_eq!(island_order(&[(Some(1), None), (Some(1), Some(1))]), [0, 1]);
    assert_eq!(island_order(&[(Some(1), None), (Some(1), Some(1)), (Some(2), None), (Some(2), Some(2)), (Some(1), Some(2))]), [2, 3, 0, 1, 4]);
    assert_eq!(island_order(&[(Some(1), Some(2)), (Some(1), None), (Some(1), Some(1)), (Some(2), None), (Some(2), Some(2))]), [0, 1, 2, 3, 4]);
    assert_eq!(island_order(&[(Some(1), None), (Some(2), None), (Some(3), None), (Some(1), Some(2)), (Some(2), Some(3))]), [0, 3, 1, 4, 2]);
}

pub(crate) fn par_each<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync) {
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get()).min(items.len() / 3);
    if cores < 2 {
        return items.iter_mut().for_each(f);
    }
    let f = &f;
    std::thread::scope(|s| items.chunks_mut(items.len().div_ceil(cores)).for_each(|c| { s.spawn(move || c.iter_mut().for_each(f)); }));
}
