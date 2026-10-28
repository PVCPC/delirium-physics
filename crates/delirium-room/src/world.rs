use bevy::prelude::*;
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::prelude::{ColliderBuilder, ColliderHandle, SharedShape, Group, ImpulseJointHandle, InteractionGroups, InteractionTestMode, PhysicsWorld,
                        FixedJointBuilder, RigidBodyBuilder, RigidBodyHandle, RopeJointBuilder, SphericalJointBuilder};

#[allow(non_snake_case)]
#[inline]
pub fn FLOOR_HALF() -> f32 { delirium_data::kv!(28) }

#[allow(non_snake_case)]
#[inline]
pub fn POOL() -> &'static [Vec2; 2] { static V: std::sync::OnceLock<[Vec2; 2]> = std::sync::OnceLock::new(); let f = || [Vec2::new(-delirium_data::kv!(5), delirium_data::kv!(60)), Vec2::new(delirium_data::kv!(5), delirium_data::kv!(35))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn POOL_DEEP() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
pub fn WATER() -> f32 { -delirium_data::kv!(73) }

pub fn floor_rects(pool: bool, half: f32) -> Vec<(Vec2, Vec2)> {
    let ([a, b], h) = ((*POOL()), half);
    if !pool {
        return vec![(Vec2::splat(-h), Vec2::splat(h))];
    }
    vec![(Vec2::new(-h, -h), Vec2::new(h, a.y)), (Vec2::new(-h, b.y), Vec2::new(h, h)),
         (Vec2::new(-h, a.y), Vec2::new(a.x, b.y)), (Vec2::new(b.x, a.y), Vec2::new(h, b.y))]
}

pub fn floor_tris(pool: bool, half: f32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let (mut p, mut t) = (vec![], vec![]);
    for (lo, hi) in floor_rects(pool, half) {
        for x in lo.x as i32..hi.x as i32 {
            for y in lo.y as i32..hi.y as i32 {
                let (a, b, k) = (x as f32, y as f32, p.len() as u32);
                p.extend([[a, b, 0.0], [a + 1.0, b, 0.0], [a + 1.0, b + 1.0, 0.0], [a, b + 1.0, 0.0]]);
                t.extend([[k, k + 1, k + 2], [k, k + 2, k + delirium_data::kv!(0 int)]]);
            }
        }
    }
    (p, t)
}

pub fn over_pool(p: Vec3) -> bool {
    ((*POOL())[0].x..(*POOL())[1].x).contains(&p.x) && ((*POOL())[0].y..(*POOL())[1].y).contains(&p.y)
}

