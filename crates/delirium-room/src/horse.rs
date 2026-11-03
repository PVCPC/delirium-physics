//! Red Dead Redemption 2's horse, built from the player's own copies of its two files: its ragdoll (the fragment,
//! meta_base_horse_01.yft, read by `rdr2frag`) and its Euphoria body (horse.xml). From the fragment: its 26 capsules
//! (size, place, turn, mass and inertia, 617 kg in all), its 22 joints' frames and which links collide with each
//! other; from horse.xml: each joint's limits and its muscle (strength and damping). Each of RDR2's 23 links is a
//! part; the three made of two capsules (the top of the neck, its middle and the head) get their second capsule as a
//! part of its own, held to the first. While it's sound it's held in its pose, as RDR2 holds its horses on their
//! animation; lamed or killed it falls as RDR2's ragdoll, and down and alive it does what RDR2's `quadrupedbase` does
//! on the ground (see `Quadruped`).
//!
//! Where it isn't RDR2's: Delirium's parts hold their mass at their capsule's centre, so the 0.2 m RDR2 moves the
//! pelvis's and the three spine links' centre of mass off theirs isn't there. The second capsules are held to their
//! first by a stiff joint with a little play, not rigidly. The knees, hocks, elbows and fetlocks, which RDR2 makes
//! three-way joints that bend on their twist with ±0.07 rad of lean, are hinges on that same axis and range, without
//! the lean. The pelvis is the root, with RDR2's root link hanging from it on the same joint (see `Horse::new`).
//! RDR2's standing pose has its pelvis, hips' twist and hind knees past RDR2's own limits; it starts turned just
//! inside them (see `in_limits`). And the muscles' Euphoria stiffness (0.5, for the blend of a joint's drive) isn't
//! used: Delirium's muscle takes RDR2's strength and damping.
use crate::rdr2frag;
use bevy::prelude::*;
use delirium::collision::Collision;
use delirium::ragdoll::{Build, Capsule, Drive, Limit, Muscle, Obstacle, PartDef, Ragdoll, GRAVITY};

/// The parts, in the fragment's order but for the pelvis first (see `Horse::new`): which of RDR2's links each is (two for a link of two capsules) and Delirium's
/// name for it (the hooves end in "Foot" for their grip).
const PARTS: [(&str, &str); 26] = [
    ("skel_pelvis", "Pelvis"), ("skel_spine_root", "Root"), ("skel_r_thigh", "R_Thigh"), ("skel_l_thigh", "L_Thigh"),
    ("skel_tail1", "Tail1"), ("skel_spine1", "Spine1"), ("skel_r_calf", "R_Calf"), ("skel_r_foot", "R_HindFoot"),
    ("skel_l_calf", "L_Calf"), ("skel_l_foot", "L_HindFoot"), ("skel_tail4", "Tail4"), ("skel_spine2", "Spine2"),
    ("skel_spine3", "Spine3"), ("skel_r_upperarm", "R_UpperArm"), ("skel_l_upperarm", "L_UpperArm"), ("skel_neck1", "Neck1"),
    ("skel_neck1", "Neck1b"), ("skel_r_forearm", "R_Forearm"), ("skel_r_hand", "R_ForeFoot"), ("skel_l_forearm", "L_Forearm"),
    ("skel_l_hand", "L_ForeFoot"), ("skel_neck3", "Neck3"), ("skel_neck3", "Neck3b"), ("skel_neck5", "Neck5"),
    ("skel_head", "Head"), ("skel_head", "Headb"),
];
/// The legs, as (thigh or upper arm, lower, hoof): hind right, hind left, fore right, fore left.
pub const LEGS: [[usize; 3]; 4] = [[2, 6, 7], [3, 8, 9], [13, 17, 18], [14, 19, 20]];
pub const HEAD: usize = 24;
/// Its neck and head.
pub const NECK_HEAD: [usize; 7] = [15, 16, 21, 22, 23, 24, 25];
const NECK1: usize = 15;
/// The barrel: pelvis, spine 1 to 3; its middle (spine 2) is what its height, level and saddle are taken from.
const BARREL: [usize; 4] = [0, 5, 11, 12];
const PELVIS: usize = 0;
/// RDR2's links for the root and the pelvis.
const ROOT_LINK: usize = 0;
const PELVIS_LINK: usize = 1;
const MID: usize = 11;
const SHOULDERS: usize = 12;
/// A link's second capsule's joint to its first: its play, rad, and the muscle holding it, rad/s.
const WELD_PLAY: f32 = 0.05;
const WELD: f32 = 60.0;
/// The second capsules (of the top of the neck, its middle, the head).
const WELDED: [usize; 3] = [16, 22, 25];
const HEALTH: f32 = 400.0;
/// Damage a leg takes before it gives.
const LAME: f32 = 60.0;
/// s it stays up after the hit that lames or kills it.
const GIVE: f32 = 0.2;
/// How high a rider's pelvis sits over the top of the barrel's middle, m.
const SEAT: f32 = 0.15;
/// Down, with alwayslifthead: the share of its neck and head's weight lifted, fresh (0.6 of it once faded).
const HEAD_LIFT: f32 = 1.3;

