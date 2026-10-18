use bevy::prelude::*;
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::prelude::{ColliderBuilder, ColliderHandle, Group, ImpulseJointHandle, InteractionGroups, InteractionTestMode, PhysicsWorld,
                        RigidBodyBuilder, RigidBodyHandle, RopeJointBuilder};

pub const KINDS: [(&str, f32, f32, f32); 5] = [("ball", 0.15, 2.0, 14.0), ("box", 0.2, 8.0, 9.0), ("crate", 0.45, 40.0, 0.0),
    ("barrel", 0.3, 30.0, 0.0), ("wrecking ball", 0.5, 250.0, 0.0)];
const PART_KG: f32 = 40.0;
pub const BARREL_HALF: f32 = 0.45;

fn rv(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}
fn bv(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}
fn pose(t: &Transform) -> Pose {
    let q = t.rotation;
    Pose::from_parts(rv(t.translation), Rotation::from_xyzw(q.x, q.y, q.z, q.w))
}

pub struct Obj {
    pub kind: usize,
    h: [RigidBodyHandle; 2],
    pub hit: [bool; 2],
    pub anchor: Option<Vec3>,
}

pub struct Room {
    pub w: [PhysicsWorld; 2],
    #[allow(dead_code)]
    fixed: [Vec<RigidBodyHandle>; 2],
    parts: [Vec<(RigidBodyHandle, ColliderHandle)>; 2],
    pub objs: Vec<Obj>,
    pub blocks: Vec<(Vec3, Vec3, u8)>,
    pub surfaces: Vec<(String, f32, f32)>,
    pub weight: f32,
    pub stage: String,
    pub start: Vec3,
    pub snap: Option<usize>,
    pub snapped: Option<f32>,
    pub fell: bool,
    pub bounced: f32,
    pub bridge: Option<Bridge>,
    pub broke: Vec<usize>,
    pub push: Vec3,
    pub com_v: Vec3,
    #[cfg(feature = "portal")]
    pub holes: Vec<delirium::collision::Hole>,
}

#[cfg(feature = "portal")]
struct Holes<'a>(&'a [delirium::collision::Hole]);

#[cfg(feature = "portal")]
impl rapier3d::prelude::PhysicsHooks for Holes<'_> {
    fn modify_solver_contacts(&self, c: &mut rapier3d::prelude::ContactModificationContext) {
        if let rapier3d::prelude::ModifiableContacts::Rigid(m) = &mut c.contacts {
            m.solver_contacts.retain(|s| !self.0.iter().any(|h| h.has(bv(s.anchor1)) || h.has(bv(s.anchor2))));
        }
    }
}

pub const PAD_HALF: Vec3 = Vec3::new(1.5, 1.5, 0.25);

pub const PLANKS: usize = 12;
pub const BRIDGE_TOP: f32 = 8.0;
pub const PLANK_HALF: Vec3 = Vec3::new(0.8, 0.15, 0.03);
pub fn plank_y(i: usize) -> f32 {
    -0.2 - 0.4 * i as f32
}
pub const BRIDGE_FAR: f32 = -0.4 * PLANKS as f32;
pub const ROPE_X: f32 = 0.85;
pub const ROPES: usize = 2 * (2 * PLANKS + 1) + 2 * (PLANKS + 1) + 2 * PLANKS;
pub const POST_HALF: Vec3 = Vec3::new(0.05, 0.05, 0.6);
const HAND: f32 = 1.0;
const SLACK: (f32, f32) = (1.03, 1.06);
const LASH: (f32, f32) = (0.01, 0.06);
const LASH_ROTTEN: (f32, f32) = (0.04, 0.12);
const WIND_N: f32 = 25.0;
const PLANK_KG: f32 = 5.0;
const KNOT_KG: f32 = 0.7;
pub const HOMER_KG: f32 = 77.7;
const HOMER_N: f32 = HOMER_KG * 9.81;
const ROT_CHANCE: f32 = 0.3;
const ROT_HOLDS: (f32, f32) = (350.0, 800.0);
const PARTS: Group = Group::GROUP_2;