#[allow(non_snake_case)]
#[inline]
pub fn KINDS() -> &'static [(&'static str, f32, f32, f32); 5] { static V: std::sync::OnceLock<[(&'static str, f32, f32, f32); 5]> = std::sync::OnceLock::new(); let f = || [("ball", delirium_data::kv!(9), 2.0, delirium_data::kv!(117)), ("box", delirium_data::kv!(57), delirium_data::kv!(61), delirium_data::kv!(127)), ("crate", delirium_data::kv!(10), delirium_data::kv!(11), 0.0),
    ("barrel", delirium_data::kv!(24), delirium_data::kv!(80), 0.0), ("wrecking ball", 0.5, delirium_data::kv!(724), 0.0)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn PART_KG() -> f32 { delirium_data::kv!(11) }
#[allow(non_snake_case)]
#[inline]
pub fn BARREL_HALF() -> f32 { delirium_data::kv!(10) }

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
    pub tilted: Vec<(Transform, Vec3, u8)>,
    pub surfaces: Vec<(String, f32, f32)>,
    pub weight: f32,
    pub stage: String,
    pub pool: bool,
    pub start: Vec3,
    pub snap: Option<usize>,
    pub snapped: Option<f32>,
    pub fell: bool,
    pub bounced: f32,
    pub shook: f32,
    pub bridge: Option<Bridge>,
    pub broke: Vec<usize>,
    pub push: Vec3,
    pub com_v: Vec3,
    pub com_a: Vec3,
    #[cfg(feature = "portal")]
    pub holes: Vec<delirium::collision::Hole>,
    pub flow: Vec<Option<Vec4>>,
    pub faucet: Vec<RigidBodyHandle>,
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

#[allow(non_snake_case)]
#[inline]
pub fn PAD_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || Vec3::new(delirium_data::kv!(12), delirium_data::kv!(12), delirium_data::kv!(17)); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
pub fn PLAT_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || Vec3::new(2.0, 2.0, delirium_data::kv!(17)); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub fn plat(stage: &str, t: f32) -> Option<(Vec3, Quat)> {
    let s = |f: f32, p: f32| (std::f32::consts::TAU * f * t + p).sin();
    let go = |over: f32| ((t - 2.0) / over).clamp(0.0, 1.0);
    Some(match stage {
        "quake" => {
            let g = go(delirium_data::kv!(27));
            (Vec3::new(delirium_data::kv!(185) * s(delirium_data::kv!(975), 0.0) + delirium_data::kv!(64) * s(delirium_data::kv!(976), 1.0), delirium_data::kv!(121) * s(delirium_data::kv!(532), 2.0) + delirium_data::kv!(64) * s(delirium_data::kv!(977), 0.5), delirium_data::kv!(122) * s(delirium_data::kv!(528), delirium_data::kv!(24))) * g + Vec3::Z * 0.5,
             Quat::from_euler(EulerRot::XYZ, g * delirium_data::kv!(122) * s(delirium_data::kv!(494), delirium_data::kv!(70)), g * delirium_data::kv!(122) * s(delirium_data::kv!(178), delirium_data::kv!(170)), 0.0))
        }
        "bull" => {
            let g = go(delirium_data::kv!(104));
            (Vec3::Z * (delirium_data::kv!(190) + g * delirium_data::kv!(131) * s(delirium_data::kv!(190), 0.5).abs()),
             Quat::from_euler(EulerRot::ZXY, g * delirium_data::kv!(20) * s(delirium_data::kv!(17), 0.0), g * delirium_data::kv!(17) * s(delirium_data::kv!(110), 2.0), g * delirium_data::kv!(17) * (s(delirium_data::kv!(115), 0.0) + delirium_data::kv!(106) * s(delirium_data::kv!(134), 1.0))))
        }
        _ => return None,
    })
}

pub const PLANKS: usize = 12;
#[allow(non_snake_case)]
#[inline]
pub fn BRIDGE_TOP() -> f32 { delirium_data::kv!(61) }
#[allow(non_snake_case)]
#[inline]
pub fn PLANK_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || Vec3::new(delirium_data::kv!(20), delirium_data::kv!(9), delirium_data::kv!(122)); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn PIECE_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || Vec3::new(delirium_data::kv!(106), delirium_data::kv!(9), delirium_data::kv!(122)); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn TEETH() -> usize { delirium_data::kv!(23 int) }
#[allow(non_snake_case)]
#[inline]
fn SPLINTER() -> f32 { delirium_data::kv!(136) }
#[allow(non_snake_case)]
#[inline]
fn SWING_DAMP() -> f32 { delirium_data::kv!(119) }
pub fn plank_y(i: usize) -> f32 {
    -delirium_data::kv!(57) - delirium_data::kv!(106) * i as f32
}
#[allow(non_snake_case)]
#[inline]
pub fn BRIDGE_FAR() -> f32 { -delirium_data::kv!(106) * PLANKS as f32 }
#[allow(non_snake_case)]
#[inline]
pub fn ROPE_X() -> f32 { delirium_data::kv!(115) }
#[allow(non_snake_case)]
#[inline]
pub fn ROPES() -> usize { 2 * (2 * PLANKS + 1) + 2 * (PLANKS + 1) + 2 * PLANKS + 2 * (*SIDE_ROPES()).len() * (PLANKS + 1) }
#[allow(non_snake_case)]
#[inline]
pub fn SIDE_ROPES() -> &'static [f32; 2] { static V: std::sync::OnceLock<[f32; 2]> = std::sync::OnceLock::new(); let f = || [1.0 / delirium_data::kv!(5), 2.0 / delirium_data::kv!(5)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn ROPE_REACH() -> f32 { delirium_data::kv!(73) }
#[allow(non_snake_case)]
#[inline]
pub fn POST_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || Vec3::new(delirium_data::kv!(64), delirium_data::kv!(64), delirium_data::kv!(110)); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn HAND() -> f32 { delirium_data::kv!(229) }
#[allow(non_snake_case)]
#[inline]
fn SLACK() -> (f32, f32) { (delirium_data::kv!(978), delirium_data::kv!(979)) }
#[allow(non_snake_case)]
#[inline]
fn WIND_N() -> f32 { delirium_data::kv!(35) }
#[allow(non_snake_case)]
#[inline]
fn PLANK_KG() -> f32 { delirium_data::kv!(21) }
#[allow(non_snake_case)]
#[inline]
fn KNOT_KG() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
pub fn HOMER_KG() -> f32 { delirium_data::kv!(980) }
#[allow(non_snake_case)]
#[inline]
pub fn HOMER_N() -> f32 { HOMER_KG() * delirium_data::kv!(105) }
#[allow(non_snake_case)]
#[inline]
fn ROTTEN() -> (usize, usize) { (2, delirium_data::kv!(1 int)) }
#[allow(non_snake_case)]
#[inline]
fn ROT_HOLDS() -> (f32, f32) { (delirium_data::kv!(724), delirium_data::kv!(725)) }
const PARTS: Group = Group::GROUP_2;

pub struct Plank {
    pub h: [RigidBodyHandle; 2],
    seam: Option<ImpulseJointHandle>,
    pub rotten: bool,
    pub teeth: Vec<f32>,
    holds: f32,
    pub load: f32,
    f: Vec3,
    at: Vec3,
}

pub struct Bridge {
    pub planks: Vec<Plank>,
    t: f32,
    knots: [[Vec<RigidBodyHandle>; 2]; 2],
    pub seed: u32,
}

fn foot(i: usize, c: usize) -> usize {
    1 + 2 * i + c
}

impl Bridge {
    fn new(w: &mut PhysicsWorld, seed: u32) -> Bridge {
        let mut rand = delirium_data::rng::Rand::new(seed);
        let z = BRIDGE_TOP() - (*PLANK_HALF()).z;
        let groups = InteractionGroups::new(Group::GROUP_3, Group::ALL ^ PARTS, InteractionTestMode::And);
        let mut knots: [[Vec<RigidBodyHandle>; 2]; 2] = Default::default();
        for (r, zr) in [(0, z), (1, z + HAND())] {
            for (s, x) in [(0, -ROPE_X()), (1, ROPE_X())] {
                let mut at: Vec<f32> = vec![0.0];
                at.extend((0..PLANKS).flat_map(|i| [plank_y(i) + (*PLANK_HALF()).y, plank_y(i) - (*PLANK_HALF()).y].into_iter().take(if r == 0 { 2 } else { 1 })));
                at.push(BRIDGE_FAR());
                let slack = rand.float_in_range(SLACK().0, SLACK().1);
                let v = &mut knots[r][s];
                for (k, &y) in at.iter().enumerate() {
                    let p = Vector::new(x, y, zr);
                    let b = if k == 0 || k == at.len() - 1 { RigidBodyBuilder::fixed() } else { RigidBodyBuilder::dynamic().additional_mass(KNOT_KG()).linear_damping(delirium_data::kv!(12)) };
                    v.push(w.insert_body(b.translation(p)));
                    if k > 0 {
                        w.insert_impulse_joint(v[k - 1], v[k], RopeJointBuilder::new((at[k - 1] - y).abs() * slack));
                    }
                }
            }
        }
        for s in 0..2 {
            for k in 1..=PLANKS {
                w.insert_impulse_joint(knots[1][s][k], knots[0][s][foot(k - 1, 0)], RopeJointBuilder::new(HAND()));
            }
        }
        let mut rot = [false; PLANKS];
        let count = ROTTEN().0 + (rand.float_in_range(0.0, (ROTTEN().1 - ROTTEN().0 + 1) as f32) as usize).min(ROTTEN().1 - ROTTEN().0);
        while rot.iter().filter(|r| **r).count() < count {
            rot[1 + (rand.float_in_range(0.0, (PLANKS - 2) as f32) as usize).min(PLANKS - delirium_data::kv!(0 int))] = true;
        }
        let planks: Vec<Plank> = (0..PLANKS).map(|i| {
            let h = [0, 1].map(|s| {
                let x = [-1.0, 1.0][s];
                let h = w.insert(RigidBodyBuilder::dynamic().translation(Vector::new(0.0, plank_y(i), z)).linear_damping(delirium_data::kv!(20)).angular_damping(delirium_data::kv!(61)),
                                 ColliderBuilder::cuboid((*PIECE_HALF()).x, (*PIECE_HALF()).y, (*PIECE_HALF()).z).translation(Vector::new(x * (*PIECE_HALF()).x, 0.0, 0.0))
                                     .mass(PLANK_KG() / 2.0).friction(delirium_data::kv!(20)).collision_groups(groups)).0;
                for c in 0..2 {
                    w.insert_impulse_joint(h, knots[0][s][foot(i, c)], SphericalJointBuilder::new().local_anchor1(Vector::new(x * ROPE_X(), [(*PLANK_HALF()).y, -(*PLANK_HALF()).y][c], 0.0)));
                }
                h
            });
            let seam = Some(w.insert_impulse_joint(h[0], h[1], FixedJointBuilder::new().contacts_enabled(false)));
            let rotten = rot[i];
            let teeth = (0..=TEETH()).map(|k| if rotten { [-1.0, 1.0][k % 2] * rand.float_in_range(delirium_data::kv!(24), 1.0) * SPLINTER() } else { 0.0 }).collect();
            let holds = if rotten { rand.float_in_range(ROT_HOLDS().0, ROT_HOLDS().1) } else { f32::INFINITY };
            Plank { h, seam, rotten, teeth, holds, load: 0.0, f: Vec3::ZERO, at: Vec3::ZERO }
        }).collect();
        w.integration_parameters.num_solver_iterations = delirium_data::kv!(274 int);
        w.integration_parameters.dt = 1.0 / delirium_data::kv!(193);
        for _ in 0..delirium_data::kv!(293 int) {
            w.step();
        }
        Bridge { planks, knots, t: 0.0, seed }
    }

    pub fn plank(&self, w: &PhysicsWorld, i: usize) -> Transform {
        self.half(w, i, 0)
    }

    pub fn half(&self, w: &PhysicsWorld, i: usize, s: usize) -> Transform {
        let b = &w.bodies[self.planks[i].h[s]];
        let q = b.rotation();
        Transform::from_translation(bv(b.translation())).with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w))
    }

    pub fn piece(&self, w: &PhysicsWorld, i: usize, s: usize) -> Transform {
        self.half(w, i, s) * Transform::from_translation(Vec3::X * [-1.0, 1.0][s] * (*PIECE_HALF()).x)
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
        for s in 0..2 {
            for f in (*SIDE_ROPES()) {
                v.extend((0..=PLANKS).map(|k| self.side(w, s, k, f)));
            }
        }
        v
    }

    fn side(&self, w: &PhysicsWorld, s: usize, k: usize, f: f32) -> (Vec3, Vec3) {
        let at = |h: RigidBodyHandle| bv(w.bodies[h].translation());
        let under = |k: usize| if k == 0 { 0 } else if k > PLANKS { 2 * PLANKS + 1 } else { foot(k - 1, 0) };
        let [f0, f1, h0, h1] = [at(self.knots[0][s][under(k)]), at(self.knots[0][s][under(k + 1)]), at(self.knots[1][s][k]), at(self.knots[1][s][k + 1])];
        (f0.lerp(h0, f), f1.lerp(h1, f))
    }

    pub fn net_wall(&self, w: &PhysicsWorld, s: usize, row: u8) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for f in [0.0].into_iter().chain((*SIDE_ROPES())).chain([1.0]) {
            for k in 0..=PLANKS {
                let (a, b) = self.side(w, s, k, f);
                let o = p.len() as u32;
                let up = Vec3::Z * ROPE_REACH();
                p.extend([a - up, b - up, b + up, a + up].map(|v| v.to_array()));
                t.extend([[o, o + 1, o + 2], [o, o + 2, o + delirium_data::kv!(0 int)]]);
                m.extend([row, row]);
            }
        }
        (p, t, m)
    }

    fn load(&mut self, w: &mut PhysicsWorld, caps: &[(Transform, f32, f32)], push: Vec3, dt: f32) -> Vec<usize> {
        let mut touch = vec![];
        let pieces: Vec<[Transform; 2]> = (0..self.planks.len()).map(|i| [0, 1].map(|s| self.piece(w, i, s))).collect();
        for (i, ps) in pieces.iter().enumerate() {
            for (c, q) in ps.iter().map(|t| (t.translation, t.rotation)) {
                for (t, r, h) in caps {
                    for e in [-1.0, 1.0] {
                        let l = q.inverse() * (t.translation + t.rotation * Vec3::Y * (h * e) - c);
                        if l.x.abs() < (*PIECE_HALF()).x + r && l.y.abs() < (*PIECE_HALF()).y + r && (-(*PIECE_HALF()).z - r..(*PIECE_HALF()).z + r + delirium_data::kv!(64)).contains(&l.z) {
                            touch.push((i, c + q * l.with_z((*PIECE_HALF()).z).clamp(-(*PIECE_HALF()), (*PIECE_HALF()))));
                        }
                    }
                }
            }
        }
        let each = push / touch.len().max(1) as f32;
        let mut want = vec![(Vec3::ZERO, Vec3::ZERO, 0.0); self.planks.len()];
        for &(i, at) in &touch {
            want[i] = (want[i].0 + each, want[i].1 + at, want[i].2 + 1.0);
        }
        for ((p, (f, at, k)), ps) in self.planks.iter_mut().zip(want).zip(&pieces) {
            if k > 0.0 {
                p.at = at / k;
            }
            if p.seam.is_none() {
                continue;
            }
            p.f += (f - p.f) * (dt / delirium_data::kv!(73)).min(1.0);
            p.load = p.f.length();
            let s = (ps[1].translation.distance(p.at) < ps[0].translation.distance(p.at)) as usize;
            w.bodies[p.h[s]].apply_impulse_at_point(rv(p.f * dt), rv(p.at), true);
        }
        self.t += dt;
        let wind = WIND_N() * (delirium_data::kv!(70) * self.t).sin() * (delirium_data::kv!(228) * self.t + 1.0).sin();
        for h in self.planks.iter().flat_map(|p| p.h) {
            w.bodies[h].apply_impulse(rv(Vec3::X * wind * dt / 2.0), true);
        }
        let broke: Vec<usize> = (0..self.planks.len()).filter(|&i| self.planks[i].load > self.planks[i].holds).collect();
        for &i in &broke {
            self.fall(w, i);
        }
        broke
    }

    pub fn rehang(&mut self, w: &mut PhysicsWorld) {
        for h in self.planks.iter().flat_map(|p| p.h).chain(self.knots.iter().flatten().flatten().copied()) {
            w.remove_body(h);
        }
        *self = Bridge::new(w, self.seed);
    }

    pub fn hold(&mut self, i: usize) {
        self.planks[i].holds = f32::INFINITY;
    }

    pub fn fall(&mut self, w: &mut PhysicsWorld, i: usize) {
        let p = &mut self.planks[i];
        if let Some(j) = p.seam.take() {
            w.remove_impulse_joint(j);
        }
        for h in p.h {
            w.bodies[h].set_angular_damping(SWING_DAMP());
            w.bodies[h].wake_up(true);
        }
        p.holds = f32::INFINITY;
    }
}

