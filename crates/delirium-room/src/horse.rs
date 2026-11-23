use crate::rdr2frag;
#[path = "horse_rdr2.rs"]
pub(super) mod rdr2;
pub use rdr2::TABLE;
#[path = "horse_ours.rs"]
pub mod ours;
#[path = "horse_object.rs"]
mod object;
use bevy::prelude::*;
use delirium::collision::Collision;
use delirium::ragdoll::{Build, Capsule, Drive, Limit, Muscle, Obstacle, PartDef, Ragdoll, GRAVITY};

const PARTS: [(&str, &str); 26] = [
    ("skel_pelvis", "Pelvis"), ("skel_spine_root", "Root"), ("skel_r_thigh", "R_Thigh"), ("skel_l_thigh", "L_Thigh"),
    ("skel_tail1", "Tail1"), ("skel_spine1", "Spine1"), ("skel_r_calf", "R_Calf"), ("skel_r_foot", "R_HindFoot"),
    ("skel_l_calf", "L_Calf"), ("skel_l_foot", "L_HindFoot"), ("skel_tail4", "Tail4"), ("skel_spine2", "Spine2"),
    ("skel_spine3", "Spine3"), ("skel_r_upperarm", "R_UpperArm"), ("skel_l_upperarm", "L_UpperArm"), ("skel_neck1", "Neck1"),
    ("skel_neck1", "Neck1b"), ("skel_r_forearm", "R_Forearm"), ("skel_r_hand", "R_ForeFoot"), ("skel_l_forearm", "L_Forearm"),
    ("skel_l_hand", "L_ForeFoot"), ("skel_neck3", "Neck3"), ("skel_neck3", "Neck3b"), ("skel_neck5", "Neck5"),
    ("skel_head", "Head"), ("skel_head", "Headb"),
];
pub const LEGS: [[usize; 3]; 4] = [[2, 6, 7], [3, 8, 9], [13, 17, 18], [14, 19, 20]];
pub const HEAD: usize = 24;
pub const NECK_HEAD: [usize; 7] = [15, 16, 21, 22, 23, 24, 25];
const NECK1: usize = 15;
const BARREL: [usize; 4] = [0, 5, 11, 12];
const PELVIS: usize = 0;
const ROOT_LINK: usize = 0;
const PELVIS_LINK: usize = 1;
pub const MID: usize = 11;
const LAIN: f32 = 1.5;
const RISE: [f32; 3] = [1.2, 0.9, 1.0];

struct Rise { t: f32, from: Vec<Transform>, xy: Vec2, yaw: f32, ground: f32 }
const CALM: f32 = 1.0;
const ANCHOR_T: f32 = 0.3;
const ANCHOR_SLIP: f32 = 0.04;
const SHOULDERS: usize = 12;
const WELD_PLAY: f32 = 0.05;
const WELD: f32 = 60.0;
const WELDED: [usize; 3] = [16, 22, 25];
const HEALTH: f32 = 400.0;
const LAME: f32 = 60.0;
const GIVE: f32 = 0.2;
const SEAT: f32 = 0.15;
const HEAD_LIFT: f32 = 1.3;

fn group(k: usize) -> usize {
    match k {
        0 | 1 | 5 | 11 | 12 => 0,
        _ if LEGS.iter().any(|l| l.contains(&k)) => 1,
        15 | 16 | 21 | 22 | 23 => 2,
        24 | 25 => 3,
        _ => 4,
    }
}

#[derive(Clone, Debug)]
pub struct Quadruped {
    pub leg_stiffness: f32,
    pub leg_stiffness_min: f32,
    pub spine_stiffness: f32,
    pub spine_stiffness_min: f32,
    pub strength_decay: f32,
    pub fail_angle: f32,
    pub weak_zero: f32,
    pub weak_ramp: f32,
    pub snap: bool,
    pub snap_mag: f32,
    pub lift_head: bool,
    pub scrabble: bool,
    pub scrabble_min: f32,
    pub scrabble_max: f32,
    pub writhe: bool,
    pub writhe_speed: f32,
    pub roll: bool,
    pub roll_velocity: f32,
    pub roll_scale: f32,
    pub righting: f32,
}

impl Default for Quadruped {
    fn default() -> Self {
        Quadruped { leg_stiffness: 15.0, leg_stiffness_min: 7.0, spine_stiffness: 15.0, spine_stiffness_min: 7.0,
                    strength_decay: 0.5, fail_angle: 0.6, weak_zero: 0.0, weak_ramp: 0.4, snap: true, snap_mag: 1.0,
                    lift_head: true, scrabble: true, scrabble_min: 3.0, scrabble_max: 11.0, writhe: true,
                    writhe_speed: 0.5, roll: false, roll_velocity: 0.7, roll_scale: 0.7, righting: 0.0 }
    }
}

