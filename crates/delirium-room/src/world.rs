use bevy::prelude::*;
use rapier3d::math::{Pose, Rotation, Vector};
use rapier3d::prelude::{ColliderBuilder, ColliderHandle, SharedShape, Group, ImpulseJointHandle, InteractionGroups, InteractionTestMode, PhysicsWorld,
                        FixedJointBuilder, RigidBodyBuilder, RigidBodyHandle, RopeJointBuilder, SphericalJointBuilder};

#[allow(non_snake_case)]
#[inline]
pub fn FLOOR_HALF() -> f32 { delirium_data::kv!(26) }

#[allow(non_snake_case)]
#[inline]
pub fn POOL() -> &'static [Vec2; 2] { static V: std::sync::OnceLock<[Vec2; 2]> = std::sync::OnceLock::new(); let f = || -> [Vec2; 2] { [Vec2::new(-delirium_data::kv!(5), delirium_data::kv!(64)), Vec2::new(delirium_data::kv!(5), delirium_data::kv!(38))] }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn POOL_DEEP() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
pub fn WATER() -> f32 { -delirium_data::kv!(79) }

pub static HOLES: std::sync::RwLock<Vec<(Vec2, Vec2, f32)>> = std::sync::RwLock::new(Vec::new());

pub fn hole_at(p: Vec3) -> Option<f32> {
    HOLES.read().ok()?.iter().find(|h| (h.0.x..h.1.x).contains(&p.x) && (h.0.y..h.1.y).contains(&p.y)).map(|h| h.2)
}

pub fn over_water(p: Vec3) -> bool {
    over_pool(p) || hole_at(p).is_some()
}

fn cut(mut r: Vec<(Vec2, Vec2)>) -> Vec<(Vec2, Vec2)> {
    for (a, b, _) in HOLES.read().map(|h| h.clone()).unwrap_or_default() {
        r = r.into_iter().flat_map(|(lo, hi)| {
            if a.x >= hi.x || b.x <= lo.x || a.y >= hi.y || b.y <= lo.y {
                return vec![(lo, hi)];
            }
            let (a, b) = (a.max(lo), b.min(hi));
            [(lo, Vec2::new(hi.x, a.y)), (Vec2::new(lo.x, b.y), hi), (Vec2::new(lo.x, a.y), Vec2::new(a.x, b.y)), (Vec2::new(b.x, a.y), Vec2::new(hi.x, b.y))]
                .into_iter().filter(|(l, h)| h.x - l.x > delirium_data::kv!(73) && h.y - l.y > delirium_data::kv!(73)).collect()
        }).collect();
    }
    r
}

pub fn floor_rects(pool: bool, half: f32) -> Vec<(Vec2, Vec2)> {
    let ([a, b], h) = ((*POOL()), half);
    if !pool {
        return vec![(Vec2::splat(-h), Vec2::splat(h))];
    }
    cut(vec![(Vec2::new(-h, -h), Vec2::new(h, a.y)), (Vec2::new(-h, b.y), Vec2::new(h, h)),
             (Vec2::new(-h, a.y), Vec2::new(a.x, b.y)), (Vec2::new(b.x, a.y), Vec2::new(h, b.y))])
}

pub fn floor_tris(pool: bool, centre: Vec2, half: f32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let (mut p, mut t) = (vec![], vec![]);
    let (c, h) = (centre.round().as_ivec2(), half as i32);
    let [a, b] = (*POOL());
    for x in c.x - h..c.x + h {
        for y in c.y - h..c.y + h {
            let (x, y) = (x as f32, y as f32);
            if pool && x >= a.x && x + 1.0 <= b.x && y >= a.y && y + 1.0 <= b.y {
                continue;
            }
            let tile = vec![(Vec2::new(x, y), Vec2::new(x + 1.0, y + 1.0))];
            for (lo, hi) in if pool { cut(tile) } else { tile } {
                let k = p.len() as u32;
                p.extend([[lo.x, lo.y, 0.0], [hi.x, lo.y, 0.0], [hi.x, hi.y, 0.0], [lo.x, hi.y, 0.0]]);
                t.extend([[k, k + 1, k + 2], [k, k + 2, k + delirium_data::kv!(0 int)]]);
            }
        }
    }
    (p, t)
}

fn floor_body(w: &mut PhysicsWorld, lo: Vec2, hi: Vec2) -> RigidBodyHandle {
    let (c, h) = ((lo + hi) * 0.5, (hi - lo) * 0.5);
    w.insert(RigidBodyBuilder::fixed().translation(Vector::new(c.x, c.y, -0.5)), ColliderBuilder::cuboid(h.x, h.y, 0.5).friction(1.0)).0
}

pub fn over_pool(p: Vec3) -> bool {
    ((*POOL())[0].x..(*POOL())[1].x).contains(&p.x) && ((*POOL())[0].y..(*POOL())[1].y).contains(&p.y)
}