#[allow(non_snake_case)]
#[inline]
pub fn GLASS_HALF() -> f32 { delirium_data::kv!(17) }

#[allow(non_snake_case)]
#[inline]
pub fn HALL() -> &'static [Vec2; 2] { static V: std::sync::OnceLock<[Vec2; 2]> = std::sync::OnceLock::new(); let f = || [Vec2::new(-delirium_data::kv!(5), -delirium_data::kv!(981)), Vec2::new(delirium_data::kv!(5), delirium_data::kv!(32))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn HALL_TOP() -> f32 { delirium_data::kv!(104) }

#[allow(non_snake_case)]
#[inline]
pub fn QUARTERS() -> &'static [(Vec3, f32, f32, f32); 3] { static V: std::sync::OnceLock<[(Vec3, f32, f32, f32); 3]> = std::sync::OnceLock::new(); let f = || [(Vec3::new(-delirium_data::kv!(117), delirium_data::kv!(30), 0.0), std::f32::consts::FRAC_PI_2, delirium_data::kv!(30), delirium_data::kv!(61)),
    (Vec3::new(-delirium_data::kv!(127), delirium_data::kv!(30), 0.0), -std::f32::consts::FRAC_PI_2, delirium_data::kv!(30), delirium_data::kv!(61)), (Vec3::new(0.0, delirium_data::kv!(117), 0.0), 0.0, delirium_data::kv!(5), delirium_data::kv!(132))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn QUARTER_SLABS() -> usize { delirium_data::kv!(89 int) }

