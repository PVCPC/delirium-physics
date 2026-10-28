use crate::{Fma, Vd, Vx};
use bevy_math::{DMat3, DVec3};
use crate::prelude::*;

use crate::artic::{self, Artic, V6, v6};
use crate::collision::Collision;
use crate::gjk;
use crate::nmdbm::{sdot, vnorm};

#[allow(non_snake_case)]
#[inline]
pub fn GRAVITY() -> f32 { delirium_data::kv!(31) }
#[allow(non_snake_case)]
#[inline]
fn MARGIN() -> f32 { delirium_data::kv!(128) }
#[allow(non_snake_case)]
#[inline]
fn WORLD_MARGIN() -> f32 { delirium_data::kv!(567) }
fn part_friction(p: &PartDef) -> f32 {
    if p.name.ends_with("Foot") { 1.0 } else if p.name.ends_with("Hand") { delirium_data::kv!(134) } else { delirium_data::kv!(18) }
}
#[allow(non_snake_case)]
#[inline]
fn PART_ELASTICITY() -> f32 { delirium_data::kv!(110) }
#[allow(non_snake_case)]
#[inline]
fn ELASTICITY() -> f32 { delirium_data::kv!(73) }
pub static SURFACES: std::sync::RwLock<Vec<[f32; 2]>> = std::sync::RwLock::new(Vec::new());
#[allow(non_snake_case)]
#[inline]
fn SLEEP_V2() -> f32 { delirium_data::kv!(48) }
#[allow(non_snake_case)]
#[inline]
fn SLEEP_W2() -> f32 { delirium_data::kv!(567) }
#[allow(non_snake_case)]
#[inline]
fn SLEEP_MEAN2() -> f32 { delirium_data::kv!(57) }
#[allow(non_snake_case)]
#[inline]
fn SLEEP_STOP() -> u32 { delirium_data::kv!(194 int) }
#[allow(non_snake_case)]
#[inline]
fn SLEEP_SLEEP() -> u32 { delirium_data::kv!(122 int) }

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
        let free: Vec<usize> = (0..delirium_data::kv!(0 int)).filter(|&k| flags & (delirium_data::kv!(51 int) << k) != 0 && flags & (0x2 << k) != 0 && lo[k] != hi[k]).collect();
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
                let t = if free.len() == delirium_data::kv!(0 int) { 2 } else { (0..delirium_data::kv!(0 int)).find(|k| !free.contains(k)).unwrap() };
                let (x, y) = ((t + 1) % delirium_data::kv!(0 int), (t + 2) % delirium_data::kv!(0 int));
                let f = Quat::from_mat3(&Mat3::from_cols(e[x], e[y], e[t]));
                let half = |k: usize| { let h = (hi[k] - lo[k]) * 0.5; if h >= pi { delirium_data::kv!(112) * pi } else { h } };
                let twist = if free.len() == delirium_data::kv!(0 int) { if (hi[2] - lo[2]) * 0.5 >= pi { f32::MAX } else { ((hi[2] - lo[2]) * 0.5).max(delirium_data::kv!(48)) } } else { delirium_data::kv!(48) };
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
    pub length: f32,
}

fn box_unit(e: Vec3) -> Vec3 {
    let [x, y, z] = [e.x * 2.0, e.y * 2.0, e.z * 2.0].map(|s| s * s);
    Vec3::new(z + y, x + z, x + y) * delirium_data::kv!(568 f32)
}

fn capsule_unit(r: f32, l: f32) -> Vec3 {
    let (rr, ll) = (r * r, l * l);
    let k = 1.0 / r.mul_add(delirium_data::kv!(569 f32), l);
    let y = r.mul_add(delirium_data::kv!(570 f32), l) * rr * k * 0.5;
    let s = (ll * l).mul_add(delirium_data::kv!(17), (rr * r).mul_add(delirium_data::kv!(487), ll * r));
    let x = (rr * l).mul_add(delirium_data::kv!(134), s) * k * delirium_data::kv!(550 f32);
    Vec3::new(x, y, x)
}

impl PartDef {
    fn unit(&self) -> Option<Vec3> {
        match self.boxed {
            Some(e) => Some(box_unit(e)),
            None => (self.length > 0.0).then(|| capsule_unit(self.radius, self.length)),
        }
    }

    fn moments(&self) -> Vec3 {
        if self.inertia.min_element() > 0.0 {
            return self.inertia;
        }
        let (m, r, l) = (self.mass.max(delirium_data::kv!(73)), self.radius, 2.0 * (self.half + self.radius));
        match self.boxed {
            Some(e) => Vec3::new(e.y * e.y + e.z * e.z, e.x * e.x + e.z * e.z, e.x * e.x + e.y * e.y) * (m / delirium_data::kv!(5)),
            None => Vec3::new(m * (delirium_data::kv!(5) * r * r + l * l) / delirium_data::kv!(21), 0.5 * m * r * r, m * (delirium_data::kv!(5) * r * r + l * l) / delirium_data::kv!(21)),
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
            let q = |i: usize| Quat::from_xyzw(f(i), f(i + 1), f(i + 2), f(i + delirium_data::kv!(0 int)));
            let limit = match w.iter().position(|&t| t == "j") {
                Some(k) if w[k + 1] == "b" => Limit::Bone { pf: q(k + 2), cf: q(k + delirium_data::kv!(6 int)), lean: [f(k + delirium_data::kv!(77 int)), f(k + delirium_data::kv!(106 int))], twist: f(k + delirium_data::kv!(52 int)) },
                Some(k) if w[k + 1] == "h" => Limit::BoneHinge { pf: q(k + 2), cf: q(k + delirium_data::kv!(6 int)), lo: f(k + delirium_data::kv!(77 int)), hi: f(k + delirium_data::kv!(106 int)) },
                Some(_) => Limit::Locked,
                None if w[delirium_data::kv!(98 int)] == "c" => Limit::Cone { swing: f(delirium_data::kv!(99 int)), twist: f(delirium_data::kv!(100 int)) },
                None => Limit::Hinge { axis: v(delirium_data::kv!(99 int)), lo: f(delirium_data::kv!(102 int)), hi: f(delirium_data::kv!(332 int)), twist: f(delirium_data::kv!(81 int)) },
            };
            parts.push(PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: parts.len(), parent: w[1].parse::<i32>().ok().filter(|&j| j >= 0).map(|j| j as usize), mass: f(2), radius: f(delirium_data::kv!(0 int)), half: f(delirium_data::kv!(1 int)),
                                 offset: Transform::from_translation(v(delirium_data::kv!(2 int))).with_rotation(q(delirium_data::kv!(23 int))), rest: q(delirium_data::kv!(52 int)), anchor_parent: v(delirium_data::kv!(51 int)), anchor: v(delirium_data::kv!(195 int)),
                                 limit, lower: w[delirium_data::kv!(477 int)] == "1", reach: w[delirium_data::kv!(277 int)] == "1", name: w[0].to_string(),
                                 bind: w.iter().position(|&t| t == "B").map_or(q(delirium_data::kv!(438 int)), |k| q(k + 1)), boxed: (w.get(delirium_data::kv!(386 int)) == Some(&"b")).then(|| v(delirium_data::kv!(422 int))),
                                 inertia: w.iter().position(|&t| t == "i").map_or(Vec3::ZERO, |k| v(k + 1)),
                                 length: w.iter().position(|&t| t == "L").map_or(0.0, |k| f(k + 1)) });
            frames.push(Transform::from_translation(v(delirium_data::kv!(457 int))).with_rotation(q(delirium_data::kv!(438 int))));
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
                    p.lower as u8, p.reach as u8, v(f.translation), q(f.rotation), p.boxed.map_or(String::new(), |e| format!(" b {}", v(e))), v(p.inertia)) + &bone + &format!(" L {}", p.length)
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
                    (theirs.length() / ours.length().max(delirium_data::kv!(68)), (arc * t0).normalize())
                }
                _ => {
                    let spread = |at: &dyn Fn(usize) -> Vec3| kids.iter().map(|&c| (at(c) - at(k)).length()).sum::<f32>();
                    let s = spread(&|c| joints[c]) / spread(&pivot).max(delirium_data::kv!(68));
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
            (p.size, p.radius, p.half, p.length, p.anchor) = (p.size * s, p.radius * s, p.half * s, p.length * s, p.anchor * s);
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
        let linked = parts.len() == delirium_data::kv!(104 int) && crate::nmrs::PARTS[nm].get().is_some_and(|p| p.len() == parts.len());
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
            let (radius, half) = if p.kind == delirium_data::kv!(0 int) { (p.extent.iter().copied().fold(f32::MAX, f32::min), 0.0) } else { (p.radius, p.half) };
            PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: p.bone, parent, mass: p.mass, radius, half,
                      offset: Transform::from_matrix(bind[p.bone].to_matrix().inverse() * f.to_matrix()),
                      rest, anchor_parent, anchor: f.rotation.inverse() * (joint - f.translation), limit,
                      lower: i == pelvis || ["Thigh", "Calf", "Foot"].iter().any(|l| name.ends_with(l)),
                      reach: name.ends_with("UpperArm") || name.ends_with("Forearm"), name: name.to_string(), bind: f.rotation,
                      boxed: (p.kind == delirium_data::kv!(0 int)).then(|| Vec3::from(p.extent)), inertia: Vec3::from(p.inertia), length: p.length }
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
    pub entity: u8,
    pub first: bool,
    pub inv_mass: f32,
    pub force: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct ObPart {
    pub shape: gjk::Shape,
    pub m: Mat3,
    pub at: Vec3,
    pub lo: Vec3,
    pub hi: Vec3,
    pub mesh: Option<&'static crate::sphere::Mesh>,
    pub last: Option<(Mat3, Vec3)>,
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

    fn friction(&self) -> f32 {
        if self.entity == delirium_data::kv!(0 int) { 0.0 } else { CAR_FRICTION() }
    }
}
#[allow(non_snake_case)]
#[inline]
fn CAR_FRICTION() -> f32 { delirium_data::kv!(57) }

pub type Capsule = (Vec3, Vec3, f32, Vec3, bool);