pub struct Plank {
    pub h: RigidBodyHandle,
    ends: [Option<[ImpulseJointHandle; 2]>; 2],
    pub rotten: bool,
    holds: f32,
    pub load: f32,
}

pub struct Bridge {
    pub planks: Vec<Plank>,
    t: f32,
    knots: [[Vec<RigidBodyHandle>; 2]; 2],
}

fn foot(i: usize, c: usize) -> usize {
    1 + 2 * i + c
}

impl Bridge {
    fn new(w: &mut PhysicsWorld, seed: u32) -> Bridge {
        let mut rand = delirium_data::rng::Rand::new(seed);
        let z = BRIDGE_TOP - PLANK_HALF.z;
        let groups = InteractionGroups::new(Group::GROUP_3, Group::ALL ^ PARTS, InteractionTestMode::And);
        let mut knots: [[Vec<RigidBodyHandle>; 2]; 2] = Default::default();
        for (r, zr) in [(0, z), (1, z + HAND)] {
            for (s, x) in [(0, -ROPE_X), (1, ROPE_X)] {
                let mut at: Vec<f32> = vec![0.0];
                at.extend((0..PLANKS).flat_map(|i| [plank_y(i) + PLANK_HALF.y, plank_y(i) - PLANK_HALF.y].into_iter().take(if r == 0 { 2 } else { 1 })));
                at.push(BRIDGE_FAR);
                let slack = rand.float_in_range(SLACK.0, SLACK.1);
                let v = &mut knots[r][s];
                for (k, &y) in at.iter().enumerate() {
                    let p = Vector::new(x, y, zr);
                    let b = if k == 0 || k == at.len() - 1 { RigidBodyBuilder::fixed() } else { RigidBodyBuilder::dynamic().additional_mass(KNOT_KG).linear_damping(0.3) };
                    v.push(w.insert_body(b.translation(p)));
                    if k > 0 {
                        w.insert_impulse_joint(v[k - 1], v[k], RopeJointBuilder::new((at[k - 1] - y).abs() * slack));
                    }
                }
            }
        }
        for s in 0..2 {
            for k in 1..=PLANKS {
                w.insert_impulse_joint(knots[1][s][k], knots[0][s][foot(k - 1, 0)], RopeJointBuilder::new(HAND));
            }
        }
        let planks = (0..PLANKS).map(|i| {
            let h = w.insert(RigidBodyBuilder::dynamic().translation(Vector::new(0.0, plank_y(i), z)).angular_damping(2.0),
                             ColliderBuilder::cuboid(PLANK_HALF.x, PLANK_HALF.y, PLANK_HALF.z).mass(PLANK_KG).friction(0.8).collision_groups(groups)).0;
            let rotten = i > 0 && i + 1 < PLANKS && rand.float_in_range(0.0, 1.0) < ROT_CHANCE;
            let lash = if rotten { LASH_ROTTEN } else { LASH };
            let ends = [0, 1].map(|s| Some([0, 1].map(|c| w.insert_impulse_joint(h, knots[0][s][foot(i, c)],
                RopeJointBuilder::new(rand.float_in_range(lash.0, lash.1)).local_anchor1(Vector::new([-ROPE_X, ROPE_X][s], [PLANK_HALF.y, -PLANK_HALF.y][c], 0.0))))));
            let holds = if rotten { rand.float_in_range(ROT_HOLDS.0, ROT_HOLDS.1) } else { f32::INFINITY };
            Plank { h, ends, rotten, holds, load: 0.0 }
        }).collect();
        w.integration_parameters.num_solver_iterations = 30;
        w.integration_parameters.dt = 1.0 / 60.0;
        for _ in 0..240 {
            w.step();
        }
        Bridge { planks, knots, t: 0.0 }
    }