fn quarter(at: Vec3, yaw: f32, r: f32, wide: f32, row: u8) -> Vec<(Transform, Vec3, u8)> {
    let (q, n, t) = (Quat::from_rotation_z(yaw), QUARTER_SLABS(), delirium_data::kv!(73));
    let put = |c: Vec3, rot: Quat, h: Vec3, row| (Transform::from_translation(at + q * c).with_rotation(q * rot), h, row);
    (0..n).map(|i| {
        let k = std::f32::consts::FRAC_PI_2 / n as f32;
        let (a, b) = (i as f32 * k, (i + 1) as f32 * k);
        let (pa, pb) = (Vec3::new(0.0, r * a.sin(), r * (1.0 - a.cos())), Vec3::new(0.0, r * b.sin(), r * (1.0 - b.cos())));
        let m = (a + b) * 0.5;
        let up = Vec3::new(0.0, -m.sin(), m.cos());
        put((pa + pb) * 0.5 - up * t * 0.5, Quat::from_rotation_x(m), Vec3::new(wide * 0.5, (pb - pa).length() * 0.5 + delirium_data::kv!(109), t * 0.5), row)
    }).collect()
}

fn quarter_deck(at: Vec3, yaw: f32, r: f32, wide: f32, deck: f32, row: u8, metal: u8) -> Vec<(Transform, Vec3, u8)> {
    let q = Quat::from_rotation_z(yaw);
    let put = |c: Vec3, h: Vec3, row| (Transform::from_translation(at + q * c).with_rotation(q), h, row);
    vec![put(Vec3::new(0.0, r + deck * 0.5, r * 0.5), Vec3::new(wide * 0.5, deck * 0.5, r * 0.5), row),
         put(Vec3::new(0.0, r + delirium_data::kv!(122), r - delirium_data::kv!(122)), Vec3::new(wide * 0.5 + delirium_data::kv!(565), delirium_data::kv!(982), delirium_data::kv!(982)), metal)]
}

pub fn quarter_mesh(i: usize) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let (pos, nor, idx) = quarter_tris(i);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_indices(Indices::U32(idx))
}

pub fn quarter_tris(i: usize) -> (Vec<Vec3>, Vec<Vec3>, Vec<u32>) {
    let (at, yaw, r, wide) = (*QUARTERS())[i];
    let (q, n, x) = (Quat::from_rotation_z(yaw), delirium_data::kv!(251 int), wide * 0.5);
    let (mut pos, mut nor, mut idx) = (vec![], vec![], vec![]);
    let curve = |k: usize| {
        let a = std::f32::consts::FRAC_PI_2 * k as f32 / n as f32;
        (Vec3::new(0.0, r * a.sin(), r * (1.0 - a.cos())), Vec3::new(0.0, -a.sin(), a.cos()))
    };
    for k in 0..=n {
        let (p, up) = curve(k);
        for s in [-x, x] {
            pos.push(at + q * (p + Vec3::X * s));
            nor.push(q * up);
        }
        if k < n {
            let b = (2 * k) as u32;
            idx.extend([b, b + 1, b + 2, b + 1, b + delirium_data::kv!(0 int), b + 2]);
        }
    }
    for s in [-x, x] {
        let b = pos.len() as u32;
        pos.push(at + q * Vec3::new(s, r, 0.0));
        nor.push(q * Vec3::X * s.signum());
        for k in 0..=n {
            pos.push(at + q * (curve(k).0 + Vec3::X * s));
            nor.push(q * Vec3::X * s.signum());
        }
        for k in 0..n as u32 {
            idx.extend(if s > 0.0 { [b, b + 2 + k, b + 1 + k] } else { [b, b + 1 + k, b + 2 + k] });
        }
    }
    (pos, nor, idx)
}

fn kicker(at: Vec3, yaw: f32, len: f32, deg: f32, wide: f32, row: u8) -> Vec<(Transform, Vec3, u8)> {
    let (q, a, t) = (Quat::from_rotation_z(yaw), deg.to_radians(), delirium_data::kv!(73));
    let up = Vec3::new(0.0, -a.sin(), a.cos());
    let top = Vec3::new(0.0, len * a.cos(), len * a.sin());
    let put = |c: Vec3, rot: Quat, h: Vec3| (Transform::from_translation(at + q * c).with_rotation(q * rot), h, row);
    vec![put(top * 0.5 - up * t * 0.5, Quat::from_rotation_x(a), Vec3::new(wide * 0.5, len * 0.5, t * 0.5)),
         put(Vec3::new(0.0, top.y - delirium_data::kv!(73), top.z * 0.5), Quat::IDENTITY, Vec3::new(wide * 0.5, delirium_data::kv!(73), top.z * 0.5))]
}

fn park(row: u8, metal: u8) -> Vec<(Transform, Vec3, u8)> {
    use std::f32::consts::PI;
    let mut v = vec![];
    for (at, yaw, r, wide) in (*QUARTERS()) {
        v.extend(quarter(at, yaw, r, wide, row));
    }
    for (at, yaw, r, wide) in (*QUARTERS()) {
        v.extend(quarter_deck(at, yaw, r, wide, 2.0, row, metal));
    }
    v.extend(kicker(Vec3::new(0.0, delirium_data::kv!(104), 0.0), 0.0, delirium_data::kv!(485), delirium_data::kv!(835), delirium_data::kv!(30), row));
    let (f, hz) = (Vec3::new(delirium_data::kv!(61), delirium_data::kv!(104), 0.0), delirium_data::kv!(24));
    let put = |c: Vec3, h: Vec3, row| (Transform::from_translation(c), h, row);
    v.push(put(f + Vec3::Z * hz, Vec3::new(delirium_data::kv!(12), 2.0, hz), row));
    let deg = (2.0 * hz / delirium_data::kv!(195)).atan().to_degrees();
    let len = (delirium_data::kv!(195 f32).powi(2) + (2.0 * hz).powi(2)).sqrt();
    v.extend(kicker(f + Vec3::new(0.0, -delirium_data::kv!(983), 0.0), 0.0, len, deg, delirium_data::kv!(5), row));
    v.extend(kicker(f + Vec3::new(0.0, delirium_data::kv!(983), 0.0), PI, len, deg, delirium_data::kv!(5), row));
    v.push(put(f + Vec3::Z * (2.0 * hz + delirium_data::kv!(106)), Vec3::new(delirium_data::kv!(253), 2.0, delirium_data::kv!(253)), metal));
    v.extend([-delirium_data::kv!(485), delirium_data::kv!(485)].map(|y| put(f + Vec3::new(0.0, y, 2.0 * hz + delirium_data::kv!(57)), Vec3::new(delirium_data::kv!(109), delirium_data::kv!(109), delirium_data::kv!(57)), metal)));
    let b = Vec3::new(delirium_data::kv!(60), -delirium_data::kv!(32), 0.0);
    v.push(put(b + Vec3::Z * delirium_data::kv!(63), Vec3::new(2.0, delirium_data::kv!(253), delirium_data::kv!(253)), metal));
    v.extend([-delirium_data::kv!(485), delirium_data::kv!(485)].map(|x| put(b + Vec3::new(x, 0.0, delirium_data::kv!(227)), Vec3::new(delirium_data::kv!(109), delirium_data::kv!(109), delirium_data::kv!(227)), metal)));
    let s = Vec3::new(-delirium_data::kv!(104), -delirium_data::kv!(61), 0.0);
    let top = delirium_data::kv!(227) * delirium_data::kv!(60);
    v.push(put(s + Vec3::new(0.0, -delirium_data::kv!(12) - delirium_data::kv!(12), top * 0.5), Vec3::new(delirium_data::kv!(12), delirium_data::kv!(12), top * 0.5), row));
    for i in 0..delirium_data::kv!(1 int) {
        let z = top - delirium_data::kv!(227) * (i + 1) as f32;
        v.push(put(s + Vec3::new(0.0, -delirium_data::kv!(12) + delirium_data::kv!(24) * i as f32 + delirium_data::kv!(9), z * 0.5), Vec3::new(delirium_data::kv!(12), delirium_data::kv!(9), z * 0.5), row));
    }
    let (a, b) = (s + Vec3::new(delirium_data::kv!(190), -delirium_data::kv!(12), top + delirium_data::kv!(20)), s + Vec3::new(delirium_data::kv!(190), -delirium_data::kv!(24), delirium_data::kv!(227) + delirium_data::kv!(20)));
    v.push((Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())),
            Vec3::new(delirium_data::kv!(253), (b - a).length() * 0.5 + delirium_data::kv!(24), delirium_data::kv!(253)), metal));
    v.push(put(s + Vec3::new(delirium_data::kv!(190), -delirium_data::kv!(485), top + delirium_data::kv!(106)), Vec3::new(delirium_data::kv!(109), delirium_data::kv!(109), delirium_data::kv!(106)), metal));
    v.push(put(s + Vec3::new(delirium_data::kv!(190), 0.0, delirium_data::kv!(106)), Vec3::new(delirium_data::kv!(109), delirium_data::kv!(109), delirium_data::kv!(106)), metal));
    v
}