/// Which muscle group a part is in: barrel, legs, neck, head, tail.
fn group(k: usize) -> usize {
    match k {
        0 | 1 | 5 | 11 | 12 => 0,
        _ if LEGS.iter().any(|l| l.contains(&k)) => 1,
        15 | 16 | 21 | 22 | 23 => 2,
        24 | 25 => 3,
        _ => 4,
    }
}

/// RDR2's `quadrupedbase` behaviour's settings, by its names and with its defaults (read from RDR2's own table); what
/// each does here is read from its name, not from RDR2's code. Down and alive, the horse lifts its head, paddles its
/// legs on its side and writhes its neck, at a strength that fades from `legStiffness` toward `legStiffnessMin`; hit,
/// it goes weak for a moment; tipped past `failangle` it goes down.
#[derive(Clone, Debug)]
pub struct Quadruped {
    /// legStiffness, legStiffnessMin, spineStiffness, spinestiffnessmin: Euphoria's stiffness (its 15 is our full
    /// muscle), fresh and once its strength has faded
    pub leg_stiffness: f32,
    pub leg_stiffness_min: f32,
    pub spine_stiffness: f32,
    pub spine_stiffness_min: f32,
    /// strengthdecay: 1/s its strength fades at once it's down
    pub strength_decay: f32,
    /// failangle: tipped till its back's up is this far from the sky's (a cosine), it goes down
    pub fail_angle: f32,
    /// initialWeaknessZeroDuration, initialWeaknessRampDuration: s it's limp after a hit, then s to get its
    /// strength back
    pub weak_zero: f32,
    pub weak_ramp: f32,
    /// snap, snapMag: a hit jolts its whole body too, by this much
    pub snap: bool,
    pub snap_mag: f32,
    /// alwayslifthead: down, it holds its head up off the ground
    pub lift_head: bool,
    /// scrabbleonside, minscrabblespeed, maxscrabblespeed: on its side, its legs paddle, at rad/s from max (fresh)
    /// to min (faded)
    pub scrabble: bool,
    pub scrabble_min: f32,
    pub scrabble_max: f32,
    /// writheonground, writhespeed: down, its neck writhes, this many times a second
    pub writhe: bool,
    pub writhe_speed: f32,
    /// rollonground, rollvelocity, rollscale: down, it rocks to roll onto its chest, up to rad/s, this hard
    pub roll: bool,
    pub roll_velocity: f32,
    pub roll_scale: f32,
    /// rightingTorque: down, a steady turn toward lying on its chest (0: none)
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
    /// The number settings by RDR2's names, with each one's range (RDR2's).
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