#[allow(non_snake_case)]
#[inline]
pub fn KINDS() -> &'static [(&'static str, f32, f32, f32); 5] { static V: std::sync::OnceLock<[(&'static str, f32, f32, f32); 5]> = std::sync::OnceLock::new(); let f = || -> [(&'static str, f32, f32, f32); 5] { [("ball", delirium_data::kv!(9), 2.0, delirium_data::kv!(127)), ("box", delirium_data::kv!(60), delirium_data::kv!(65), delirium_data::kv!(138)), ("crate", delirium_data::kv!(10), delirium_data::kv!(11), 0.0),
    ("barrel", delirium_data::kv!(24), delirium_data::kv!(27), 0.0), ("wrecking ball", 0.5, delirium_data::kv!(759), 0.0)] }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn PART_KG() -> f32 { delirium_data::kv!(11) }
#[allow(non_snake_case)]
#[inline]
fn SPLIT_MAX() -> usize { delirium_data::kv!(804 int) }
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

pub type PieceBody = Option<RigidBodyHandle>;

pub struct Obj {
    pub kind: usize,
    h: [RigidBodyHandle; 2],
    pub hit: [bool; 2],
    pub anchor: Option<Vec3>,
    pub scale: f32,
}

#[derive(Clone, Default)]
pub struct StageEdit {
    pub stage: String,
    pub cut: Vec<(bool, usize)>,
    pub start: Option<Vec3>,
}
pub static STAGE_EDIT: std::sync::Mutex<Option<StageEdit>> = std::sync::Mutex::new(None);
#[allow(non_snake_case)]
#[inline]
pub fn GONE_Z() -> f32 { -delirium_data::kv!(760) }

pub struct Room {
    pub w: [PhysicsWorld; 2],
    #[allow(dead_code)]
    fixed: [Vec<RigidBodyHandle>; 2],
    parts: [Vec<(RigidBodyHandle, ColliderHandle)>; 2],
    pub objs: Vec<Obj>,
    pub loose: Vec<(RigidBodyHandle, usize, bool, f32)>,
    pub blocks: Vec<(Vec3, Vec3, u8)>,
    pub tilted: Vec<(Transform, Vec3, u8)>,
    pub surfaces: Vec<(String, f32, f32)>,
    pub weight: f32,
    pub stage: String,
    pub pool: bool,
    floor_n: usize,
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
pub fn PAD_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || -> Vec3 { Vec3::new(delirium_data::kv!(12), delirium_data::kv!(12), delirium_data::kv!(17)) }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
pub fn PLAT_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || -> Vec3 { Vec3::new(2.0, 2.0, delirium_data::kv!(17)) }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub fn plat(stage: &str, t: f32) -> Option<(Vec3, Quat)> {
    let s = |f: f32, p: f32| (std::f32::consts::TAU * f * t + p).sin();
    let go = |over: f32| ((t - 2.0) / over).clamp(0.0, 1.0);
    Some(match stage {
        "quake" => {
            let g = go(delirium_data::kv!(31));
            (Vec3::new(delirium_data::kv!(200) * s(delirium_data::kv!(1086), 0.0) + delirium_data::kv!(68) * s(delirium_data::kv!(1087), 1.0), delirium_data::kv!(131) * s(delirium_data::kv!(538), 2.0) + delirium_data::kv!(68) * s(delirium_data::kv!(1088), 0.5), delirium_data::kv!(132) * s(delirium_data::kv!(534), delirium_data::kv!(24))) * g + Vec3::Z * 0.5,
             Quat::from_euler(EulerRot::XYZ, g * delirium_data::kv!(132) * s(delirium_data::kv!(501), delirium_data::kv!(75)), g * delirium_data::kv!(132) * s(delirium_data::kv!(193), delirium_data::kv!(180)), 0.0))
        }
        "bull" => {
            let g = go(delirium_data::kv!(85));
            (Vec3::Z * (delirium_data::kv!(81) + g * delirium_data::kv!(143) * s(delirium_data::kv!(81), 0.5).abs()),
             Quat::from_euler(EulerRot::ZXY, g * delirium_data::kv!(20) * s(delirium_data::kv!(17), 0.0), g * delirium_data::kv!(17) * s(delirium_data::kv!(120), 2.0), g * delirium_data::kv!(17) * (s(delirium_data::kv!(125), 0.0) + delirium_data::kv!(117) * s(delirium_data::kv!(145), 1.0))))
        }
        _ => return None,
    })
}

pub const PLANKS: usize = 12;
#[allow(non_snake_case)]
#[inline]
pub fn BRIDGE_TOP() -> f32 { delirium_data::kv!(65) }
#[allow(non_snake_case)]
#[inline]
pub fn PLANK_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || -> Vec3 { Vec3::new(delirium_data::kv!(20), delirium_data::kv!(9), delirium_data::kv!(132)) }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn PIECE_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || -> Vec3 { Vec3::new(delirium_data::kv!(117), delirium_data::kv!(9), delirium_data::kv!(132)) }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn TEETH() -> usize { delirium_data::kv!(23 int) }
#[allow(non_snake_case)]
#[inline]
fn SPLINTER() -> f32 { delirium_data::kv!(147) }
#[allow(non_snake_case)]
#[inline]
fn SWING_DAMP() -> f32 { delirium_data::kv!(129) }
pub fn plank_y(i: usize) -> f32 {
    -delirium_data::kv!(60) - delirium_data::kv!(117) * i as f32
}
#[allow(non_snake_case)]
#[inline]
pub fn BRIDGE_FAR() -> f32 { -delirium_data::kv!(117) * PLANKS as f32 }
#[allow(non_snake_case)]
#[inline]
pub fn ROPE_X() -> f32 { delirium_data::kv!(125) }
#[allow(non_snake_case)]
#[inline]
pub fn ROPES() -> usize { 2 * (2 * PLANKS + 1) + 2 * (PLANKS + 1) + 2 * PLANKS + 2 * (*SIDE_ROPES()).len() * (PLANKS + 1) }
#[allow(non_snake_case)]
#[inline]
pub fn SIDE_ROPES() -> &'static [f32; 2] { static V: std::sync::OnceLock<[f32; 2]> = std::sync::OnceLock::new(); let f = || -> [f32; 2] { [1.0 / delirium_data::kv!(5), 2.0 / delirium_data::kv!(5)] }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn ROPE_REACH() -> f32 { delirium_data::kv!(79) }
#[allow(non_snake_case)]
#[inline]
pub fn POST_HALF() -> &'static Vec3 { static V: std::sync::OnceLock<Vec3> = std::sync::OnceLock::new(); let f = || -> Vec3 { Vec3::new(delirium_data::kv!(68), delirium_data::kv!(68), delirium_data::kv!(120)) }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn HAND() -> f32 { delirium_data::kv!(238) }
#[derive(Default)]
pub struct Alive { t: f32, seed: u32, mode: u32, until: f32, turn: f32, hopped: bool, v: Vec3, shiver: f32, cool: f32, step: f32, foot: f32 }

impl Alive {
    pub fn new(seed: u32) -> Alive {
        Alive { seed: seed.wrapping_mul(delirium_data::kv!(1089 int)) ^ delirium_data::kv!(1090 int), until: 1.0, ..Default::default() }
    }
}

#[allow(non_snake_case)]
#[inline]
fn SLACK() -> (f32, f32) { (delirium_data::kv!(1091), delirium_data::kv!(1092)) }
#[allow(non_snake_case)]
#[inline]
fn WIND_N() -> f32 { delirium_data::kv!(38) }
#[allow(non_snake_case)]
#[inline]
fn PLANK_KG() -> f32 { delirium_data::kv!(21) }
#[allow(non_snake_case)]
#[inline]
fn KNOT_KG() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
pub fn HOMER_KG() -> f32 { delirium_data::kv!(1093) }
#[allow(non_snake_case)]
#[inline]
pub fn HOMER_N() -> f32 { HOMER_KG() * delirium_data::kv!(116) }
#[allow(non_snake_case)]
#[inline]
fn ROTTEN() -> (usize, usize) { (2, delirium_data::kv!(1 int)) }
#[allow(non_snake_case)]
#[inline]
fn ROT_HOLDS() -> (f32, f32) { (delirium_data::kv!(759), delirium_data::kv!(760)) }
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
                let h = w.insert(RigidBodyBuilder::dynamic().translation(Vector::new(0.0, plank_y(i), z)).linear_damping(delirium_data::kv!(20)).angular_damping(delirium_data::kv!(65)),
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
        w.integration_parameters.num_solver_iterations = delirium_data::kv!(283 int);
        w.integration_parameters.dt = 1.0 / delirium_data::kv!(104);
        for _ in 0..delirium_data::kv!(303 int) {
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
                        if l.x.abs() < (*PIECE_HALF()).x + r && l.y.abs() < (*PIECE_HALF()).y + r && (-(*PIECE_HALF()).z - r..(*PIECE_HALF()).z + r + delirium_data::kv!(68)).contains(&l.z) {
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
            p.f += (f - p.f) * (dt / delirium_data::kv!(79)).min(1.0);
            p.load = p.f.length();
            let s = (ps[1].translation.distance(p.at) < ps[0].translation.distance(p.at)) as usize;
            w.bodies[p.h[s]].apply_impulse_at_point(rv(p.f * dt), rv(p.at), true);
        }
        self.t += dt;
        let wind = WIND_N() * (delirium_data::kv!(75) * self.t).sin() * (delirium_data::kv!(237) * self.t + 1.0).sin();
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
pub fn HALL() -> &'static [Vec2; 2] { static V: std::sync::OnceLock<[Vec2; 2]> = std::sync::OnceLock::new(); let f = || -> [Vec2; 2] { [Vec2::new(-delirium_data::kv!(5), -delirium_data::kv!(1094)), Vec2::new(delirium_data::kv!(5), delirium_data::kv!(35))] }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn HALL_TOP() -> f32 { delirium_data::kv!(85) }

#[allow(non_snake_case)]
#[inline]
pub fn QUARTERS() -> &'static [(Vec3, f32, f32, f32); 3] { static V: std::sync::OnceLock<[(Vec3, f32, f32, f32); 3]> = std::sync::OnceLock::new(); let f = || -> [(Vec3, f32, f32, f32); 3] { [(Vec3::new(-delirium_data::kv!(127), delirium_data::kv!(33), 0.0), std::f32::consts::FRAC_PI_2, delirium_data::kv!(33), delirium_data::kv!(65)),
    (Vec3::new(-delirium_data::kv!(138), delirium_data::kv!(33), 0.0), -std::f32::consts::FRAC_PI_2, delirium_data::kv!(33), delirium_data::kv!(65)), (Vec3::new(0.0, delirium_data::kv!(127), 0.0), 0.0, delirium_data::kv!(5), delirium_data::kv!(25))] }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn QUARTER_SLABS() -> usize { delirium_data::kv!(94 int) }

fn quarter(at: Vec3, yaw: f32, r: f32, wide: f32, row: u8) -> Vec<(Transform, Vec3, u8)> {
    let (q, n, t) = (Quat::from_rotation_z(yaw), QUARTER_SLABS(), delirium_data::kv!(79));
    let put = |c: Vec3, rot: Quat, h: Vec3, row| (Transform::from_translation(at + q * c).with_rotation(q * rot), h, row);
    (0..n).map(|i| {
        let k = std::f32::consts::FRAC_PI_2 / n as f32;
        let (a, b) = (i as f32 * k, (i + 1) as f32 * k);
        let (pa, pb) = (Vec3::new(0.0, r * a.sin(), r * (1.0 - a.cos())), Vec3::new(0.0, r * b.sin(), r * (1.0 - b.cos())));
        let m = (a + b) * 0.5;
        let up = Vec3::new(0.0, -m.sin(), m.cos());
        put((pa + pb) * 0.5 - up * t * 0.5, Quat::from_rotation_x(m), Vec3::new(wide * 0.5, (pb - pa).length() * 0.5 + delirium_data::kv!(119), t * 0.5), row)
    }).collect()
}

fn quarter_deck(at: Vec3, yaw: f32, r: f32, wide: f32, deck: f32, row: u8, metal: u8) -> Vec<(Transform, Vec3, u8)> {
    let q = Quat::from_rotation_z(yaw);
    let put = |c: Vec3, h: Vec3, row| (Transform::from_translation(at + q * c).with_rotation(q), h, row);
    vec![put(Vec3::new(0.0, r + deck * 0.5, r * 0.5), Vec3::new(wide * 0.5, deck * 0.5, r * 0.5), row),
         put(Vec3::new(0.0, r + delirium_data::kv!(132), r - delirium_data::kv!(132)), Vec3::new(wide * 0.5 + delirium_data::kv!(571), delirium_data::kv!(1095), delirium_data::kv!(1095)), metal)]
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
    let (q, n, x) = (Quat::from_rotation_z(yaw), delirium_data::kv!(189 int), wide * 0.5);
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
    let (q, a, t) = (Quat::from_rotation_z(yaw), deg.to_radians(), delirium_data::kv!(79));
    let up = Vec3::new(0.0, -a.sin(), a.cos());
    let top = Vec3::new(0.0, len * a.cos(), len * a.sin());
    let put = |c: Vec3, rot: Quat, h: Vec3| (Transform::from_translation(at + q * c).with_rotation(q * rot), h, row);
    vec![put(top * 0.5 - up * t * 0.5, Quat::from_rotation_x(a), Vec3::new(wide * 0.5, len * 0.5, t * 0.5)),
         put(Vec3::new(0.0, top.y - delirium_data::kv!(79), top.z * 0.5), Quat::IDENTITY, Vec3::new(wide * 0.5, delirium_data::kv!(79), top.z * 0.5))]
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
    v.extend(kicker(Vec3::new(0.0, delirium_data::kv!(85), 0.0), 0.0, delirium_data::kv!(492), delirium_data::kv!(937), delirium_data::kv!(33), row));
    let (f, hz) = (Vec3::new(delirium_data::kv!(65), delirium_data::kv!(85), 0.0), delirium_data::kv!(24));
    let put = |c: Vec3, h: Vec3, row| (Transform::from_translation(c), h, row);
    v.push(put(f + Vec3::Z * hz, Vec3::new(delirium_data::kv!(12), 2.0, hz), row));
    let deg = (2.0 * hz / delirium_data::kv!(208)).atan().to_degrees();
    let len = (delirium_data::kv!(208 f32).powi(2) + (2.0 * hz).powi(2)).sqrt();
    v.extend(kicker(f + Vec3::new(0.0, -delirium_data::kv!(1096), 0.0), 0.0, len, deg, delirium_data::kv!(5), row));
    v.extend(kicker(f + Vec3::new(0.0, delirium_data::kv!(1096), 0.0), PI, len, deg, delirium_data::kv!(5), row));
    v.push(put(f + Vec3::Z * (2.0 * hz + delirium_data::kv!(117)), Vec3::new(delirium_data::kv!(261), 2.0, delirium_data::kv!(261)), metal));
    v.extend([-delirium_data::kv!(492), delirium_data::kv!(492)].map(|y| put(f + Vec3::new(0.0, y, 2.0 * hz + delirium_data::kv!(60)), Vec3::new(delirium_data::kv!(119), delirium_data::kv!(119), delirium_data::kv!(60)), metal)));
    let b = Vec3::new(delirium_data::kv!(64), -delirium_data::kv!(35), 0.0);
    v.push(put(b + Vec3::Z * delirium_data::kv!(141), Vec3::new(2.0, delirium_data::kv!(261), delirium_data::kv!(261)), metal));
    v.extend([-delirium_data::kv!(492), delirium_data::kv!(492)].map(|x| put(b + Vec3::new(x, 0.0, delirium_data::kv!(236)), Vec3::new(delirium_data::kv!(119), delirium_data::kv!(119), delirium_data::kv!(236)), metal)));
    let s = Vec3::new(-delirium_data::kv!(85), -delirium_data::kv!(65), 0.0);
    let top = delirium_data::kv!(236) * delirium_data::kv!(64);
    v.push(put(s + Vec3::new(0.0, -delirium_data::kv!(12) - delirium_data::kv!(12), top * 0.5), Vec3::new(delirium_data::kv!(12), delirium_data::kv!(12), top * 0.5), row));
    for i in 0..delirium_data::kv!(1 int) {
        let z = top - delirium_data::kv!(236) * (i + 1) as f32;
        v.push(put(s + Vec3::new(0.0, -delirium_data::kv!(12) + delirium_data::kv!(24) * i as f32 + delirium_data::kv!(9), z * 0.5), Vec3::new(delirium_data::kv!(12), delirium_data::kv!(9), z * 0.5), row));
    }
    let (a, b) = (s + Vec3::new(delirium_data::kv!(81), -delirium_data::kv!(12), top + delirium_data::kv!(20)), s + Vec3::new(delirium_data::kv!(81), -delirium_data::kv!(24), delirium_data::kv!(236) + delirium_data::kv!(20)));
    v.push((Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())),
            Vec3::new(delirium_data::kv!(261), (b - a).length() * 0.5 + delirium_data::kv!(24), delirium_data::kv!(261)), metal));
    v.push(put(s + Vec3::new(delirium_data::kv!(81), -delirium_data::kv!(492), top + delirium_data::kv!(117)), Vec3::new(delirium_data::kv!(119), delirium_data::kv!(119), delirium_data::kv!(117)), metal));
    v.push(put(s + Vec3::new(delirium_data::kv!(81), 0.0, delirium_data::kv!(117)), Vec3::new(delirium_data::kv!(119), delirium_data::kv!(119), delirium_data::kv!(117)), metal));
    v
}

