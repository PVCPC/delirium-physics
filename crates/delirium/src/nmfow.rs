use bevy_math::Vec3;
use crate::Vd;
use crate::Vx;

use crate::nmrs::{NECK, SPINE};
use crate::nmshot::{limb, Ctx};

const PELVIS: usize = 0;

#[derive(Clone, Copy, Debug)]
pub struct FallOverWallParams {
    pub fall_over_point: Vec3,
    pub fall_over_edge: Vec3,
    pub damping: f32,
    pub body_stiffness: f32,
    pub mag_of_force: f32,
    pub max_time: f32,
    pub min_up: f32,
    pub max_distance_from_pel_to_hit_point: f32,
    pub twist_min: f32,
    pub twist_max: f32,
    pub use_specified_point: bool,
}

impl Default for FallOverWallParams {
    fn default() -> FallOverWallParams {
        FallOverWallParams { fall_over_point: Vec3::ZERO, fall_over_edge: Vec3::ZERO, damping: 0.5, body_stiffness: delirium_data::kv!(137),
                             mag_of_force: delirium_data::kv!(24), max_time: 1.0, min_up: -delirium_data::kv!(60), max_distance_from_pel_to_hit_point: delirium_data::kv!(17),
                             twist_min: delirium_data::kv!(38), twist_max: delirium_data::kv!(503), use_specified_point: false }
    }
}

impl FallOverWallParams {
    pub fn body_twist(&mut self, t: f32) {
        let t = t.clamp(0.0, 1.0);
        (self.twist_min, self.twist_max) = (t * delirium_data::kv!(503), t * delirium_data::kv!(195));
    }
}

#[derive(Clone, Debug)]
pub struct FallOverWall {
    pub p: FallOverWallParams,
    edge: Vec3,
    at: Vec3,
    t: f32,
    vb: f32,
    vu: f32,
    split: f32,
    force: f32,
    found: bool,
    side: bool,
    pub active: bool,
}

impl Default for FallOverWall {
    fn default() -> FallOverWall {
        FallOverWall { p: FallOverWallParams::default(), edge: Vec3::ZERO, at: Vec3::ZERO, t: 0.0, vb: 0.0, vu: 0.0, split: 0.0,
                       force: delirium_data::kv!(48), found: false, side: false, active: false }
    }
}

fn touching(c: &Ctx, k: usize) -> bool {
    c.b.doll.touch[k] & delirium_data::kv!(95 int) == delirium_data::kv!(7 int)
}

fn rise(v: Vec3, up: Vec3) -> Vec3 {
    v - (v - up * up.vd(v))
}