fn stage(name: &str, brick: u8, concrete: u8) -> Option<(Vec<(Vec3, Vec3, u8)>, Vec3)> {
    let b = |c: [f32; 3], h: [f32; 3], r| (Vec3::from(c), Vec3::from(h), r);
    Some(match name {
        "bigstairs" => {
            let mut v = vec![b([0.0, -delirium_data::kv!(10), delirium_data::kv!(5)], [delirium_data::kv!(12), delirium_data::kv!(984), delirium_data::kv!(5)], concrete)];
            v.extend([-delirium_data::kv!(485), delirium_data::kv!(485)].map(|x| b([x, -delirium_data::kv!(10), delirium_data::kv!(119)], [delirium_data::kv!(73), delirium_data::kv!(984), delirium_data::kv!(119)], brick)));
            for i in 0..delirium_data::kv!(418 int) {
                let (top, y) = (delirium_data::kv!(32) - delirium_data::kv!(9) * (i + 1) as f32, delirium_data::kv!(18) + delirium_data::kv!(24) * i as f32);
                v.push(b([0.0, y, top * 0.5], [delirium_data::kv!(12), delirium_data::kv!(9), top * 0.5], concrete));
                v.extend([-delirium_data::kv!(485), delirium_data::kv!(485)].map(|x| b([x, y, (top + 1.0) * 0.5], [delirium_data::kv!(73), delirium_data::kv!(9), (top + 1.0) * 0.5], brick)));
            }
            (v, Vec3::Z * delirium_data::kv!(32))
        }
        "stairsdown" => {
            let (v, start) = stage("bigstairs", brick, concrete)?;
            (v.into_iter().map(|(c, h, r)| (c * Vec3::new(1.0, -1.0, 1.0), h, r)).collect(), start)
        }
        "portalwall" => (vec![b([-delirium_data::kv!(493), 0.0, delirium_data::kv!(30)], [delirium_data::kv!(9), delirium_data::kv!(104), delirium_data::kv!(30)], concrete)], Vec3::new(delirium_data::kv!(985), -delirium_data::kv!(986), 0.0)),
        "rooftop" => (vec![b([0.0, -delirium_data::kv!(130), 2.0], [delirium_data::kv!(5), delirium_data::kv!(987), 2.0], concrete)], Vec3::Z * delirium_data::kv!(104)),
        "highledge" => (vec![b([0.0, delirium_data::kv!(552), delirium_data::kv!(32)], [delirium_data::kv!(60), delirium_data::kv!(60), delirium_data::kv!(32)], concrete)], Vec3::Z * delirium_data::kv!(21)),
        "lowwall" => (vec![b([0.0, delirium_data::kv!(10), delirium_data::kv!(10)], [delirium_data::kv!(104), delirium_data::kv!(9), delirium_data::kv!(10)], brick)], Vec3::ZERO),
        "kerb" => (vec![b([0.0, delirium_data::kv!(10), delirium_data::kv!(194)], [delirium_data::kv!(104), delirium_data::kv!(9), delirium_data::kv!(194)], brick)], Vec3::ZERO),
        "open" => (vec![], Vec3::ZERO),
        "skatepark" => (vec![], Vec3::ZERO),
        "lights" => {
            let (c, h) = (((*HALL())[0] + (*HALL())[1]) * 0.5, ((*HALL())[1] - (*HALL())[0]) * 0.5);
            let t = delirium_data::kv!(9);
            (vec![b([c.x, c.y, -delirium_data::kv!(988)], [h.x, h.y, delirium_data::kv!(73)], concrete), b([c.x, c.y, HALL_TOP() + t], [h.x + 2.0 * t, h.y + 2.0 * t, t], concrete),
                  b([(*HALL())[0].x - t, c.y, HALL_TOP() * 0.5], [t, h.y, HALL_TOP() * 0.5], concrete), b([(*HALL())[1].x + t, c.y, HALL_TOP() * 0.5], [t, h.y, HALL_TOP() * 0.5], concrete),
                  b([c.x, (*HALL())[0].y - t, HALL_TOP() * 0.5], [h.x + 2.0 * t, t, HALL_TOP() * 0.5], concrete),
                  b([c.x, (*HALL())[1].y + t, HALL_TOP() * 0.5], [h.x + 2.0 * t, t, HALL_TOP() * 0.5], concrete)], Vec3::ZERO)
        }
        "rail" => (vec![b([0.0, -0.5, delirium_data::kv!(989)], [delirium_data::kv!(12), delirium_data::kv!(122), delirium_data::kv!(122)], concrete), b([-delirium_data::kv!(12), -0.5, delirium_data::kv!(110)], [delirium_data::kv!(122), delirium_data::kv!(122), delirium_data::kv!(110)], concrete),
                        b([delirium_data::kv!(12), -0.5, delirium_data::kv!(110)], [delirium_data::kv!(122), delirium_data::kv!(122), delirium_data::kv!(110)], concrete)], Vec3::ZERO),
        "street" => {
            let mut v = vec![b([0.0, delirium_data::kv!(32), delirium_data::kv!(224)], [delirium_data::kv!(80), 2.0, delirium_data::kv!(224)], concrete), b([0.0, -delirium_data::kv!(32), delirium_data::kv!(224)], [delirium_data::kv!(80), 2.0, delirium_data::kv!(224)], concrete)];
            v.extend((0..delirium_data::kv!(23 int)).map(|i| b([0.0, -delirium_data::kv!(119) + i as f32, delirium_data::kv!(990)], [delirium_data::kv!(12), delirium_data::kv!(17), delirium_data::kv!(990)], concrete)));
            (v, Vec3::ZERO)
        }
        "bridge" => {
            let mut v = vec![b([0.0, delirium_data::kv!(5), delirium_data::kv!(104)], [2.0, delirium_data::kv!(5), delirium_data::kv!(104)], concrete), b([0.0, BRIDGE_FAR() - delirium_data::kv!(5), delirium_data::kv!(104)], [2.0, delirium_data::kv!(5), delirium_data::kv!(104)], concrete)];
            for (y, x) in [((*POST_HALF()).y, -1.0), ((*POST_HALF()).y, 1.0), (BRIDGE_FAR() - (*POST_HALF()).y, -1.0), (BRIDGE_FAR() - (*POST_HALF()).y, 1.0)] {
                v.push(b([x * (ROPE_X() + (*POST_HALF()).x), y, BRIDGE_TOP() + (*POST_HALF()).z], (*POST_HALF()).to_array(), concrete));
            }
            (v, Vec3::new(0.0, delirium_data::kv!(20), BRIDGE_TOP()))
        }
        "cab" => (vec![], Vec3::ZERO),
        "trampoline" => (vec![b([0.0, 0.0, (*PAD_HALF()).z], (*PAD_HALF()).to_array(), concrete)], Vec3::new(0.0, 0.0, 2.0 * (*PAD_HALF()).z)),
        "quake" | "bull" => (vec![], Vec3::Z * (plat(name, 0.0)?.0.z + (*PLAT_HALF()).z)),
        _ => return None,
    })
}

