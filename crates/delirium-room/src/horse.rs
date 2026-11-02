//! Red Dead Redemption 2's horse, from its body file (horse.xml, from the player's own copy): a physics horse that
//! stands, takes hits, and goes down when a leg is lamed or it's killed. RDR2's file gives where its parts are; the
//! shapes and masses are Delirium's own, for a horse of about 600 kg, and its trunk is one rigid barrel. Its legs,
//! neck, head and tail are jointed to the barrel and held in their pose by muscles (the neck's can't hold the head up,
//! so it hangs low, as if grazing). There's no balance yet: see `step`.
//!
//! No part weighs under 10 kg, the tail and the top of the neck included: Delirium's joint solver blows up with a very
//! light part on the end of a chain under a heavy one (a 1 kg tail tip on a 260 kg barrel), which a person's body
//! never has. Every part at 6 kg or more held in tests; 10 leaves room.
use bevy::prelude::*;
use delirium::collision::Collision;
use delirium::ragdoll::{Build, Capsule, Drive, Limit, Muscle, Obstacle, PartDef, Ragdoll, GRAVITY};

/// Where a capsule starts or ends: one of RDR2's parts' places, that offset (in RDR2's frame: up, forward, left), or
/// offset from the start.
#[derive(Clone, Copy)]
enum At {
    Part(usize),
    Off(usize, [f32; 3]),
    Stub([f32; 3]),
}

/// A part: Delirium's name for it (the hooves end in "Foot" for their grip), its capsule's start and end, the part it
/// hangs from, its radius and mass, and which muscle group it's in (see `STIFF`).
struct Shape {
    name: &'static str,
    from: At,
    to: At,
    parent: Option<usize>,
    radius: f32,
    mass: f32,
    muscle: usize,
}

const fn leg(name: &'static str, from: usize, to: At, parent: usize, radius: f32, mass: f32) -> Shape {
    Shape { name, from: At::Part(from), to, parent: Some(parent), radius, mass, muscle: 1 }
}

const HOOF: At = At::Stub([-0.1, 0.03, 0.0]);

const SHAPES: [Shape; 19] = [
    // the barrel, from behind RDR2's pelvis to its third spine part
    Shape { name: "Body", from: At::Off(1, [0.0, -0.5, 0.0]), to: At::Part(12), parent: None, radius: 0.33, mass: 260.0, muscle: 0 },
    leg("R_Thigh", 2, At::Part(6), 0, 0.13, 38.0),
    leg("R_Calf", 6, At::Part(7), 1, 0.06, 20.0),
    leg("R_HindFoot", 7, HOOF, 2, 0.06, 12.0),
    leg("L_Thigh", 3, At::Part(8), 0, 0.13, 38.0),
    leg("L_Calf", 8, At::Part(9), 4, 0.06, 20.0),
    leg("L_HindFoot", 9, HOOF, 5, 0.06, 12.0),
    leg("R_UpperArm", 13, At::Part(16), 0, 0.11, 30.0),
    leg("R_Forearm", 16, At::Part(17), 7, 0.06, 18.0),
    leg("R_ForeFoot", 17, HOOF, 8, 0.06, 12.0),
    leg("L_UpperArm", 14, At::Part(18), 0, 0.11, 30.0),
    leg("L_Forearm", 18, At::Part(19), 10, 0.06, 18.0),
    leg("L_ForeFoot", 19, HOOF, 11, 0.06, 12.0),
    Shape { name: "Neck1", from: At::Part(15), to: At::Part(20), parent: Some(0), radius: 0.14, mass: 14.0, muscle: 2 },
    Shape { name: "Neck3", from: At::Part(20), to: At::Part(21), parent: Some(13), radius: 0.12, mass: 11.0, muscle: 2 },
    Shape { name: "Neck5", from: At::Part(21), to: At::Part(22), parent: Some(14), radius: 0.11, mass: 10.0, muscle: 2 },
    Shape { name: "Head", from: At::Part(22), to: At::Stub([-0.25, 0.42, 0.0]), parent: Some(15), radius: 0.11, mass: 18.0, muscle: 3 },
    Shape { name: "Tail1", from: At::Part(4), to: At::Part(10), parent: Some(0), radius: 0.05, mass: 10.0, muscle: 4 },
    Shape { name: "Tail4", from: At::Part(10), to: At::Stub([-0.35, -0.08, 0.0]), parent: Some(17), radius: 0.04, mass: 10.0, muscle: 4 },
];