    /// The yes/no settings by RDR2's names.
    pub fn flags(&mut self) -> [(&'static str, &mut bool); 5] {
        [("snap", &mut self.snap), ("alwayslifthead", &mut self.lift_head), ("scrabbleonside", &mut self.scrabble),
         ("writheonground", &mut self.writhe), ("rollonground", &mut self.roll)]
    }

    /// Sets one by RDR2's name (yes/no as 1/0 or true/false); false if there's none by that name or it isn't a value.
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

pub struct Horse {
    pub build: Build,
    pub doll: Ragdoll,
    targets: Vec<Quat>,
    /// each part's own muscle from horse.xml: stiffness (rad/s) and damping ratio
    muscles: Vec<(f32, f32)>,
    pub health: f32,
    pub legs: [f32; 4],
    pub dead: bool,
    /// s since it was lamed or killed: past `GIVE` it goes down
    sunk: f32,
    kept: delirium::ragdoll::Kept,
    /// which way the sky and its forward were in the barrel's middle's frame as it stood
    stand: Vec3,
    fwd: Vec3,
    /// the sky's up and its left, in each part's frame as it stood
    ups: Vec<Vec3>,
    lefts: Vec<Vec3>,
    pub q: Quadruped,
    /// tipped past `failangle`, it went down
    pub fell: bool,
    /// s since its last hit, and since it went down alive
    since_hit: f32,
    pub down: f32,
    /// the pose it stands in, held while it's sound; whether it's held still; the hits' pushes meanwhile, given
    /// to it as it goes down
    pose: Vec<Transform>,
    held: bool,
    pushes: Vec<(usize, Vec3, Vec3)>,
}

/// RAGE's margin inside a joint's limits, rad, as the fragment stores them.
const RAGE_MARGIN: f32 = 0.03;
/// What's kept off a limit when the pose is turned inside it, rad (the solver's own margin is 0.03).
const INSIDE: f32 = 0.04;
/// A joint with less lean than this either way, rad, and more twist, is a hinge.
const HINGE_LEAN: f32 = 0.1;
const HINGE_TWIST: f32 = 0.5;

/// `frames` with each joint turned to the nearest it may (RDR2's model pose has a few joints past RDR2's own limits:
/// the pelvis on its root, the hips' twist, the hind knees' lean, which in the game a ragdoll never starts from),
/// rebuilt from the root out and stood back on the floor at `floor`.
fn in_limits(parts: &mut [PartDef], frames: &[Transform], floor: f32) -> Vec<Transform> {
    let mut out = frames.to_vec();
    for k in 0..parts.len() {
        let Some(p) = parts[k].parent else { continue };
        let turn = match parts[k].limit {
            Limit::Bone { pf, cf, lean, twist } => {
                let rel = (frames[p].rotation * pf).inverse() * (frames[k].rotation * cf);
                // (the quaternion with w >= 0, so the twist comes out within half a turn)
                let rel = if rel.w < 0.0 { -rel } else { rel };
                // swing and twist (about the joint's z)
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

/// A turn on the whole barrel, shared out over its links.
fn spread(torque: &mut [Vec3], t: Vec3) {
    for &k in &BARREL {
        torque[k] += t / BARREL.len() as f32;
    }
}

/// The numbers in a tag's text.
fn field(block: &str, tag: &str) -> Option<f32> {
    let a = block.find(&format!("<{tag}>"))? + tag.len() + 2;
    block[a..a + block[a..].find('<')?].trim().parse().ok()
}

/// One of horse.xml's joints: the part it moves (joint number + 1), the part it hangs from, its limits (lean 1,
/// lean 2, twist: min, max) and its muscle (stiffness, strength, damping).
struct XmlJoint {
    child: usize,
    parent: usize,
    limits: [(f32, f32); 3],
    muscle: [f32; 3],
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
        })
    }).collect()
}

/// The fragment's frame (x left, y back, z up) to the room's (x forward, y left, z up).
fn room(v: Vec3) -> Vec3 {
    Vec3::new(-v.y, v.x, v.z)
}

fn room_turn(m: Mat3) -> Quat {
    Quat::from_mat3(&Mat3::from_cols(room(m.x_axis), room(m.y_axis), room(m.z_axis))).normalize()
}

impl Horse {
    /// A horse standing with its hooves on the floor at `at`, facing `facing` (radians about up), from horse.xml's text
    /// and the fragment's bytes.
    pub fn new(xml: &str, frag: &[u8], at: Vec3, facing: f32) -> Result<Horse, String> {
        let joints = read_xml(xml)?;
        let fr = rdr2frag::read(frag)?;
        // the parts: each link's capsules in turn
        // the pelvis is the root: RDR2's root link (0.475 kg, under the 111 kg pelvis on a joint with almost no play)
        // at the base of the body unsettled the solver every time that joint met its limit, so it hangs from the
        // pelvis instead, on the same joint seen from the other side
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
        // placed: lifted so its lowest capsule (a hoof) just clears the floor, and turned to face `facing`
        let low = fr.links.iter().flat_map(|l| &l.capsules).map(|c| c.at.z - c.turn.y_axis.z.abs() * (c.half - c.radius) - c.radius).fold(f32::MAX, f32::min);
        let turn = Quat::from_rotation_z(facing);
        let place = |p: Vec3| at + turn * (room(p) + Vec3::Z * (0.02 - low));
        let frames: Vec<Transform> = part_of.iter().map(|&(l, c)| {
            let cap = &fr.links[l].capsules[c];
            Transform::from_translation(place(cap.at)).with_rotation(turn * room_turn(cap.turn))
        }).collect();
        let mut muscles = vec![(0.0, 1.0); part_of.len()];
        let mut parts = vec![];
        for (k, &(l, c)) in part_of.iter().enumerate() {
            let cap = &fr.links[l].capsules[c];
            let f = frames[k];
            let (parent, limit, joint) = if c > 0 {
                // a link's second capsule, held to its first: Delirium's locked joint is only for a root (between two
                // parts its zero-width limit turned the solver's numbers invalid), so a joint with almost no play
                muscles[k] = (WELD, 1.0);
                (Some(first(l)), Limit::Cone { swing: WELD_PLAY, twist: WELD_PLAY }, f.translation)
            } else if let Some((j, flip)) = if l == ROOT_LINK { fr.joints.iter().find(|j| j.child == PELVIS_LINK && j.parent == ROOT_LINK).map(|j| (j, true)) }
                                             else if l == PELVIS_LINK { None } else { fr.joints.iter().find(|j| j.child == l).map(|j| (j, false)) } {
                let x = joints.iter().find(|x| x.child == j.child).ok_or_else(|| format!("horse.xml has no joint for {}", fr.links[j.child].name))?;
                if x.parent != j.parent {
                    return Err(format!("horse.xml and the fragment disagree on what {} hangs from", fr.links[j.child].name));
                }
                // (its stiffness, 0.5, is for Euphoria's blend of the joint's drive, not used here)
                let [_, strength, damping] = x.muscle;
                let s = strength.max(0.0).sqrt();
                muscles[k] = (s, if s > 0.0 { damping / (2.0 * s) } else { 1.0 });
                let half = |(lo, hi): (f32, f32)| (hi - lo) * 0.5;
                // the fragment keeps the same ranges, less RAGE's margin: a check the two files belong together
                let theirs = Vec3::new(half(x.limits[0]), half(x.limits[1]), half(x.limits[2])) - Vec3::splat(RAGE_MARGIN);
                if (theirs.max(Vec3::ZERO) - j.half).abs().max_element() > 0.01 {
                    return Err(format!("horse.xml and the fragment disagree on {}'s limits: not the same horse", fr.links[j.child].name));
                }
                // its frames are in the links' frames, which turn as their first capsules do
                let qq = |x: Mat3| Quat::from_mat3(&x).normalize();
                let (in_parent, in_child) = if flip { (j.in_child, j.in_parent) } else { (j.in_parent, j.in_child) };
                let limit = Limit::Bone { pf: qq(in_parent), cf: qq(in_child),
                                          lean: [half(x.limits[0]), half(x.limits[1])], twist: half(x.limits[2]) };
                // a joint RDR2 uses as a hinge (its bend on the twist axis, almost no lean either way): Delirium's hinge
                // on that axis, through RDR2's twist range (its ball joint with so little lean overcorrected under
                // load and threw the legs about; only GTA IV's true hinges had ever been through it)
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
        let frames = in_limits(&mut parts, &frames, at.z);
        let build = Build { parts, nm: delirium::nmrs::HOMER, links: vec![] };
        let targets = (0..part_of.len()).map(|k| build.parts[k].parent.map_or(Quat::IDENTITY, |p| frames[p].rotation.inverse() * frames[k].rotation)).collect();
        let mut doll = Ragdoll::new(&build, &frames, Vec3::ZERO);
        // the links RDR2 lets collide with each other, each capsule of one with each of the other
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
        let ups = frames.iter().map(|f| f.rotation.inverse() * Vec3::Z).collect();
        let lefts = frames.iter().map(|f| f.rotation.inverse() * (turn * Vec3::Y)).collect();
        Ok(Horse { build, doll, targets, muscles, health: HEALTH, legs: [0.0; 4], dead: false, sunk: 0.0, kept: Default::default(),
                   stand, fwd, ups, lefts, q: Quadruped::default(), fell: false, since_hit: f32::MAX, down: 0.0,
                   pose: frames.clone(), held: true, pushes: vec![] })
    }

    /// One physics step. While it's sound it's held in its pose, as RDR2 holds its horses on their animation (which
    /// isn't here; standing on Delirium's muscles, a ragdoll this heavy folded or tipped within seconds). A moment
    /// after the hit that lames or kills it it goes down: let go, it falls as RDR2's ragdoll with the hits' pushes, on
    /// RDR2's own muscles (horse.xml's). Dead, it lies limp; alive, it does `Quadruped`'s ground behaviours.
    pub fn step(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
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
        // a hit's weakness: none left for `weak_zero`, back over `weak_ramp`
        let weak = ((self.since_hit - q.weak_zero) / q.weak_ramp.max(1e-3)).clamp(0.0, 1.0);
        // down, its strength fades: 1 fresh, 0 at the `..Min` stiffnesses
        let strength = (-q.strength_decay * self.down).exp();
        let n = self.build.parts.len();
        let (mut force, mut torque) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);
        // standing, held in its pose as RDR2 holds it on its animation; down, let go to fall as a ragdoll, with the
        // push of the hit that brought it down
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
            // how much a turn about its length toward lying on its chest is wanted (+ or -)
            let toward = back.cross(Vec3::Z).dot(fwd);
            if q.lift_head {
                // lying, its neck's pose points along the ground: it bends it up toward the sky
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
                // it rocks: every other second it heaves toward its chest, up to `roll_velocity`
                let way = toward.signum();
                let want = q.roll_velocity * (self.down * std::f32::consts::PI).sin().max(0.0);
                spread(&mut torque, fwd * way * (want - way * w.dot(fwd)).max(0.0) * q.roll_scale * m * GRAVITY() * 0.6 * weak);
            }
            let t = self.down;
            // on its side, its legs paddle: hind right with fore left, hind left with fore right
            if q.scrabble && back.z.abs() < 0.6 {
                let speed = q.scrabble_min + (q.scrabble_max - q.scrabble_min) * strength;
                let reach = 0.25 + 0.45 * strength;
                for (l, leg) in LEGS.iter().enumerate() {
                    let phase = t * speed + [0.0, std::f32::consts::PI, std::f32::consts::PI, 0.0][l];
                    let parent = self.build.parts[leg[0]].parent.unwrap_or(MID);
                    targets[leg[0]] = Quat::from_axis_angle(self.lefts[parent], reach * phase.sin()) * self.targets[leg[0]];
                    // hocks bend back, knees forward
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
                // RDR2's muscle, or quadrupedbase's stiffness (Euphoria's stiffness is the muscle's frequency)
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
        let drive: Drive = self.kept.drive();
        self.doll.step(&self.build, world, dt, drive, obstacles, others);
    }

    /// Still on its feet.
    pub fn standing(&self) -> bool {
        self.sunk <= GIVE
    }

    /// Its saddle: where a rider's pelvis sits (over the barrel's middle), the barrel's up and which way it faces, and
    /// the stirrups (left, right), all world.
    pub fn saddle(&self) -> (Vec3, Vec3, Vec3, [Vec3; 2]) {
        let f = self.doll.frames().nth(MID).unwrap_or_default();
        let (up, fwd) = (f.rotation * self.stand, f.rotation * self.fwd);
        let left = up.cross(fwd).normalize_or(Vec3::Y);
        let top = self.build.parts[MID].half + self.build.parts[MID].radius;
        let seat = f.translation + up * (top + SEAT);
        let stirrup = |side: f32| seat - up * 0.62 + left * (0.42 * side) + fwd * 0.15;
        (seat, up, fwd, [stirrup(1.0), stirrup(-1.0)])
    }

    /// What it's doing, for the panel.
    pub fn state(&self) -> String {
        if self.dead {
            "dead".into()
        } else if self.sunk > GIVE {
            let (fresh, min) = (self.q.leg_stiffness, self.q.leg_stiffness_min);
            format!("down {:.0} s, legs at {:.0}% strength", self.down, (min + (fresh - min) * (-self.q.strength_decay * self.down).exp()) / fresh.max(1e-3) * 100.0)
        } else {
            "standing".into()
        }
    }

    /// How far each joint is from its muscle's target, radians (for the headless check).
    pub fn errors(&self) -> Vec<f32> {
        let q: Vec<Quat> = self.doll.frames().map(|f| f.rotation).collect();
        (0..q.len()).map(|k| self.build.parts[k].parent.map_or(0.0, |p| (q[p] * self.targets[k]).angle_between(q[k]))).collect()
    }

    /// Its capsules, for others to bump into.
    pub fn capsules(&self) -> Vec<Capsule> {
        self.doll.capsules().zip(&self.build.parts).enumerate().map(|(k, (t, p))| {
            let a = t.rotation * Vec3::Y * p.half;
            (t.translation - a, t.translation + a, p.radius, self.doll.motion(k).0, true)
        }).collect()
    }

    /// A hit on `part`: an impulse there, `damage` to its health and, in a leg, to that leg. What happened, for the log.
    pub fn hit(&mut self, part: usize, at: Vec3, impulse: Vec3, damage: f32) -> String {
        // (a zero impulse, with it going limp, turned the solver's numbers invalid)
        if impulse != Vec3::ZERO {
            if self.held {
                self.pushes.push((part, at, impulse));
            } else {
                self.doll.impulse(part, at, impulse);
            }
        }
        if self.dead {
            return format!("{}: hit, dead already", self.build.parts[part].name);
        }
        if damage > 0.0 {
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

/// The horse's look: a capsule per part, following its body.
#[derive(Component)]
pub struct HorsePart(pub usize);

pub fn view(mut commands: Commands, sim: NonSend<crate::Sim>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
            mut parts: Query<(Entity, &HorsePart, &mut Transform)>) {
    let Some(h) = sim.horse.as_ref() else {
        for (e, ..) in &parts {
            commands.entity(e).despawn();
        }
        return;
    };
    if parts.is_empty() {
        let coat = mats.add(StandardMaterial { base_color: Color::srgb(0.42, 0.27, 0.16), perceptual_roughness: 0.8, ..default() });
        let hoof = mats.add(StandardMaterial { base_color: Color::srgb(0.12, 0.1, 0.09), perceptual_roughness: 0.6, ..default() });
        for (k, (t, p)) in h.doll.capsules().zip(&h.build.parts).enumerate() {
            let mat = if p.name.ends_with("Foot") { hoof.clone() } else { coat.clone() };
            commands.spawn((Mesh3d(meshes.add(Capsule3d::new(p.radius, p.half * 2.0))), MeshMaterial3d(mat), t, HorsePart(k)));
        }
        return;
    }
    let caps: Vec<Transform> = h.doll.capsules().collect();
    for (_, HorsePart(k), mut t) in &mut parts {
        if let Some(c) = caps.get(*k) {
            *t = *c;
        }
    }
}
