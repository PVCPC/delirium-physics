use bevy::prelude::*;
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::prelude::{ColliderBuilder, ColliderHandle, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle};

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
    w: [PhysicsWorld; 2],
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
}

pub const PAD_HALF: Vec3 = Vec3::new(1.5, 1.5, 0.25);

pub const PLANKS: usize = 12;
pub const SNAP_PLANK: usize = 5;
pub const BRIDGE_TOP: f32 = 8.0;
pub const PLANK_HALF: Vec3 = Vec3::new(0.8, 0.15, 0.03);
pub fn plank_y(i: usize) -> f32 {
    -0.2 - 0.4 * i as f32
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
            let far = plank_y(PLANKS - 1) - PLANK_HALF.y - 0.05;
            let mut v = vec![b([0.0, 3.0, 4.0], [2.0, 3.0, 4.0], concrete), b([0.0, far - 3.0, 4.0], [2.0, 3.0, 4.0], concrete)];
            v.extend((0..PLANKS).map(|i| b([0.0, plank_y(i), BRIDGE_TOP - PLANK_HALF.z], PLANK_HALF.to_array(), concrete)));
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
        let snap = match name {
            "bridge" => Some(blocks.len() - PLANKS + SNAP_PLANK),
            "trampoline" => Some(0),
            _ => None,
        };
        Room { w, fixed, parts: [vec![], vec![]], objs: vec![], blocks, surfaces, weight: 1.0, stage: name.to_string(), start, snap, snapped: None, fell: false, bounced: 9.0, }
    }

    fn blocks(row: &dyn Fn(&str) -> u8) -> Vec<(Vec3, Vec3, u8)> {
        let mut blocks = vec![(Vec3::new(0.0, 3.5, 0.5), Vec3::new(4.0, 0.15, 0.5), row("BRICK_WALL"))];
        for i in 0..10 {
            let top = 0.18 * (i + 1) as f32;
            blocks.push((Vec3::new(-3.75, -1.5 + 0.3 * i as f32 + 0.15, top * 0.5), Vec3::new(0.75, 0.15, top * 0.5), row("CONCRETE")));
        }
        blocks.push((Vec3::new(-3.75, 2.0, 0.9), Vec3::new(0.75, 0.5, 0.9), row("CONCRETE")));
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

    fn tris(&self, which: impl Iterator<Item = usize>) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for (c, h, row) in which.map(|i| self.blocks[i]) {
            let o = p.len() as u32;
            for i in 0..8 {
                p.push((c + h * Vec3::new([-1.0, 1.0][i & 1], [-1.0, 1.0][i >> 1 & 1], [-1.0, 1.0][i >> 2])).to_array());
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

    pub fn step(&mut self, i: usize, caps: &[(Transform, f32, f32)], dt: f32) -> Vec<(usize, usize, Vec3, Vec3)> {
        let w = &mut self.w[i];
        if self.parts[i].is_empty() {
            self.parts[i] = caps.iter().map(|(t, r, h)| w.insert(RigidBodyBuilder::kinematic_position_based().pose(pose(t)), ColliderBuilder::capsule_y(*h, *r))).collect();
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