pub fn nearest_point(a: Vec3, b: Vec3, x: Vec3) -> (Vec3, f32) {
    let d = b - a;
    let t = if d.length_squared() > delirium_data::kv!(72) { ((x - a).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t, t)
}

fn nearest(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (a1 - a0, b1 - b0, a0 - b0);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (c, b) = (d1.dot(r), d1.dot(d2));
    let den = a * e - b * b;
    let mut s = if den > delirium_data::kv!(72) { ((b * f - c * e) / den).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = if e > delirium_data::kv!(72) { (b * s + f) / e } else { 0.0 };
    if t < 0.0 {
        (t, s) = (0.0, if a > delirium_data::kv!(72) { (-c / a).clamp(0.0, 1.0) } else { 0.0 });
    } else if t > 1.0 {
        (t, s) = (1.0, if a > delirium_data::kv!(72) { ((b - c) / a).clamp(0.0, 1.0) } else { 0.0 });
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
        Joint { parent, pf, cf, kind, stiff, mode: if rigid.is_some() { delirium_data::kv!(0 int) } else { 0 }, jc: Default::default() }
    }
}

const LOCKED: f32 = 1.0;

type Acc = (Vec3, Vec3);

fn moment(f: Vec3, d: Vec3, t: Vec3) -> Vec3 {
    Vec3::new((-f.y).mul_add(d.z, f.z.mul_add(d.y, t.x)), (-f.z).mul_add(d.x, f.x.mul_add(d.z, t.y)), (-f.x).mul_add(d.y, f.y.mul_add(d.x, t.z)))
}

fn pair(_joints: &[Option<Joint>], k: usize, t: DVec3, ev: &mut Vec<(usize, Vec3)>) {
    ev.push((k, t.as_vec3()));
}

fn apply(joints: &[Option<Joint>], ev: &[(usize, Vec3)], acc: &mut [Acc]) {
    for &(k, t) in ev {
        acc[k].1 += t;
        if let Some(j) = joints[k] {
            acc[j.parent].1 -= t;
        }
    }
}

fn answer(artic: &Artic, k: usize, t: DVec3) -> DVec3 {
    artic.answer(k, t.as_vec3()).as_dvec3()
}

fn solve3(a: [Vec3; 3], b: Vec3) -> Vec3 {
    let [r0, r1, r2] = a;
    let big = a.iter().flat_map(|r| r.to_array()).fold(0.0f32, |m, v| if m < v.abs() { v.abs() } else { m });
    if big < delirium_data::kv!(245) {
        return Vec3::ZERO;
    }
    let row = a.iter().position(|r| r.to_array().iter().any(|v| v.abs() == big)).unwrap_or(0);
    let col = a[row].to_array().iter().position(|v| v.abs() == big).unwrap_or(0);
    let c1 = Vec3::new(r1.y.mul_add(r2.z, -(r1.z * r2.y)), (-r1.x).mul_add(r2.z, r2.x * r1.z), r1.x.mul_add(r2.y, -(r2.x * r1.y)));
    let c2 = Vec3::new((-r0.y).mul_add(r2.z, r0.z * r2.y), r0.x.mul_add(r2.z, -(r0.z * r2.x)), (-r0.x).mul_add(r2.y, r0.y * r2.x));
    let c0 = Vec3::new(r0.y.mul_add(r1.z, -(r0.z * r1.y)), (-r0.x).mul_add(r1.z, r0.z * r1.x), r0.x.mul_add(r1.y, -(r0.y * r1.x)));
    let cof = [c1, c2, c0];
    let rowmax = cof.map(|c| c.abs().max_element());
    let top = rowmax.iter().fold(0.0f32, |m, &v| if m < v { v } else { m });
    let det = c1.y.mul_add(r0.y, r0.x.mul_add(c1.x, c1.z * r0.z));
    let tol = big * delirium_data::kv!(571 f32);
    let (t3, tm) = (tol * tol * tol, top * tol);
    let thr = if t3 - tm >= 0.0 { t3 } else { tm };
    if !(det.abs() > thr && rowmax[(row + 1) % delirium_data::kv!(0 int)] > tol * tol && rowmax[(row + 2) % delirium_data::kv!(0 int)] > tol * tol) {
        let sp = Vec3::splat;
        if top > tol * tol {
            let at = rowmax.iter().position(|&v| v == top).unwrap_or(0);
            let n = cof[at];
            let big_n = n.to_array().iter().position(|v| v.abs() == rowmax[at]).unwrap_or(0);
            let (bn, nn) = (n.x.mul_add(b.x, b.y.mul_add(n.y, b.z * n.z)), n.x.mul_add(n.x, n.y.mul_add(n.y, n.z * n.z)));
            let b = -n.mul_add(sp(bn / nn), -b);
            let (p, q, j, k) = ((at + 1) % delirium_data::kv!(0 int), (at + 2) % delirium_data::kv!(0 int), (big_n + 1) % delirium_data::kv!(0 int), (big_n + 2) % delirium_data::kv!(0 int));
            let (m, bb) = (|r: usize, c: usize| a[r].to_array()[c], b.to_array());
            let d = 1.0 / m(q, k).mul_add(m(p, j), -(m(q, j) * m(p, k)));
            let mut x = [0.0f32; 3];
            x[p] = m(q, k).mul_add(bb[j], -(m(q, j) * bb[k])) * d;
            x[q] = -(m(p, k).mul_add(bb[j], -(m(p, j) * bb[k])) * d);
            let x = Vec3::from_array(x);
            let (cj, ck) = (Vec3::new(m(0, j), m(1, j), m(2, j)), Vec3::new(m(0, k), m(1, k), m(2, k)));
            let w = -Vec3::new(ck.y.mul_add(cj.z, -(ck.z * cj.y)), ck.z.mul_add(cj.x, -(ck.x * cj.z)), ck.x.mul_add(cj.y, -(ck.y * cj.x)));
            let (xw, ww) = (w.x.mul_add(x.x, x.y.mul_add(w.y, x.z * w.z)), w.x.mul_add(w.x, w.y.mul_add(w.y, w.z * w.z)));
            return -w.mul_add(sp(xw / ww), -x);
        }
        let ro = a[row];
        let lam = ro.y.mul_add(b.y, b.x.mul_add(ro.x, ro.z * b.z)) / ro.y.mul_add(ro.y, ro.z.mul_add(ro.z, ro.x * ro.x));
        let cc = Vec3::new(a[0][col], a[1][col], a[2][col]);
        let mu = (ro * lam)[col] / cc.x.mul_add(cc.x, cc.z.mul_add(cc.z, cc.y * cc.y));
        return cc * mu;
    }
    let r = crate::estimate::vrefp(det);
    let r = r.mul_add((-r).mul_add(det, 1.0), r);
    Vec3::new(c1.vd(b) * r, c2.vd(b) * r, c0.vd(b) * r)
}

#[derive(Clone)]
struct Row {
    j: Vec<f64>,
    mj: Vec<f64>,
    eff: f64,
    target: f64,
    lambda: f64,
    kind: RowKind,
    at: Spot,
    g: Option<G11>,
}

#[derive(Clone, Copy)]
struct G11 {
    c: usize,
    p: usize,
    u: Vec3,
    t: f32,
    k: f32,
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
        let eff = j.iter().zip(&mj).map(|(a, b)| a * b).sum::<f64>().max(delirium_data::kv!(66));
        Row { j, mj, eff, target, lambda: 0.0, kind, at: Spot::OWN, g: None }
    }
}

fn frame(n: Vec3) -> [Vec3; 3] {
    let t = vnorm(if n.x.abs() >= n.y.abs() { Vec3::new(-n.z, 0.0, n.x) } else { Vec3::new(0.0, n.z, -n.y) });
    [n, t, n.vx(t)]
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
    body: Option<(usize, f64, [DVec3; 3])>,
    at: Spot,
    g: Option<G7>,
}

#[derive(Clone, Copy)]
struct G7 {
    k: usize,
    r: Vec3,
    f: [Vec3; 3],
    kinv: [Vec3; 3],
    axis: Vec3,
    target: Vec3,
    mu: f32,
}

impl G7 {
    fn new(artic: &Artic, k: usize, r: Vec3, f: [Vec3; 3], target: Vec3, mu: f32) -> G7 {
        if std::env::var("NM_SWDUMP").is_ok() { let p = artic.point_k(k, r); eprintln!("okp {} {} {} m {} {} {} {} {} {} {} {} {} f {} {} {} {} {} {} {} {} {}", r.x, r.y, r.z, p[0].x, p[0].y, p[0].z, p[1].x, p[1].y, p[1].z, p[2].x, p[2].y, p[2].z, f[0].x, f[0].y, f[0].z, f[1].x, f[1].y, f[1].z, f[2].x, f[2].y, f[2].z); }
        let kk = crate::artic::frame_k(&artic.point_k(k, r), &f);
        G7 { k, r, f, kinv: crate::artic::ldl_inverse(&kk), axis: kk[0] * (1.0 / kk[0].x), target, mu }
    }
}

impl Block {
    fn new(artic: &Artic, j: [Vec<f64>; 3], target: DVec3, mu: f64, held: Option<(usize, usize)>, warm: Vec3, self_contact: bool) -> Block {
        let (mj, k) = Block::k(artic, &j, self_contact);
        Block::with(j, mj, k, target, mu, held, warm)
    }

    fn moving(artic: &Artic, j: [Vec<f64>; 3], target: DVec3, mu: f64, held: Option<(usize, usize)>, warm: Vec3, body: (usize, f64, [DVec3; 3])) -> Block {
        let (mj, k) = Block::k(artic, &j, false);
        let (_, m, t) = body;
        let kb = DMat3::from_cols_array_2d(&std::array::from_fn(|a| std::array::from_fn(|b| m * t[a].dot(t[b]))));
        Block { body: Some(body), ..Block::with(j, mj, k + kb, target, mu, held, warm) }
    }

    fn k(artic: &Artic, j: &[Vec<f64>; 3], self_contact: bool) -> ([Vec<f64>; 3], DMat3) {
        let mj = artic.solve_rows([&j[0][..], &j[1], &j[2]]);
        let on: [Vec<bool>; 3] = std::array::from_fn(|a| if self_contact { vec![] } else { j[a].chunks(delirium_data::kv!(6 int)).map(|x| x.iter().any(|&v| v != 0.0)).collect() });
        let ans: [Vec<V6>; 3] = std::array::from_fn(|b| on[0].iter().enumerate().map(|(c, _)| if on[0][c] || on[1][c] || on[2][c] {
            artic.own(c, std::array::from_fn(|i| j[b][delirium_data::kv!(6 int) * c + i]))
        } else { [0.0; 6] }).collect());
        let own = |a: usize, b: usize| -> f64 {
            (0..on[a].len()).filter(|&c| on[a][c] || on[b][c]).map(|c| j[a][delirium_data::kv!(6 int) * c..delirium_data::kv!(6 int) * c + delirium_data::kv!(6 int)].iter().zip(ans[b][c]).map(|(x, y)| x * y).sum::<f64>()).sum()
        };
        let k = DMat3::from_cols_array_2d(&std::array::from_fn(|a| std::array::from_fn(|b| {
            if self_contact { j[b].iter().zip(&mj[a]).map(|(j, m)| j * m).sum() } else { own(a, b) }
        })));
        (mj, k)
    }

    fn with(j: [Vec<f64>; 3], mj: [Vec<f64>; 3], k: DMat3, target: DVec3, mu: f64, held: Option<(usize, usize)>, warm: Vec3) -> Block {
        Block { j, mj, kinv: ldl_inverse(k), axis: k.row(0) / k.x_axis.x, target, lambda: warm.as_dvec3(), mu, held, scale: DVec3::ONE, ob: None, body: None, at: Spot::OWN, g: None }
    }

    fn apply(&self, d: DVec3, qd: &mut [f64], vb: &mut [DVec3]) {
        for (q, e) in self.at.runs(self.mj[0].len()) {
            for (((q, x), y), z) in qd[q].iter_mut().zip(&self.mj[0][e.clone()]).zip(&self.mj[1][e.clone()]).zip(&self.mj[2][e]) {
                *q += x * d.x + y * d.y + z * d.z;
            }
        }
        if let Some((o, m, t)) = self.body {
            vb[o] -= (t[0] * d.x + t[1] * d.y + t[2] * d.z) * m;
        }
    }

    fn solve(&mut self, qd: &mut [f64], nz: &[(usize, usize)], vb: &mut [DVec3]) -> f64 {
        let mut v = DVec3::from_array(std::array::from_fn(|a| nz.iter().map(|&(i, g)| self.j[a][i] * qd[g]).sum()));
        if let Some((o, _, t)) = self.body {
            v -= DVec3::new(t[0].dot(vb[o]), t[1].dot(vb[o]), t[2].dot(vb[o]));
        }
        let mut l = self.lambda - self.kinv * (v - self.target);
        let an = self.axis.dot(l);
        let tt = l.y * l.y + l.z * l.z;
        let g = DVec3::new(-tt.sqrt() * self.mu, l.y, l.z);
        let way = DVec3::new(l.x - an, l.y, l.z);
        let over = g.dot(l);
        if over > 0.0 {
            l -= way * (over / (g.dot(way) + delirium_data::kv!(245)));
        }
        if an < 0.0 {
            l = DVec3::ZERO;
        }
        let d = l - self.lambda;
        self.lambda = l;
        self.apply(d, qd, vb);
        d.length_squared()
    }
}

fn ldl_inverse(k: DMat3) -> DMat3 {
    let (a00, a10, a20, a11, a21, a22) = (k.x_axis.x, k.x_axis.y, k.x_axis.z, k.y_axis.y, k.y_axis.z, k.z_axis.z);
    let eps = (a11 + a22 + a00) * delirium_data::kv!(572);
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

fn impulses(rows: &mut [Row], blocks: &mut [Block], qd: &mut [f64], vb: &mut [DVec3]) {
    let apply = |row: &Row, d: f64, qd: &mut [f64]| {
        for (q, e) in row.at.runs(row.mj.len()) {
            for (q, m) in qd[q].iter_mut().zip(&row.mj[e]) {
                *q += m * d;
            }
        }
    };
    if std::env::var("NM_SWDUMP").is_ok() { for (i, g) in blocks.iter().enumerate().filter_map(|(i, b)| b.g.map(|g| (i, g))) { eprintln!("og {i} r {:e} {:e} {:e} k {:e} {:e} {:e} {:e} {:e} {:e} ax {:e} {:e} {:e} f {:?}", g.r.x, g.r.y, g.r.z, g.kinv[0].x, g.kinv[0].y, g.kinv[0].z, g.kinv[1].y, g.kinv[1].z, g.kinv[2].z, g.axis.x, g.axis.y, g.axis.z, g.f); } }
    if std::env::var("NM_SWDUMP").is_ok() { for (i, b) in blocks.iter().enumerate() { eprintln!("oblk {i} t {:.9e} {:.9e} {:.9e} k {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} ax {:.9e} {:.9e} {:.9e} l {:.9e} {:.9e} {:.9e} mu {}", b.target.x, b.target.y, b.target.z, b.kinv.x_axis.x, b.kinv.y_axis.x, b.kinv.z_axis.x, b.kinv.y_axis.y, b.kinv.z_axis.y, b.kinv.z_axis.z, b.axis.x, b.axis.y, b.axis.z, b.lambda.x, b.lambda.y, b.lambda.z, b.mu); } }
    for b in blocks.iter() {
        b.apply(b.lambda, qd, vb);
    }
    let mut flat = Vec::with_capacity(delirium_data::kv!(23 int) * (blocks.len() + rows.len()));
    let mut nz = |j: &[&Vec<f64>], at: Spot| {
        let from = flat.len();
        for c in (0..j[0].len() / delirium_data::kv!(6 int)).filter(|&c| j.iter().fold(0, |a, j| j[delirium_data::kv!(6 int) * c..delirium_data::kv!(6 int) * c + delirium_data::kv!(6 int)].iter().fold(a, |a, x| a | x.to_bits() << 1)) != 0) {
            flat.extend((delirium_data::kv!(6 int) * c..delirium_data::kv!(6 int) * c + delirium_data::kv!(6 int)).filter(|&i| j.iter().any(|j| j[i] != 0.0)).map(|i| (i, at.of(i))));
        }
        from..flat.len()
    };
    let (bnz, rnz): (Vec<_>, Vec<_>) = (blocks.iter().map(|b| nz(&[&b.j[0], &b.j[1], &b.j[2]], b.at)).collect(), rows.iter().map(|r| nz(&[&r.j], r.at)).collect());
    for _ in 0..SWEEPS() {
        let mut moved: f64 = blocks.iter_mut().zip(&bnz).map(|(b, nz)| b.solve(qd, &flat[nz.clone()], vb)).sum();
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
        if std::env::var("NM_SWDUMP").is_ok() { for (i, b) in blocks.iter().enumerate() { eprintln!("osw b {i} {:.9e} {:.9e} {:.9e}", b.lambda.x, b.lambda.y, b.lambda.z); } for (i, r) in rows.iter().enumerate() { eprintln!("osw r {i} {:.9e}", r.lambda); } eprintln!("osw end"); }
        if moved == 0.0 {
            break;
        }
    }
}

fn applied(rows: &[Row], blocks: &[Block], mut acc: Vec<artic::P2>) -> Option<Vec<artic::P2>> {
    if std::env::var("NM_F32").is_err() || blocks.iter().any(|b| b.g.is_none() || b.body.is_some()) || rows.iter().any(|r| r.g.is_none()) {
        return None;
    }
    if std::env::var("NM_ROWS1").is_ok() { for r in rows { let g = r.g.unwrap(); let a = g.u * -(r.lambda as f32); acc[g.c].1 += a; acc[g.p].1 += Vec3::ZERO - a; } }
    for b in blocks {
        let g = b.g.as_ref().unwrap();
        let l = Vec3::ZERO - -b.lambda.as_vec3();
        let col = |i: usize| Vec3::new(g.f[0][i], g.f[1][i], g.f[2][i]);
        let imp = Vec3::new(col(0).vd(l), col(1).vd(l), col(2).vd(l)) * b.scale.as_vec3();
        if std::env::var("NM_SWDUMP").is_ok() { eprintln!("oapply {} i {:.9e} {:.9e} {:.9e} r {:.9e} {:.9e} {:.9e} l {:?} lam {:?} sc {:?} f {:?}", g.k, imp.x, imp.y, imp.z, g.r.x, g.r.y, g.r.z, [l.x.to_bits(), l.y.to_bits(), l.z.to_bits()], b.lambda, b.scale, g.f.map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])); }
        acc[g.k].1 = artic::at_point((g.r, acc[g.k].1), imp);
        acc[g.k].0 += imp;
    }
    for r in rows.iter().filter(|_| std::env::var("NM_ROWS1").is_err()) {
        let g = r.g.unwrap();
        let a = g.u * -(r.lambda as f32);
        let a = if std::env::var("NM_FLIP").ok().and_then(|s| s.parse::<usize>().ok()) == Some(g.c) { g.u * (r.lambda as f32) } else { a };
        acc[g.c].1 += a;
        acc[g.p].1 += Vec3::ZERO - a;
    }
    Some(acc)
}