impl Quadruped {
    pub fn fields(&mut self) -> [(&'static str, &mut f32, f32, f32); 15] {
        [("legStiffness", &mut self.leg_stiffness, 6.0, 17.0), ("legStiffnessMin", &mut self.leg_stiffness_min, 3.0, 30.0),
         ("spineStiffness", &mut self.spine_stiffness, 6.0, 30.0), ("spinestiffnessmin", &mut self.spine_stiffness_min, 3.0, 30.0),
         ("strengthdecay", &mut self.strength_decay, 0.0, 10.0), ("failangle", &mut self.fail_angle, -1.0, 1.0),
         ("initialWeaknessZeroDuration", &mut self.weak_zero, 0.0, 10.0), ("initialWeaknessRampDuration", &mut self.weak_ramp, 0.0, 10.0),
         ("snapMag", &mut self.snap_mag, -5.0, 5.0), ("minscrabblespeed", &mut self.scrabble_min, 0.0, 20.0),
         ("maxscrabblespeed", &mut self.scrabble_max, 0.0, 20.0), ("writhespeed", &mut self.writhe_speed, 0.0, 10.0),
         ("rollvelocity", &mut self.roll_velocity, 0.0, 10.0), ("rollscale", &mut self.roll_scale, 0.0, 10.0),
         ("rightingTorque", &mut self.righting, 0.0, 10.0)]
    }

    pub fn flags(&mut self) -> [(&'static str, &mut bool); 5] {
        [("snap", &mut self.snap), ("alwayslifthead", &mut self.lift_head), ("scrabbleonside", &mut self.scrabble),
         ("writheonground", &mut self.writhe), ("rollonground", &mut self.roll)]
    }

    pub fn set(&mut self, name: &str, value: &str) -> bool {
        if let Some((_, f)) = self.flags().into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            return match value {
                "1" | "true" => { *f = true; true }
                "0" | "false" => { *f = false; true }
                _ => false,
            };
        }
        match (self.fields().into_iter().find(|(n, ..)| n.eq_ignore_ascii_case(name)), value.parse::<f32>()) {
            (Some((_, f, lo, hi)), Ok(v)) => { *f = v.clamp(lo, hi); true }
            _ => false,
        }
    }
}

const WALK_LEAN: f32 = 0.0;

pub struct Horse {
    pub build: Build,
    pub doll: Ragdoll,
    knee_lock: bool,
    knee_free: Vec<(usize, Limit)>,
    pub rest_shift: Vec<Mat4>,
    waddle: f32,
    obj_free: Vec<(Quat, Limit)>,
    obj_tilt: f32,
    obj_amp: f32,
    obj_way: f32,
    pub animal: usize,
    pub body: u8,
    pub leg_cap: f32,
    pub wide: f32,
    targets: Vec<Quat>,
    muscles: Vec<(f32, f32)>,
    pub health: f32,
    pub legs: [f32; 4],
    pub dead: bool,
    sunk: f32,
    kept: delirium::ragdoll::Kept,
    stand: Vec3,
    fwd: Vec3,
    ups: Vec<Vec3>,
    lefts: Vec<Vec3>,
    pub q: Quadruped,
    pub fell: bool,
    since_hit: f32,
    pub down: f32,
    pub(crate) pose: Vec<Transform>,
    pub(crate) rest: Option<Vec<Transform>>,
    held: bool,
    pushes: Vec<(usize, Vec3, Vec3)>,
    pub rdr2: bool,
    pub hold: bool,
    calm: f32,
    stand0: Vec<Transform>,
    side: Vec3,
    rise: Option<Rise>,
    ours: Option<ours::Ours>,
    ours_down: bool,
    anchor: [Option<Vec3>; 4],
    pub walk: Vec3,
    hyb_h: Option<f32>,
    lain: f32,
    pub r: rdr2::Params,
    core: Option<Box<rdr2::Core>>,
    rdr2_hits: Vec<(usize, Vec3, Vec3)>,
    pub rider: Option<(Vec3, Vec3, f32)>,
    pub task_asked: bool,
    centres: Vec<Vec3>,
    rig_ms: Vec<f32>,
    rig_caps: Vec<[f32; 2]>,
    spine_axis: Vec3,
    spine_cg: Vec3,
}

const RAGE_MARGIN: f32 = 0.03;
const INSIDE: f32 = 0.04;
const HINGE_LEAN: f32 = 0.1;
const HINGE_TWIST: f32 = 0.5;

fn in_limits(parts: &mut [PartDef], frames: &[Transform], floor: f32) -> Vec<Transform> {
    let mut out = frames.to_vec();
    for k in 0..parts.len() {
        let Some(p) = parts[k].parent else { continue };
        let turn = match parts[k].limit {
            Limit::Bone { pf, cf, lean, twist } => {
                let rel = (frames[p].rotation * pf).inverse() * (frames[k].rotation * cf);
                let rel = if rel.w < 0.0 { -rel } else { rel };
                let tw = Quat::from_xyzw(0.0, 0.0, rel.z, rel.w).normalize();
                let sw = (rel * tw.inverse()).to_scaled_axis();
                let (a, b) = ((lean[0] - INSIDE).max(1e-3), (lean[1] - INSIDE).max(1e-3));
                let e = ((sw.y / a).powi(2) + (sw.x / b).powi(2)).sqrt().max(1.0);
                let t = (2.0 * tw.z.atan2(tw.w)).clamp(-(twist - INSIDE).max(0.0), (twist - INSIDE).max(0.0));
                let rel = Quat::from_scaled_axis(sw / e) * Quat::from_rotation_z(t);
                out[p].rotation * pf * rel * cf.inverse()
            }
            Limit::BoneHinge { pf, cf, hi, .. } => {
                let rel = (frames[p].rotation * pf).inverse() * (frames[k].rotation * cf);
                let rel = if rel.w < 0.0 { -rel } else { rel };
                let tw = Quat::from_xyzw(0.0, 0.0, rel.z, rel.w).normalize();
                let t = (2.0 * tw.z.atan2(tw.w)).clamp(-(hi - INSIDE).max(0.0), (hi - INSIDE).max(0.0));
                out[p].rotation * pf * Quat::from_rotation_z(t) * cf.inverse()
            }
            _ => out[p].rotation * frames[p].rotation.inverse() * frames[k].rotation,
        };
        let at = out[p].translation + out[p].rotation * parts[k].anchor_parent - turn * parts[k].anchor;
        out[k] = Transform::from_translation(at).with_rotation(turn);
    }
    let low = out.iter().zip(parts.iter()).map(|(f, p)| f.translation.z - (f.rotation * Vec3::Y).z.abs() * p.half - p.radius).fold(f32::MAX, f32::min);
    for (f, p) in out.iter_mut().zip(parts.iter_mut()) {
        f.translation.z += floor + 0.02 - low;
        p.bind = f.rotation;
    }
    out
}

fn spread(torque: &mut [Vec3], t: Vec3) {
    for &k in &BARREL {
        torque[k] += t / BARREL.len() as f32;
    }
}

fn field(block: &str, tag: &str) -> Option<f32> {
    let a = block.find(&format!("<{tag}>"))? + tag.len() + 2;
    block[a..a + block[a..].find('<')?].trim().parse().ok()
}

struct XmlJoint {
    child: usize,
    parent: usize,
    limits: [(f32, f32); 3],
    muscle: [f32; 3],
    caps: [f32; 2],
}

fn read_xml(text: &str) -> Result<Vec<XmlJoint>, String> {
    if !text.contains("<bodyType>quadruped</bodyType>") {
        return Err("not RDR2's horse.xml (not a quadruped's body)".into());
    }
    text.split("<threedof ").skip(1).map(|b| {
        let b = &b[..b.find("</threedof>").unwrap_or(b.len())];
        let n = |t: &str| field(b, t).ok_or_else(|| format!("a joint without its {t}"));
        Ok(XmlJoint {
            child: n("jointNumber")? as usize + 1, parent: n("parentIndex")? as usize,
            limits: [(n("minfirstleanangle")?, n("maxfirstleanangle")?), (n("minsecondleanangle")?, n("maxsecondleanangle")?),
                     (n("mintwistangle")?, n("maxtwistangle")?)],
            muscle: [n("defaultMuscleStiffness")?, n("defaultMuscleStrength")?, n("defaultMuscleDamping")?],
            caps: [n("defaultLeanForceCap")?, n("defaultTwistForceCap")?],
        })
    }).collect()
}

fn room(v: Vec3) -> Vec3 {
    Vec3::new(-v.y, v.x, v.z)
}

fn room_turn(m: Mat3) -> Quat {
    Quat::from_mat3(&Mat3::from_cols(room(m.x_axis), room(m.y_axis), room(m.z_axis))).normalize()
}

impl Horse {
    pub fn new(xml: &str, fr: rdr2frag::Frag, at: Vec3, facing: f32) -> Result<Horse, String> {
        let joints = read_xml(xml)?;
        let mut part_of = vec![];
        for l in [1, 0].into_iter().chain(2..fr.links.len()) {
            for c in 0..fr.links.get(l).map_or(0, |k| k.capsules.len()) {
                part_of.push((l, c));
            }
        }
        if part_of.len() != PARTS.len() || part_of.iter().zip(&PARTS).any(|(&(l, _), (name, _))| fr.links[l].name != *name) {
            return Err("not RDR2's horse fragment (meta_base_horse_01.yft): its links aren't the horse's".into());
        }
        let first = |l: usize| part_of.iter().position(|&(k, c)| k == l && c == 0).unwrap_or(0);
        let low = fr.links.iter().flat_map(|l| &l.capsules).map(|c| c.at.z - c.turn.y_axis.z.abs() * (c.half - c.radius) - c.radius).fold(f32::MAX, f32::min);
        let turn = Quat::from_rotation_z(facing);
        let place = |p: Vec3| at + turn * (room(p) + Vec3::Z * (0.02 - low));
        let frames: Vec<Transform> = part_of.iter().map(|&(l, c)| {
            let cap = &fr.links[l].capsules[c];
            Transform::from_translation(place(cap.at)).with_rotation(turn * room_turn(cap.turn))
        }).collect();
        let mut muscles = vec![(0.0, 1.0); part_of.len()];
        let mut centres = vec![Vec3::ZERO; part_of.len()];
        let (mut rig_ms, mut rig_caps) = (vec![1.0; part_of.len()], vec![[3000.0; 2]; part_of.len()]);
        let mut parts = vec![];
        for (k, &(l, c)) in part_of.iter().enumerate() {
            let cap = &fr.links[l].capsules[c];
            let f = frames[k];
            let (parent, limit, joint) = if c > 0 {
                muscles[k] = (WELD, 1.0);
                (Some(first(l)), Limit::Cone { swing: WELD_PLAY, twist: WELD_PLAY }, f.translation)
            } else if let Some((j, flip)) = if l == ROOT_LINK { fr.joints.iter().find(|j| j.child == PELVIS_LINK && j.parent == ROOT_LINK).map(|j| (j, true)) }
                                             else if l == PELVIS_LINK { None } else { fr.joints.iter().find(|j| j.child == l).map(|j| (j, false)) } {
                let x = joints.iter().find(|x| x.child == j.child).ok_or_else(|| format!("horse.xml has no joint for {}", fr.links[j.child].name))?;
                if x.parent != j.parent {
                    return Err(format!("horse.xml and the fragment disagree on what {} hangs from", fr.links[j.child].name));
                }
                let [_, strength, damping] = x.muscle;
                let s = strength.max(0.0).sqrt();
                muscles[k] = (s, if s > 0.0 { damping / (2.0 * s) } else { 1.0 });
                let half = |(lo, hi): (f32, f32)| (hi - lo) * 0.5;
                let mid = |(lo, hi): (f32, f32)| (hi + lo) * 0.5;
                centres[k] = Vec3::new(mid(x.limits[0]), mid(x.limits[1]), mid(x.limits[2]));
                (rig_ms[k], rig_caps[k]) = (x.muscle[0], x.caps);
                let theirs = Vec3::new(half(x.limits[0]), half(x.limits[1]), half(x.limits[2])) - Vec3::splat(RAGE_MARGIN);
                if (theirs.max(Vec3::ZERO) - j.half).abs().max_element() > 0.01 {
                    return Err(format!("horse.xml and the fragment disagree on {}'s limits: not the same horse", fr.links[j.child].name));
                }
                let qq = |x: Mat3| Quat::from_mat3(&x).normalize();
                let (in_parent, in_child) = if flip { (j.in_child, j.in_parent) } else { (j.in_parent, j.in_child) };
                let nm = &fr.links[j.child].name;
                let wide = if nm.ends_with("thigh") || nm.ends_with("upperarm") { std::env::var("HORSE_LEAN").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0) } else { 1.0 };
                let limit = Limit::Bone { pf: qq(in_parent), cf: qq(in_child),
                                          lean: [half(x.limits[0]), half(x.limits[1])].map(|l| l * wide), twist: half(x.limits[2]) };
                let limit = match limit {
                    Limit::Bone { pf, cf, lean, twist } if lean[0] < HINGE_LEAN && lean[1] < HINGE_LEAN && twist > HINGE_TWIST => Limit::BoneHinge { pf, cf, lo: -twist, hi: twist },
                    l => l,
                };
                let (up, at_up) = if flip { (j.child, j.at_child) } else { (j.parent, j.at_parent) };
                let (pturn, pat) = fr.links[up].frame();
                (Some(first(up)), limit, place(pat + pturn * at_up))
            } else {
                (None, Limit::Locked, f.translation)
            };
            let (rest, anchor_parent) = parent.map_or((Quat::IDENTITY, Vec3::ZERO), |q| {
                let pf = frames[q];
                (f.rotation.inverse() * pf.rotation, pf.rotation.inverse() * (joint - pf.translation))
            });
            parts.push(PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: k, parent, mass: cap.mass, radius: cap.radius,
                                 half: (cap.half - cap.radius).max(0.0), offset: Transform::IDENTITY, rest, anchor_parent,
                                 anchor: f.rotation.inverse() * (joint - f.translation), limit, lower: group(k) == 1, reach: false,
                                 name: PARTS[k].1.to_string(), bind: f.rotation, boxed: None, inertia: cap.inertia, length: 0.0 });
        }
        let raw = frames.clone();
        let frames = in_limits(&mut parts, &frames, at.z);
        let build = Build { parts, nm: delirium::nmrs::HOMER, links: vec![] };
        let targets = (0..part_of.len()).map(|k| build.parts[k].parent.map_or(Quat::IDENTITY, |p| frames[p].rotation.inverse() * frames[k].rotation)).collect();
        let mut doll = Ragdoll::new(&build, &frames, Vec3::ZERO);
        let mut pairs = vec![];
        for &(a, b) in &fr.pairs {
            for (i, _) in part_of.iter().enumerate().filter(|(_, p)| p.0 == a) {
                for (j, _) in part_of.iter().enumerate().filter(|(_, p)| p.0 == b) {
                    pairs.push((i.min(j), i.max(j)));
                }
            }
        }
        pairs.sort();
        pairs.dedup();
        doll.pairs = pairs;
        let mid = frames[MID].rotation;
        let stand = mid.inverse() * Vec3::Z;
        let fwd = mid.inverse() * (frames[SHOULDERS].translation - frames[PELVIS].translation).normalize_or(turn * Vec3::X);
        let spine_axis = if (mid * Vec3::Y).z < 0.0 { Vec3::NEG_Y } else { Vec3::Y };
        let spine_cg = fr.links[part_of[MID].0].capsules[part_of[MID].1].cg;
        let ups = frames.iter().map(|f| f.rotation.inverse() * Vec3::Z).collect();
        let (un, o) = (turn.inverse(), frames[PELVIS].translation.with_z(0.0));
        let stand0: Vec<Transform> = frames.iter().map(|f| Transform::from_translation(un * (f.translation - o)).with_rotation(un * f.rotation)).collect();
        let side = Vec3::Z.cross((stand0[SHOULDERS].translation - stand0[PELVIS].translation).with_z(0.0)).normalize_or(Vec3::Y);
        let lefts = frames.iter().map(|f| f.rotation.inverse() * (turn * Vec3::Y)).collect();
        let rest_shift = frames.iter().zip(&raw).map(|(f, r)| f.to_matrix().inverse() * r.to_matrix()).collect();
        Ok(Horse { build, doll, knee_lock: false, knee_free: vec![], rest_shift, waddle: 0.0, obj_free: vec![], obj_tilt: f32::NAN, obj_amp: 0.0, obj_way: 1.0, animal: 0, body: 2, targets, muscles, health: HEALTH, legs: [0.0; 4], dead: false, sunk: 0.0, kept: Default::default(),
                   stand, fwd, ups, lefts, q: Quadruped::default(), fell: false, since_hit: f32::MAX, down: 0.0,
                   pose: frames.clone(), rest: None, held: true, pushes: vec![], rdr2: false, hold: !live(), calm: 0.0, stand0, side, rise: None, ours: None, ours_down: false, leg_cap: 1.0, wide: 1.0, anchor: [None; 4], walk: Vec3::ZERO, hyb_h: None, lain: 0.0, r: Default::default(), core: None, rdr2_hits: vec![], rider: None, task_asked: false,
                   centres, rig_ms, rig_caps, spine_axis, spine_cg })
    }

    pub fn step(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        self.think(world, dt, obstacles, others);
        let drive: Drive = self.kept.drive();
        self.doll.step(&self.build, world, dt, drive, obstacles, others);
    }

    fn lock_knees(&mut self, on: bool) {
        if self.knee_free.is_empty() {
            self.knee_free = LEGS.iter().flat_map(|l| l[1..].iter().map(|&k| (k, self.build.parts[k].limit))).collect();
        }
        for &(k, l) in &self.knee_free {
            self.build.parts[k].limit = if on { Limit::Cone { swing: WELD_PLAY, twist: WELD_PLAY } } else { l };
        }
        self.remake();
        self.knee_lock = on;
    }

    fn remake(&mut self) {
        let frames: Vec<Transform> = self.doll.frames().collect();
        let pairs = std::mem::take(&mut self.doll.pairs);
        self.doll = Ragdoll::new(&self.build, &frames, Vec3::ZERO);
        self.doll.pairs = pairs;
    }

    pub fn member<'a>(&'a mut self, id: usize, obstacles: &'a [Obstacle]) -> delirium::ragdoll::Member<'a> {
        delirium::ragdoll::Member { doll: &mut self.doll, id, build: &self.build, drive: self.kept.drive(), obstacles, others: &[] }
    }

    pub fn think(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        self.think_brain(world, dt, obstacles, others);
        let (stiff, lock) = if crate::animal_fit::object(self.animal) { (0.0, self.knee_lock) } else { crate::animal_fit::legs(self.animal) };
        if lock != self.knee_lock {
            self.lock_knees(lock);
        }
        let s = if lock { 1.0 } else { stiff };
        if s > 0.0 {
            for &k in LEGS.iter().flat_map(|l| &l[1..]) {
                if let Some(m) = self.kept.muscles.get_mut(k) {
                    m.target = m.target.slerp(self.targets[k], s);
                    m.stiffness += (WELD - m.stiffness) * s;
                }
            }
        }
        if std::env::var_os("HORSE_NONET").is_some() && self.kept.torque.len() > MID {
            let t: Vec3 = self.kept.torque.iter().sum();
            self.kept.torque[MID] -= t;
        }
        self.grip_hooves();
        if std::env::var_os("HORSE_HOOFLOG").is_some() {
            let s: Vec<String> = LEGS.iter().map(|l| { let k = l[2]; let (v, w) = self.doll.vel(k);
                { let p = self.doll.frames().nth(k).map_or(Vec3::ZERO, |f| f.translation); format!("{} v{:.0} w{:.1} p {:.4} {:.4} {:.3}", (self.doll.touch[k] & 0x20 != 0) as u8, v.truncate().length() * 100.0, w.length(), p.x, p.y, p.z) } }).collect();
            let (f, t) = (self.kept.force.iter().sum::<Vec3>(), self.kept.torque.iter().sum::<Vec3>());
            let (_, v) = self.doll.com();
            eprintln!("hoof {} || ext f {:.1} {:.1} {:.1} t {:.1} {:.1} {:.1} v {:.3} {:.3}", s.join(" | "), f.x, f.y, f.z, t.x, t.y, t.z, v.x, v.y);
        }
        if self.walk != Vec3::ZERO && !self.dead && self.kept.force.len() > MID && !crate::animal_fit::object(self.animal) {
            let lean = std::env::var("HORSE_WALKLEAN").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(WALK_LEAN);
            self.kept.force[MID] += self.walk.with_z(0.0) * self.doll.mass() * delirium::ragdoll::GRAVITY() * lean;
        }
    }

    fn grip_hooves(&mut self) {
        let n = self.build.parts.len();
        self.doll.anchor_v.resize(n, Vec3::ZERO);
        let off = self.dead || std::env::var("HORSE_ANCHOR").is_ok_and(|v| v == "0");
        let env = |n: &str, d: f32| std::env::var(n).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let (at, slip) = (env("HORSE_ANCHOR_T", ANCHOR_T), env("HORSE_ANCHOR_SLIP", ANCHOR_SLIP));
        for (i, l) in LEGS.iter().enumerate() {
            let k = l[2];
            let p = self.doll.frames().nth(k).map_or(Vec3::ZERO, |f| f.translation);
            let planted = self.doll.touch[k] & 0x20 != 0 && self.doll.hit[k].is_some_and(|h| h.1.z > 0.5) && !self.core_swinging(i);
            self.doll.anchor_v[k] = Vec3::ZERO;
            if off || !planted { self.anchor[i] = None; continue; }
            let a = *self.anchor[i].get_or_insert(p);
            let d = (a - p).with_z(0.0);
            if d.length() > slip {
                self.anchor[i] = Some(p);
            } else {
                self.doll.anchor_v[k] = d / at;
            }
        }
    }

    fn think_brain(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        if crate::animal_fit::object(self.animal) {
            return self.step_object(dt);
        }
        if !self.obj_free.is_empty() {
            self.end_object();
            self.kept.rigid.clear();
        }
        if self.ours_down && self.hold { self.ours_down = false; }
        if self.rdr2 && ours::on() && !self.dead && !self.ours_down && self.rise.is_none() && !self.down_rdr2() {
            if self.step_ours(dt) { return; }
            (self.ours, self.ours_down) = (None, true);
        }
        if self.rdr2 && self.hold && self.rise.is_none() && live() {
            (self.hold, self.calm) = (false, 0.0);
        }
        if !self.hold || self.fell {
            self.rise = None;
        }
        if (self.rdr2 || self.rise.is_some()) && self.hold && !self.dead {
            if let Some(mut r) = self.rise.take() {
                r.t += dt;
                let (pose, done) = self.rise_pose(&r);
                self.pose = pose;
                if !done {
                    self.rise = Some(r);
                }
            }
            for (k, &p) in self.pose.iter().enumerate() {
                self.doll.pin(k, p);
            }
            return;
        }
        if self.rdr2 {
            self.step_rdr2(world, dt, obstacles, others);
            let ((v, w), up) = (self.doll.vel(MID), self.steady_rdr2());
            self.calm = if up && v.length() < 0.15 && w.length() < 0.3 { self.calm + dt } else { 0.0 };
            if self.calm > CALM && !live() {
                self.rest_rdr2();
            }
            self.lain = if self.down_rdr2() && v.length() < 0.4 && w.length() < 1.0 { self.lain + dt } else { 0.0 };
            if self.lain > LAIN && !self.dead {
                self.lain = 0.0;
                self.begin_rise();
            }
            return;
        }
        if self.core.is_some() {
            self.stop_rdr2();
        }
        let (v, w) = self.doll.vel(MID);
        self.lain = if !self.held && v.length() < 0.4 && w.length() < 1.0 { self.lain + dt } else { 0.0 };
        if self.lain > LAIN && !self.dead && self.legs.iter().all(|&l| l < LAME) {
            self.lain = 0.0;
            self.begin_rise();
            return;
        }
        let (legs, dead) = (self.legs, self.dead);
        let level = self.stand;
        let fr: Vec<Transform> = self.doll.frames().collect();
        let back = fr[MID].rotation * level;
        if self.sunk == 0.0 && back.z < self.q.fail_angle {
            self.fell = true;
        }
        if dead || self.fell || legs.iter().any(|&l| l >= LAME) {
            self.sunk += dt;
        }
        let up = self.sunk <= GIVE;
        if !up && !dead {
            self.down += dt;
        }
        self.since_hit += dt;
        let q = &self.q;
        let weak = ((self.since_hit - q.weak_zero) / q.weak_ramp.max(1e-3)).clamp(0.0, 1.0);
        let strength = (-q.strength_decay * self.down).exp();
        let n = self.build.parts.len();
        let (mut force, mut torque) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);
        if up {
            for (k, &p) in self.pose.iter().enumerate() {
                self.doll.pin(k, p);
            }
        } else if self.held {
            self.held = false;
            self.doll.unpin();
            for (k, at, imp) in std::mem::take(&mut self.pushes) {
                self.doll.impulse(k, at, imp);
            }
        }
        let mut targets = self.targets.clone();
        let ground = !up && !dead;
        if ground {
            let m = self.doll.mass();
            let (_, w) = self.doll.vel(MID);
            let fwd = fr[MID].rotation * self.fwd;
            let toward = back.cross(Vec3::Z).dot(fwd);
            if q.lift_head {
                let parent = fr[SHOULDERS].rotation;
                let posed = parent * self.targets[NECK1] * Vec3::Y;
                let axis = parent.inverse() * posed.cross(Vec3::Z);
                if let Some(axis) = axis.try_normalize() {
                    let a = 0.7 * (1.0 - posed.z.max(0.0));
                    targets[NECK1] = Quat::from_axis_angle(axis, a) * targets[NECK1];
                }
                if fr[HEAD].translation.z < fr[MID].translation.z + 0.6 {
                    let neck: f32 = NECK_HEAD.iter().map(|&k| self.doll.part_mass(k)).sum();
                    force[HEAD] += Vec3::Z * neck * GRAVITY() * HEAD_LIFT * (0.6 + 0.4 * strength) * weak;
                }
            }
            if q.righting > 0.0 {
                spread(&mut torque, fwd * toward * q.righting * m * GRAVITY() * 0.1);
            }
            if q.roll && back.z < 0.7 {
                let way = toward.signum();
                let want = q.roll_velocity * (self.down * std::f32::consts::PI).sin().max(0.0);
                spread(&mut torque, fwd * way * (want - way * w.dot(fwd)).max(0.0) * q.roll_scale * m * GRAVITY() * 0.6 * weak);
            }
            let t = self.down;
            if q.scrabble && back.z.abs() < 0.6 {
                let speed = q.scrabble_min + (q.scrabble_max - q.scrabble_min) * strength;
                let reach = 0.25 + 0.45 * strength;
                for (l, leg) in LEGS.iter().enumerate() {
                    let phase = t * speed + [0.0, std::f32::consts::PI, std::f32::consts::PI, 0.0][l];
                    let parent = self.build.parts[leg[0]].parent.unwrap_or(MID);
                    targets[leg[0]] = Quat::from_axis_angle(self.lefts[parent], reach * phase.sin()) * self.targets[leg[0]];
                    let bend = if l < 2 { -1.0 } else { 1.0 };
                    targets[leg[1]] = Quat::from_axis_angle(self.lefts[leg[0]], bend * reach * 0.8 * (0.5 + 0.5 * phase.cos())) * self.targets[leg[1]];
                }
            }
            if q.writhe {
                let phase = t * q.writhe_speed * std::f32::consts::TAU;
                let (left, sky) = (self.lefts[SHOULDERS], self.ups[SHOULDERS]);
                let side = sky.cross(left).normalize_or(Vec3::Z);
                let (a, b) = (0.25 * strength.max(0.3), 0.2 * strength.max(0.3));
                targets[NECK1] = Quat::from_axis_angle(left, a * phase.sin()) * Quat::from_axis_angle(side, b * (phase * 0.7).sin()) * targets[NECK1];
            }
        }
        self.kept.force = force;
        self.kept.torque = torque;
        self.kept.muscles = (0..n).map(|k| {
            let g = group(k);
            let leg = LEGS.iter().position(|l| l.contains(&k));
            let (own, ratio) = self.muscles[k];
            let mut w = if WELDED.contains(&k) {
                WELD
            } else if up {
                own
            } else if ground {
                let fade = |fresh: f32, min: f32| min + (fresh - min) * strength;
                match g {
                    1 => fade(q.leg_stiffness, q.leg_stiffness_min),
                    0 | 2 => fade(q.spine_stiffness, q.spine_stiffness_min),
                    _ => own,
                }
            } else {
                0.0
            };
            if !WELDED.contains(&k) {
                w *= weak.max(0.05);
            }
            if let Some(leg) = leg {
                w *= (1.0 - legs[leg] / LAME).clamp(0.08, 1.0);
            }
            Muscle { target: targets[k], stiffness: w, damping: ratio }
        }).collect();
    }

    fn bent(&self, bend: &[f32], pitch: f32, yaw: f32, xy: Vec2, ground: f32) -> Vec<Transform> {
        let n = self.build.parts.len();
        let mut out: Vec<Option<Mat4>> = vec![None; n];
        let m0: Vec<Mat4> = self.stand0.iter().map(|t| t.to_matrix()).collect();
        while out.iter().any(|o| o.is_none()) {
            for k in 0..n {
                if out[k].is_some() {
                    continue;
                }
                out[k] = match self.build.parts[k].parent {
                    None => {
                        let c = self.stand0[k].translation;
                        Some(Mat4::from_translation(c) * Mat4::from_quat(Quat::from_axis_angle(self.side, pitch)) * Mat4::from_translation(-c) * m0[k])
                    }
                    Some(p) => out[p].map(|pm| {
                        let axis = pm.to_scale_rotation_translation().1.inverse() * self.side;
                        let pv = self.build.parts[k].anchor_parent;
                        let rel = m0[p].inverse() * m0[k];
                        pm * Mat4::from_translation(pv) * Mat4::from_quat(Quat::from_axis_angle(axis, bend[k])) * Mat4::from_translation(-pv) * rel
                    }),
                };
            }
        }
        let place = Mat4::from_translation(xy.extend(0.0)) * Mat4::from_rotation_z(yaw);
        let mut f: Vec<Transform> = out.into_iter().map(|m| Transform::from_matrix(place * m.unwrap())).collect();
        let low = f.iter().zip(&self.build.parts).map(|(t, p)| t.translation.z - (t.rotation * Vec3::Y * p.half).z.abs() - p.radius).fold(f32::MAX, f32::min);
        for t in &mut f {
            t.translation.z += ground - low;
        }
        f
    }

    fn rise_key(&self, i: usize) -> (Vec<f32>, f32) {
        let parts = &self.build.parts;
        let bend = parts.iter().map(|p| {
            if p.parent.is_some_and(|q| parts[q].name == p.name) {
                return 0.0;
            }
            let n = p.name.to_lowercase();
            let front = if i == 0 { [("upperarm", 0.5), ("forearm", 2.2), ("hand", -1.3)] } else { [("upperarm", 0.3), ("forearm", 0.0), ("hand", 0.0)] };
            let rest = [("thigh", -0.4), ("calf", 2.4), ("foot", -2.0), ("neck1", -0.25), ("head", 0.4)];
            front.iter().chain(&rest).find(|(k, _)| n.contains(k)).map_or(0.0, |b| b.1)
        }).collect();
        (bend, if i == 0 { 0.0 } else { -0.3 })
    }

    pub fn begin_rise(&mut self) {
        let fr: Vec<Transform> = self.doll.frames().collect();
        let ground = self.capsules().iter().map(|c| c.0.z.min(c.1.z) - c.2).fold(f32::MAX, f32::min);
        let fwd = (fr[SHOULDERS].translation - fr[PELVIS].translation).with_z(0.0);
        let canon = (self.stand0[SHOULDERS].translation - self.stand0[PELVIS].translation).with_z(0.0);
        let yaw = fwd.y.atan2(fwd.x) - canon.y.atan2(canon.x);
        self.rest_rdr2();
        (self.fell, self.sunk, self.down) = (false, 0.0, 0.0);
        self.rise = Some(Rise { t: 0.0, xy: fr[PELVIS].translation.truncate(), from: fr, yaw, ground });
    }

    fn rise_pose(&self, r: &Rise) -> (Vec<Transform>, bool) {
        let ease = |x: f32| { let x = x.clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) };
        let at = |b: &[f32], p: f32| self.bent(b, p, r.yaw, r.xy, r.ground);
        let ((b0, p0), (b1, p1)) = (self.rise_key(0), self.rise_key(1));
        let t = r.t;
        if t < RISE[0] {
            let (a, k) = (at(&b0, p0), ease(t / RISE[0]));
            let pose = r.from.iter().zip(&a).map(|(f, a)| Transform::from_translation(f.translation.lerp(a.translation, k)).with_rotation(f.rotation.slerp(a.rotation, k))).collect();
            return (pose, false);
        }
        let mix = |ba: &[f32], pa: f32, bb: &[f32], pb: f32, k: f32| -> (Vec<f32>, f32) { (ba.iter().zip(bb).map(|(a, b)| a + (b - a) * k).collect(), pa + (pb - pa) * k) };
        let (b, p) = if t < RISE[0] + RISE[1] {
            mix(&b0, p0, &b1, p1, ease((t - RISE[0]) / RISE[1]))
        } else {
            let zero = vec![0.0; b1.len()];
            mix(&b1, p1, &zero, 0.0, ease((t - RISE[0] - RISE[1]) / RISE[2]))
        };
        (at(&b, p), t >= RISE.iter().sum::<f32>())
    }

    pub fn let_go(&mut self) {
        if self.rdr2 {
            (self.hold, self.calm) = (false, 0.0);
        } else if self.held {
            self.fell = true;
            self.sunk = self.sunk.max(GIVE + 1e-3);
        }
    }

    pub fn pull(&mut self, part: usize, at: Vec3, impulse: Vec3) {
        self.let_go();
        if !self.held {
            self.doll.impulse(part, at, impulse);
        } else {
            self.pushes.push((part, at, impulse));
        }
    }

    pub fn poke(&mut self, part: usize, at: Vec3, impulse: Vec3) {
        if self.rdr2 {
            self.hold = false;
        }
        if self.held {
            self.pushes.push((part, at, impulse));
        } else {
            self.doll.impulse(part, at, impulse);
        }
    }

    pub fn standing(&self) -> bool {
        if self.rdr2 {
            return self.standing_rdr2();
        }
        self.sunk <= GIVE
    }

    pub fn set(&mut self, name: &str, value: &str) -> bool {
        let a = self.q.set(name, value);
        let b = self.r.set(name, value);
        a || b
    }

    pub fn wants_task(&mut self) -> bool {
        !std::mem::replace(&mut self.task_asked, true)
    }

    pub fn task(&mut self, messages: &[crate::gametask::SentMessage]) -> Vec<String> {
        use crate::gametask::{joaat, Val};
        let num = |m: &crate::gametask::SentMessage, h: u32| m.params.iter().find(|p| p.hash == h).and_then(|p| match p.val { Val::Float(f) => Some(f), Val::Int(i) => Some(i as f32), _ => None });
        let vect = |m: &crate::gametask::SentMessage, h: u32| m.params.iter().find(|p| p.hash == h).and_then(|p| match p.val { Val::Vec3(v) => Some(Vec3::from_array(v)), _ => None });
        let flag = |m: &crate::gametask::SentMessage, h: u32| m.params.iter().find(|p| p.hash == h).and_then(|p| match p.val { Val::Bool(b) => Some(b), _ => None });
        let mut said = vec![];
        for m in messages {
            if m.hash == joaat("quadrupedbase") {
                if flag(m, joaat("start")) == Some(true) {
                    let (limits, stay, oppose) = (self.r.limits, self.r.stay, self.r.oppose);
                    self.r = Default::default();
                    (self.r.limits, self.r.stay, self.r.oppose) = (limits, stay, oppose);
                }
                let list: Vec<(u32, Val)> = m.params.iter().map(|p| (p.hash, p.val.clone())).collect();
                let left = self.r.message(&list);
                self.r.env();
                if std::env::var_os("RDR2_TRACE").is_some() {
                    eprintln!("quadruped 0x2a6bcd8: the handler: {} values, {} left", m.params.len(), left.len());
                }
                said.push(format!("quadrupedbase: {} values{}", m.params.len(), if left.is_empty() { String::new() } else { format!(", not the behaviour's: {left:x?}") }));
            } else if m.hash == 0xe35a5d1a {
                let Some(limb) = num(m, 0x4e70182d) else { continue };
                let l = rdr2::LimbLimit { kind: num(m, 0x3165b05d).unwrap_or(0.0) as i32, dir: num(m, 0x34df9828).unwrap_or(-1.0),
                                          min: num(m, 0xfe30e6f3).unwrap_or(-std::f32::consts::PI), max: num(m, 0x606068c4).unwrap_or(std::f32::consts::PI),
                                          a38: num(m, 0x7fb46a89).unwrap_or(0.0), a3c: num(m, 0xd2e95bdb).unwrap_or(0.0) };
                self.limb_message(limb as usize, l);
                said.push(format!("quadLimbLimit: limb {limb} type {} [{}, {}]", l.kind, l.min, l.max));
            } else if m.hash == joaat("setOpposeGravity") {
                self.r.oppose = num(m, joaat("oppose")).or(num(m, joaat("gravityCompensation"))).unwrap_or(1.0);
                said.push(format!("setOpposeGravity: {}", self.r.oppose));
            } else if m.hash == joaat("stayUpright") {
                if flag(m, joaat("useTorques")) == Some(true) {
                    self.r.stay = Some(rdr2::Stay { strength: vect(m, joaat("torqueStrength")).unwrap_or(Vec3::ZERO), damping: vect(m, joaat("torqueDamping")).unwrap_or(Vec3::ZERO),
                                                    parts: flag(m, joaat("torqueparts")).unwrap_or(false), reduction: num(m, joaat("torqueleanreduction")).unwrap_or(0.0),
                                                    lean_min: num(m, joaat("torqueleanmin")).unwrap_or(0.0), lean_max: num(m, joaat("torqueleanmax")).unwrap_or(0.0) });
                    said.push("stayUpright: torques".into());
                }
            } else {
                said.push(format!("{}: not the horse's", m.label));
            }
        }
        said
    }

    pub fn saddle(&self) -> (Vec3, Vec3, Vec3, [Vec3; 2]) {
        let f = self.doll.frames().nth(MID).unwrap_or_default();
        let (up, fwd) = (f.rotation * self.stand, f.rotation * self.fwd);
        let left = up.cross(fwd).normalize_or(Vec3::Y);
        let top = self.build.parts[MID].half + self.build.parts[MID].radius;
        let seat = f.translation + up * (top + SEAT);
        let stirrup = |side: f32| seat - up * 0.62 + left * (0.42 * side) + fwd * 0.15;
        (seat, up, fwd, [stirrup(1.0), stirrup(-1.0)])
    }

    pub fn state(&self) -> String {
        if crate::animal_fit::object(self.animal) {
            let up = self.doll.frames().nth(MID).map_or(1.0, |f| (f.rotation * self.stand).z);
            return format!("Object (furniture): {}", if self.dead { "dead" } else if up < 0.5 { "tipped over" } else if self.waddle > 0.0 { "waddling" } else { "standing" });
        }
        if self.rdr2 && !self.dead {
            return self.state_rdr2();
        }
        if self.dead {
            "dead".into()
        } else if self.sunk > GIVE {
            let (fresh, min) = (self.q.leg_stiffness, self.q.leg_stiffness_min);
            format!("down {:.0} s, legs at {:.0}% strength", self.down, (min + (fresh - min) * (-self.q.strength_decay * self.down).exp()) / fresh.max(1e-3) * 100.0)
        } else {
            "standing".into()
        }
    }

    pub fn errors(&self) -> Vec<f32> {
        let q: Vec<Quat> = self.doll.frames().map(|f| f.rotation).collect();
        (0..q.len()).map(|k| self.build.parts[k].parent.map_or(0.0, |p| (q[p] * self.targets[k]).angle_between(q[k]))).collect()
    }

    pub fn capsules(&self) -> Vec<Capsule> {
        self.doll.capsules().zip(&self.build.parts).enumerate().map(|(k, (t, p))| {
            let a = t.rotation * Vec3::Y * p.half;
            (t.translation - a, t.translation + a, p.radius, self.doll.motion(k).0, true)
        }).collect()
    }

    pub fn hit(&mut self, part: usize, at: Vec3, impulse: Vec3, damage: f32) -> String {
        if impulse != Vec3::ZERO {
            if self.rdr2 {
                self.hold = false;
            }
            if self.held {
                self.pushes.push((part, at, impulse));
            } else {
                self.doll.impulse(part, at, impulse);
            }
        }
        if self.dead {
            return format!("{}: hit, dead already", self.build.parts[part].name);
        }
        if damage > 0.0 && self.rdr2 {
            self.rdr2_hits.push((part, at, impulse));
        } else if damage > 0.0 {
            self.since_hit = 0.0;
            if self.q.snap && impulse != Vec3::ZERO {
                self.doll.impulse(0, at, impulse.normalize() * self.q.snap_mag * 60.0);
            }
        }
        self.health -= damage;
        let mut s = format!("{} hit: health {:.0}", self.build.parts[part].name, self.health.max(0.0));
        if let Some(leg) = LEGS.iter().position(|l| l.contains(&part)) {
            let was = self.legs[leg];
            self.legs[leg] = (self.legs[leg] + damage).min(LAME);
            if was < LAME && self.legs[leg] >= LAME {
                s += &format!(", its {} leg gives", ["hind right", "hind left", "fore right", "fore left"][leg]);
            }
        }
        if self.health <= 0.0 {
            self.dead = true;
            s += ", dead";
        }
        s
    }
}