pub fn cast_boxes(boxes: impl Iterator<Item = (Vec3, Vec3)>, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
    boxes.filter_map(|(c, h)| {
        let (a, b) = ((c - h - from) / dir, (c + h - from) / dir);
        let (lo, t0, t1) = (a.min(b), a.min(b).max_element(), a.max(b).min_element());
        (t0 <= t1 && (0.0..=len).contains(&t0)).then(|| {
            let k = (0..delirium_data::kv!(0 int)).find(|&k| lo[k] == t0).unwrap_or(2);
            let mut n = Vec3::ZERO;
            n[k] = -dir[k].signum();
            (t0, n)
        })
    }).min_by(|a, b| a.0.total_cmp(&b.0))
}

impl Room {
    pub fn ground(&self, p: Vec3) -> f32 {
        self.blocks.iter().filter(|(c, h, _)| (p.x - c.x).abs() <= h.x && (p.y - c.y).abs() <= h.y && c.z + h.z <= p.z + delirium_data::kv!(64))
            .map(|(c, h, _)| c.z + h.z).fold(if self.pool && over_pool(p) { -POOL_DEEP() } else { 0.0 }, f32::max)
    }

    pub fn cast(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
        cast_boxes(self.blocks.iter().map(|&(c, h, _)| (c, h)), from, dir, len)
    }