fn impulses_g(rows: &mut [Row], blocks: &mut [Block], artic: &Artic, base: Vec<artic::P2>) -> bool {
    if std::env::var("NM_SWDUMP").is_ok() { eprintln!("ofall {:?} gnone {} body {} rows {}", blocks.iter().map(|b| (b.g.is_some(), b.held)).collect::<Vec<_>>(), blocks.iter().filter(|b| b.g.is_none()).count(), blocks.iter().filter(|b| b.body.is_some()).count(), rows.iter().filter(|r| r.g.is_none()).count()); }
    if std::env::var("NM_F32").is_err() || blocks.iter().any(|b| b.g.is_none() || b.body.is_some()) || rows.iter().any(|r| r.g.is_none()) {
        return false;
    }
    if std::env::var("NM_SWDUMP").is_ok() { eprintln!("osw new"); }
    let mut lz = artic::Lazy::new(artic, base);
    let col = |m: &[Vec3; 3], i: usize| Vec3::new(m[0][i], m[1][i], m[2][i]);
    let push7 = |lz: &mut artic::Lazy, g: &G7, l: Vec3, moved: &mut Vec3| {
        *moved = l.mul_add(l, *moved);
        let imp = Vec3::new(col(&g.f, 0).vd(l), col(&g.f, 1).vd(l), col(&g.f, 2).vd(l));
        lz.push(g.k, (imp, g.r.vx(imp)));
    };
    let mut r7: Vec<Vec3> = blocks.iter().map(|b| -b.lambda.as_vec3()).collect();
    let mut r11 = vec![0.0f32; rows.len()];
    if std::env::var("NM_SWDUMP").is_ok() { for (i, b) in blocks.iter().enumerate() { let g = b.g.unwrap(); eprintln!("og7 {i} {} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e} {:e}", g.k, g.target.x, g.target.y, g.target.z, g.kinv[0].x, g.kinv[0].y, g.kinv[0].z, g.kinv[1].y, g.kinv[1].z, g.kinv[2].z, g.axis.x, g.axis.y, g.axis.z, g.mu, g.r.x, g.r.y, g.r.z, -r7[i].x, -r7[i].y, -r7[i].z); } }
    if std::env::var("NM_SWDUMP").is_ok() { for (i, r) in rows.iter().enumerate() { let g = r.g.unwrap(); eprintln!("og11 {i} {} {} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e}", g.c, g.p, g.t, g.k, g.u.x, g.u.y, g.u.z, r.eff); } }
    let mut moved = Vec3::ZERO;
    for (b, r) in blocks.iter().zip(&r7) {
        push7(&mut lz, b.g.as_ref().unwrap(), Vec3::ZERO - *r, &mut moved);
    }
    let nms = |a: Vec3, b: f32, c: Vec3| Vec3::new(-a.x.mul_add(b, -c.x), -a.y.mul_add(b, -c.y), -a.z.mul_add(b, -c.z));
    for _ in 0..SWEEPS() {
        moved = Vec3::ZERO;
        for (b, r0) in blocks.iter().zip(&mut r7) {
            let g = b.g.as_ref().unwrap();
            let vp = lz.at(g.k, g.r);
            let e = Vec3::new(g.f[0].vd(vp), g.f[1].vd(vp), g.f[2].vd(vp)) - g.target;
            let l = (Vec3::ZERO - Vec3::new(e.vd(col(&g.kinv, 0)), e.vd(col(&g.kinv, 1)), e.vd(col(&g.kinv, 2)))) - *r0;
            let tang = Vec3::new(0.0, 1.0, 1.0) * l;
            let an = l.vd(g.axis);
            let tt = tang.vd(tang);
            let y = crate::estimate::vrsqrtefp(tt);
            let y = if y == f32::INFINITY { 0.0 } else { y };
            let s = (tt * (-tt).mul_add(y * y, 1.0).mul_add(0.5 * y, y)) * g.mu;
            let gv = Vec3::new(-(1.0f32.mul_add(s, -tang.x)), -(0.0f32.mul_add(s, -l.y)), -(0.0f32.mul_add(s, -l.z)));
            let way = l - Vec3::new(an, 0.0 * an, 0.0 * an);
            let over = gv.vd(l);
            let den = gv.vd(way) + delirium_data::kv!(245);
            let rc = crate::estimate::vrefp(den);
            let rc = rc.mul_add(-(rc.mul_add(den, -1.0)), rc);
            let f = if over > 0.0 { over * rc } else { 0.0 };
            let l = if an < 0.0 { Vec3::ZERO } else { nms(way, f, l) };
            let d = l + *r0;
            push7(&mut lz, g, d, &mut moved);
            *r0 -= d;
        }
        for (r, r0) in rows.iter().zip(&mut r11) {
            let g = r.g.unwrap();
            let (wc, wp) = lz.spins(g.c);
            let v = wc.vd(g.u) - wp.vd(g.u);
            let x = ((0.0 - v) - g.t) * g.k;
            let d = if 0.0 > x - *r0 { *r0 } else { x };
            let f = 0.0 - d;
            moved = Vec3::new(f.mul_add(f, moved.x), x.mul_add(x, moved.y), x.mul_add(x, moved.z));
            let pp = (Vec3::ZERO * -f, g.u * -f);
            lz.push(g.p, pp);
            lz.push(g.c, (Vec3::ZERO - pp.0, Vec3::ZERO - pp.1));
            *r0 -= d;
        }
        if std::env::var("NM_SWDUMP").is_ok() { for (i, r) in r7.iter().enumerate() { eprintln!("osw b {i} {:.9e} {:.9e} {:.9e}", -r.x, -r.y, -r.z); } for (i, r) in r11.iter().enumerate() { eprintln!("osw r {i} {:.9e}", -r); } eprintln!("osw end"); }
        if !(moved.vd(moved) >= f32::MIN_POSITIVE) {
            break;
        }
    }
    for (b, r) in blocks.iter_mut().zip(&r7) {
        b.lambda = (-*r).as_dvec3();
    }
    for (row, r) in rows.iter_mut().zip(&r11) {
        row.lambda = -*r as f64;
    }
    true
}

#[allow(non_snake_case)]
#[inline]
fn ALLOW() -> f32 { delirium_data::kv!(573) }
macro_rules! env_on {
    ($n:literal) => {{ static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new(); *ON.get_or_init(|| std::env::var($n).is_ok()) }};
}
fn elastic(e: f64) -> DVec3 {
    if env_on!("NM_NOSCALE") { return DVec3::ONE; }
    DVec3::splat(1.0 + e)
}
#[allow(non_snake_case)]
#[inline]
fn BOX_MARGIN() -> f32 { delirium_data::kv!(48) }

#[derive(Clone, Copy)]
struct Held { local: Vec3, ground: Vec3, gap: f32, age: u16, m: u8, imp: Vec3, pimp: Vec3, n: Vec3, inst: u32, off: bool, world: Vec3, wc: Vec3 }
fn hold(held: &mut Vec<Held>, new: Held) {
    if new.gap >= delirium_data::kv!(113) {
        return;
    }
    let mut near = (None, delirium_data::kv!(574));
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
    for k in delirium_data::kv!(0 int)..last {
        if n.length_squared() >= delirium_data::kv!(47) {
            break;
        }
        n = a.cross(p[k] - p[0]);
    }
    let d = n.cross(a);
    let (mut m, mut bar) = (0, Vec3::splat(delirium_data::kv!(575)));
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
    let (mut at, mut least) = (0, delirium_data::kv!(575));
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
    let reach = |u: Vec3, ax: &[Vec3; 3], h: Vec3| (0..delirium_data::kv!(0 int)).map(|i| ax[i].dot(u).abs() * h[i]).sum::<f32>();
    let crosses = ax.into_iter().flat_map(|u| bx.map(|v| u.cross(v))).filter(|c| c.length_squared() > delirium_data::kv!(251));
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
        Some(e) => gjk::Shape::Box(e - Vec3::splat(BOX_MARGIN()), BOX_MARGIN()),
        None => gjk::Shape::Capsule(p.half, p.radius),
    }
}

fn bound_half(p: &PartDef) -> Vec3 {
    p.boxed.unwrap_or(Vec3::new(p.radius, p.half + p.radius, p.radius))
}

#[allow(non_snake_case)]
#[inline]
fn PUSH_KEEP() -> f32 { delirium_data::kv!(573) }
fn out(gap: f32) -> f32 {
    -gap - (-gap * 0.5).clamp(0.0, PUSH_KEEP())
}

impl Ragdoll {
    fn push_block(b: &Block, over: f32, h: f32, last: Vec3) -> Block {
        let g = b.g.map(|g| G7 { target: Vec3::new(out(over) * (1.0 / h), 0.0, 0.0), ..g });
        Block { target: DVec3::new((out(over) / h) as f64, 0.0, 0.0), lambda: last.as_dvec3(), held: None, scale: DVec3::ONE, g, ..b.clone() }
    }
}

#[allow(non_snake_case)]
#[inline]
fn SWEEPS() -> usize { delirium_data::kv!(23 int) }
#[allow(non_snake_case)]
#[inline]
fn MAX_SPEED() -> f32 { delirium_data::kv!(59) }
const MAX_SPIN: f32 = std::f32::consts::TAU;
#[allow(non_snake_case)]
#[inline]
pub(crate) fn MAX_SPIN_NM() -> f32 { delirium_data::kv!(5) * std::f32::consts::TAU }
#[allow(non_snake_case)]
#[inline]
pub(crate) fn DAMPING() -> f32 { delirium_data::kv!(73) }
#[allow(non_snake_case)]
#[inline]
fn LIMIT_MARGIN() -> f32 { delirium_data::kv!(126) }

#[allow(non_snake_case)]
#[inline]
fn LIMIT_GAIN() -> f32 { delirium_data::kv!(108) }

fn hinge_limits(lo: f32, hi: f32) -> (f32, f32) {
    let (l, h) = (LIMIT_MARGIN() + lo, hi - LIMIT_MARGIN());
    if l > h { ((lo + hi) * 0.5, (lo + hi) * 0.5) } else { (l, h) }
}

fn lean_limit(st: &crate::rigid::Swing, lean: [f32; 2]) -> Option<f32> {
    let [l0, l1] = lean.map(|l| (l - LIMIT_MARGIN()).max(0.0));
    let (l0, l1) = (l0.max(delirium_data::kv!(68)), l1.max(delirium_data::kv!(68)));
    let (c1, c0) = (sdot(st.s, crate::rigid::col(&st.pr, 1), b"yxz") / l0, sdot(st.s, crate::rigid::col(&st.pr, 0), b"yxz") / l1);
    let e = c0.fma(c0, c1 * c1) * st.theta * st.theta;
    (!(e < 1.0)).then(|| st.theta / e.sqrt())
}