/// RDR2's horse has 23 parts.
const RDR2_PARTS: usize = 23;
/// Muscle stiffness (natural frequency, rad/s) by group: trunk, legs, neck, head, tail. Damping ratio 1.
const STIFF: [f32; 5] = [0.0, 30.0, 35.0, 25.0, 5.0];
/// What's left of a group's stiffness once dead.
const LIMP: f32 = 0.0;
/// The legs, as (thigh or upper arm, lower, hoof): hind right, hind left, fore right, fore left.
pub const LEGS: [[usize; 3]; 4] = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]];
pub const HEAD: usize = 16;
const HEALTH: f32 = 400.0;
/// Damage a leg takes before it gives.
const LAME: f32 = 60.0;
/// s it stays up after the hit that lames or kills it.
const GIVE: f32 = 0.2;
/// Share of its weight the lift carries (the legs stand under the rest), and the springs holding the barrel's
/// height (1/s², 1/s) and keeping it level (1/s², 1/s).
const CARRY: f32 = 0.85;
const HEIGHT: (f32, f32) = (20.0, 10.0);
const LEVEL: (f32, f32) = (20.0, 9.0);

pub struct Horse {
    pub build: Build,
    pub doll: Ragdoll,
    targets: Vec<Quat>,
    pub health: f32,
    pub legs: [f32; 4],
    pub dead: bool,
    /// s since it was lamed or killed: past `GIVE` it goes down
    sunk: f32,
    kept: delirium::ragdoll::Kept,
    /// the barrel's height and the floor's when it stood, and which way the barrel's up pointed in its own frame
    stand: (f32, f32, Vec3),
}

/// The numbers in a tag's text, or an attribute's value.
fn field(block: &str, tag: &str) -> Option<f32> {
    let a = block.find(&format!("<{tag}>"))? + tag.len() + 2;
    block[a..a + block[a..].find('<')?].trim().parse().ok()
}

fn attr(line: &str, name: &str) -> Option<f32> {
    let a = line.find(&format!("{name}=\""))? + name.len() + 2;
    line[a..a + line[a..].find('"')?].parse().ok()
}

/// RDR2's frame (x up, y forward, z left) to the room's (x forward, y left, z up).
fn room(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[1], v[2], v[0])
}

/// Reads horse.xml: each of RDR2's parts' places.
fn read(text: &str) -> Result<Vec<Vec3>, String> {
    if !text.contains("<bodyType>quadruped</bodyType>") {
        return Err("not RDR2's horse.xml (not a quadruped's body)".into());
    }
    let mut at = vec![None; RDR2_PARTS];
    for block in text.split("<bodyPart ").skip(1) {
        let block = &block[..block.find("</bodyPart>").unwrap_or(block.len())];
        let k = field(block, "partNumber").ok_or("a part without its number")? as usize;
        let r3 = block.lines().find(|l| l.trim_start().starts_with("<r3")).ok_or("a part without its place")?;
        let [Some(x), Some(y), Some(z)] = [attr(r3, "c0"), attr(r3, "c1"), attr(r3, "c2")] else {
            return Err("a part's place isn't numbers".into());
        };
        if let Some(slot) = at.get_mut(k) {
            *slot = Some(room([x, y, z]));
        }
    }
    let seen = at.iter().flatten().count();
    if seen != RDR2_PARTS {
        return Err(format!("not RDR2's horse.xml ({seen} of {RDR2_PARTS} parts)"));
    }
    Ok(at.into_iter().flatten().collect())
}