    pub fn plank(&self, w: &PhysicsWorld, i: usize) -> Transform {
        let b = &w.bodies[self.planks[i].h];
        let q = b.rotation();
        Transform::from_translation(bv(b.translation())).with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w))
    }

    pub fn ropes(&self, w: &PhysicsWorld) -> Vec<(Vec3, Vec3)> {
        let at = |h: RigidBodyHandle| bv(w.bodies[h].translation());
        let mut v = vec![];
        for r in &self.knots {
            for s in r {
                v.extend(s.windows(2).map(|p| (at(p[0]), at(p[1]))));
            }
        }
        for s in 0..2 {
            v.extend((1..=PLANKS).map(|k| (at(self.knots[1][s][k]), at(self.knots[0][s][foot(k - 1, 0)]))));
        }
        v
    }

    pub fn net_wall(&self, w: &PhysicsWorld, s: usize, row: u8) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let at = |h: RigidBodyHandle| bv(w.bodies[h].translation()).to_array();
        let under = |k: usize| if k == 0 { 0 } else if k > PLANKS { 2 * PLANKS + 1 } else { foot(k - 1, 0) };
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for k in 0..=PLANKS {
            let o = p.len() as u32;
            p.extend([at(self.knots[0][s][under(k)]), at(self.knots[0][s][under(k + 1)]), at(self.knots[1][s][k + 1]), at(self.knots[1][s][k])]);
            t.extend([[o, o + 1, o + 2], [o, o + 2, o + 3]]);
            m.extend([row, row]);
        }
        (p, t, m)
    }

    pub fn net(&self, w: &PhysicsWorld) -> Vec<(Vec3, Vec3)> {
        const ALONG: usize = 3;
        const UP: usize = 7;
        let at = |h: RigidBodyHandle| bv(w.bodies[h].translation());
        let mut v = vec![];
        for s in 0..2 {
            let under = |k: usize| if k == 0 { 0 } else if k > PLANKS { 2 * PLANKS + 1 } else { foot(k - 1, 0) };
            for k in 0..=PLANKS {
                let (f0, f1, h0, h1) = (at(self.knots[0][s][under(k)]), at(self.knots[0][s][under(k + 1)]), at(self.knots[1][s][k]), at(self.knots[1][s][k + 1]));
                let p = |a: usize, b: usize| {
                    let u = a as f32 / ALONG as f32;
                    f0.lerp(f1, u).lerp(h0.lerp(h1, u), b as f32 / UP as f32)
                };
                for a in 0..ALONG {
                    for b in 0..UP {
                        v.extend([(p(a, b), p(a + 1, b + 1)), (p(a + 1, b), p(a, b + 1))]);
                    }
                }
            }
        }
        v
    }

    fn load(&mut self, w: &mut PhysicsWorld, caps: &[(Transform, f32, f32)], push: Vec3, dt: f32) -> Vec<usize> {
        let mut touch = vec![];
        for (i, p) in self.planks.iter().enumerate() {
            let b = &w.bodies[p.h];
            let q = b.rotation();
            let (c, q) = (bv(b.translation()), Quat::from_xyzw(q.x, q.y, q.z, q.w));
            for (t, r, h) in caps {
                for e in [-1.0, 1.0] {
                    let l = q.inverse() * (t.translation + t.rotation * Vec3::Y * (h * e) - c);
                    if l.x.abs() < PLANK_HALF.x + r && l.y.abs() < PLANK_HALF.y + r && (-PLANK_HALF.z - r..PLANK_HALF.z + r + 0.05).contains(&l.z) {
                        touch.push((i, c + q * l.with_z(PLANK_HALF.z).clamp(-PLANK_HALF, PLANK_HALF)));
                    }
                }
            }
        }
        for p in &mut self.planks {
            p.load = 0.0;
        }
        let each = push / touch.len().max(1) as f32;
        for &(i, at) in &touch {
            self.planks[i].load += each.length();
            w.bodies[self.planks[i].h].apply_impulse_at_point(rv(each * dt), rv(at), true);
        }
        self.t += dt;
        let wind = WIND_N * (0.7 * self.t).sin() * (0.23 * self.t + 1.0).sin();
        for p in &self.planks {
            w.bodies[p.h].apply_impulse(rv(Vec3::X * wind * dt), true);
        }
        let broke: Vec<usize> = (0..self.planks.len()).filter(|&i| self.planks[i].load > self.planks[i].holds).collect();
        for &i in &broke {
            self.fall(w, i);
        }
        broke
    }

    pub fn fall(&mut self, w: &mut PhysicsWorld, i: usize) {
        let p = &mut self.planks[i];
        for j in p.ends.iter_mut().filter_map(Option::take).flatten() {
            w.remove_impulse_joint(j);
        }
        p.holds = f32::INFINITY;
    }
}

