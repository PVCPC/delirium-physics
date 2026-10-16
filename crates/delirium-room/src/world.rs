use bevy::prelude::*;
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::prelude::{ColliderBuilder, ColliderHandle, PhysicsWorld, RigidBodyBuilder, RigidBodyHandle};

pub const KINDS: [(&str, f32, f32, f32); 3] = [("ball", 0.15, 2.0, 14.0), ("box", 0.2, 8.0, 9.0), ("crate", 0.45, 40.0, 0.0)];

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
}

impl Room {
    pub fn new(surfaces: Vec<(String, f32, f32)>) -> Room {
        let row = |n: &str| surfaces.iter().position(|s| s.0 == n).unwrap_or(0) as u8;
        let mut blocks = vec![(Vec3::new(0.0, 3.5, 0.5), Vec3::new(4.0, 0.15, 0.5), row("BRICK_WALL"))];
        for i in 0..10 {
            let top = 0.18 * (i + 1) as f32;
            blocks.push((Vec3::new(-3.75, -1.5 + 0.3 * i as f32 + 0.15, top * 0.5), Vec3::new(0.75, 0.15, top * 0.5), row("CONCRETE")));
        }
        blocks.push((Vec3::new(-3.75, 2.0, 0.9), Vec3::new(0.75, 0.5, 0.9), row("CONCRETE")));
        let mut w = [PhysicsWorld::new(), PhysicsWorld::new()];
        let mut fixed = [vec![], vec![]];
        for (i, w) in w.iter_mut().enumerate() {
            w.gravity = Vector::new(0.0, 0.0, -9.81);
            fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(Vector::new(0.0, 0.0, -0.5)), ColliderBuilder::cuboid(50.0, 50.0, 0.5).friction(1.0)).0);
            for &(c, h, _) in blocks.iter().filter(|_| i == 0) {
                fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(rv(c)), ColliderBuilder::cuboid(h.x, h.y, h.z).friction(1.0)).0);
            }
        }
        Room { w, fixed, parts: [vec![], vec![]], objs: vec![], blocks, surfaces, weight: 1.0 }
    }

    pub fn triangles(&self) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for &(c, h, row) in &self.blocks {
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
        let (_, s, kg, _) = KINDS[kind];
        let kg = kg * self.weight;
        let h = [0, 1].map(|i| {
            let b = RigidBodyBuilder::dynamic().translation(rv(from[i])).linvel(rv(vel)).angvel(Vector::new(1.5, -2.0, 0.7) * (kind > 0) as u8 as f32).ccd_enabled(true);
            let c = if kind == 0 { ColliderBuilder::ball(s) } else { ColliderBuilder::cuboid(s, s, s) };
            self.w[i].insert(b, c.mass(kg).friction(0.6).restitution(0.2)).0
        });
        self.objs.push(Obj { kind, h, hit: [false; 2] });
    }

    pub fn clear(&mut self) {
        for o in self.objs.drain(..) {
            for i in 0..2 {
                self.w[i].remove_body_with_colliders(o.h[i], true);
            }
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
                out.push((j, k, hit, if imp.dot(t.translation - at) < 0.0 { -imp } else { imp }));
            }
        }
        out
    }
}