#[derive(Component)]
pub struct HorsePart(pub u32, pub usize);

pub(crate) fn coat(animal: usize, k: usize, name: &str) -> Color {
    let c = |r: f32, g: f32, b: f32| Color::srgb(r, g, b);
    let foot = name.ends_with("Foot");
    let is = |n: &[&str]| n.iter().any(|x| name.starts_with(x));
    let lower = is(&["R_Calf", "L_Calf", "R_Forearm", "L_Forearm"]);
    if let Some(c) = crate::animal_fit::colour(animal) {
        return c;
    }
    match animal {
        1 if foot => c(0.25, 0.24, 0.23),
        1 if is(&["Spine1", "Spine3", "Neck3", "L_Thigh", "R_UpperArm", "Tail4"]) => c(0.08, 0.08, 0.08),
        1 => c(0.9, 0.88, 0.84),
        2 if foot => c(0.06, 0.06, 0.06),
        2 => if k % 2 == 0 { c(0.92, 0.92, 0.9) } else { c(0.06, 0.06, 0.06) },
        3 if foot => c(0.3, 0.22, 0.15),
        3 => c(0.86, 0.76, 0.6),
        4 if foot => c(0.62, 0.38, 0.38),
        4 => c(0.95, 0.66, 0.66),
        5 if foot || lower || is(&["Head"]) => c(0.1, 0.09, 0.09),
        5 => c(0.95, 0.93, 0.88),
        6 if foot => c(0.15, 0.12, 0.1),
        6 => c(0.85, 0.7, 0.5),
        _ if foot => c(0.12, 0.1, 0.09),
        _ => c(0.42, 0.27, 0.16),
    }
}