impl Horse {
    /// A horse standing with its hooves on the floor at `at`, facing `facing` (radians about up).
    pub fn new(text: &str, at: Vec3, facing: f32) -> Result<Horse, String> {
        let place = read(text)?;
        let point = |a: At, from: Vec3| match a {
            At::Part(k) => place[k],
            At::Off(k, d) => place[k] + room(d),
            At::Stub(d) => from + room(d),
        };
        let raw: Vec<(Vec3, Vec3)> = SHAPES.iter().map(|s| {
            let a = point(s.from, Vec3::ZERO);
            (a, point(s.to, a))
        }).collect();
        // lifted so the lowest capsule (a hoof) just clears the floor, and turned to face `facing`
        let low = SHAPES.iter().zip(&raw).map(|(s, (a, b))| a.z.min(b.z) - s.radius).fold(f32::MAX, f32::min);
        let turn = Quat::from_rotation_z(facing);
        let world = |p: Vec3| at + turn * (p + Vec3::Z * (0.02 - low));
        let ends: Vec<(Vec3, Vec3)> = raw.iter().map(|&(a, b)| (world(a), world(b))).collect();
        let frames: Vec<Transform> = ends.iter().map(|&(a, b)| {
            Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize_or(Vec3::Z)))
        }).collect();
        let parts = SHAPES.iter().enumerate().map(|(k, s)| {
            let f = frames[k];
            let joint = ends[k].0;
            let (rest, anchor_parent) = s.parent.map_or((Quat::IDENTITY, Vec3::ZERO), |q| {
                let pf = frames[q];
                (f.rotation.inverse() * pf.rotation, pf.rotation.inverse() * (joint - pf.translation))
            });
            let (swing, twist) = if s.muscle == 1 || s.muscle == 4 { (1.5, 0.5) } else { (0.8, 0.3) };
            PartDef { size: 1.0, heft: 1.0, grown: 1.0, bone: k, parent: s.parent, mass: s.mass, radius: s.radius,
                      half: (ends[k].1 - ends[k].0).length() * 0.5, offset: Transform::IDENTITY, rest, anchor_parent,
                      anchor: f.rotation.inverse() * (joint - f.translation),
                      limit: if s.parent.is_some() { Limit::Cone { swing, twist } } else { Limit::Locked },
                      lower: s.muscle == 1, reach: false, name: s.name.to_string(), bind: f.rotation, boxed: None,
                      inertia: Vec3::ZERO, length: 0.0 }
        }).collect();
        let build = Build { parts, nm: delirium::nmrs::HOMER, links: vec![] };
        let targets = SHAPES.iter().enumerate().map(|(k, s)| s.parent.map_or(Quat::IDENTITY, |p| frames[p].rotation.inverse() * frames[k].rotation)).collect();
        let doll = Ragdoll::new(&build, &frames, Vec3::ZERO);
        let stand = (frames[0].translation.z, at.z, frames[0].rotation.inverse() * Vec3::Z);
        Ok(Horse { build, doll, targets, health: HEALTH, legs: [0.0; 4], dead: false, sunk: 0.0, kept: Default::default(), stand })
    }

    /// One physics step. While it's sound it stands, as RDR2's horses do on their animation: the muscles hold the legs,
    /// neck, head and tail in their pose (a hurt leg's weaker), and since Delirium's muscles push by how light the part
    /// below them is, not by the weight on the hoof, they couldn't hold it up alone; so, as Euphoria's oppose gravity
    /// does, while a hoof is down most of its weight is carried by a lift on the barrel at the hips and the shoulders,
    /// held to the height it stood at, and a turn keeps the barrel level. A moment after the hit that lames or kills
    /// it, it goes down: the lift and the muscles let go, and it falls as a ragdoll.
    pub fn step(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        let (legs, dead) = (self.legs, self.dead);
        if dead || legs.iter().any(|&l| l >= LAME) {
            self.sunk += dt;
        }
        let up = self.sunk <= GIVE;
        let n = self.build.parts.len();
        let (mut force, mut torque) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);
        let fr: Vec<Transform> = self.doll.frames().collect();
        let (height, floor, level) = self.stand;
        let start = |k: usize| fr[k].translation - fr[k].rotation * Vec3::Y * self.build.parts[k].half;
        if up && LEGS.iter().any(|l| fr[l[2]].translation.z < floor + 0.35) {
            let m = self.doll.mass();
            let body = fr[0].translation;
            let (v, w) = self.doll.vel(0);
            let hold = (HEIGHT.0 * (height - body.z) - HEIGHT.1 * v.z).clamp(-0.3 * GRAVITY(), 0.3 * GRAVITY());
            for ends in [[1, 4], [7, 10]] {
                let f = Vec3::Z * (m * 0.5 * (GRAVITY() * CARRY + hold));
                let at = (start(ends[0]) + start(ends[1])) * 0.5;
                force[0] += f;
                torque[0] += (at - body).cross(f);
            }
            let tilt = (fr[0].rotation * level).cross(Vec3::Z);
            // scaled by its turning inertia, roughly its mass at 0.6 m
            torque[0] += (tilt * LEVEL.0 - (w - Vec3::Z * w.z) * LEVEL.1) * m * 0.36;
        }
        self.kept.force = force;
        self.kept.torque = torque;
        self.kept.muscles = SHAPES.iter().enumerate().map(|(k, s)| {
            let mut w = STIFF[s.muscle];
            if let Some(leg) = LEGS.iter().position(|l| l.contains(&k)) {
                w *= (1.0 - legs[leg] / LAME).clamp(0.08, 1.0);
            }
            if !up {
                w = LIMP;
            }
            Muscle { target: self.targets[k], stiffness: w, damping: 1.0 }
        }).collect();
        let drive: Drive = self.kept.drive();
        self.doll.step(&self.build, world, dt, drive, obstacles, others);
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
            self.doll.impulse(part, at, impulse);
        }
        if self.dead {
            return format!("{}: hit, dead already", SHAPES[part].name);
        }
        self.health -= damage;
        let mut s = format!("{} hit: health {:.0}", SHAPES[part].name, self.health.max(0.0));
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