fn stage(name: &str, brick: u8, concrete: u8) -> Option<(Vec<(Vec3, Vec3, u8)>, Vec3)> {
    let b = |c: [f32; 3], h: [f32; 3], r| (Vec3::from(c), Vec3::from(h), r);
    Some(match name {
        "bigstairs" => {
            let mut v = vec![b([0.0, -0.45, 3.0], [1.5, 1.05, 3.0], concrete)];
            v.extend([-1.6, 1.6].map(|x| b([x, -0.45, 3.5], [0.1, 1.05, 3.5], brick)));
            for i in 0..40 {
                let (top, y) = (6.0 - 0.15 * (i + 1) as f32, 0.75 + 0.3 * i as f32);
                v.push(b([0.0, y, top * 0.5], [1.5, 0.15, top * 0.5], concrete));
                v.extend([-1.6, 1.6].map(|x| b([x, y, (top + 1.0) * 0.5], [0.1, 0.15, (top + 1.0) * 0.5], brick)));
            }
            (v, Vec3::Z * 6.0)
        }
        "rooftop" => (vec![b([0.0, -2.25, 2.0], [3.0, 2.75, 2.0], concrete)], Vec3::Z * 4.0),
        "lowwall" => (vec![b([0.0, 0.45, 0.45], [4.0, 0.15, 0.45], brick)], Vec3::ZERO),
        "kerb" => (vec![b([0.0, 0.45, 0.22], [4.0, 0.15, 0.22], brick)], Vec3::ZERO),
        "open" => (vec![], Vec3::ZERO),
        "rail" => (vec![b([0.0, -0.5, 1.18], [1.5, 0.03, 0.03], concrete), b([-1.5, -0.5, 0.6], [0.03, 0.03, 0.6], concrete),
                        b([1.5, -0.5, 0.6], [0.03, 0.03, 0.6], concrete)], Vec3::ZERO),
        "street" => {
            let mut v = vec![b([0.0, 6.0, 0.075], [30.0, 2.0, 0.075], concrete), b([0.0, -6.0, 0.075], [30.0, 2.0, 0.075], concrete)];
            v.extend((0..8).map(|i| b([0.0, -3.5 + i as f32, 0.003], [1.5, 0.25, 0.003], concrete)));
            (v, Vec3::ZERO)
        }
        "bridge" => {
            let mut v = vec![b([0.0, 3.0, 4.0], [2.0, 3.0, 4.0], concrete), b([0.0, BRIDGE_FAR - 3.0, 4.0], [2.0, 3.0, 4.0], concrete)];
            for (y, x) in [(POST_HALF.y, -1.0), (POST_HALF.y, 1.0), (BRIDGE_FAR - POST_HALF.y, -1.0), (BRIDGE_FAR - POST_HALF.y, 1.0)] {
                v.push(b([x * (ROPE_X + POST_HALF.x), y, BRIDGE_TOP + POST_HALF.z], POST_HALF.to_array(), concrete));
            }
            (v, Vec3::new(0.0, 0.8, BRIDGE_TOP))
        }
        "cab" => (vec![], Vec3::ZERO),
        "trampoline" => (vec![b([0.0, 0.0, PAD_HALF.z], PAD_HALF.to_array(), concrete)], Vec3::new(0.0, 0.0, 2.0 * PAD_HALF.z)),
        _ => return None,
    })
}