fn stage(name: &str, brick: u8, concrete: u8) -> Option<(Vec<(Vec3, Vec3, u8)>, Vec3)> {
    let b = |c: [f32; 3], h: [f32; 3], r| (Vec3::from(c), Vec3::from(h), r);
    Some(match name {
        "bigstairs" => {
            let mut v = vec![b([0.0, -delirium_data::kv!(10), delirium_data::kv!(5)], [delirium_data::kv!(12), delirium_data::kv!(655), delirium_data::kv!(5)], concrete)];
            v.extend([-delirium_data::kv!(492), delirium_data::kv!(492)].map(|x| b([x, -delirium_data::kv!(10), delirium_data::kv!(129)], [delirium_data::kv!(79), delirium_data::kv!(655), delirium_data::kv!(129)], brick)));
            for i in 0..delirium_data::kv!(428 int) {
                let (top, y) = (delirium_data::kv!(35) - delirium_data::kv!(9) * (i + 1) as f32, delirium_data::kv!(18) + delirium_data::kv!(24) * i as f32);
                v.push(b([0.0, y, top * 0.5], [delirium_data::kv!(12), delirium_data::kv!(9), top * 0.5], concrete));
                v.extend([-delirium_data::kv!(492), delirium_data::kv!(492)].map(|x| b([x, y, (top + 1.0) * 0.5], [delirium_data::kv!(79), delirium_data::kv!(9), (top + 1.0) * 0.5], brick)));
            }
            (v, Vec3::Z * delirium_data::kv!(35))
        }
        "stairsdown" => {
            let (v, start) = stage("bigstairs", brick, concrete)?;
            (v.into_iter().map(|(c, h, r)| (c * Vec3::new(1.0, -1.0, 1.0), h, r)).collect(), start)
        }
        "portalwall" => (vec![b([-delirium_data::kv!(500), 0.0, delirium_data::kv!(33)], [delirium_data::kv!(9), delirium_data::kv!(85), delirium_data::kv!(33)], concrete)], Vec3::new(delirium_data::kv!(1097), -delirium_data::kv!(1098), 0.0)),
        "rooftop" => (vec![b([0.0, -delirium_data::kv!(142), 2.0], [delirium_data::kv!(5), delirium_data::kv!(1099), 2.0], concrete)], Vec3::Z * delirium_data::kv!(85)),
        "highledge" => (vec![b([0.0, delirium_data::kv!(187), delirium_data::kv!(35)], [delirium_data::kv!(64), delirium_data::kv!(64), delirium_data::kv!(35)], concrete)], Vec3::Z * delirium_data::kv!(21)),
        "empty" => (vec![], Vec3::ZERO),
        "lowwall" => (vec![b([0.0, delirium_data::kv!(10), delirium_data::kv!(10)], [delirium_data::kv!(85), delirium_data::kv!(9), delirium_data::kv!(10)], brick)], Vec3::ZERO),
        "kerb" => (vec![b([0.0, delirium_data::kv!(10), delirium_data::kv!(207)], [delirium_data::kv!(85), delirium_data::kv!(9), delirium_data::kv!(207)], brick)], Vec3::ZERO),
        "open" => (vec![], Vec3::ZERO),
        "skatepark" => (vec![], Vec3::ZERO),
        "lights" => {
            let (c, h) = (((*HALL())[0] + (*HALL())[1]) * 0.5, ((*HALL())[1] - (*HALL())[0]) * 0.5);
            let t = delirium_data::kv!(9);
            (vec![b([c.x, c.y, -delirium_data::kv!(1100)], [h.x, h.y, delirium_data::kv!(79)], concrete), b([c.x, c.y, HALL_TOP() + t], [h.x + 2.0 * t, h.y + 2.0 * t, t], concrete),
                  b([(*HALL())[0].x - t, c.y, HALL_TOP() * 0.5], [t, h.y, HALL_TOP() * 0.5], concrete), b([(*HALL())[1].x + t, c.y, HALL_TOP() * 0.5], [t, h.y, HALL_TOP() * 0.5], concrete),
                  b([c.x, (*HALL())[0].y - t, HALL_TOP() * 0.5], [h.x + 2.0 * t, t, HALL_TOP() * 0.5], concrete),
                  b([c.x, (*HALL())[1].y + t, HALL_TOP() * 0.5], [h.x + 2.0 * t, t, HALL_TOP() * 0.5], concrete)], Vec3::ZERO)
        }
        "rail" => (vec![b([0.0, -0.5, delirium_data::kv!(1101)], [delirium_data::kv!(12), delirium_data::kv!(132), delirium_data::kv!(132)], concrete), b([-delirium_data::kv!(12), -0.5, delirium_data::kv!(120)], [delirium_data::kv!(132), delirium_data::kv!(132), delirium_data::kv!(120)], concrete),
                        b([delirium_data::kv!(12), -0.5, delirium_data::kv!(120)], [delirium_data::kv!(132), delirium_data::kv!(132), delirium_data::kv!(120)], concrete)], Vec3::ZERO),
        "street" => {
            let mut v = vec![b([0.0, delirium_data::kv!(35), delirium_data::kv!(233)], [delirium_data::kv!(27), 2.0, delirium_data::kv!(233)], concrete), b([0.0, -delirium_data::kv!(35), delirium_data::kv!(233)], [delirium_data::kv!(27), 2.0, delirium_data::kv!(233)], concrete)];
            v.extend((0..delirium_data::kv!(23 int)).map(|i| b([0.0, -delirium_data::kv!(129) + i as f32, delirium_data::kv!(578)], [delirium_data::kv!(12), delirium_data::kv!(17), delirium_data::kv!(578)], concrete)));
            (v, Vec3::ZERO)
        }
        "bridge" => {
            let mut v = vec![b([0.0, delirium_data::kv!(5), delirium_data::kv!(85)], [2.0, delirium_data::kv!(5), delirium_data::kv!(85)], concrete), b([0.0, BRIDGE_FAR() - delirium_data::kv!(5), delirium_data::kv!(85)], [2.0, delirium_data::kv!(5), delirium_data::kv!(85)], concrete)];
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
        self.blocks.iter().filter(|(c, h, _)| (p.x - c.x).abs() <= h.x && (p.y - c.y).abs() <= h.y && c.z + h.z <= p.z + delirium_data::kv!(68))
            .map(|(c, h, _)| c.z + h.z).fold(if self.pool && over_pool(p) { -POOL_DEEP() } else if self.pool { hole_at(p).map_or(0.0, |d| -d) } else { 0.0 }, f32::max)
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
        let g = [0.0, delirium_data::kv!(24), delirium_data::kv!(120), delirium_data::kv!(28), delirium_data::kv!(137)].map(|d| self.ground(p + dir * d));
        g.windows(2).all(|w| (0.0..0.4).contains(&(w[0] - w[1]))) && g[0] - g[delirium_data::kv!(1 int)] > delirium_data::kv!(24)
    }

    pub fn new(surfaces: Vec<(String, f32, f32)>, name: &str) -> Room {
        let (blocks, tilted, start) = Room::own(&surfaces, name);
        Room::build(surfaces, name, blocks, tilted, start)
    }

    #[allow(clippy::type_complexity)]
    fn own(surfaces: &[(String, f32, f32)], name: &str) -> (Vec<(Vec3, Vec3, u8)>, Vec<(Transform, Vec3, u8)>, Vec3) {
        let (mut blocks, mut tilted, mut start) = Room::stage_boxes(surfaces, name);
        if let Some(e) = STAGE_EDIT.lock().unwrap().as_ref().filter(|e| e.stage == name) {
            for &(t, i) in &e.cut {
                if t {
                    if let Some(b) = tilted.get_mut(i) {
                        (b.0, b.1) = (Transform::from_xyz(0.0, 0.0, GONE_Z()), Vec3::splat(delirium_data::kv!(135)));
                    }
                } else if let Some(b) = blocks.get_mut(i) {
                    (b.0, b.1) = (Vec3::new(0.0, 0.0, GONE_Z()), Vec3::splat(delirium_data::kv!(135)));
                }
            }
            if let Some(s) = e.start {
                start = s;
            }
        }
        (blocks, tilted, start)
    }

    fn build(surfaces: Vec<(String, f32, f32)>, name: &str, blocks: Vec<(Vec3, Vec3, u8)>, tilted: Vec<(Transform, Vec3, u8)>, start: Vec3) -> Room {
        let row = |n: &str| surfaces.iter().position(|s| s.0 == n).unwrap_or(0) as u8;
        let pool = stage(name, 0, 0).is_none();
        let mut w = [PhysicsWorld::new(), PhysicsWorld::new()];
        let mut fixed = [vec![], vec![]];
        for (i, w) in w.iter_mut().enumerate() {
            w.gravity = Vector::new(0.0, 0.0, -delirium_data::kv!(116));
            for (lo, hi) in floor_rects(pool && i == 0, delirium_data::kv!(1102)) {
                fixed[i].push(floor_body(w, lo, hi));
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
                ColliderBuilder::capsule_from_endpoints(rv(a), rv(b), crate::fluid_gpu::PIPE_R).friction(delirium_data::kv!(120))).0).collect()
        } else { vec![] };
        fixed[0].extend(&faucet);
        let snap = (name == "trampoline").then_some(0);
        let seed = std::env::var("BRIDGE_SEED").ok().and_then(|s| s.parse().ok())
            .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos()));
        let bridge = (name == "bridge").then(|| Bridge::new(&mut w[0], seed));
        let floor_n = floor_rects(pool, 1.0).len();
        Room { floor_n, w, fixed, parts: [vec![], vec![]], objs: vec![], loose: vec![], blocks, tilted, surfaces, weight: 1.0, stage: name.to_string(), pool, start, snap, snapped: None, fell: false, bounced: delirium_data::kv!(138), shook: 0.0,
               bridge, broke: vec![], push: Vec3::NEG_Z * HOMER_N(), com_v: Vec3::ZERO, com_a: Vec3::ZERO, #[cfg(feature = "portal")] holes: vec![], flow: vec![], faucet }
    }

    #[allow(clippy::type_complexity)]
    pub fn stage_boxes(surfaces: &[(String, f32, f32)], name: &str) -> (Vec<(Vec3, Vec3, u8)>, Vec<(Transform, Vec3, u8)>, Vec3) {
        let row = |n: &str| surfaces.iter().position(|s| s.0 == n).unwrap_or(0) as u8;
        let (blocks, start) = stage(name, row("BRICK_WALL"), row("CONCRETE")).unwrap_or_else(|| (Room::blocks(&row, name == "pttest"), Vec3::ZERO));
        let tilted = if name == "skatepark" { park(row("CONCRETE"), surfaces.iter().position(|s| s.0.starts_with("METAL")).unwrap_or(0) as u8) } else { vec![] };
        (blocks, tilted, start)
    }

    fn blocks(row: &dyn Fn(&str) -> u8, glass: bool) -> Vec<(Vec3, Vec3, u8)> {
        let mut blocks = vec![(Vec3::new(0.0, delirium_data::kv!(129), 0.5), Vec3::new(delirium_data::kv!(85), delirium_data::kv!(9), 0.5), row("BRICK_WALL"))];
        for i in 0..delirium_data::kv!(82 int) {
            let top = delirium_data::kv!(143) * (i + 1) as f32;
            blocks.push((Vec3::new(-delirium_data::kv!(1103), -delirium_data::kv!(12) + delirium_data::kv!(24) * i as f32 + delirium_data::kv!(9), top * 0.5), Vec3::new(delirium_data::kv!(18), delirium_data::kv!(9), top * 0.5), row("CONCRETE")));
        }
        blocks.push((Vec3::new(-delirium_data::kv!(1103), 2.0, delirium_data::kv!(28)), Vec3::new(delirium_data::kv!(18), 0.5, delirium_data::kv!(28)), row("CONCRETE")));
        let ([a, b], d, rim) = ((*POOL()), POOL_DEEP(), delirium_data::kv!(24));
        let (m, hw, z) = ((a + b) * 0.5, (b - a) * 0.5, -(d + delirium_data::kv!(60)) * 0.5 - delirium_data::kv!(135));
        for (c, h) in [(Vec2::new(a.x - rim * 0.5, m.y), Vec2::new(rim * 0.5, hw.y + rim)), (Vec2::new(b.x + rim * 0.5, m.y), Vec2::new(rim * 0.5, hw.y + rim)),
                       (Vec2::new(m.x, a.y - rim * 0.5), Vec2::new(hw.x, rim * 0.5)), (Vec2::new(m.x, b.y + rim * 0.5), Vec2::new(hw.x, rim * 0.5))] {
            blocks.push((c.extend(z), h.extend(z.abs() - delirium_data::kv!(135)), row("CONCRETE")));
        }
        blocks.push((m.extend(-d - delirium_data::kv!(79)), hw.extend(delirium_data::kv!(79)), row("CONCRETE")));
        for i in 0..delirium_data::kv!(23 int) {
            let top = -delirium_data::kv!(143) * (i + 1) as f32;
            blocks.push((Vec3::new(b.x - delirium_data::kv!(24) * i as f32 - delirium_data::kv!(9), a.y + delirium_data::kv!(120), (top - d) * 0.5), Vec3::new(delirium_data::kv!(9), delirium_data::kv!(120), (top + d) * 0.5), row("CONCRETE")));
        }
        #[cfg(feature = "portal")]
        blocks.extend([(Vec3::new(delirium_data::kv!(31), 0.0, delirium_data::kv!(5)), Vec3::new(delirium_data::kv!(9), delirium_data::kv!(85), delirium_data::kv!(5)), row("CONCRETE")),
                       (Vec3::new(delirium_data::kv!(35), delirium_data::kv!(5), delirium_data::kv!(64)), Vec3::new(delirium_data::kv!(5), delirium_data::kv!(5), delirium_data::kv!(79)), row("CONCRETE"))].into_iter().skip(glass as usize));
        if glass {
            blocks.extend((0..delirium_data::kv!(461 int)).map(|i| (Vec3::new(delirium_data::kv!(31), -delirium_data::kv!(1103) + 0.5 * (i % delirium_data::kv!(54 int)) as f32, delirium_data::kv!(17) + 0.5 * (i / delirium_data::kv!(54 int)) as f32), Vec3::splat(GLASS_HALF()), row("CONCRETE"))));
            let (at, half, deep, g) = (crate::SKY_POOL, crate::SKY_HALF, crate::SKY_DEEP, delirium_data::kv!(24));
            blocks.extend([(Vec3::new(0.0, 0.0, -g * 0.5), half.extend(g * 0.5) + Vec3::new(g, g, 0.0)),
                           (Vec3::new(half.x + g * 0.5, 0.0, deep * delirium_data::kv!(120)), Vec3::new(g * 0.5, half.y + g, deep * delirium_data::kv!(120))),
                           (Vec3::new(-half.x - g * 0.5, 0.0, deep * delirium_data::kv!(120)), Vec3::new(g * 0.5, half.y + g, deep * delirium_data::kv!(120))),
                           (Vec3::new(0.0, half.y + g * 0.5, deep * delirium_data::kv!(120)), Vec3::new(half.x, g * 0.5, deep * delirium_data::kv!(120))),
                           (Vec3::new(0.0, -half.y - g * 0.5, deep * delirium_data::kv!(120)), Vec3::new(half.x, g * 0.5, deep * delirium_data::kv!(120)))].map(|(c, h)| (at + c, h, row("CONCRETE"))));
        }
        blocks
    }

    pub fn recut_floor(&mut self) {
        let w = &mut self.w[0];
        for h in self.fixed[0].drain(..self.floor_n).collect::<Vec<_>>() {
            w.remove_body_with_colliders(h, true);
        }
        let floor: Vec<_> = floor_rects(self.pool, delirium_data::kv!(1102)).into_iter().map(|(lo, hi)| floor_body(w, lo, hi)).collect();
        self.floor_n = floor.len();
        self.fixed[0].splice(0..0, floor);
    }

    pub fn add_piece(&mut self, v: &[[f32; 3]], tris: &[[u32; 3]]) -> PieceBody {
        ColliderBuilder::trimesh(v.iter().map(|&p| rv(Vec3::from(p))).collect(), tris.to_vec()).ok()
            .map(|c| self.w[0].insert(RigidBodyBuilder::fixed(), c.friction(1.0)).0)
    }

    pub fn add_loose(&mut self, local: &[Vec3], tris: &[[u32; 3]], at: Vec3, rot: Quat, kg: f32, friction: f32, bounce: f32) -> PieceBody {
        let pts: Vec<_> = local.iter().map(|&p| rv(p)).collect();
        let t0 = std::time::Instant::now();
        let c = if tris.len() <= SPLIT_MAX() { ColliderBuilder::convex_decomposition(&pts, tris) } else { ColliderBuilder::convex_hull(&pts)? };
        let parts = c.shape.as_compound().map_or(1, |k| k.shapes().len());
        eprintln!("loose  {} triangles as {parts} convex part(s), {:.0} ms", tris.len(), t0.elapsed().as_secs_f32() * delirium_data::kv!(83));
        let (axis, angle) = rot.to_axis_angle();
        let b = RigidBodyBuilder::dynamic().translation(rv(at)).rotation(rv(axis * angle)).ccd_enabled(true);
        Some(self.w[0].insert(b, c.mass(kg.max(delirium_data::kv!(51))).friction(friction).restitution(bounce)).0)
    }

    pub fn alive_step(&mut self, h: RigidBodyHandle, a: &mut Alive, placed: Quat, homer: Vec3, dt: f32) -> Option<&'static str> {
        let b = &mut self.w[0].bodies[h];
        let r = b.rotation();
        let q = Quat::from_xyzw(r.x, r.y, r.z, r.w);
        let (at, mut v, mut w) = (bv(b.translation()), bv(b.linvel()), bv(b.angvel()));
        let mut said = None;
        a.t += dt;
        if (v - a.v).length() > delirium_data::kv!(5) && a.cool < 1.0 && a.t > 1.0 {
            a.shiver = delirium_data::kv!(120);
            said = Some("flinches");
        }
        let up = q * (placed.inverse() * Vec3::Z);
        let fwd = (q * (placed.inverse() * Vec3::X)).with_z(0.0).normalize_or(Vec3::X);
        let tilt = up.angle_between(Vec3::Z);
        let ground = v.z.abs() < delirium_data::kv!(17);
        if a.t > a.until {
            a.seed = a.seed.wrapping_mul(delirium_data::kv!(122 int)).wrapping_add(delirium_data::kv!(123 int));
            let r = |k: u32| ((a.seed >> k) & delirium_data::kv!(939 int)) as f32 / delirium_data::kv!(1104);
            a.mode = (a.seed >> delirium_data::kv!(128 int)) % delirium_data::kv!(0 int);
            a.until = a.t + 2.0 + delirium_data::kv!(5) * r(delirium_data::kv!(1 int));
            a.turn = (r(delirium_data::kv!(55 int)) - 0.5) * delirium_data::kv!(137);
            a.hopped = false;
            said = said.or(Some(["breathes", "waddles", "hops"][a.mode as usize]));
        }
        let off = (at - homer).with_z(0.0);
        let side = Vec3::Z.cross(fwd);
        let mut lean = match a.mode {
            0 => side * delirium_data::kv!(261) * (a.t * delirium_data::kv!(66)).sin() + fwd * delirium_data::kv!(586) * (a.t * delirium_data::kv!(28)).sin(),
            1 => Vec3::ZERO,
            _ => Vec3::ZERO,
        };
        if tilt < 0.5 && ground {
            match a.mode {
                1 if a.t > a.step => {
                    a.step = a.t + delirium_data::kv!(493);
                    a.foot = -a.foot.signum();
                    v += Vec3::Z * delirium_data::kv!(28) + fwd * delirium_data::kv!(133);
                    w += fwd * a.foot * delirium_data::kv!(492);
                    let away = off.normalize_or(fwd);
                    let turn = if off.length() < delirium_data::kv!(33) { fwd.cross(away).z.signum() * delirium_data::kv!(137) } else { a.turn };
                    w.z += turn - w.z;
                }
                2 if !a.hopped => {
                    v += Vec3::Z * delirium_data::kv!(560) + fwd * delirium_data::kv!(120);
                    w += side * 2.0;
                    a.hopped = true;
                }
                _ => {}
            }
        }
        if a.shiver > 0.0 {
            a.shiver -= dt;
            lean += Vec3::new((a.t * delirium_data::kv!(1105)).sin(), (a.t * delirium_data::kv!(1106)).cos(), 0.0) * delirium_data::kv!(147);
        }
        a.cool -= dt;
        if off.length() < delirium_data::kv!(81) && a.cool < 0.0 && ground && tilt < 0.5 {
            v += off.normalize_or(fwd) * delirium_data::kv!(492) + Vec3::Z * delirium_data::kv!(537);
            (a.cool, a.shiver) = (delirium_data::kv!(492), delirium_data::kv!(117));
            said = Some("hops away from Homer");
        }
        if tilt > 1.0 && ground && v.length() < delirium_data::kv!(117) && a.cool < 0.0 {
            v += Vec3::Z * delirium_data::kv!(503);
            a.cool = delirium_data::kv!(137);
            said = Some("rights itself");
        }
        let want = Quat::from_scaled_axis(lean) * Vec3::Z;
        let err = up.cross(want);
        let spin = w - Vec3::Z * w.z;
        let k = if tilt > 1.0 { delirium_data::kv!(757) } else { delirium_data::kv!(46) };
        w += ((err * k).clamp_length_max(delirium_data::kv!(63)) - spin * delirium_data::kv!(127)) * dt;
        b.set_linvel(rv(v), true);
        b.set_angvel(rv(w), true);
        a.v = v;
        said
    }

    pub fn body_at(&self, h: RigidBodyHandle) -> Transform {
        let b = &self.w[0].bodies[h];
        let q = b.rotation();
        Transform::from_translation(bv(b.translation())).with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w))
    }

    pub fn hold(&mut self, h: RigidBodyHandle, at: Vec3, rot: Quat) {
        let b = &mut self.w[0].bodies[h];
        b.set_translation(rv(at), true);
        b.set_rotation(rapier3d::math::Rotation::from_xyzw(rot.x, rot.y, rot.z, rot.w), true);
        b.set_linvel(rv(Vec3::ZERO), true);
        b.set_angvel(rv(Vec3::ZERO), true);
    }

    pub fn remove_piece(&mut self, b: PieceBody) {
        if let Some(h) = b {
            self.w[0].remove_body_with_colliders(h, true);
        }
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
            let mut b = RigidBodyBuilder::dynamic().translation(rv(from[i])).linvel(rv(vel)).angvel(Vector::new(delirium_data::kv!(12), -2.0, delirium_data::kv!(75)) * spin as u8 as f32).ccd_enabled(true);
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
            let h = w.insert(b, c.mass(kg).friction(delirium_data::kv!(120)).restitution(delirium_data::kv!(60))).0;
            if let Some(a) = anchor {
                let a = from[i] + (a - from[0]);
                let pin = w.insert_body(RigidBodyBuilder::fixed().translation(rv(a)));
                w.insert_impulse_joint(pin, h, rapier3d::prelude::RopeJointBuilder::new(a.distance(from[i])));
            }
            h
        });
        self.objs.push(Obj { kind, h, hit: [false; 2], anchor, scale: 1.0 });
    }

    pub fn blast(&mut self, at: [Vec3; 2], dv: f32, r: f32) {
        for o in &self.objs {
            for i in 0..2 {
                let b = &mut self.w[i].bodies[o.h[i]];
                let d = bv(b.translation()) - at[i];
                let f = 1.0 - d.length() / r;
                if f > 0.0 {
                    let push = (d.normalize_or(Vec3::Z) + Vec3::Z * delirium_data::kv!(120)).normalize() * dv * f * b.mass();
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

    pub fn set_obj(&mut self, j: usize, pos: Vec3, rot: Quat, scale: f32) {
        let o = &mut self.objs[j];
        let (_, s, kg, _) = (*KINDS())[o.kind];
        let rescale = (o.scale - scale).abs() > delirium_data::kv!(4);
        o.scale = scale;
        let off = pos - bv(self.w[0].bodies[o.h[0]].translation());
        for i in 0..2 {
            let w = &mut self.w[i];
            let b = &mut w.bodies[o.h[i]];
            let t = bv(b.translation()) + off;
            b.set_translation(rv(t), true);
            b.set_rotation(rapier3d::math::Rotation::from_xyzw(rot.x, rot.y, rot.z, rot.w), true);
            b.set_linvel(rv(Vec3::ZERO), true);
            b.set_angvel(rv(Vec3::ZERO), true);
            if rescale {
                for c in b.colliders().to_vec() {
                    if let Some(col) = w.colliders.get_mut(c) {
                        col.set_shape(match o.kind {
                            0 | 4 => SharedShape::ball(s * scale),
                            3 => SharedShape::cylinder(BARREL_HALF() * scale, s * scale),
                            _ => SharedShape::cuboid(s * scale, s * scale, s * scale),
                        });
                        col.set_mass(kg * self.weight * scale.powi(3));
                    }
                }
            }
        }
    }

    pub fn obj_body(&self, j: usize) -> ((Vec3, Vec3, f32), f32, Vec3) {
        let (t, v) = self.obj(0, j);
        let (kind, k) = (self.objs[j].kind, self.objs[j].scale);
        let s = (*KINDS())[kind].1 * k;
        let half = if kind == delirium_data::kv!(0 int) { t.rotation * Vec3::Y * (BARREL_HALF() * k - s) } else { Vec3::ZERO };
        let vol = match kind {
            0 | 4 => delirium_data::kv!(85) / delirium_data::kv!(5) * std::f32::consts::PI * s * s * s,
            3 => std::f32::consts::PI * s * s * 2.0 * BARREL_HALF() * k,
            _ => delirium_data::kv!(65) * s * s * s,
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
            w.bodies[*b].set_position(pose(&Transform::from_xyz(0.0, 0.0, -delirium_data::kv!(83))), true);
        }
        let mut part_v = vec![];
        for ((b, c), (t, r, h)) in self.parts[i].iter().zip(caps) {
            if let Some(col) = w.colliders.get_mut(*c) && col.shape().as_capsule().is_some_and(|s| (s.radius - r).abs() > delirium_data::kv!(73) || (s.half_height() - h).abs() > delirium_data::kv!(73)) {
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
                if under > 0.0 && over_water(at) {
                    let (m, v) = (body.mass(), bv(body.linvel()));
                    body.apply_impulse(rv((Vec3::Z * m * delirium_data::kv!(116) * 2.0 - v * m * 2.0) * under * dt), true);
                    let spin = bv(body.angvel());
                    body.apply_torque_impulse(rv(-spin * m * r * r * under * dt), true);
                }
            }
            for &(h, _, _, float) in &self.loose {
                let Some(r) = w.bodies[h].colliders().first().map(|c| w.colliders[*c].compute_aabb().half_extents().z) else { continue };
                let body = &mut w.bodies[h];
                let at = bv(body.translation());
                let under = ((WATER() - at.z + r) / (2.0 * r)).clamp(0.0, 1.0);
                if under > 0.0 && over_water(at) {
                    let (m, v) = (body.mass(), bv(body.linvel()));
                    body.apply_impulse(rv((Vec3::Z * m * delirium_data::kv!(116) * float - v * m * 2.0) * under * dt), true);
                    let spin = bv(body.angvel());
                    body.apply_torque_impulse(rv(-spin * m * r * r * under * dt), true);
                }
            }
        }
        let hs: Vec<RigidBodyHandle> = self.objs.iter().map(|o| o.h[i]).chain(self.loose.iter().filter(|_| i == 0).map(|l| l.0)).collect();
        let obj_v: Vec<Vec3> = hs.iter().map(|h| bv(w.bodies[*h].linvel())).collect();
        w.integration_parameters.dt = dt;
        #[cfg(feature = "portal")]
        w.step_with_events(&Holes(&self.holes), &());
        #[cfg(not(feature = "portal"))]
        w.step();
        let mut out = vec![];
        for (k, ((_, c), (t, r, h))) in self.parts[i].iter().zip(caps).enumerate() {
            for pair in w.narrow_phase.contact_pairs_with(*c) {
                let other = if pair.collider1 == *c { pair.collider2 } else { pair.collider1 };
                let Some(j) = w.colliders.get(other).and_then(|o| o.parent()).and_then(|b| hs.iter().position(|h| *h == b)) else { continue };
                let imp = bv(pair.total_impulse());
                if imp.length_squared() < delirium_data::kv!(251) {
                    continue;
                }
                let at = bv(w.bodies[hs[j]].translation());
                let axis = t.rotation * Vec3::Y;
                let on = t.translation + axis * (at - t.translation).dot(axis).clamp(-*h, *h);
                let hit = on + (at - on).normalize_or_zero() * *r;
                let imp = if imp.dot(t.translation - at) < 0.0 { -imp } else { imp };
                let m = w.bodies[hs[j]].mass();
                let closing = (obj_v[j] - part_v[k]).dot(imp.normalize_or_zero()).max(0.0);
                let imp = imp.clamp_length_max(m * (closing + delirium_data::kv!(116) * dt));
                let obj = &mut w.bodies[hs[j]];
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