type Touch = (&'static str, i8, [f32; 3], u8, [f32; 3], [f32; 3]);
fn touches(animal: usize) -> Vec<Touch> {
    const PINK: [f32; 3] = [0.93, 0.55, 0.6];
    const BLACK: [f32; 3] = [0.06, 0.06, 0.06];
    const CREAM: [f32; 3] = [0.9, 0.85, 0.7];
    let mut t: Vec<Touch> = vec![];
    if animal >= crate::rdr2frag::ANIMALS.len() {
        return t;
    }
    let curl = |t: &mut Vec<Touch>, col: [f32; 3]| for i in 0..9 {
        let a = i as f32 * 0.8;
        t.push(("Pelvis", -1, [0.0, -1.0 - 0.06 * i as f32 - 0.25 * a.cos(), 0.5 + 0.25 * a.sin()], 0, [0.09, 0.0, 0.0], col));
    };
    let ears = |t: &mut Vec<Touch>, col: [f32; 3], out: f32, len: f32| for s in [-1.0, 1.0] {
        t.push(("Head", -1, [0.55 * s, 0.0, 0.7], 1, [0.22, len, 0.0], col));
        t.last_mut().unwrap().4[2] = out * s;
    };
    match animal {
        1 => {
            t.push(("Spine1", 0, [0.0, 0.0, -0.95], 0, [0.5, 0.0, 0.0], PINK));
            for (x, y) in [(-0.22, -0.2), (0.22, -0.2), (-0.22, 0.2), (0.22, 0.2)] {
                t.push(("Spine1", 0, [x, y, -1.4], 0, [0.12, 0.0, 0.0], PINK));
            }
            for s in [-1.0, 1.0] {
                t.push(("Head", -1, [0.6 * s, 0.0, 0.75], 1, [0.16, 0.9, 0.7 * s], CREAM));
            }
            t.push(("Head", 1, [0.0, 0.1, -0.1], 0, [0.62, 0.0, 0.0], PINK));
        }
        2 => for n in ["Neck1", "Neck3", "Neck5"] {
            t.push((n, 0, [0.0, 0.0, 0.9], 0, [0.42, 0.0, 0.0], BLACK));
        },
        3 => ears(&mut t, [0.86, 0.76, 0.6], 0.25, 1.3),
        4 => {
            curl(&mut t, [0.95, 0.62, 0.64]);
            t.push(("Head", 1, [0.0, 0.95, 0.3], 2, [0.42, 0.3, 0.0], [0.9, 0.5, 0.55]));
            for s in [-1.0, 1.0] {
                t.push(("Head", 1, [0.15 * s, 1.12, 0.3], 0, [0.08, 0.0, 0.0], [0.35, 0.15, 0.15]));
            }
            ears(&mut t, [0.95, 0.62, 0.64], 0.6, 0.8);
        }
        5 => {
            for n in ["Pelvis", "Root", "Spine1", "Spine2", "Spine3"] {
                for o in [[-0.55, 0.0, 0.35], [0.55, 0.0, 0.35], [0.0, 0.0, 0.75]] {
                    t.push((n, 0, o, 0, [0.6, 0.0, 0.0], [0.96, 0.94, 0.9]));
                }
            }
            ears(&mut t, BLACK, 1.4, 0.7);
        }
        6 => {
            t.push(("Head", 1, [0.0, 0.1, -0.1], 0, [0.65, 0.0, 0.0], BLACK));
            for s in [-1.0, 1.0] {
                t.push(("Head", 0, [0.85 * s, -0.2, 0.45], 0, [0.35, 0.0, 0.0], BLACK));
            }
            curl(&mut t, [0.85, 0.7, 0.5]);
        }
        _ => for n in ["Neck1", "Neck3", "Neck5"] {
            t.push((n, 0, [0.0, 0.0, 0.9], 0, [0.4, 0.0, 0.0], [0.1, 0.07, 0.05]));
        },
    }
    t
}

pub fn view(mut commands: Commands, sim: NonSend<crate::Sim>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
            mut parts: Query<(Entity, &HorsePart, &mut Transform)>, mut looks: Local<Vec<(Handle<StandardMaterial>, Color)>>,
            (mut images, mut binds): (ResMut<Assets<Image>>, ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>)) {
    let all: Vec<(u32, &Horse)> = sim.horse.iter().map(|h| (sim.horse_id, h)).chain(sim.herd.iter().map(|(i, h)| (*i, h))).collect();
    let mut have = std::collections::HashSet::new();
    for (e, HorsePart(id, _), _) in &parts {
        if all.iter().any(|a| a.0 == *id) {
            have.insert(*id);
        } else {
            commands.entity(e).despawn();
        }
    }
    if have.is_empty() {
        looks.clear();
    }
    let alpha = if sim.clean { 1.0 } else { sim.body_alpha };
    for (m, c) in looks.iter() {
        let mode = if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque };
        if let Some(mut m) = mats.get_mut(m).filter(|m| m.base_color.alpha() != alpha || m.alpha_mode != mode) {
            m.base_color = c.with_alpha(alpha);
            m.alpha_mode = if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque };
        }
    }
    for &(id, h) in all.iter().filter(|a| !have.contains(&a.0)) {
        let mut mat = |c: Color| {
            let m = mats.add(StandardMaterial { base_color: c.with_alpha(alpha), perceptual_roughness: 0.8,
                                                alpha_mode: if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque }, ..default() });
            looks.push((m.clone(), c));
            m
        };
        let caps: Vec<Transform> = h.doll.capsules().collect();
        let fr: Vec<Transform> = h.doll.frames().collect();
        let fwd = (fr[SHOULDERS].translation - fr[PELVIS].translation).with_z(0.0).normalize_or(Vec3::X);
        let side = fwd.cross(Vec3::Z);
        let touch = touches(h.animal);
        let layer = bevy::camera::visibility::RenderLayers::layer(1);
        if let Some(own) = crate::animal_fit::look(h.animal) {
            let joints: Vec<Entity> = caps.iter().enumerate().map(|(k, &t)| commands.spawn((t, HorsePart(id, k), Visibility::Inherited)).id()).collect();
            let bind = binds.add(bevy::mesh::skinning::SkinnedMeshInverseBindposes::from(
                own.bind.iter().zip(&h.rest_shift).map(|(b, s)| *s * *b).collect::<Vec<Mat4>>()));
            let mut tex = |t: &Option<crate::usd::Tex>, srgb: bool| -> Option<Handle<Image>> {
                let t = t.as_ref()?;
                let ext = std::path::Path::new(&t.name).extension().map_or("png".into(), |e| e.to_string_lossy().to_lowercase());
                let img = Image::from_buffer(t.bytes.as_ref()?, bevy::image::ImageType::Extension(&ext), bevy::image::CompressedImageFormats::NONE, srgb,
                                             bevy::image::ImageSampler::Default, bevy::asset::RenderAssetUsages::default()).ok()?;
                Some(images.add(img))
            };
            let made: Vec<Handle<StandardMaterial>> = own.mats.iter().map(|q| mats.add(StandardMaterial {
                base_color: Color::srgb(q.base[0], q.base[1], q.base[2]),
                base_color_texture: tex(&q.base_tex, true),
                normal_map_texture: tex(&q.normal_tex, false),
                metallic: q.metallic,
                perceptual_roughness: q.roughness,
                alpha_mode: if q.base[3] < 1.0 { AlphaMode::Mask(0.5) } else { AlphaMode::Opaque },
                double_sided: true,
                cull_mode: None,
                ..default()
            })).collect();
            let plain = mats.add(StandardMaterial { base_color: Color::srgb(0.7, 0.7, 0.7), perceptual_roughness: 0.8, double_sided: true, cull_mode: None, ..default() });
            for q in &own.pieces {
                let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, q.pos.clone())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, q.nrm.clone())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, bevy::mesh::VertexAttributeValues::Uint16x4(q.joints.clone()))
                    .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, q.weights.clone());
                if !q.uv.is_empty() {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, q.uv.clone());
                    let _ = mesh.generate_tangents();
                }
                let m = q.mat.and_then(|k| made.get(k).cloned()).unwrap_or(plain.clone());
                let skin = commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(m), Transform::default(), Visibility::Inherited,
                                           bevy::mesh::skinning::SkinnedMesh { inverse_bindposes: bind.clone(), joints: joints.clone() },
                                           bevy::camera::visibility::NoFrustumCulling, layer.clone())).id();
                commands.entity(joints[0]).add_child(skin);
            }
            continue;
        }
        for (k, (&t, p)) in caps.iter().zip(&h.build.parts).enumerate() {
            let m = mat(coat(h.animal, k, &p.name));
            let shown = if crate::animal_fit::tailless(h.animal) && p.name.starts_with("Tail") { Visibility::Hidden } else { Visibility::Inherited };
            let shape = meshes.add(Capsule3d::new(p.radius, p.half * 2.0));
            let mut e = commands.spawn((Mesh3d(shape), MeshMaterial3d(m), t, HorsePart(id, k),
                                        layer.clone(), shown));
            for &(_, end, o, shape, size, col) in touch.iter().filter(|x| x.0 == p.name) {
                let r = h.build.parts.iter().filter(|q| q.name.starts_with(p.name.as_str())).map(|q| q.radius).fold(p.radius, f32::max);
                let axis = t.rotation * Vec3::Y * p.half;
                let from = match end { 0 => t.translation, e => if (axis.dot(fwd) - axis.z) * e as f32 >= 0.0 { t.translation + axis } else { t.translation - axis } };
                let ax = axis.normalize_or(Vec3::X);
                let (up, ahead) = if ax.z.abs() > 0.95 { (Vec3::Z, fwd) } else {
                    let up = (Vec3::Z - ax * ax.z).normalize_or(Vec3::Z);
                    (up, (fwd - ax * ax.dot(fwd) - up * up.dot(fwd)).normalize_or(fwd))
                };
                let at = from + (side * o[0] + ahead * o[1] + up * o[2]) * r;
                let (mesh, turn) = if shape == 0 {
                    (meshes.add(Sphere::new(size[0] * r)), Quat::IDENTITY)
                } else if shape == 2 {
                    (meshes.add(Cylinder::new(size[0] * r, size[1] * r)), Quat::from_rotation_arc(Vec3::Y, ahead))
                } else {
                    let dir = (Vec3::Z + side * size[2]).normalize();
                    (meshes.add(Cone::new(size[0] * r * 1.4, size[1] * r * 1.7)), Quat::from_rotation_arc(Vec3::Y, dir))
                };
                let local = Transform::from_translation(t.rotation.inverse() * (at - t.translation)).with_rotation(t.rotation.inverse() * turn);
                let m = mat(Color::srgb(col[0], col[1], col[2]));
                e.with_child((Mesh3d(mesh), MeshMaterial3d(m), local, layer.clone()));
            }
        }
    }
    let caps: Vec<(u32, Vec<Transform>)> = all.iter().map(|(id, h)| (*id, h.doll.capsules().collect())).collect();
    for (_, HorsePart(id, k), mut t) in &mut parts {
        if let Some(c) = caps.iter().find(|c| c.0 == *id).and_then(|c| c.1.get(*k)) {
            *t = *c;
        }
    }
}