    pub fn resting(&self) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
        (0..self.objs.len()).filter(|&j| matches!(self.objs[j].kind, 2 | 3) && self.objs[j].anchor.is_none()).filter_map(|j| {
            let (t, v) = self.obj(0, j);
            let r = (*KINDS())[self.objs[j].kind].1;
            (v.length() < 1.0).then(|| (t.translation, if self.objs[j].kind == delirium_data::kv!(0 int) { Vec3::new(r, r, BARREL_HALF()) } else { Vec3::splat(r) }))
        })
    }

    pub fn stairs_ahead(&self, p: Vec3, dir: Vec3) -> bool {
        let g = [0.0, delirium_data::kv!(24), delirium_data::kv!(110), delirium_data::kv!(108), delirium_data::kv!(74)].map(|d| self.ground(p + dir * d));
        g.windows(2).all(|w| (0.0..0.4).contains(&(w[0] - w[1]))) && g[0] - g[delirium_data::kv!(1 int)] > delirium_data::kv!(24)
    }

    pub fn new(surfaces: Vec<(String, f32, f32)>, name: &str) -> Room {
        let row = |n: &str| surfaces.iter().position(|s| s.0 == n).unwrap_or(0) as u8;
        let pool = stage(name, 0, 0).is_none();
        let (blocks, start) = stage(name, row("BRICK_WALL"), row("CONCRETE")).unwrap_or_else(|| (Room::blocks(&row, name == "pttest"), Vec3::ZERO));
        let tilted = if name == "skatepark" { park(row("CONCRETE"), surfaces.iter().position(|s| s.0.starts_with("METAL")).unwrap_or(0) as u8) } else { vec![] };
        let mut w = [PhysicsWorld::new(), PhysicsWorld::new()];
        let mut fixed = [vec![], vec![]];
        for (i, w) in w.iter_mut().enumerate() {
            w.gravity = Vector::new(0.0, 0.0, -delirium_data::kv!(105));
            for (lo, hi) in floor_rects(pool && i == 0, delirium_data::kv!(76)) {
                let (c, h) = ((lo + hi) * 0.5, (hi - lo) * 0.5);
                fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(Vector::new(c.x, c.y, -0.5)), ColliderBuilder::cuboid(h.x, h.y, 0.5).friction(1.0)).0);
            }
            for &(c, h, _) in blocks.iter().filter(|_| i == 0) {
                fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(rv(c)), ColliderBuilder::cuboid(h.x, h.y, h.z).friction(1.0)).0);
            }
            for (t, h, _) in tilted.iter().filter(|_| i == 0) {
                fixed[i].push(w.insert(RigidBodyBuilder::fixed().translation(rv(t.translation)).rotation(rv(t.rotation.to_scaled_axis())),
                                       ColliderBuilder::cuboid(h.x, h.y, h.z).friction(1.0)).0);
            }
        }
        let faucet: Vec<_> = if name == "pttest" {
            crate::fluid_gpu::faucet_caps().iter().map(|&(a, b)| w[0].insert(RigidBodyBuilder::fixed(),
                ColliderBuilder::capsule_from_endpoints(rv(a), rv(b), crate::fluid_gpu::PIPE_R).friction(delirium_data::kv!(110))).0).collect()
        } else { vec![] };
        fixed[0].extend(&faucet);
        let snap = (name == "trampoline").then_some(0);
        let seed = std::env::var("BRIDGE_SEED").ok().and_then(|s| s.parse().ok())
            .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos()));
        let bridge = (name == "bridge").then(|| Bridge::new(&mut w[0], seed));
        Room { w, fixed, parts: [vec![], vec![]], objs: vec![], blocks, tilted, surfaces, weight: 1.0, stage: name.to_string(), pool, start, snap, snapped: None, fell: false, bounced: delirium_data::kv!(127), shook: 0.0,
               bridge, broke: vec![], push: Vec3::NEG_Z * HOMER_N(), com_v: Vec3::ZERO, com_a: Vec3::ZERO, #[cfg(feature = "portal")] holes: vec![], flow: vec![], faucet }
    }

    fn blocks(row: &dyn Fn(&str) -> u8, glass: bool) -> Vec<(Vec3, Vec3, u8)> {
        let mut blocks = vec![(Vec3::new(0.0, delirium_data::kv!(119), 0.5), Vec3::new(delirium_data::kv!(104), delirium_data::kv!(9), 0.5), row("BRICK_WALL"))];
        for i in 0..delirium_data::kv!(77 int) {
            let top = delirium_data::kv!(131) * (i + 1) as f32;
            blocks.push((Vec3::new(-delirium_data::kv!(991), -delirium_data::kv!(12) + delirium_data::kv!(24) * i as f32 + delirium_data::kv!(9), top * 0.5), Vec3::new(delirium_data::kv!(18), delirium_data::kv!(9), top * 0.5), row("CONCRETE")));
        }
        blocks.push((Vec3::new(-delirium_data::kv!(991), 2.0, delirium_data::kv!(108)), Vec3::new(delirium_data::kv!(18), 0.5, delirium_data::kv!(108)), row("CONCRETE")));
        let ([a, b], d, rim) = ((*POOL()), POOL_DEEP(), delirium_data::kv!(24));
        let (m, hw, z) = ((a + b) * 0.5, (b - a) * 0.5, -(d + delirium_data::kv!(57)) * 0.5 - delirium_data::kv!(125));
        for (c, h) in [(Vec2::new(a.x - rim * 0.5, m.y), Vec2::new(rim * 0.5, hw.y + rim)), (Vec2::new(b.x + rim * 0.5, m.y), Vec2::new(rim * 0.5, hw.y + rim)),
                       (Vec2::new(m.x, a.y - rim * 0.5), Vec2::new(hw.x, rim * 0.5)), (Vec2::new(m.x, b.y + rim * 0.5), Vec2::new(hw.x, rim * 0.5))] {
            blocks.push((c.extend(z), h.extend(z.abs() - delirium_data::kv!(125)), row("CONCRETE")));
        }
        blocks.push((m.extend(-d - delirium_data::kv!(73)), hw.extend(delirium_data::kv!(73)), row("CONCRETE")));
        for i in 0..delirium_data::kv!(23 int) {
            let top = -delirium_data::kv!(131) * (i + 1) as f32;
            blocks.push((Vec3::new(b.x - delirium_data::kv!(24) * i as f32 - delirium_data::kv!(9), a.y + delirium_data::kv!(110), (top - d) * 0.5), Vec3::new(delirium_data::kv!(9), delirium_data::kv!(110), (top + d) * 0.5), row("CONCRETE")));
        }
        #[cfg(feature = "portal")]
        blocks.extend([(Vec3::new(delirium_data::kv!(27), 0.0, delirium_data::kv!(5)), Vec3::new(delirium_data::kv!(9), delirium_data::kv!(104), delirium_data::kv!(5)), row("CONCRETE")),
                       (Vec3::new(delirium_data::kv!(32), delirium_data::kv!(5), delirium_data::kv!(60)), Vec3::new(delirium_data::kv!(5), delirium_data::kv!(5), delirium_data::kv!(73)), row("CONCRETE"))].into_iter().skip(glass as usize));
        if glass {
            blocks.extend((0..delirium_data::kv!(452 int)).map(|i| (Vec3::new(delirium_data::kv!(27), -delirium_data::kv!(991) + 0.5 * (i % delirium_data::kv!(51 int)) as f32, delirium_data::kv!(17) + 0.5 * (i / delirium_data::kv!(51 int)) as f32), Vec3::splat(GLASS_HALF()), row("CONCRETE"))));
            let (at, half, deep, g) = (crate::SKY_POOL, crate::SKY_HALF, crate::SKY_DEEP, delirium_data::kv!(24));
            blocks.extend([(Vec3::new(0.0, 0.0, -g * 0.5), half.extend(g * 0.5) + Vec3::new(g, g, 0.0)),
                           (Vec3::new(half.x + g * 0.5, 0.0, deep * delirium_data::kv!(110)), Vec3::new(g * 0.5, half.y + g, deep * delirium_data::kv!(110))),
                           (Vec3::new(-half.x - g * 0.5, 0.0, deep * delirium_data::kv!(110)), Vec3::new(g * 0.5, half.y + g, deep * delirium_data::kv!(110))),
                           (Vec3::new(0.0, half.y + g * 0.5, deep * delirium_data::kv!(110)), Vec3::new(half.x, g * 0.5, deep * delirium_data::kv!(110))),
                           (Vec3::new(0.0, -half.y - g * 0.5, deep * delirium_data::kv!(110)), Vec3::new(half.x, g * 0.5, deep * delirium_data::kv!(110)))].map(|(c, h)| (at + c, h, row("CONCRETE"))));
        }
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
        self.tilted.clear();
        self.pool = false;
    }

    pub fn triangles(&self) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = self.tris((0..self.blocks.len()).filter(|&i| Some(i) != self.snap));
        let curves = if self.stage == "skatepark" { QUARTER_SLABS() * (*QUARTERS()).len() } else { 0 };
        let (q, u, n) = Self::boxes(self.tilted.iter().skip(curves).copied());
        let o = p.len() as u32;
        p.extend(q);
        t.extend(u.into_iter().map(|f| f.map(|k| k + o)));
        m.extend(n);
        for i in (0..(*QUARTERS()).len()).filter(|_| curves > 0 && self.tilted.len() >= curves) {
            let (q, _, u) = quarter_tris(i);
            let o = p.len() as u32;
            p.extend(q.iter().map(|v| v.to_array()));
            t.extend(u.chunks(delirium_data::kv!(0 int)).map(|f| [f[0] + o, f[1] + o, f[2] + o]));
            m.extend(std::iter::repeat_n(self.tilted[0].2, u.len() / delirium_data::kv!(0 int)));
        }
        (p, t, m)
    }

    pub fn block_triangles(&self, i: usize) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        self.tris(std::iter::once(i))
    }

    pub fn plank_triangles(&self, i: usize) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let b = self.bridge.as_ref().expect("the bridge");
        let row = self.surfaces.iter().position(|s| s.0.starts_with("WOOD")).or_else(|| self.surfaces.iter().position(|s| s.0 == "CONCRETE")).unwrap_or(0) as u8;
        Self::boxes([0, 1].into_iter().map(|s| (b.piece(&self.w[0], i, s), (*PIECE_HALF()), row)))
    }

    pub fn plat_triangles(&self) -> Option<(Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>)> {
        let (at, rot) = plat(&self.stage, self.shook)?;
        let row = self.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
        Some(Self::boxes(std::iter::once((Transform::from_translation(at).with_rotation(rot), (*PLAT_HALF()), row))))
    }

    fn tris(&self, which: impl Iterator<Item = usize>) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        Self::boxes(which.map(|i| self.blocks[i]).map(|(c, h, row)| (Transform::from_translation(c), h, row)))
    }

    fn boxes(which: impl Iterator<Item = (Transform, Vec3, u8)>) -> (Vec<[f32; 3]>, Vec<[u32; 3]>, Vec<u8>) {
        let (mut p, mut t, mut m) = (vec![], vec![], vec![]);
        for (c, h, row) in which {
            let o = p.len() as u32;
            for i in 0..delirium_data::kv!(23 int) {
                p.push(c.transform_point(h * Vec3::new([-1.0, 1.0][i & 1], [-1.0, 1.0][i >> 1 & 1], [-1.0, 1.0][i >> 2])).to_array());
            }
            for f in [[0, 2, delirium_data::kv!(0 int), 1], [delirium_data::kv!(1 int), delirium_data::kv!(2 int), delirium_data::kv!(22 int), delirium_data::kv!(6 int)], [0, 1, delirium_data::kv!(2 int), delirium_data::kv!(1 int)], [2, delirium_data::kv!(6 int), delirium_data::kv!(22 int), delirium_data::kv!(0 int)], [0, delirium_data::kv!(1 int), delirium_data::kv!(6 int), 2], [1, delirium_data::kv!(0 int), delirium_data::kv!(22 int), delirium_data::kv!(2 int)]] {
                t.extend([[o + f[0], o + f[1], o + f[2]], [o + f[0], o + f[2], o + f[delirium_data::kv!(0 int)]]]);
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
        let (_, s, kg, _) = (*KINDS())[kind];
        let kg = kg * self.weight;
        let h = [0, 1].map(|i| {
            let mut b = RigidBodyBuilder::dynamic().translation(rv(from[i])).linvel(rv(vel)).angvel(Vector::new(delirium_data::kv!(12), -2.0, delirium_data::kv!(70)) * spin as u8 as f32).ccd_enabled(true);
            let c = match kind {
                0 | 4 => ColliderBuilder::ball(s),
                3 => {
                    b = b.rotation(Vector::X * std::f32::consts::FRAC_PI_2);
                    ColliderBuilder::cylinder(BARREL_HALF(), s)
                }
                _ => ColliderBuilder::cuboid(s, s, s),
            };
            #[cfg(feature = "portal")]
            let c = c.active_hooks(rapier3d::prelude::ActiveHooks::MODIFY_SOLVER_CONTACTS);
            let w = &mut self.w[i];
            let h = w.insert(b, c.mass(kg).friction(delirium_data::kv!(110)).restitution(delirium_data::kv!(57))).0;
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
                    let push = (d.normalize_or(Vec3::Z) + Vec3::Z * delirium_data::kv!(110)).normalize() * dv * f * b.mass();
                    b.apply_impulse(rv(push), true);
                    b.apply_torque_impulse(rv(Vec3::new(d.y, -d.x, delirium_data::kv!(24)) * b.mass() * f), true);
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

    pub fn obj_body(&self, j: usize) -> ((Vec3, Vec3, f32), f32, Vec3) {
        let (t, v) = self.obj(0, j);
        let (kind, s) = (self.objs[j].kind, (*KINDS())[self.objs[j].kind].1);
        let half = if kind == delirium_data::kv!(0 int) { t.rotation * Vec3::Y * (BARREL_HALF() - s) } else { Vec3::ZERO };
        let vol = match kind {
            0 | 4 => delirium_data::kv!(104) / delirium_data::kv!(5) * std::f32::consts::PI * s * s * s,
            3 => std::f32::consts::PI * s * s * 2.0 * BARREL_HALF(),
            _ => delirium_data::kv!(61) * s * s * s,
        };
        ((t.translation - half, t.translation + half, s), vol, v)
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
        let have = self.parts[i].len();
        if have < caps.len() {
            let groups = InteractionGroups::new(PARTS, Group::ALL, InteractionTestMode::And);
            self.parts[i].extend(caps[have..].iter().map(|(t, r, h)| w.insert(RigidBodyBuilder::kinematic_position_based().pose(pose(t)), ColliderBuilder::capsule_y(*h, *r).collision_groups(groups))));
        }
        for (b, _) in self.parts[i].iter().skip(caps.len()) {
            w.bodies[*b].set_position(pose(&Transform::from_xyz(0.0, 0.0, -delirium_data::kv!(78))), true);
        }
        let mut part_v = vec![];
        for ((b, c), (t, r, h)) in self.parts[i].iter().zip(caps) {
            if let Some(col) = w.colliders.get_mut(*c) && col.shape().as_capsule().is_some_and(|s| (s.radius - r).abs() > delirium_data::kv!(68) || (s.half_height() - h).abs() > delirium_data::kv!(68)) {
                col.set_shape(SharedShape::capsule_y(*h, *r));
            }
            let body = &mut w.bodies[*b];
            let from = bv(body.translation());
            if from.distance(t.translation) > delirium_data::kv!(24) {
                body.set_position(pose(t), true);
                part_v.push(Vec3::ZERO);
            } else {
                body.set_next_kinematic_position(pose(t));
                part_v.push((t.translation - from) / dt);
            }
        }
        if i == 0 && !self.flow.is_empty() {
            for j in 0..self.objs.len() {
                let Some(Some(f)) = self.flow.get(j) else { continue };
                let (cap, vol, v) = self.obj_body(j);
                let body = &mut self.w[0].bodies[self.objs[j].h[0]];
                let m = body.mass();
                let imp = crate::fluid_gpu::push(*f, cap, vol, m, v, dt);
                if imp != Vec3::ZERO {
                    body.apply_impulse(rv(imp), true);
                    let spin = bv(body.angvel());
                    body.apply_torque_impulse(rv(-spin * m * cap.2 * cap.2 * dt), true);
                }
            }
        }
        let w = &mut self.w[i];
        if i == 0 && self.pool && self.flow.is_empty() {
            for o in &self.objs {
                let body = &mut w.bodies[o.h[0]];
                let (at, r) = (bv(body.translation()), (*KINDS())[o.kind].1);
                let under = ((WATER() - at.z + r) / (2.0 * r)).clamp(0.0, 1.0);
                if under > 0.0 && over_pool(at) {
                    let (m, v) = (body.mass(), bv(body.linvel()));
                    body.apply_impulse(rv((Vec3::Z * m * delirium_data::kv!(105) * 2.0 - v * m * 2.0) * under * dt), true);
                    let spin = bv(body.angvel());
                    body.apply_torque_impulse(rv(-spin * m * r * r * under * dt), true);
                }
            }
        }
        let obj_v: Vec<Vec3> = self.objs.iter().map(|o| bv(w.bodies[o.h[i]].linvel())).collect();
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
                if imp.length_squared() < delirium_data::kv!(242) {
                    continue;
                }
                let at = bv(w.bodies[self.objs[j].h[i]].translation());
                let axis = t.rotation * Vec3::Y;
                let on = t.translation + axis * (at - t.translation).dot(axis).clamp(-*h, *h);
                let hit = on + (at - on).normalize_or_zero() * *r;
                let imp = if imp.dot(t.translation - at) < 0.0 { -imp } else { imp };
                let m = w.bodies[self.objs[j].h[i]].mass();
                let closing = (obj_v[j] - part_v[k]).dot(imp.normalize_or_zero()).max(0.0);
                let imp = imp.clamp_length_max(m * (closing + delirium_data::kv!(105) * dt));
                let obj = &mut w.bodies[self.objs[j].h[i]];
                let mine = imp * (PART_KG() / (PART_KG() + m));
                obj.apply_impulse(rv(imp - mine), true);
                let n = (on - at).normalize_or_zero();
                let v = bv(obj.linvel());
                let over = v.dot(n) - obj_v[j].dot(n).max(part_v[k].dot(n)).max(0.0);
                if over > 0.0 {
                    obj.set_linvel(rv(v - n * over), true);
                }
                out.push((j, k, hit, mine));
            }
        }
        out
    }
}