impl Room {
    pub fn new(surfaces: Vec<(String, f32, f32)>, name: &str) -> Room {
        let row = |n: &str| surfaces.iter().position(|s| s.0 == n).unwrap_or(0) as u8;
        let (blocks, start) = stage(name, row("BRICK_WALL"), row("CONCRETE")).unwrap_or_else(|| (Room::blocks(&row), Vec3::ZERO));
        let mut w = [PhysicsWorld::new(), PhysicsWorld::new()];
        let mut fixed = [vec![], vec![]];
        for (i, w) in w.iter_mut().enumerate() {
            w.gravity = Vector::new(0.0, 0.0, -9.81);
            fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(Vector::new(0.0, 0.0, -0.5)), ColliderBuilder::cuboid(50.0, 50.0, 0.5).friction(1.0)).0);
            for &(c, h, _) in blocks.iter().filter(|_| i == 0) {
                fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(rv(c)), ColliderBuilder::cuboid(h.x, h.y, h.z).friction(1.0)).0);
            }
        }
        let snap = (name == "trampoline").then_some(0);
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos());
        let bridge = (name == "bridge").then(|| Bridge::new(&mut w[0], seed));
        Room { w, fixed, parts: [vec![], vec![]], objs: vec![], blocks, surfaces, weight: 1.0, stage: name.to_string(), start, snap, snapped: None, fell: false, bounced: 9.0,
               bridge, broke: vec![], push: Vec3::NEG_Z * HOMER_N, com_v: Vec3::ZERO, #[cfg(feature = "portal")] holes: vec![] }
    }

    fn blocks(row: &dyn Fn(&str) -> u8) -> Vec<(Vec3, Vec3, u8)> {
        let mut blocks = vec![(Vec3::new(0.0, 3.5, 0.5), Vec3::new(4.0, 0.15, 0.5), row("BRICK_WALL"))];
        for i in 0..10 {
            let top = 0.18 * (i + 1) as f32;
            blocks.push((Vec3::new(-3.75, -1.5 + 0.3 * i as f32 + 0.15, top * 0.5), Vec3::new(0.75, 0.15, top * 0.5), row("CONCRETE")));
        }
        blocks.push((Vec3::new(-3.75, 2.0, 0.9), Vec3::new(0.75, 0.5, 0.9), row("CONCRETE")));
        #[cfg(feature = "portal")]
        blocks.extend([(Vec3::new(10.0, 0.0, 3.0), Vec3::new(0.15, 4.0, 3.0), row("CONCRETE")),
                       (Vec3::new(6.0, 3.0, 5.0), Vec3::new(1.5, 1.5, 0.1), row("CONCRETE"))]);
        blocks
    }

    pub fn scene(&mut self, v: [&[Vec3]; 2], tris: &[[u32; 3]]) {
        for i in 0..2 {
            for h in std::mem::take(&mut self.fixed[i]) {
                self.w[i].remove_body_with_colliders(h, true);
            }
            if let Ok(c) = ColliderBuilder::trimesh(v[i].iter().map(|&p| rv(p)).collect(), tris.to_vec()) {
                self.fixed[i].push(self.w[i].insert(RigidBodyBuilder::fixed(), c.friction(1.0)).0);
            }
        }
        self.blocks.clear();
    }

    pub fn triangles(&self) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        self.tris((0..self.blocks.len()).filter(|&i| Some(i) != self.snap))
    }

    pub fn block_triangles(&self, i: usize) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        self.tris(std::iter::once(i))
    }

    pub fn plank_triangles(&self, i: usize) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let b = self.bridge.as_ref().expect("the bridge");
        let row = self.surfaces.iter().position(|s| s.0.starts_with("WOOD")).or_else(|| self.surfaces.iter().position(|s| s.0 == "CONCRETE")).unwrap_or(0) as u8;
        Self::boxes(std::iter::once((b.plank(&self.w[0], i), PLANK_HALF, row)))
    }

    fn tris(&self, which: impl Iterator<Item = usize>) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        Self::boxes(which.map(|i| self.blocks[i]).map(|(c, h, row)| (Transform::from_translation(c), h, row)))
    }

    fn boxes(which: impl Iterator<Item = (Transform, Vec3, u8)>) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for (c, h, row) in which {
            let o = p.len() as u32;
            for i in 0..8 {
                p.push(c.transform_point(h * Vec3::new([-1.0, 1.0][i & 1], [-1.0, 1.0][i >> 1 & 1], [-1.0, 1.0][i >> 2])).to_array());
            }
            for f in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
                t.extend([[o + f[0], o + f[1], o + f[2]], [o + f[0], o + f[2], o + f[3]]]);
                m.extend([row, row]);
            }
        }
        (p, t, m)
    }

    pub fn throw(&mut self, kind: usize, from: [Vec3; 2], vel: Vec3) {
        self.add(kind, from, vel, kind > 0, None);
    }

    pub fn place(&mut self, kind: usize, from: [Vec3; 2], anchor: Option<Vec3>) {
        self.add(kind, from, Vec3::ZERO, false, anchor);
    }

    fn add(&mut self, kind: usize, from: [Vec3; 2], vel: Vec3, spin: bool, anchor: Option<Vec3>) {
        let (_, s, kg, _) = KINDS[kind];
        let kg = kg * self.weight;
        let h = [0, 1].map(|i| {
            let mut b = RigidBodyBuilder::dynamic().translation(rv(from[i])).linvel(rv(vel)).angvel(Vector::new(1.5, -2.0, 0.7) * spin as u8 as f32).ccd_enabled(true);
            let c = match kind {
                0 | 4 => ColliderBuilder::ball(s),
                3 => {
                    b = b.rotation(Vector::X * std::f32::consts::FRAC_PI_2);
                    ColliderBuilder::cylinder(BARREL_HALF, s)
                }
                _ => ColliderBuilder::cuboid(s, s, s),
            };
            #[cfg(feature = "portal")]
            let c = c.active_hooks(rapier3d::prelude::ActiveHooks::MODIFY_SOLVER_CONTACTS);
            let w = &mut self.w[i];
            let h = w.insert(b, c.mass(kg).friction(0.6).restitution(0.2)).0;
            if let Some(a) = anchor {
                let a = from[i] + (a - from[0]);
                let pin = w.insert_body(RigidBodyBuilder::fixed().translation(rv(a)));
                w.insert_impulse_joint(pin, h, rapier3d::prelude::RopeJointBuilder::new(a.distance(from[i])));
            }
            h
        });
        self.objs.push(Obj { kind, h, hit: [false; 2], anchor });
    }

    pub fn blast(&mut self, at: [Vec3; 2], dv: f32, r: f32) {
        for o in &self.objs {
            for i in 0..2 {
                let b = &mut self.w[i].bodies[o.h[i]];
                let d = bv(b.translation()) - at[i];
                let f = 1.0 - d.length() / r;
                if f > 0.0 {
                    let push = (d.normalize_or(Vec3::Z) + Vec3::Z * 0.6).normalize() * dv * f * b.mass();
                    b.apply_impulse(rv(push), true);
                    b.apply_torque_impulse(rv(Vec3::new(d.y, -d.x, 0.3) * b.mass() * f), true);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        for o in self.objs.drain(..) {
            for i in 0..2 {
                let w = &mut self.w[i];
                let pins: Vec<_> = w.impulse_joints.attached_joints(o.h[i]).map(|(a, b, _, _)| if a == o.h[i] { b } else { a }).collect();
                w.remove_body_with_colliders(o.h[i], true);
                for p in pins {
                    w.remove_body_with_colliders(p, true);
                }
            }
        }
    }

    pub fn steer(&mut self, j: usize, at: Vec3) {
        let h = self.objs[j].h[0];
        let w = &mut self.w[0];
        let pins: Vec<_> = w.impulse_joints.attached_joints(h).map(|(a, b, _, _)| if a == h { b } else { a }).collect();
        for p in pins {
            let b = &mut w.bodies[p];
            let z = bv(b.translation()).z;
            b.set_translation(rv(at.with_z(z)), true);
        }
    }

    pub fn obj(&self, i: usize, j: usize) -> (Transform, Vec3) {
        let b = &self.w[i].bodies[self.objs[j].h[i]];
        let q = b.rotation();
        (Transform::from_translation(bv(b.translation())).with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w)), bv(b.linvel()))
    }

    #[cfg(feature = "portal")]
    pub fn obj_motion(&self, j: usize) -> Option<(Vec3, Vec3, f32)> {
        let o = self.objs.get(j).filter(|o| o.anchor.is_none())?;
        let w = &self.w[0];
        let b = &w.bodies[o.h[0]];
        let r = b.colliders().iter().map(|&c| w.colliders[c].shape().compute_local_bounding_sphere().radius()).fold(0.0, f32::max);
        Some((bv(b.linvel()), bv(b.angvel()), r))
    }

    #[cfg(feature = "portal")]
    pub fn obj_set(&mut self, j: usize, t: Transform, v: Vec3, w: Vec3) {
        for i in 0..2 {
            let b = &mut self.w[i].bodies[self.objs[j].h[i]];
            b.set_position(pose(&t), true);
            b.set_linvel(rv(v), true);
            b.set_angvel(rv(w), true);
        }
    }

    pub fn step(&mut self, i: usize, caps: &[(Transform, f32, f32)], dt: f32) -> Vec<(usize, usize, Vec3, Vec3)> {
        if let (0, Some(b)) = (i, self.bridge.as_mut()) {
            self.broke.extend(b.load(&mut self.w[0], caps, self.push, dt));
        }
        let w = &mut self.w[i];
        if self.parts[i].is_empty() {
            let groups = InteractionGroups::new(PARTS, Group::ALL, InteractionTestMode::And);
            self.parts[i] = caps.iter().map(|(t, r, h)| w.insert(RigidBodyBuilder::kinematic_position_based().pose(pose(t)), ColliderBuilder::capsule_y(*h, *r).collision_groups(groups))).collect();
        }
        for ((b, _), (t, _, _)) in self.parts[i].iter().zip(caps) {
            let body = &mut w.bodies[*b];
            if bv(body.translation()).distance(t.translation) > 0.3 {
                body.set_position(pose(t), true);
            } else {
                body.set_next_kinematic_position(pose(t));
            }
        }
        w.integration_parameters.dt = dt;
        #[cfg(feature = "portal")]
        w.step_with_events(&Holes(&self.holes), &());
        #[cfg(not(feature = "portal"))]
        w.step();
        let mut out = vec![];
        for (k, ((_, c), (t, r, h))) in self.parts[i].iter().zip(caps).enumerate() {
            for pair in w.narrow_phase.contact_pairs_with(*c) {
                let other = if pair.collider1 == *c { pair.collider2 } else { pair.collider1 };
                let Some(j) = w.colliders.get(other).and_then(|o| o.parent()).and_then(|b| self.objs.iter().position(|o| o.h[i] == b)) else { continue };
                let imp = bv(pair.total_impulse());
                if imp.length_squared() < 1e-8 {
                    continue;
                }
                let at = bv(w.bodies[self.objs[j].h[i]].translation());
                let axis = t.rotation * Vec3::Y * *h;
                let on = t.translation + (at - t.translation).project_onto(axis).clamp_length_max(*h);
                let hit = on + (at - on).normalize_or_zero() * *r;
                let imp = if imp.dot(t.translation - at) < 0.0 { -imp } else { imp };
                let obj = &mut w.bodies[self.objs[j].h[i]];
                let m = obj.mass();
                let mine = imp * (PART_KG / (PART_KG + m));
                obj.apply_impulse(rv(imp - mine), true);
                out.push((j, k, hit, mine));
            }
        }
        out
    }
}