pub static FREEZE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn live() -> bool {
    std::env::var_os("HORSE_LIVE").is_some() || !FREEZE.load(std::sync::atomic::Ordering::Relaxed)
}

pub static BALANCE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(255);
const WIDE: &str = "1.4,1.1,1.5,1.4,1.5,1,1.5,1";

pub fn balance() -> u8 {
    match BALANCE.load(std::sync::atomic::Ordering::Relaxed) {
        255 if std::env::var("HORSE_SHARE").is_ok_and(|v| v != "0") =>
            if std::env::var("HORSE_STANDPOSE").is_ok_and(|v| v.ends_with("-normal")) { 1 } else { 2 },
        255 => 0,
        m => m,
    }
}

pub fn set_balance(m: u8) {
    let m = m.min(2);
    BALANCE.store(m, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::current_exe().ok().and_then(|e| e.ancestors().skip(1)
        .map(|d| d.join("tools").join(if m == 2 { "standpose" } else { "standpose-normal" })).find(|d| d.is_dir()));
    unsafe {
        let set = [("HORSE_LIVE", "1"), ("HORSE_SHARE", "1"), ("HORSE_LANDPICK", "0.5"), ("HORSE_MINSTEP", "0.03"), ("HORSE_NOCROSS", "1")];
        for (k, v) in set { if m == 0 { std::env::remove_var(k) } else { std::env::set_var(k, v) } }
        match (&dir, m) {
            (Some(d), 1 | 2) => std::env::set_var("HORSE_STANDPOSE", d),
            _ => std::env::remove_var("HORSE_STANDPOSE"),
        }
        if m == 2 { std::env::set_var("HORSE_WIDE", WIDE) } else { std::env::remove_var("HORSE_WIDE") }
    }
}