impl FallOverWall {
    fn init(&mut self) {
        *self = FallOverWall { p: self.p, ..FallOverWall::default() };
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn deactivate(&mut self) {
        if self.active {
            self.init();
        }
    }

    fn push(&self, c: &mut Ctx, k: usize, mass: f32, scale: f32) {
        if let Some(f) = self.push_force(c.b.part(k).m.d, c.b.doll.part_mass(k), mass, scale) {
            c.out.force(k, f);
        }
    }

    fn push_force(&self, pos: Vec3, mk: f32, mass: f32, scale: f32) -> Option<Vec3> {
        let d = pos - self.at;
        let mut dir = self.edge.vx(crate::nmdbm::vnorm(d));
        if dir.z < 0.0 {
            dir.z = 0.0;
        }
        let f = (mk / mass * self.force * self.p.mag_of_force).clamp(-delirium_data::kv!(504), delirium_data::kv!(504)) * scale * delirium_data::kv!(103);
        (d.y.mul_add(d.y, d.x * d.x).sqrt() < self.p.max_distance_from_pel_to_hit_point * 2.0 && d.z > 0.0).then_some(dir * f)
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) -> bool {
        if !self.active {
            return false;
        }
        let p = self.p;
        let up = c.up;
        let pel = c.b.part(PELVIS).m.d;
        let spine: Vec<usize> = (*SPINE()).iter().map(|&j| c.ch.joints[j].child).collect();
        if self.found {
            self.t += dt;
        } else {
            if !p.use_specified_point {
                let near = |c: &Ctx, k: usize, flat_only: bool| -> Option<bool> {
                    if !touching(c, k) {
                        return None;
                    }
                    let (pt, _, _) = c.b.doll.hit[k]?;
                    let d = pel - pt;
                    Some(if flat_only { (d.x * d.x + d.y * d.y).sqrt() < p.max_distance_from_pel_to_hit_point && d.z < delirium_data::kv!(116) }
                         else { d.length() < p.max_distance_from_pel_to_hit_point })
                };
                let mut normal = Vec3::ZERO;
                let mut take = |c: &Ctx, k: usize| {
                    if let Some((_, n, _)) = touching(c, k).then(|| c.b.doll.hit[k]).flatten() {
                        normal = n;
                    }
                };
                for k in [PELVIS, spine[0], spine[1]] {
                    if touching(c, k) {
                        take(c, k);
                        self.found = true;
                        break;
                    }
                }
                let (l0, l1, l2, l3) = (limb(0), limb(1), limb(2), limb(delirium_data::kv!(0 int)));
                for (k, fl) in [(l2.p[1], true), (l3.p[1], true), (l1.p[2], false), (l0.p[2], false), (l0.p[delirium_data::kv!(0 int)], false), (l1.p[delirium_data::kv!(0 int)], false)] {
                    if self.found {
                        break;
                    }
                    take(c, k);
                    if near(c, k, fl) == Some(true) {
                        self.found = true;
                    }
                }
                self.at = c.b.part(spine[1]).m.d;
                if self.found {
                    let feet = [l2.p[delirium_data::kv!(0 int)], l3.p[delirium_data::kv!(0 int)]].map(|k| rise(c.b.part(k).m.d - pel, up).length() > 0.5);
                    if feet[0] && feet[1] && up.vd(normal).abs() < delirium_data::kv!(74) {
                        self.edge = crate::nmdbm::vnorm(normal.vx(up));
                        let (pm, v) = (c.b.part(PELVIS).m, crate::nmdbm::vnorm(c.vel));
                        let along = |r: Vec3| v.z.mul_add(r.z, v.x.mul_add(r.x, v.y * r.y));
                        (self.vb, self.vu) = (along(pm.c), along(pm.b));
                    } else {
                        self.found = false;
                    }
                }
            } else if (pel - p.fall_over_point).length() < p.max_distance_from_pel_to_hit_point {
                self.found = true;
                self.at = p.fall_over_point;
                self.edge = crate::nmdbm::vnorm(p.fall_over_edge);
                self.force *= 1.0 / delirium_data::kv!(201);
                if (pel - self.at).normalize_or_zero().vx(self.edge).vd(up) > 0.0 {
                    self.force = -self.force;
                }
            }
            if self.found {
                c.ch.set_stiffness(p.body_stiffness, p.damping, "fb", None);
                self.split = (c.ch.random_bits() & delirium_data::kv!(196 int)) as f32 * delirium_data::kv!(505) - delirium_data::kv!(60);
                self.side = c.ch.random_bits() & 1 != 0;
            }
        }
        if !self.found {
            return false;
        }
        let tip = c.m.b.vd(up);
        if tip < p.min_up || self.t > p.max_time {
            c.out.send(delirium_data::kv!(55 int), "FallOverWall", 0.0);
            self.deactivate();
            return true;
        }
        let a = (std::f32::consts::PI - delirium_data::kv!(24)) as f64;
        let (s, co) = (delirium_data::mathrt::sin(a) as f32, delirium_data::mathrt::cos(a) as f32);
        for (i, elbow, cl, sh) in [(0, delirium_data::kv!(506), [s.mul_add(delirium_data::kv!(119), -delirium_data::kv!(60)), co - 0.5], [s * 0.5, co * delirium_data::kv!(12), s.mul_add(-delirium_data::kv!(33), -1.0)]),
                                   (1, delirium_data::kv!(507), [s.mul_add(delirium_data::kv!(508), -delirium_data::kv!(145)), co - delirium_data::kv!(74)], [s * delirium_data::kv!(116), co * delirium_data::kv!(491), s.mul_add(-delirium_data::kv!(509), -delirium_data::kv!(138))])] {
            let l = limb(i);
            c.ch.eff[l.e[2]].desired[0] = elbow;
            (c.ch.eff[l.e[0]].desired[0], c.ch.eff[l.e[0]].desired[1]) = (cl[0], cl[1]);
            c.ch.eff[l.e[1]].desired = sh;
        }
        c.spine_lean(self.vb * -delirium_data::kv!(5), self.vu * delirium_data::kv!(84));
        let pm = c.b.part(PELVIS).m;
        let lift = pm.a * crate::fneg(crate::fneg((1.0 - tip.abs()).mul_add(delirium_data::kv!(80), -delirium_data::kv!(20))).clamp(0.0, 1.0));
        for (i, side) in [(2, 1.0), (delirium_data::kv!(0 int), -1.0)] {
            let l = limb(i);
            if !touching(c, l.p[delirium_data::kv!(0 int)]) {
                let t = c.b.joint_pos(l.e[1]) + lift;
                crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], t, 0.0, 1.0, side, -1.0, Vec3::ZERO, None);
            }
        }
        let mass = c.b.doll.mass();
        let neck: Vec<usize> = (*NECK()).iter().map(|&j| c.ch.joints[j].child).collect();
        let (l0, l1, l2, l3) = (limb(0), limb(1), limb(2), limb(delirium_data::kv!(0 int)));
        for (k, sc) in [(PELVIS, 1.0), (spine[0], 1.0), (spine[1], delirium_data::kv!(28)), (spine[2], delirium_data::kv!(20)), (spine[delirium_data::kv!(0 int)], delirium_data::kv!(18)), (neck[0], 0.5),
                        (neck[1], delirium_data::kv!(17)), (l0.p[1], delirium_data::kv!(119)), (l1.p[1], delirium_data::kv!(116)), (l0.p[0], delirium_data::kv!(20)), (l1.p[0], delirium_data::kv!(119)),
                        (l2.p[1], 1.0 + self.split), (l3.p[1], 1.0 - self.split)] {
            self.push(c, k, mass, sc);
        }
        let mut d = crate::nmdbm::vnorm(c.b.joint_pos((*SPINE())[delirium_data::kv!(0 int)]) - c.b.joint_pos((*SPINE())[0]));
        if self.side {
            d = -d;
        }
        let r = (c.ch.random_bits() & delirium_data::kv!(196 int)) as f32 * delirium_data::kv!(230);
        let k = p.twist_min + (p.twist_max - p.twist_min) * r;
        c.out.torque(spine[0], d * k);
        c.out.torque(spine[1], d * k);
        c.out.torque(spine[2], d * k * delirium_data::kv!(119));
        c.out.torque(spine[delirium_data::kv!(0 int)], d * k * delirium_data::kv!(119));
        false
    }
}