fn twist_limit(st: &crate::rigid::Swing, twist: f32) -> Option<f32> {
    let l = (twist - LIMIT_MARGIN()).max(0.0);
    if !(st.twist - l < 0.0) { Some(l) } else if !(l + st.twist > 0.0) { Some(-l) } else { None }
}
pub const SUBSTEPS: usize = 2;
#[allow(non_snake_case)]
#[inline]
fn GHOST() -> f32 { delirium_data::kv!(57) }

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
    pub root_moment: Vec<(usize, Vec3, Vec3)>,
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
    last_a: Vec<Vec3>,
    seen_w: Vec<Vec3>,
    pair_prev: Vec<((Vec3, Mat3), (Vec3, Mat3), (Vec3, Vec3))>,
    held_pairs: Vec<Vec<Held>>,
    held_keys: Vec<Option<(usize, usize)>>,
    held_car: Vec<(usize, i32, usize, Vec<Held>)>,
    pub prep_twist: Vec<Option<f32>>,
    hinge_b: Vec<Option<Vec3>>,
    pub rigid: Option<(Vec<crate::rigid::Link>, Vec3, Vec<crate::nmdbm::M34>)>,
    pub inertia: Option<Vec<Vec3>>,
    pub inst: Option<[Vec3; 4]>,
    pending: Vec<(usize, Vec3, Vec3, Vec3)>,
    pub out: Vec<(Vec3, Vec3)>,
    pub joint_torque: Vec<Vec3>,
    pub armature: Vec<f32>,
    o: Vec3,
    pub bound_c: Vec3,
    rejuv: u32,
    mwc: (u32, u32),
    held_inst: Option<[Vec3; 3]>,
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
    let dp = b[delirium_data::kv!(0 int)] - a[delirium_data::kv!(0 int)];
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
            let m = p.mass.max(delirium_data::kv!(73));
            let s = nm.map_or(1.0, |n| n[k] / m);
            let l = nm.map_or(Quat::IDENTITY, |_| crate::nmdbm::rage_link(build.nm, k));
            let z = p.size;
            let att = if nm.is_some() && k == 0 { Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2) } else { Mat3::IDENTITY };
            let att = [att.x_axis, att.y_axis, att.z_axis];
            let i = match (nm, p.unit()) {
                (Some(n), Some(u)) => u * n[k],
                _ => p.moments() * s,
            };
            let i = if nm.is_some() && k == 0 { Vec3::new(i.y, i.x, i.z) } else { i };
            let mut b = Body { x: f.translation - o, r: [Vec3::ZERO; 3], q: f.rotation, v, w: Vec3::ZERO, m: nm.map_or(m, |n| n[k]) * z.powi(3) * p.heft, i: i * z.powi(3) * p.heft, l, att };
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
        Ragdoll { nm: build.nm, pins: vec![None; n], cons: vec![(None, None); n], grips: vec![], snapped: vec![], gone: 0, grip_rows: vec![], touch: vec![0; n], contact_off: 0, hit: vec![None; n], contact: crate::bump::ContactRecord::new(), ob_hits: vec![], held: vec![vec![]; n], stage_vel: vec![], stage_lt: vec![], root_moment: vec![], stage_lim: vec![], stiff: vec![], bodies, order, mode: 1, still: [SLEEP_STOP() - 2, SLEEP_SLEEP() - 2],
                  sleep_tol: [SLEEP_V2(), SLEEP_W2(), SLEEP_MEAN2()], asleep: false, age: 0.0, fresh: false, pending: vec![], pairs: vec![], seen: vec![], coll: vec![], last_a: vec![], seen_w: vec![], pair_prev: vec![], held_pairs: vec![], held_keys: vec![], held_car: vec![], prep_twist: vec![], hinge_b: vec![], rigid: None, inertia: None, inst: None, out: vec![(Vec3::ZERO, Vec3::ZERO); n], joint_torque: vec![Vec3::ZERO; n], armature: vec![0.0; n], o, bound_c: Vec3::ZERO,
                  rejuv: 0, mwc: (1, 0), held_inst: None }
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
            (m, x, v) = (m + b.m, x + b.x * b.m, b.v.mul_add(Vec3::splat(b.m), v));
        }
        (x / m + self.o, v * (1.0 / m))
    }

    pub fn part_mass(&self, k: usize) -> f32 {
        self.bodies[k].m
    }

    pub fn mass(&self) -> f32 {
        self.bodies.iter().map(|b| b.m).sum()
    }

    pub fn kinetic(&self) -> f32 {
        let n4 = self.bodies.len() / delirium_data::kv!(1 int) * delirium_data::kv!(1 int);
        let (mut e, mut mass) = (0.0f32, 0.0f32);
        let links = self.rigid.as_ref().map(|r| &r.0);
        for (k, b) in self.bodies.iter().enumerate() {
            let (m, v, w) = match links.and_then(|l| l.get(k)) {
                Some(l) => (l.m, l.v, l.w),
                None => (b.m, b.v, b.w),
            };
            let sq = |v: Vec3| if k < n4 { v.z.fma(v.z, v.x.fma(v.x, v.y * v.y)) } else { v.x.fma(v.x, v.y.fma(v.y, v.z * v.z)) };
            e = (sq(v) * m).fma(0.5, e);
            let i = b.i;
            e = (sq(w) * ((i.z + i.y + i.x) * delirium_data::kv!(576))).fma(0.5, e);
            mass += m;
        }
        e / mass
    }

    pub fn collided(&self, parts: &[usize]) -> bool {
        parts.iter().any(|&k| self.touch[k] & delirium_data::kv!(91 int) == delirium_data::kv!(7 int))
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
        1.0 / a.dot(b.i.recip() * a).max(delirium_data::kv!(72))
    }

    pub fn link_rows(&self, k: usize) -> [Vec3; 3] {
        self.bodies[k].r
    }

    pub fn centre(&self, k: usize) -> Vec3 {
        self.bodies[k].x + self.o
    }

    pub fn held_world(&self, i: usize) -> Vec<(Vec3, Vec3, f32, u16)> {
        let (b, m) = (&self.bodies[i], self.turns()[i]);
        self.held[i].iter().map(|r| (b.x + self.o + m * r.local, r.ground, r.gap, r.age)).collect()
    }

    fn inst(&self) -> [Vec3; 3] {
        if let Some(r) = self.held_inst {
            return r;
        }
        let r = &self.bodies[self.order[0]].r;
        let c = crate::rigid::nrm(crate::rigid::col(r, 2));
        let a = crate::rigid::nrm(crate::rigid::col(r, 1).vx(c));
        [a, c.vx(a), c]
    }

    fn comp(&self, inst: &[Vec3; 3]) -> Vec<(Vec3, [Vec3; 3])> {
        let xr = self.bodies[self.order[0]].x;
        self.bodies.iter().map(|b| (crate::rigid::ldot(inst, b.x - xr), b.bound().map(|w| crate::rigid::ldot(inst, w)))).collect()
    }

    fn bound_world(&self, i: usize) -> Vec3 {
        let r = self.inst();
        let ip = self.inst_place(&r);
        Self::chain(self.bound_local(i, &r, ip), &r, ip)
    }

    fn inst_place(&self, r: &[Vec3; 3]) -> Vec3 {
        let c = self.bound_c;
        self.o - Vec3::new(c.vd(crate::rigid::col(r, 0)), c.vd(crate::rigid::col(r, 1)), c.vd(crate::rigid::col(r, 2)))
    }

    fn bound_local(&self, i: usize, r: &[Vec3; 3], ip: Vec3) -> Vec3 {
        crate::rigid::ldot(r, (self.bodies[i].x + self.o) - ip)
    }

    fn chain(a: Vec3, r: &[Vec3; 3], ip: Vec3) -> Vec3 {
        r[2].mul_add(Vec3::splat(a.z), r[1].mul_add(Vec3::splat(a.y), r[0].mul_add(Vec3::splat(a.x), ip)))
    }

    pub(crate) fn turns(&self) -> Vec<Mat3> {
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
            (b.m, b.i) = (b.m * GHOST(), b.i * GHOST());
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
        let inst = [r[0], r[1], r[2], self.bodies[self.order[0]].x + self.o];
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
        delirium_data::kv!(110) + delirium_data::kv!(114) * (self.bodies[k].m / mean).clamp(delirium_data::kv!(57), delirium_data::kv!(12))
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
                if self.still[0] >= SLEEP_STOP() {
                    self.mode = 0;
                }
            }
            if self.mode != 1 {
                self.still[1] += 1;
                if self.still[1] >= SLEEP_SLEEP() {
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

    fn muscles(&self, artic: &Artic, joints: &[Option<Joint>], drive: &Drive, h: f32, lt: &mut Vec<(usize, Vec3)>, out: &mut [Vec3], hb: &mut [Option<Vec3>]) {
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
                let most = |m: f32| m / (1.0 - r.blend).max(delirium_data::kv!(129));
                let speed = |a: Vec3, o: &[u8; 3]| sdot(cl.w, a, o) - sdot(pl.w, a, o);
                let held = |acc: f32, v: f32, at: f32, to: f32| if acc.abs() > delirium_data::kv!(47) && !((acc.fma(h, v).fma(h, at) - to) * (at - to) >= 0.0) { 0.0 } else { acc };
                if !r.hinge {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    let (cs, sn) = (sdot(st.s, crate::rigid::col(&st.pr, 1), b"zyx"), -sdot(st.s, crate::rigid::col(&st.pr, 0), b"yzx"));
                    let (q, ls, ts) = (speed(st.across, b"zyx") / st.k, speed(st.s, b"zyx"), speed(st.bis, b"zyx") * st.k);
                    let at = [st.theta * cs, st.theta * sn, st.twist];
                    let v = [ls.fma(cs, q * sn), ls.fma(sn, -(q * cs)), ts];
                    let mut e = std::array::from_fn::<f32, 3, _>(|k| r.target[k] - at[k]);
                    if matches!(j.kind, Kind::Ball { twist, .. } if twist - LIMIT_MARGIN() >= f32::MAX) {
                        use std::f32::consts::{PI, TAU};
                        e[2] = if e[2] > PI { e[2] - TAU } else if e[2] < -PI { e[2] + TAU } else { e[2] };
                    }
                    let acc: [f32; 3] = std::array::from_fn(|k| held(e[k] * s + (0.0 - v[k]) * d, v[k], at[k], r.target[k]));
                    let (sw, qw, tw) = (acc[0].fma(cs, acc[1] * sn), acc[0].fma(sn, -(acc[1] * cs)) * st.k, acc[2] / st.k);
                    let world = st.bis.mul_add(Vec3::splat(tw), st.across.mul_add(Vec3::splat(qw), st.s * sw));
                    let cols = [Vec3::X, Vec3::Y, Vec3::Z].map(|e| artic.answer(i, e));
                    let t = solve3(cols, world);
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
                hb[i] = Some(n);
                let acc = held((0.0 - w) * d + (r.target[0] - at) * s, w, at, r.target[0]);
                let k = crate::nmdbm::sdot(n, artic.answer(i, n), b"yzx");
                let mt = most(r.max[0]);
                let t = if k > delirium_data::kv!(66) { n * (acc / k).clamp(-mt, mt) } else { Vec3::ZERO };
                pair(joints, i, t.as_dvec3(), lt);
                out[i] += t;
            } else if drive.rigid.is_empty() {
                let (s, d) = (m.stiffness * m.stiffness, 2.0 * m.damping * m.stiffness);
                let a = (err * s - rel * (d + h * s)) / (1.0 + h * d + h * h * s);
                let k = kn(a);
                if k > delirium_data::kv!(72) {
                    pair(joints, i, (a / k).as_dvec3(), lt);
                    out[i] += a / k;
                }
            }
        }
    }

    fn soft_limits(&self, artic: &Artic, joints: &[Option<Joint>], drive: &Drive, lt: &mut Vec<(usize, Vec3)>, out: &mut [Vec3]) {
        let b = &self.bodies;
        for (i, r) in drive.rigid.iter().enumerate().filter(|(_, r)| r.soft.iter().any(|l| l.enabled)) {
            let Some(j) = joints.get(i).copied().flatten() else { continue };
            let (pl, cl) = (b[j.parent].link(), b[i].link());
            let speed = |a: Vec3, o: &[u8; 3]| sdot(cl.w, a, o) - sdot(pl.w, a, o);
            let t = if r.hinge {
                let n = crate::rigid::hinge_axis(&j.jc, &cl);
                let acc = r.soft[0].accel(crate::rigid::hinge(&j.jc, &pl, &cl), speed(n, b"yzx"));
                let k = n.as_dvec3().dot(answer(artic, i, n.as_dvec3())) as f32;
                if k > delirium_data::kv!(66) { n * (acc / k) } else { Vec3::ZERO }
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

    fn limits(&self, artic: &Artic, joints: &[Option<Joint>], h: f32, lt: &mut Vec<(usize, Vec3)>) {
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
                    let r = sdot(n, artic.answer(k, n), b"yzx");
                    let t = -((if r != 0.0 { 1.0 / r } else { delirium_data::kv!(3) }) * LIMIT_GAIN() * f);
                    pair(joints, k, (n * t).as_dvec3(), lt);
                    continue;
                }
                Kind::Ball { lean, twist } => {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    if let Some(lim) = lean_limit(&st, lean) {
                        let (over, ls) = (st.theta - lim, speed(st.s));
                        let f = (if ls >= 0.0 { 0.0 } else { ls }).fma(h, over);
                        if f > 0.0 || ls > 0.0 {
                            past.extend([(st.s, -(LIMIT_GAIN() * f)), (st.across, -speed(st.across))]);
                        }
                    }
                    if let Some(lim) = twist_limit(&st, twist) {
                        let (mut over, mut ts, mut axis) = (st.twist - lim, speed(st.bis) * st.k, st.bis);
                        if lim < 0.0 {
                            (over, ts, axis) = (-over, -ts, -axis);
                        }
                        let f = (if ts >= 0.0 { 0.0 } else { ts }).fma(h, if over >= 0.0 { over } else { 0.0 });
                        if f > 0.0 || ts >= 0.0 {
                            past.push((axis, (if st.k != 0.0 { 1.0 / st.k } else { delirium_data::kv!(3) }) * -(LIMIT_GAIN() * f)));
                        }
                    }
                }
            }
            let t: Vec3 = {
                let resp: Vec<Vec3> = past.iter().map(|&(u, _)| artic.answer(k, u)).collect();
                let rec = |d: f32| if d != 0.0 { 1.0 / d } else { delirium_data::kv!(3) };
                let w: Vec<f32> = past.iter().map(|&(_, w)| w).collect();
                let sp = |x: f32| Vec3::splat(x);
                match past.len() {
                    1 => past[0].0 * sp(rec(sdot(past[0].0, resp[0], b"yzx")) * w[0]),
                    2 => {
                        let o = [b"yxz", b"yzx"];
                        let m = |i: usize, c: usize| sdot(past[i].0, resp[c], o[i]);
                        let r = rec(m(1, 1).mul_add(m(0, 0), -(m(0, 1) * m(1, 0))));
                        let x0 = m(1, 1).mul_add(w[0], -(w[1] * m(0, 1))) * r;
                        let x1 = w[1].mul_add(m(0, 0), -(m(1, 0) * w[0])) * r;
                        past[1].0.mul_add(sp(x1), past[0].0 * sp(x0))
                    }
                    3 => {
                        let o = [b"xzy", b"yzx", b"yxz"];
                        let mm: [Vec3; 3] = std::array::from_fn(|i| Vec3::from_array(std::array::from_fn(|c| sdot(past[i].0, resp[c], o[i]))));
                        let x = solve3(mm, Vec3::new(w[0], w[1], w[2]));
                        past[2].0.mul_add(sp(x.z), past[1].0.mul_add(sp(x.y), past[0].0 * sp(x.x)))
                    }
                    _ => continue,
                }
            };
            if t != Vec3::ZERO {
                pair(joints, k, t.as_dvec3(), lt);
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
        for h in self.held_car.iter().filter(|h| !h.3.is_empty()) {
            touched[h.0] = true;
        }
        let b = &self.bodies;
        let (mut rows, mut push) = (vec![], vec![]);
        let mut found = vec![];
        let n = delirium_data::kv!(6 int) * b.len();
        for (k, j) in joints.iter().enumerate() {
            let Some(j) = *j else { continue };
            let (pl, cl) = (b[j.parent].link(), b[k].link());
            let mut dofs: Vec<(Vec3, f32, f32, Option<f32>, Option<&[u8; 3]>)> = vec![];
            match j.kind {
                Kind::Hinge { lo, hi } => {
                    let (lo, hi) = hinge_limits(lo, hi);
                    let (a, n) = (crate::rigid::hinge(&j.jc, &pl, &cl), crate::rigid::hinge_axis(&j.jc, &cl));
                    let side = if !(a - hi < 0.0) { 1 } else if a - lo <= 0.0 { -1 } else if touched[k] { if hi - a < a - lo { 1 } else { -1 } } else { 0 };
                    let stale = if !(a - hi > 0.0) && a - lo >= 0.0 { self.hinge_b.get(k).copied().flatten().map(|n| sdot(cl.w, n, b"yzx") - sdot(pl.w, n, b"yzx")) } else { None };
                    match side {
                        1 => dofs.push((n, a - hi, 1.0, stale, None)),
                        -1 => dofs.push((-n, lo - a, 1.0, stale.map(|w| -w), None)),
                        _ => {}
                    }
                }
                Kind::Ball { lean, twist } => {
                    let st = crate::rigid::swing3(&j.jc, &pl, &cl);
                    self.prep_twist[k] = Some(st.twist);
                    if let Some(lim) = lean_limit(&st, lean) {
                        dofs.push((st.s, st.theta - lim, 1.0, None, Some(b"yzx")));
                        let w = sdot(cl.w, st.across, b"zyx") - sdot(pl.w, st.across, b"zyx");
                        dofs.push((if w < 0.0 { -st.across } else { st.across }, 0.0, 1.0, None, Some(b"zyx")));
                    }
                    if let Some(lim) = twist_limit(&st, twist) {
                        dofs.push(if lim > 0.0 { (st.bis, st.twist - lim, st.k, None, Some(b"yzx")) } else { (-st.bis, -st.twist + lim, st.k, None, Some(b"yzx")) });
                    }
                }
            }
            for (u, o, c, bs, order) in dofs {
                let mut jr = vec![0.0; n];
                for i in 0..delirium_data::kv!(0 int) {
                    jr[delirium_data::kv!(6 int) * k + delirium_data::kv!(0 int) + i] = -u[i] as f64;
                    jr[delirium_data::kv!(6 int) * j.parent + delirium_data::kv!(0 int) + i] += u[i] as f64;
                }
                let b0 = -jr.iter().zip(qd0).map(|(x, y)| x * y).sum::<f64>();
                let to = (c as f64 - 1.0) * b0 - ((LIMIT_MARGIN() - o).max(0.0) / h) as f64 + bs.map_or(0.0, |w| b0 - w as f64);
                if std::env::var("NM_SWDUMP").is_ok() { eprintln!("olim {k} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:?} {}", o, c, b0, sdot(cl.w, u, b"yzx") - sdot(pl.w, u, b"yzx"), sdot(cl.w, u, b"zyx") - sdot(pl.w, u, b"zyx"), sdot(cl.w, u, b"xyz") - sdot(pl.w, u, b"xyz"), u.x, u.y, u.z, bs, h); }
                let t = match order {
                    Some(order) => {
                        let m = LIMIT_MARGIN() - o;
                        let m = if -m >= 0.0 { 0.0 } else { m };
                        -m.mul_add(1.0 / h, -((sdot(cl.w, u, order) - sdot(pl.w, u, order)) * c))
                    }
                    None => (to + b0) as f32,
                };
                let q = sdot(u, artic.answer(k, u), b"yzx") * c;
                let r = crate::estimate::vrefp(q);
                found.push((jr, to, o, c, G11 { c: k, p: j.parent, u, t, k: 0.0 - r.mul_add(-r.mul_add(q, -1.0), r) }));
            }
        }
        for g in found.chunks(delirium_data::kv!(1 int)) {
            let mj = artic.solve_rows(std::array::from_fn::<_, 4, _>(|i| &g[i.min(g.len() - 1)].0[..]));
            for ((jr, to, o, c, g11), mj) in g.iter().cloned().zip(mj) {
                let mut r = Row::with(jr, mj, to, RowKind::Push);
                r.eff *= c as f64;
                push.push(Row { j: r.j.clone(), mj: r.mj.clone(), target: ((o - LIMIT_MARGIN()) / h) as f64, g: Some(G11 { t: (o - LIMIT_MARGIN()) * (1.0 / h), ..g11 }), ..r.clone() });
                r.g = Some(g11);
                rows.push(r);
            }
        }
        (rows, push)
    }

    #[allow(clippy::too_many_arguments)]
    fn contacts(&mut self, build: &Build, world: &Collision, artic: &Artic, h: f32, pose: &[Option<(Vec3, Quat)>], obstacles: &[Obstacle],
                others: &[Capsule], free: &[f64], fc: &[artic::P2]) -> (Vec<Row>, Vec<Block>, Vec<Block>, Vec<usize>, Vec<bool>) {
        let (mut rows, mut blocks, mut push_blocks) = (vec![], vec![], vec![]);
        let fv = |j: &[f64]| j.iter().zip(free).map(|(a, b)| a * b).sum::<f64>();
        let (o, turns) = (self.o, self.turns());
        let (r, ip) = (self.inst(), self.inst_place(&self.inst()));
        self.last_a = (0..self.bodies.len()).map(|i| self.bound_local(i, &r, ip)).collect();
        let d = |v: Vec3| v.as_dvec3();
        let (n_links, xr) = (self.bodies.len(), self.bodies[self.order[0]].x.as_dvec3());
        let lrow = |k: usize, at: DVec3, n: DVec3, t: DVec3| -> Vec<f64> {
            let mut r = vec![0.0; delirium_data::kv!(6 int) * n_links];
            r[delirium_data::kv!(6 int) * k..delirium_data::kv!(6 int) * k + delirium_data::kv!(6 int)].copy_from_slice(&v6(n, (at - xr).cross(n) + t));
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
            if dist > len - delirium_data::kv!(48) && dist > delirium_data::kv!(47) {
                self.grip_rows.push((rows.len(), k, strength));
                let j = lrow(k, d(at), d(gap / dist), DVec3::ZERO);
                let to = ((dist - len) / h) as f64 - fv(&j);
                rows.push(Row::new(artic, j, to, RowKind::Push));
            }
        }
        let mut hits: Vec<(usize, Vec3, Vec3, f32, f32, Vec3, f32, Option<(usize, usize)>, Vec3)> = vec![];
        let fresh = self.fresh;
        self.held_car.retain(|h| !h.3.is_empty() && !fresh && pose[h.0].is_none() && obstacles.iter().any(|o| o.id == h.1 && !o.parts.is_empty()));
        let mut old: Vec<((usize, usize), Vec<Held>)> = std::mem::take(&mut self.held_keys).into_iter().zip(std::mem::take(&mut self.held_pairs)).filter_map(|(k, h)| Some((k?, h))).collect();
        (self.held_pairs, self.held_keys) = (vec![vec![]; self.pairs.len()], vec![None; self.pairs.len()]);
        let mut next_old = 0;
        let car_base = self.held.len() + self.held_pairs.len();
        let mut hit_ob: Vec<(usize, i32, Vec3, Vec3)> = vec![];
        let mut ghosts: Vec<bool> = vec![];
        for (_, held) in self.held.iter_mut().enumerate().filter(|&(i, _)| pose[i].is_some() || self.fresh) {
            held.clear();
        }
        for (i, p) in build.parts.iter().enumerate().filter(|&(i, _)| pose[i].is_none() && !self.fresh) {
            let (b, tm) = (&self.bodies[i], turns[i]);
            let x = b.x;
            let bw = self.bound_world(i);
            let xw = self.bound_world(i);
            let mut acc = (Vec3::ZERO, Vec3::ZERO, 0.0, -1);
            let mut add = |at: Vec3, n: Vec3, depth: f32, inst: i32| {
                let w = depth.max(delirium_data::kv!(48));
                acc = (acc.0 + at * w, acc.1 + n * w, acc.2 + w, inst);
            };
            let held = &mut self.held[i];
            let shape = match p.boxed {
                Some(e) => gjk::Shape::Box(e - Vec3::splat(BOX_MARGIN()), BOX_MARGIN()),
                None => gjk::Shape::Capsule(p.half, p.radius),
            };
            let last = self.seen.get(i).map_or(x, |s| s.0);
            let fast = (tm.transpose() * (x - last)).abs().cmpgt(bound_half(p)).any();
            let leaving = |local: Vec3, face: Vec3| self.seen.get(i).is_some_and(|&(lx, lm)| { let lw = self.seen_w.get(i).map_or(lx + o, |&w| w); ((tm - lm) * local + ((b.x + o) - lw)).dot(face) > delirium_data::kv!(129) });
            let last_m = self.seen.get(i).map_or(tm, |s| s.1);
            let (sc, sh) = swept((x, tm), (last, last_m), bound_half(p));
            let (sm, sh) = (tm, sh + Vec3::splat(WORLD_MARGIN()));
            let (sc, sl) = (x + o + sm * sc, sc);
            let found = {
                let reach = Vec3::splat(bound_half(p).length() + delirium_data::kv!(73));
                let new = |q: gjk::Closest, at: Vec3, t0: Vec3| {
                    let (back, world) = (t0 - o, q.a + t0);
                    let local = tm.transpose() * (q.a + back - at);
                    let ground = q.b + back - q.n * WORLD_MARGIN() + o;
                    Held { local, ground, gap: q.dist, age: 0, m: 0, imp: Vec3::ZERO, pimp: Vec3::ZERO, n: q.n, inst: 0, off: false, world, wc: Vec3::ZERO }
                };
                let frame = |m: Mat3, at: Vec3| crate::bvh::Frame { rows: [m.x_axis, m.y_axis, m.z_axis], t: at + o };
                let mut found: Vec<(Held, crate::collision::Poly)> = world.tris_in_part(x.min(last) + o - reach, x.max(last) + o + reach,
                    &frame(tm, x), &frame(last_m, last), -bound_half(p), bound_half(p)).into_iter().filter_map(|(t, m, dirs, mask, inst, poly)| {
                    let meets = match poly.frame {
                        Some(f) => box_meets_tri(f.turn.transpose() * ((x + o - f.at) + sm * sl), f.turn.transpose() * sm, sh, f.tri),
                        None => box_meets_tri(sc, sm, sh, t),
                    };
                    if !meets {
                        return None;
                    }
                    let (t0, tri, turn, pen) = match poly.frame {
                        Some(f) => (Vec3::new(f.turn.row(0).vd(f.tri[0]), f.turn.row(1).vd(f.tri[0]), f.turn.row(2).vd(f.tri[0])) + f.at,
                            gjk::Shape::Tri(f.tri.map(|v| v - f.tri[0]), WORLD_MARGIN()), f.turn, dirs.map(|d| f.turn.transpose() * d)),
                        None => (t[0], gjk::Shape::Tri(t.map(|v| v - t[0]), WORLD_MARGIN()), Mat3::IDENTITY, dirs),
                    };
                    let qr = |rel: Vec3, axis: Vec3| gjk::closest(&shape, tm, rel, &tri, turn, Vec3::ZERO, axis, f32::INFINITY, Some(gjk::Pen::Tri(pen)));
                    let qw = |w: Vec3| qr(w - t0, Vec3::X);
                    let h = if fast {
                        let (lr, xr) = (self.seen_w.get(i).map_or(last + o, |&w| w) - t0, xw - t0);
                        let axis = std::cell::Cell::new(Vec3::X);
                        let (s, q) = Self::advance(|s| qr(Self::cast_at(lr, xr, s), axis.get()).inspect(|q| axis.set(q.n)), lr - xr)?;
                        let c = Self::cast_at(lr, xr, s);
                        let h = new(q, c + t0 - o, t0);
                        Held { world: q.a + (xr - c) + t0, gap: q.dist + (xr - c).dot(q.n), ..h }
                    } else {
                        let q = qw(xw).filter(|q| q.dist < delirium_data::kv!(113))?;
                        new(q, x, t0)
                    };
                    let d = h.world - xw;
                    let h = Held { local: Vec3::new(d.vd(tm.x_axis), d.vd(tm.y_axis), d.vd(tm.z_axis)), ..h };
                    let across_ok = |at: Vec3, n: Vec3| (0..delirium_data::kv!(0 int)).filter(|k| mask & 1 << k != 0).all(|k| {
                        let (p, e) = (t[k], t[(k + 1) % delirium_data::kv!(0 int)] - t[k]);
                        let (s, ee) = ((at - p).dot(e), e.dot(e));
                        let c = if !(s > delirium_data::kv!(66)) { p } else if ee - s > delirium_data::kv!(66) { p + e * (s / ee) } else { p + e };
                        if (c - at).length_squared() > delirium_data::kv!(65) * WORLD_MARGIN() * WORLD_MARGIN() {
                            return true;
                        }
                        let u = e.normalize();
                        let across = if u.dot(n) == 0.0 { n } else { (n - u * u.dot(n)).normalize() };
                        dirs[k + 1].dot(across) >= dirs[0].dot(dirs[k + 1]).min(1.0 - delirium_data::kv!(47)) - delirium_data::kv!(129)
                    });
                    if leaving(tm.transpose() * (h.world - xw), dirs[0]) || !across_ok(h.ground + h.n * WORLD_MARGIN(), h.n) {
                        return None;
                    }
                    Some((Held { m, inst, ..h }, poly))
                }).collect();
                for k in 0..found.len() {
                    let poly = found[k].1;
                    if found[..k].iter().any(|f| f.1.first == poly.first) {
                        continue;
                    }
                    let mut hits: Vec<(Vec3, Vec3)> = found[k..].iter().filter(|f| f.1.first == poly.first).map(|f| (f.0.ground + f.0.n * WORLD_MARGIN(), f.0.n)).collect();
                    if std::env::var("NM_SWDUMP").is_ok() { for h in &hits { eprintln!("onpre {:08x} {:08x} {:08x}", h.1.x.to_bits(), h.1.y.to_bits(), h.1.z.to_bits()); } }
                    poly.clamp(&mut hits, WORLD_MARGIN());
                    if std::env::var("NM_SWDUMP").is_ok() { for h in &hits { eprintln!("onpost {:08x} {:08x} {:08x}", h.1.x.to_bits(), h.1.y.to_bits(), h.1.z.to_bits()); } }
                    for (f, (at, n)) in found[k..].iter_mut().filter(|f| f.1.first == poly.first).zip(hits) {
                        (f.0.ground, f.0.n) = (at - n * WORLD_MARGIN(), n);
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
                        r.gap = (crate::vm4(tm.x_axis, tm.y_axis, tm.z_axis, bw, r.local) - (r.ground + r.n * WORLD_MARGIN())).vd(r.n);
                    }
                    r.age += if r.age == 1 { 2 } else { 1 };
                }
                let slid = |r: &Held| {
                    let a = crate::vm4(tm.x_axis, tm.y_axis, tm.z_axis, bw, r.local);
                    let d = (r.ground + r.n * WORLD_MARGIN()) - (-r.n).mul_add(Vec3::splat(r.gap), a);
                    d.vd(d)
                };
                if std::env::var("NM_SWDUMP").is_ok() { for r in held.iter() { eprintln!("oref {i} age {} gap {:e} slid {:e} local {}", r.age, r.gap, slid(r), r.local); } }
                let (mut k, mut worst) = (0, (None, 0.0));
                while k < held.len() {
                    if held[k].gap >= MARGIN() {
                        held.swap_remove(k);
                        continue;
                    }
                    let d = slid(&held[k]);
                    if held[k].age > delirium_data::kv!(1 int) && d > worst.1 && d > delirium_data::kv!(574) {
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
                let at = if r.age == 1 { r.world } else { crate::vm4(tm.x_axis, tm.y_axis, tm.z_axis, bw, r.local) } - o;
                let row = SURFACES.read().ok().and_then(|s| s.get(r.m as usize).copied()).unwrap_or([1.0, ELASTICITY()]);
                self.touch[i] |= delirium_data::kv!(7 int);
                add(at + o, r.n, -r.gap, -1);
                hits.push((i, at, r.n, r.gap, (part_friction(p) * row[0]).max(0.0), Vec3::ZERO, (PART_ELASTICITY() * row[1]).min(1.0), Some((i, k)), at));
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
                    let ghost = ob.entity == delirium_data::kv!(0 int) && c == 1;
                    let (cq, cx) = (rot * cp.m, ob.at - o + rot * cp.at);
                    let (cent, half) = ((cp.lo + cp.hi) * 0.5, (cp.hi - cp.lo) * 0.5);
                    let ac = cq.transpose() * (x + tm * sc - cx) - cent;
                    let r = cq.transpose() * tm;
                    let ah = Mat3::from_cols(r.x_axis.abs(), r.y_axis.abs(), r.z_axis.abs()) * sh;
                    let meet = ac.abs().cmple(ah + half + Vec3::splat(delirium_data::kv!(577))).all();
                    let (lm, la) = cp.last.unwrap_or((cp.m, cp.at));
                    let side_c = (cp.shape, half, (ob.at + rot * cp.at, cq), (lat + lrot * la, lrot * lm));
                    let a_world = (self.seen_w.get(i).map_or(a_last.0 + o, |&w| w), a_last.1);
                    let slot = self.held_car.iter().position(|h| (h.0, h.1, h.2) == (i, ob.id, c));
                    if !meet {
                        if let Some(s) = slot { self.held_car[s].3.clear(); }
                        continue;
                    }
                    let s = slot.unwrap_or_else(|| { self.held_car.push((i, ob.id, c, vec![])); self.held_car.len() - 1 });
                    Self::manifold(&mut self.held_car[s].3, (shape, bound_half(p), (bw, tm), a_world), side_c, ob.first);
                    if ghost {
                        ghosts.extend(std::iter::repeat(ob.first).take(self.held_car[s].3.len()));
                        self.held_car[s].3.clear();
                        continue;
                    }
                    for (k, r) in self.held_car[s].3.iter().enumerate() {
                        let (at, on) = (x + tm * r.local, cx + cq * r.ground);
                        self.touch[i] |= delirium_data::kv!(7 int);
                        add(at + o, r.n, -r.gap, ob.id);
                        hit_ob.push((hits.len(), ob.id, at + o, r.n));
                        let v = if ob.inv_mass > 0.0 { ob.vel_at(on + o) - ob.vel } else { ob.vel_at(on + o) };
                        hits.push((i, at, r.n, r.gap, part_friction(p) * ob.friction(), v, PART_ELASTICITY() * ELASTICITY(), Some((car_base + s, k)), on));
                    }
                }
            }
            for ob in obstacles.iter().filter(|ob| ob.parts.is_empty()) {
                let car = gjk::Shape::Box(ob.half, 0.0);
                let Some(q) = gjk::closest(&shape, tm, x, &car, Mat3::from_quat(ob.rot), ob.centre - o, Vec3::Z, f32::MAX.sqrt(), Some(gjk::Pen::Sphere)) else { continue };
                if q.dist >= MARGIN() {
                    continue;
                }
                self.touch[i] |= delirium_data::kv!(7 int);
                add(q.a + o, q.n, -q.dist, ob.id);
                hit_ob.push((hits.len(), ob.id, q.a + o, q.n));
                hits.push((i, q.a, q.n, q.dist, part_friction(p) * ob.friction(), ob.vel_at(q.b + o), PART_ELASTICITY() * ELASTICITY(), None, q.a));
            }
            if acc.2 > 0.0 {
                self.hit[i] = Some((acc.0 / acc.2, (acc.1 / acc.2).normalize_or_zero(), acc.3));
            }
            for &(o0, o1, or, ov, doll) in others {
                let axis = tm * Vec3::Y * p.half;
                let (here, there) = nearest(b.x - axis, b.x + axis, o0 - o, o1 - o);
                let (gap, room) = (here - there, p.radius + or);
                let dist = gap.length();
                if dist < room && dist > delirium_data::kv!(4) {
                    hits.push((i, here, gap / dist, (dist - room) * if doll { 0.5 } else { 1.0 }, part_friction(p) * delirium_data::kv!(18), ov, PART_ELASTICITY() * PART_ELASTICITY(), None, here));
                }
            }
        }
        let boxes: Vec<(Vec3, Vec3)> = (0..n_links).map(|k| { let f = (self.bodies[k].x, turns[k]); swept_box(&build.parts[k], f, self.seen.get(k).copied().unwrap_or(f)) }).collect();
        let world = |k: usize, f: (Vec3, Mat3)| ((self.bound_world(k), f.1), self.seen.get(k).map_or((f.0 + o, f.1), |s| (self.seen_w.get(k).copied().unwrap_or(s.0 + o), s.1)));
        let cur: Vec<_> = (0..n_links).map(|k| { let (w, s) = world(k, (self.bodies[k].x, turns[k])); (w, s, boxes[k]) }).collect();
        let prev = std::mem::replace(&mut self.pair_prev, cur.clone());
        let prev = if prev.len() == n_links { prev } else { cur };
        for (pi, &(a, c)) in self.pairs.clone().iter().enumerate() {
            let ((wa, sa, ba), (wc, sc, bc)) = (prev[a], prev[c]);
            let ((_, qa), (_, qc)) = (wa, wc);
            if boxes_meet(ba.0, qa, ba.1, bc.0, qc, bc.1) {
                while old.get(next_old).is_some_and(|((oa, oc), _)| (*oc, *oa) < (c, a)) { next_old += 1; }
                if old.get(next_old).is_some_and(|o| o.0 == (a, c)) { self.held_pairs[pi] = std::mem::take(&mut old[next_old].1); next_old += 1; }
                self.held_keys[pi] = Some((a, c));
            }
            Self::pair_manifold(&mut self.held_pairs[pi], (&build.parts[a], wa, sa), (&build.parts[c], wc, sc), [ba, bc]);
            let held = &mut self.held_pairs[pi];
            let off = (self.contact_off >> a | self.contact_off >> c) & 1 != 0;
            held.iter_mut().for_each(|r| r.off |= off);
            if std::env::var("NM_SWDUMP").is_ok() { for r in held.iter() { eprintln!("opair {a} {c} gap {:e} off {} age {} n {}", r.gap, r.off, r.age, r.n); } }
            for (x, r) in held.iter().enumerate().filter(|(_, r)| !r.off) {
                let (n, over) = (r.n, r.gap + ALLOW());
                let rw = self.bodies[self.order[0]].x + o;
                let lever = |p: Vec3| d(p - rw) + xr;
                let (at_a, at_c) = if r.age == 1 { (r.world, r.wc) } else { (wa.0 + wa.1 * r.local, wc.0 + wc.1 * r.ground) };
                let (at_a, at_c) = (lever(at_a), lever(at_c));
                let pair = |dir: Vec3| -> Vec<f64> {
                    lrow(a, at_a, d(dir), DVec3::ZERO).iter().zip(lrow(c, at_c, d(dir), DVec3::ZERO)).map(|(x, y)| x - y).collect()
                };
                let e = (PART_ELASTICITY() * PART_ELASTICITY()) as f64;
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
            held.iter_mut().for_each(|r| r.off = crate::rigid::contact_pass(if r.off { 0 } else { delirium_data::kv!(252 int) }, 0, on) < delirium_data::kv!(252 int));
        }
        let on = |&(_, h): &(usize, &(usize, Vec3, Vec3, f32, f32, Vec3, f32, Option<(usize, usize)>, Vec3))| self.contact_off >> h.0 & 1 == 0 && h.7.map_or(true, |r| !self.rec(r).off);
        if std::env::var("NM_SWDUMP").is_ok() { for h in &hits { eprintln!("ohit k {} at {} gap {:e} held {:?} on {}", h.0, h.1 + xr.as_vec3(), h.3, h.7, on(&(0, h))); } }
        let mut hits: Vec<_> = hits.iter().enumerate().filter(on).map(|(i, h)| (i, *h)).collect();
        hits.sort_by_key(|&(_, h)| match h.7 {
            Some((k, x)) if k < n_links => (0, self.rec((k, x)).inst as i64, 0, k, x),
            Some((s, x)) if s >= car_base => { let (i, id, c, _) = &self.held_car[s - car_base]; (1, *id as u32 as i64, *c, *i, x) }
            _ => (2, 0, 0, 0, 0),
        });
        for (i, (k, at, n, gap, mu, v, e, held, vat)) in hits {
            let gap0 = gap;
            let gap = gap + ALLOW();
            let ts = frame(n);
            let j = ts.map(|t| lrow(k, d(at), d(t), DVec3::ZERO));
            let jv = if vat == at { j.clone() } else { ts.map(|t| lrow(k, d(vat), d(t), DVec3::ZERO)) };
            let to = DVec3::new((-gap.max(0.0) / h + v.dot(n)) as f64 - fv(&jv[0]) / (1.0 + e as f64), v.dot(ts[1]) as f64 - fv(&jv[1]), v.dot(ts[2]) as f64 - fv(&jv[2]));
            let warm = held.map_or(gwarm.get(i).copied().unwrap_or(Vec3::ZERO), |r| self.rec(r).imp);
            let ob = hit_ob.iter().find(|x| x.0 == i).map(|&(_, id, at, n)| (id, k, at, n));
            let body = ob.and_then(|o| obstacles.iter().position(|x| x.id == o.0 && x.inv_mass > 0.0));
            let b = match body {
                Some(o) => {
                    let dv = obstacles[o].force * (obstacles[o].inv_mass * h);
                    let to = to + DVec3::new(dv.dot(ts[0]) as f64 / (1.0 + e as f64), dv.dot(ts[1]) as f64, dv.dot(ts[2]) as f64);
                    Block::moving(artic, j, to, mu as f64, held, warm, (o, obstacles[o].inv_mass as f64, ts.map(|t| t.as_dvec3())))
                }
                None => Block { g: Some({
                    let rv = (Vec3::ZERO - artic::at_point(fc[k], vat - xr.as_vec3())) + v;
                    let dot = |t: Vec3| t.x.mul_add(rv.x, t.y.mul_add(rv.y, t.z * rv.z));
                    let over = ALLOW() + gap0;
                    let over = if -over >= 0.0 { 0.0 } else { over };
                    let to = Vec3::new(over.mul_add(-(1.0 / h), dot(ts[0]) / (e + 1.0)), dot(ts[1]), dot(ts[2]));
                    G7::new(artic, k, at - xr.as_vec3(), ts, to, mu)
                }), ..Block::new(artic, j, to, mu as f64, held, warm, false) },
            };
            blocks.push(Block { scale: elastic(e as f64), ob, ..b });
            push_blocks.push(Self::push_block(blocks.last().unwrap(), gap, h, held.map_or(Vec3::ZERO, |r| self.rec(r).pimp)));
        }
        let nb = blocks.len();
        ghosts.extend(obstacles.iter().filter(|o| o.first && o.entity == delirium_data::kv!(0 int) && o.inv_mass > 0.0).map(|_| true));
        let mut order: Vec<usize> = (n_self..nb).chain(nb..nb + ghosts.len()).chain(0..n_self).collect();
        sgi_sort(&mut order, |&i| usize::from(!blocks.get(i).map_or_else(|| ghosts[i - nb], |b| ahead(b, obstacles))));
        order.retain(|&i| i < nb);
        (blocks, push_blocks) = (order.iter().map(|&i| blocks[i].clone()).collect(), order.iter().map(|&i| push_blocks[i].clone()).collect());
        (rows, blocks, push_blocks, order, ghosts)
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
        Self::manifold(held, (part_shape(a), bound_half(a), fa, sa), (part_shape(c), bound_half(c), fc, sc), false);
    }

    fn manifold(held: &mut Vec<Held>, (a, ha, fa, sa): (gjk::Shape, Vec3, (Vec3, Mat3), (Vec3, Mat3)), (c, hc, fc, sc): (gjk::Shape, Vec3, (Vec3, Mat3), (Vec3, Mat3)), flip: bool) {
        let ((xa, qa), (xc, qc)) = (fa, fc);
        let ends = |r: &Held| (crate::vm4(qa.x_axis, qa.y_axis, qa.z_axis, xa, r.local), crate::vm4(qc.x_axis, qc.y_axis, qc.z_axis, xc, r.ground));
        let (la, lc) = (sa.0, sc.0);
        let moved = (xa - la) - (xc - lc);
        let fast = (qa.transpose() * moved).abs().cmpgt(ha).any() || (qc.transpose() * moved).abs().cmpgt(hc).any();
        let found = if held.is_empty() && fast {
            let hit = if flip { Self::cast_shapes((&c, &a), (lc, xc, qc), (la, xa, qa)).map(|(at_c, at_a, n)| (at_a, at_c, -n)) } else { Self::cast_shapes((&a, &c), (la, xa, qa), (lc, xc, qc)) };
            hit.map(|(at_a, at_c, n)| {
                let (pa, pc) = (xa + qa * at_a, xc + qc * at_c);
                (pa, pc, n, (pa - pc).dot(n))
            })
        } else if flip {
            Self::shape_contacts((&c, &a), fc, fa, MARGIN(), Vec3::Z).into_iter().min_by(|x, y| x.3.total_cmp(&y.3)).map(|(pc, pa, n, gap)| (pa, pc, -n, gap))
        } else {
            Self::shape_contacts((&a, &c), fa, fc, MARGIN(), Vec3::Z).into_iter().min_by(|x, y| x.3.total_cmp(&y.3))
        };
        if std::env::var("NM_SWDUMP").is_ok() { eprintln!("omani fast {} held {} found {:?} sc {:?}", fast, held.len(), found.map(|f| (f.3, f.2)), Self::shape_contacts((&a, &c), fa, fc, MARGIN(), Vec3::Z).into_iter().map(|x| (x.3, x.2)).collect::<Vec<_>>()); }
        if let Some((at_a, at_c, n, gap)) = found {
            let into = |q: Mat3, v: Vec3| Vec3::new(v.vd(q.x_axis), v.vd(q.y_axis), v.vd(q.z_axis));
            let new = Held { local: into(qa, at_a - xa), ground: into(qc, at_c - xc), gap, age: 0, m: 0, imp: Vec3::ZERO, pimp: Vec3::ZERO, n, inst: 0, off: false, world: at_a, wc: at_c };
            let swap = |h: &mut Vec<Held>| if flip { h.iter_mut().for_each(|r| std::mem::swap(&mut r.local, &mut r.ground)) };
            swap(held);
            hold(held, if flip { Held { local: new.ground, ground: new.local, ..new } } else { new });
            swap(held);
        }
        for r in held.iter_mut() {
            if r.age != 0 {
                let (pa, pc) = ends(r);
                r.gap = (pa - pc).vd(r.n);
            }
            r.age += if r.age == 1 { 2 } else { 1 };
        }
        let slid = |r: &Held| { let (pa, pc) = ends(r); let d = pc - (-r.n).mul_add(Vec3::splat((pa - pc).vd(r.n)), pa); d.vd(d) };
        let (mut k, mut worst) = (0, (None, 0.0));
        while k < held.len() {
            if held[k].gap >= MARGIN() {
                held.swap_remove(k);
                continue;
            }
            let d = slid(&held[k]);
            if held[k].age > delirium_data::kv!(1 int) && d > worst.1 && d > delirium_data::kv!(574) {
                worst = (Some(k), d);
            }
            k += 1;
        }
        if let Some(k) = worst.0 {
            held.swap_remove(k);
        }
    }

    fn cast_shapes((a, c): (&gjk::Shape, &gjk::Shape), (la, xa, qa): (Vec3, Vec3, Mat3), (lc, xc, qc): (Vec3, Vec3, Mat3)) -> Option<(Vec3, Vec3, Vec3)> {
        let at = |t: f32| (Self::cast_at(la, xa, t), Self::cast_at(lc, xc, t));
        let axis = std::cell::Cell::new(Vec3::Z);
        let query = |t: f32| { let (pa, pc) = at(t); Self::shape_contacts((a, c), (pa, qa), (pc, qc), f32::INFINITY, axis.get()).into_iter().min_by(|x, y| x.3.total_cmp(&y.3))
            .map(|(a, b, n, dist)| { axis.set(n); gjk::Closest { a, b, n, dist } }) };
        let (t, q) = Self::advance(query, (xc - lc) - (xa - la))?;
        let (pa, pc) = at(t);
        Some((qa.transpose() * (q.a - pa), qc.transpose() * (q.b - pc), q.n))
    }

    fn cast_at(last: Vec3, now: Vec3, t: f32) -> Vec3 {
        (last - now).mul_add(Vec3::splat(1.0 - t), now)
    }

    fn advance(query: impl Fn(f32) -> Option<gjk::Closest>, r: Vec3) -> Option<(f32, gjk::Closest)> {
        let (mut t, mut prev, mut q) = (0.0f32, 0.0f32, query(0.0)?);
        let mut step = q.dist;
        if step > delirium_data::kv!(129) {
            for it in 1.. {
                let nr = q.n.dot(r);
                let dt = if nr != 0.0 { step / nr } else { f32::MAX };
                t += dt;
                if it > delirium_data::kv!(52 int) || dt < 0.0 || t > 1.0 {
                    return None;
                }
                if t <= prev {
                    break;
                }
                prev = t;
                q = query(t)?;
                if q.dist <= delirium_data::kv!(578) {
                    break;
                }
                step = q.dist - delirium_data::kv!(113);
            }
        }
        Some((t, q))
    }
    fn shape_contacts((a, c): (&gjk::Shape, &gjk::Shape), ba: (Vec3, Mat3), bc: (Vec3, Mat3), reach: f32, axis: Vec3) -> Vec<(Vec3, Vec3, Vec3, f32)> {
        gjk::closest(a, ba.1, ba.0, c, bc.1, bc.0, axis, f32::MAX.sqrt(), Some(gjk::Pen::Sphere))
            .filter(|q| q.dist < reach).map(|q| (q.a, q.b, q.n, q.dist)).into_iter().collect()
    }

    fn velocities(&self, artic: &Artic, lt: &[Acc], h: f32, forces: bool) -> Vec<(Vec3, Vec3)> {
        let root = self.order[0];
        let xr = self.bodies[root].x;
        let x: Vec<Vec3> = self.bodies.iter().map(|b| b.x - xr).collect();
        let (v, w): (Vec<Vec3>, Vec<Vec3>) = self.bodies.iter().map(|b| (b.v, b.w)).unzip();
        let acc: &[artic::P2] = lt;
        let mut imp: Vec<artic::P2> = vec![(Vec3::ZERO, Vec3::ZERO); x.len()];
        for &(k, at, i, a) in self.pending.iter().filter(|_| !forces) {
            let a = a.as_dvec3() + (at - xr).as_dvec3().cross(i.as_dvec3());
            imp[k] = (imp[k].0 + i, imp[k].1 + a.as_vec3());
        }
        artic.velocities(root, &x, &v, &w, acc, &imp, h, forces)
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

    pub(crate) fn rejuvenate_next(&mut self, on: bool) {
        self.rejuv = if on { delirium_data::kv!(7 int) } else { 0 };
    }

    fn rejuvenate(&mut self) {
        self.rejuv += 1;
        if self.rejuv <= delirium_data::kv!(7 int) {
            return;
        }
        self.held_inst = Some(crate::rigid::resquare(self.inst()));
        self.bodies.iter_mut().for_each(|b| b.set_r(crate::rigid::resquare(b.r)));
        let x = self.mwc.0 as u64 * delirium_data::kv!(187 int) + self.mwc.1 as u64;
        self.mwc = (x as u32, (x >> delirium_data::kv!(7 int)) as u32);
        self.rejuv = (x as u32 & delirium_data::kv!(529 int)) % delirium_data::kv!(51 int);
    }

    pub fn step(&mut self, build: &Build, world: &Collision, dt: f32, drive: Drive, obstacles: &[Obstacle], others: &[Capsule]) {
        let Some(st) = self.begin(build, world, dt, &drive, obstacles, others) else { return };
        self.finish(st, drive.lift);
    }

    fn finish(&mut self, mut st: Stage, lift: f32) {
        let base = st.qd0.chunks(delirium_data::kv!(6 int)).map(|q| (Vec3::new(q[delirium_data::kv!(0 int)] as f32, q[delirium_data::kv!(1 int)] as f32, q[delirium_data::kv!(2 int)] as f32), Vec3::new(q[0] as f32, q[1] as f32, q[2] as f32))).collect();
        if !impulses_g(&mut st.rows, &mut st.blocks, &st.artic, base) {
            impulses(&mut st.rows, &mut st.blocks, &mut st.qd0.clone(), &mut st.vb.clone());
        }
        if !self.middle(&mut st) {
            return;
        }
        let mut qp = vec![0.0; st.qd0.len()];
        let n = st.qd0.len() / delirium_data::kv!(6 int);
        if std::env::var("NM_NOPG").is_err() && impulses_g(&mut st.push, &mut st.push_blocks, &st.artic, vec![(Vec3::ZERO, Vec3::ZERO); n]) {
            let acc = applied(&st.push, &st.push_blocks, vec![(Vec3::ZERO, Vec3::ZERO); n]).unwrap();
            if std::env::var("NM_PUSHDUMP").is_ok() { for (k, a) in acc.iter().enumerate() { eprintln!("opacc {k} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e}", a.0.x, a.0.y, a.0.z, a.1.x, a.1.y, a.1.z); } }
            let xr = self.bodies[self.order[0]].x;
            st.pv = Some(st.artic.solve_f(&acc).into_iter().zip(&self.bodies).map(|((w, l), b)| (artic::at_point((w, l), b.x - xr), w)).collect());
        } else {
            impulses(&mut st.push, &mut st.push_blocks, &mut qp, &mut vec![DVec3::ZERO; st.vb.len()]);
        }
        let pushed = !(st.push.is_empty() && st.push_blocks.is_empty());
        self.end(st, &qp, lift, pushed);
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
        if d.length_squared() > delirium_data::kv!(28) * delirium_data::kv!(28) {
            self.o += d;
            self.bodies.iter_mut().for_each(|b| b.x -= d);
            self.pins.iter_mut().flatten().for_each(|p| p.translation -= d);
            self.pending.iter_mut().for_each(|p| p.1 -= d);
        }
        self.age += dt;
        for t in &mut self.touch {
            *t = (*t & !delirium_data::kv!(90 int)) | ((*t >> delirium_data::kv!(0 int)) & delirium_data::kv!(1 int));
        }
        self.hit.iter_mut().for_each(|h| *h = None);
        let joints: Vec<Option<Joint>> = build.parts.iter().enumerate().map(|(i, p)| p.parent.map(|j| {
            let j = Joint::new(p, j, drive.rigid.get(i));
            Joint { jc: self.jconst(build, i, &j, drive.rigid.get(i)), ..j }
        })).collect();
        let subs = 1;
        let h = dt / subs as f32;
        let spin = if drive.rigid.is_empty() { MAX_SPIN } else { MAX_SPIN_NM() };
        let fall = Vec3::Z * -GRAVITY() * (1.0 - drive.lift) + drive.accel;
        let (start, o0) = (self.bodies.iter().map(|b| (b.x, b.q)).collect::<Vec<_>>(), self.o);
        let along = 1.0 / subs as f32;
        let mut pose: Vec<Option<(Vec3, Quat)>> = self.pins.iter().zip(&start).map(|(p, &(x, q))| {
            p.map(|to| (x.lerp(to.translation, along), q.slerp(to.rotation, along)))
        }).collect();
        let was: Vec<Option<Joint>> = joints.iter().zip(&self.stiff).map(|(j, &stiff)| j.map(|j| Joint { stiff, ..j })).collect();
        let artic = self.artic(if was.len() == joints.len() { &was } else { &joints });
        self.stiff = joints.iter().map(|j| j.map_or(0.0, |j| j.stiff)).collect();
        let mut lt: Vec<Acc> = vec![(Vec3::ZERO, Vec3::ZERO); self.bodies.len()];
        let xr0 = self.bodies[self.order[0]].x;
        for (k, b) in self.bodies.iter().enumerate() {
            lt[k].1 += drive.torque.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO);
            let f = drive.force.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO);
            if f != Vec3::ZERO {
                lt[k].0 += f;
                lt[k].1 = moment(f, b.x - xr0, lt[k].1);
            }
        }
        for &(k, f, d) in &self.root_moment {
            lt[k].0 += f;
            lt[k].1 = moment(f, d, lt[k].1);
        }
        let mut jt = vec![Vec3::ZERO; self.bodies.len()];
        let mut drive_ev = vec![];
        let mut hb = vec![None; self.bodies.len()];
        self.muscles(&artic, &joints, &drive, h, &mut drive_ev, &mut jt, &mut hb);
        self.soft_limits(&artic, &joints, &drive, &mut drive_ev, &mut jt);
        self.hinge_b = hb;
        apply(&joints, &drive_ev, &mut lt);
        for (k, b) in self.bodies.iter().enumerate() {
            let f = (fall + Vec3::Z * GRAVITY()) * b.m + drive.force.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO);
            self.out[k] = (f, drive.torque.get(k).filter(|_| self.gone >> k & 1 == 0).copied().unwrap_or(Vec3::ZERO) + jt[k]);
        }
        self.joint_torque = jt.clone();
        for (k, j) in joints.iter().enumerate() {
            if let Some(j) = j {
                self.out[j.parent].1 -= jt[k];
            }
        }
        self.held_inst = None;
        let inst = self.inst();
        if self.coll.len() != self.bodies.len() {
            self.coll = self.comp(&inst);
        }
        let xr = self.bodies[self.order[0]].x;
        let ip = self.inst_place(&inst);
        let seen_w: Vec<Vec3> = self.last_a.iter().map(|&a| Self::chain(a, &inst, ip)).collect();
        let mut now: Vec<(Vec3, Mat3)> = self.coll.iter().map(|(d, c)| (xr + crate::rigid::row_mul(*d, &inst), turn(&crate::rigid::rows_mul(c, &inst)))).collect();
        let d = self.bodies[self.order[0]].v * h;
        self.pending.iter_mut().for_each(|p| p.1 += d);
        let mut vo: Vec<Vec3> = self.bodies.iter().map(|b| b.v - b.w.vx(b.x)).collect();
        for b in &mut self.bodies {
            let (v, w) = (b.v, b.w);
            b.moved(v, w, h);
        }
        self.rejoin(&joints, &pose);
        for (k, b) in self.bodies.iter().enumerate().filter(|&(k, _)| joints[k].is_some()) {
            vo[k] = b.v - b.w.vx(b.x);
        }
        let back = self.recentre(&mut pose);
        for (vo, b) in vo.iter_mut().zip(&self.bodies) {
            let (w, s) = (b.w, back);
            *vo = Vec3::new(-w.y, -w.z, -w.x).mul_add(Vec3::new(s.z, s.x, s.y), Vec3::new(w.z, w.x, w.y).mul_add(Vec3::new(s.y, s.z, s.x), *vo));
        }
        now.iter_mut().for_each(|f| f.0 += back);
        let artic = self.artic(&joints);
        self.rejuvenate();
        for (k, f, d) in std::mem::take(&mut self.root_moment) {
            self.out[k].0 += f;
            self.out[k].1 += moment(f, d, Vec3::ZERO);
        }
        let xr = self.bodies[self.order[0]].x;
        for (k, b) in self.bodies.iter().enumerate() {
            let f = fall * b.m;
            lt[k].0 += f;
            lt[k].1 = moment(f, b.x - xr, lt[k].1);
        }
        let before: Vec<Acc> = lt.clone();
        let mut limit_ev = vec![];
        self.limits(&artic, &joints, h, &mut limit_ev);
        apply(&joints, &limit_ev, &mut lt);
        self.stage_lim = lt.iter().zip(&before).map(|(a, b)| a.1 - b.1).collect();
        self.stage_lt = lt.iter().map(|&(f, t)| v6(f.as_dvec3(), t.as_dvec3())).collect();
        let dvw: Vec<(Vec3, Vec3)> = self.velocities(&artic, &lt, h, true).iter().zip(&self.bodies).map(|(&(v, w), b)| (v - b.v, w - b.w)).collect();
        if env_on!("NM_DVW") { eprintln!("dvw {}", dvw.iter().zip(&self.bodies).enumerate().map(|(k, ((v, w), b))| format!("{k}:{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.4},{:.4},{:.4}", v.x, v.y, v.z, w.x, w.y, w.z, b.x.x, b.x.y, b.x.z)).collect::<Vec<_>>().join(" ")); }
        let free_vel: Vec<(Vec3, Vec3)> = self.bodies.iter().zip(&dvw).map(|(b, d)| (b.v + d.0, b.w + d.1)).collect();
        self.armature = artic.arm.clone();
        let x0 = self.bodies[self.order[0]].x.as_dvec3();
        let qd0: Vec<f64> = vo.iter().zip(&self.bodies).flat_map(|(vo, b)| { let w = b.w.as_dvec3(); v6(vo.as_dvec3() + w.cross(x0), w) }).collect();
        let free = spatial_of(&dvw, &self.bodies, x0);
        self.seen = now;
        self.seen_w = seen_w;
        self.coll = self.comp(&self.inst());
        let (mut rows, blocks, push_blocks, order, ghosts) = self.contacts(build, world, &artic, h, &pose, obstacles, others, &free, &artic.forces(&lt, h));
        let (lr, push) = self.limit_rows(&artic, &joints, h, &qd0);
        rows.extend(lr);
        let vb = obstacles.iter().map(|o| o.vel.as_dvec3()).collect();
        Some(Stage { joints, pose, start, o0, artic, lt, x0, pv: None, qd0, rows, blocks, push, push_blocks, h, spin, free_vel, damp: 0.0, free, vb, order, ghosts })
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
        let xr = self.bodies[self.order[0]].x;
        let mut acc0 = vec![(Vec3::ZERO, Vec3::ZERO); self.bodies.len()];
        for &(k, at, i, a) in &self.pending {
            acc0[k].1 = artic::at_point((at - xr, acc0[k].1), i) + a;
            acc0[k].0 += i;
        }
        if let Some(acc) = applied(&st.rows, &st.blocks, acc0) {
            if std::env::var("NM_SWDUMP").is_ok() { for p in &self.pending { eprintln!("opend {} {:?} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e}", p.0, p.1 - xr, p.2.x, p.2.y, p.2.z, p.3.x, p.3.y, p.3.z); } }
            self.pending.clear();
            if std::env::var("NM_SWDUMP").is_ok() { for (k, a) in acc.iter().enumerate() { eprintln!("ovacc {k} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}", a.0.x.to_bits(), a.0.y.to_bits(), a.0.z.to_bits(), a.1.x.to_bits(), a.1.y.to_bits(), a.1.z.to_bits()); } }
            self.pending.extend(acc.into_iter().enumerate().map(|(k, (f, m))| (k, xr, f, m)));
        } else {
            for (k, g) in gi.chunks(delirium_data::kv!(6 int)).enumerate().filter(|(_, g)| g.iter().any(|&x| x != 0.0)) {
                self.pending.push((k, st.x0.as_vec3(), DVec3::new(g[0], g[1], g[2]).as_vec3(), DVec3::new(g[delirium_data::kv!(0 int)], g[delirium_data::kv!(1 int)], g[delirium_data::kv!(2 int)]).as_vec3()));
            }
        }
        let vw = self.velocities(&st.artic, &st.lt, st.h, false);
        self.pending.clear();
        for (b, (v, w)) in self.bodies.iter_mut().zip(vw) {
            (b.v, b.w) = (v, w);
        }
        self.stage_vel = self.bodies.iter().map(|b| (b.v, b.w)).collect();
        if env_on!("NM_SVFREE") { self.stage_vel = st.free_vel.clone(); }
        if env_on!("NM_VELONLY") { return false; }
        let damp = (-DAMPING()).fma(st.h, 1.0);
        for b in &mut self.bodies {
            b.v = b.v.clamp_length_max(MAX_SPEED()) * damp;
            b.w = b.w.clamp_length_max(st.spin) * damp;
        }
        if env_on!("NM_SVPOST") { self.stage_vel = self.bodies.iter().map(|b| (b.v, b.w)).collect(); }
        st.damp = damp;
        true
    }

    fn end(&mut self, mut st: Stage, qp: &[f64], lift: f32, pushed: bool) {
        for (p, b) in st.push_blocks.iter().zip(&st.blocks) {
            match b.held {
                Some(r) => self.rec_mut(r).pimp = p.lambda.as_vec3(),
                None => {}
            }
        }
        if pushed {
            for (k, b) in self.bodies.iter_mut().enumerate() {
                let (v, w) = match &st.pv {
                    Some(pv) => pv[k],
                    None => {
                        let (pv, pw) = artic::parts(std::array::from_fn(|c| qp[delirium_data::kv!(6 int) * k + c]));
                        ((pv + pw.cross(b.x.as_dvec3() - st.x0)).as_vec3(), pw.as_vec3())
                    }
                };
                b.moved(v.clamp_length_max(delirium_data::kv!(556) / st.h) * st.damp, w.clamp_length_max(delirium_data::kv!(579) / st.h) * st.damp, st.h);
            }
            let root = &mut self.bodies[self.order[0]];
            let (p, w) = (root.x, root.w);
            root.v = (root.v + Vec3::new(p.z * w.y, p.x * w.z, p.y * w.x)) - Vec3::new(p.y * w.z, p.z * w.x, p.x * w.y);
            self.rejoin(&st.joints, &st.pose);
            self.recentre(&mut st.pose);
        }
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
    lt: Vec<Acc>,
    x0: DVec3,
    pv: Option<Vec<(Vec3, Vec3)>>,
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
    vb: Vec<DVec3>,
    order: Vec<usize>,
    ghosts: Vec<bool>,
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
                let r = Vec3::splat(bound_half(p).length() + MARGIN());
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
                            let (nn, over) = (r.n, r.gap + ALLOW());
                            let rows = |d: &Ragdoll, s: &Stage, k: usize, at: Vec3, sign: f64| frame(nn).map(|t| {
                                let mut j = point_row(d.bodies.len(), s.x0, k, (at - d.o).as_dvec3(), t.as_dvec3());
                                j.iter_mut().for_each(|x| *x *= sign);
                                let imp: Vec<V6> = j.chunks(delirium_data::kv!(6 int)).map(|c| std::array::from_fn(|i| c[i])).collect();
                                let mj: Vec<f64> = s.artic.solve(&imp).into_iter().flatten().collect();
                                (j, mj)
                            });
                            let (ra, rc) = (rows(di, si, a, at_a, 1.0), rows(dk, sk, c, at_c, -1.0));
                            let dot = |x: &[f64], y: &[f64]| x.iter().zip(y).map(|(x, y)| x * y).sum::<f64>();
                            let k3 = DMat3::from_cols_array_2d(&std::array::from_fn(|p| std::array::from_fn(|q| dot(&ra[q].0, &ra[p].1) + dot(&rc[q].0, &rc[p].1))));
                            let fv = |t: usize| dot(&ra[t].0, &si.free) + dot(&rc[t].0, &sk.free);
                            let e = (PART_ELASTICITY() * PART_ELASTICITY()) as f64;
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
            let (mut vb, mut vb_at) = (vec![], vec![0; n]);
            let shift = |b: Block, by: isize| Block { body: b.body.map(|(o, m, t)| (o.wrapping_add_signed(by), m, t)), ..b };
            for &i in &is {
                let s = st[i].as_mut().unwrap();
                vb_at[i] = vb.len();
                vb.extend(s.vb.iter().copied());
                let (rb, pbase) = (rows.len(), push.len());
                (firsts[i], counts[i]) = ((rb, pbase), (s.rows.len(), s.push.len()));
                rows.extend(std::mem::take(&mut s.rows).into_iter().map(|r| lay_row(r, alone_at(off[i]), rb as isize)));
                push.extend(std::mem::take(&mut s.push).into_iter().map(|r| lay_row(r, alone_at(off[i]), pbase as isize)));
                let s = &*s;
                let mut pre = vec![0; s.order.len()];
                for (p, &o) in s.order.iter().enumerate() {
                    pre[o] = p;
                }
                let n_links = ms[i].doll.bodies.len();
                let is_self = |p: usize| s.blocks[p].held.is_some_and(|(k, _)| k >= n_links);
                let (ground, own) = (nodes.len(), ms[i].id + 1);
                if pre.iter().any(|&p| !is_self(p)) || !s.ghosts.is_empty() {
                    nodes.push((Some(own), None));
                }
                let own_self = nodes.len();
                if pre.iter().any(|&p| is_self(p)) {
                    nodes.push((Some(own), Some(own)));
                }
                let base = bank.len();
                list.extend(pre.iter().map(|&p| Entry { node: if is_self(p) { own_self } else { ground }, a: if ahead(&s.blocks[p], ms[i].obstacles) { 0 } else { own }, b: if is_self(p) { own } else { 0 },
                    at: base + p, src: Src::Own(i, p) }));
                list.extend(s.ghosts.iter().map(|&g| Entry { node: ground, a: if g { 0 } else { own }, b: 0, at: usize::MAX, src: Src::Own(i, usize::MAX) }));
                let s = st[i].as_mut().unwrap();
                bank.extend(std::mem::take(&mut s.blocks).into_iter().map(|b| Some(Block { at: alone_at(off[i]), ..shift(b, vb_at[i] as isize) })));
                push_bank.extend(std::mem::take(&mut s.push_blocks).into_iter().map(|b| Some(Block { at: alone_at(off[i]), ..shift(b, vb_at[i] as isize) })));
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
            list.retain(|x| x.at != usize::MAX);
            pushed.retain(|x| x.at != usize::MAX);
            let mut blocks: Vec<Block> = list.iter().map(|x| bank[x.at].take().unwrap()).collect();
            let mut push_blocks: Vec<Block> = pushed.iter().map(|x| push_bank[x.at].take().unwrap()).collect();
            let mut qd: Vec<f64> = is.iter().flat_map(|&i| st[i].as_ref().unwrap().qd0.clone()).collect();
            impulses(&mut rows, &mut blocks, &mut qd, &mut vb.clone());
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
                        own[i][p] = Some(Block { at: Spot::OWN, ..shift(b, -(vb_at[i] as isize)) });
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
            let any_push = !(push.is_empty() && push_blocks.is_empty());
            impulses(&mut push, &mut push_blocks, &mut qp, &mut vec![DVec3::ZERO; vb.len()]);
            let mut own: Vec<Vec<Option<Block>>> = (0..n).map(|_| vec![]).collect();
            for (x, b) in pushed.iter().zip(push_blocks) {
                match x.src {
                    Src::Own(i, p) => {
                        if own[i].len() <= p {
                            own[i].resize_with(p + 1, || None);
                        }
                        own[i][p] = Some(Block { at: Spot::OWN, ..shift(b, -(vb_at[i] as isize)) });
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
                    ms[i].doll.end(s, &qp[off[i]..off[i] + len], ms[i].drive.lift, any_push);
                }
            }
        }
    }
}

fn point_row(n_links: usize, xr: DVec3, k: usize, at: DVec3, n: DVec3) -> Vec<f64> {
    let mut r = vec![0.0; delirium_data::kv!(6 int) * n_links];
    r[delirium_data::kv!(6 int) * k..delirium_data::kv!(6 int) * k + delirium_data::kv!(6 int)].copy_from_slice(&v6(n, (at - xr).cross(n)));
    r
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
                (_, b.v, b.w) = crate::ragstart::start_part(rows(c), Some(&rows(&p)), Vec3::ZERO, dt, [0.5, delirium_data::kv!(5), 2.0], v_ref);
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

fn ahead(b: &Block, obstacles: &[Obstacle]) -> bool {
    b.ob.is_some_and(|o| obstacles.iter().any(|x| x.id == o.0 && x.first))
}

fn sgi_sort<T>(v: &mut [T], key: impl Fn(&T) -> usize + Copy) {
    fn split<T>(mut v: &mut [T], key: impl Fn(&T) -> usize + Copy) {
        while v.len() > delirium_data::kv!(51 int) {
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
    let sorted = |n: usize| { let mut v: Vec<usize> = (0..n).collect(); sgi_sort(&mut v, |_| 0); v };
    assert_eq!(sorted(delirium_data::kv!(51 int)), (0..delirium_data::kv!(51 int)).collect::<Vec<_>>());
    assert_eq!(sorted(delirium_data::kv!(457 int)), (0..delirium_data::kv!(457 int)).rev().collect::<Vec<_>>());
    assert_eq!(sorted(delirium_data::kv!(91 int)), (delirium_data::kv!(122 int)..delirium_data::kv!(91 int)).chain(0..delirium_data::kv!(122 int)).collect::<Vec<_>>());
}

#[test]
fn island_order_as_the_game() {
    assert_eq!(island_order(&[(Some(1), None), (Some(1), Some(1))]), [0, 1]);
    assert_eq!(island_order(&[(Some(1), None), (Some(1), Some(1)), (Some(2), None), (Some(2), Some(2)), (Some(1), Some(2))]), [2, delirium_data::kv!(0 int), 0, 1, delirium_data::kv!(1 int)]);
    assert_eq!(island_order(&[(Some(1), Some(2)), (Some(1), None), (Some(1), Some(1)), (Some(2), None), (Some(2), Some(2))]), [0, 1, 2, delirium_data::kv!(0 int), delirium_data::kv!(1 int)]);
    assert_eq!(island_order(&[(Some(1), None), (Some(2), None), (Some(delirium_data::kv!(0 int)), None), (Some(1), Some(2)), (Some(2), Some(delirium_data::kv!(0 int)))]), [0, delirium_data::kv!(0 int), 1, delirium_data::kv!(1 int), 2]);
}

pub(crate) fn par_each<T: Send>(items: &mut [T], f: impl Fn(&mut T) + Sync) {
    let cores = std::thread::available_parallelism().map_or(1, |c| c.get()).min(items.len() / delirium_data::kv!(0 int));
    if cores < 2 {
        return items.iter_mut().for_each(f);
    }
    let f = &f;
    std::thread::scope(|s| items.chunks_mut(items.len().div_ceil(cores)).for_each(|c| { s.spawn(move || c.iter_mut().for_each(f)); }));
}

