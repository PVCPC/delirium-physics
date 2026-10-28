use bevy_math::Vec3;
use crate::Vd;
use crate::Vx;

use crate::nmru::RollUp;
use crate::nmshot::Ctx;

#[derive(Clone, Debug)]
pub struct RollDownStairs {
    pub stiffness: f32,
    pub damping: f32,
    pub forcemag: f32,
    #[allow(dead_code)]
    pub asymmetry: f32,
    pub asymmetrical_legs: f32,
    pub use_arm_to_slow_down: f32,
    pub arm_reach_amount: f32,
    pub use_zero_pose: bool,
    pub spin_when_in_air: bool,
    pub leg_push: f32,
    pub arm_reach_length: f32,
    pub stiffness_decay_time: f32,
    pub stiffness_decay_target: f32,
    pub use_custom_roll_dir: bool,
    pub z_axis_spin_reduction: f32,
    pub custom_roll_dir: Vec3,
    pub helper_only: bool,
    mass: f32,
    dir: Vec3,
    sum: Vec3,
    t: f32,
    slope: f32,
    pub active: bool,
}

impl Default for RollDownStairs {
    fn default() -> RollDownStairs {
        RollDownStairs { stiffness: delirium_data::kv!(107), damping: delirium_data::kv!(170), forcemag: delirium_data::kv!(123), asymmetry: 0.0, asymmetrical_legs: 0.0,
                         use_arm_to_slow_down: -delirium_data::kv!(178), arm_reach_amount: delirium_data::kv!(170), use_zero_pose: false, spin_when_in_air: false,
                         leg_push: 1.0, arm_reach_length: delirium_data::kv!(106), stiffness_decay_time: -1.0, stiffness_decay_target: delirium_data::kv!(127),
                         use_custom_roll_dir: false, z_axis_spin_reduction: 0.0, custom_roll_dir: Vec3::ZERO, helper_only: false, mass: -1.0,
                         dir: Vec3::ZERO, sum: Vec3::ZERO, t: 0.0, slope: 0.0, active: false }
    }
}

fn safe_normalize(v: Vec3) -> Vec3 {
    if v.abs().max_element() > delirium_data::kv!(4) { v.normalize_or_zero() } else { Vec3::ZERO }
}

impl RollDownStairs {
    pub fn toward(dir: Vec3) -> RollDownStairs {
        RollDownStairs { use_custom_roll_dir: true, custom_roll_dir: dir, ..Default::default() }
    }

    pub fn activate(&mut self, c: &mut Ctx, roll_up: &mut RollUp) {
        if self.active {
            return;
        }
        self.active = true;
        if !self.helper_only {
            let r = roll_up;
            (r.use_arm_to_slow_down, r.arm_reach_amount, r.stiffness) = (self.use_arm_to_slow_down, self.arm_reach_amount, self.stiffness);
            (r.leg_push, r.arm_reach_length, r.asymmetrical_legs) = (self.leg_push, self.arm_reach_length, self.asymmetrical_legs);
            r.activate(c.ch);
        }
        c.ch.set_stiffness(self.stiffness, self.damping, "fb", None);
        self.dir = safe_normalize(if self.use_custom_roll_dir { self.custom_roll_dir } else { c.vel });
        self.slope = self.dir.vd(c.up).clamp(-1.0, 1.0).asin();
        self.sum = -c.up;
        self.t = 0.0;
        if self.mass < 0.0 {
            self.mass = c.b.doll.mass();
        }
    }

    pub fn deactivate(&mut self, roll_up: &mut RollUp) {
        if self.active {
            roll_up.deactivate();
            *self = RollDownStairs::default();
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if !self.active {
            return;
        }
        self.t += dt;
        if !self.helper_only && self.stiffness_decay_time > 0.0 {
            let k = (self.t / self.stiffness_decay_time).min(1.0);
            c.ch.set_stiffness(crate::Fma::fma(1.0 - k, self.stiffness, self.stiffness_decay_target * k), self.damping, "fb", None);
        }
        self.sum = safe_normalize(self.sum + c.vel);
        self.dir = safe_normalize(if self.use_custom_roll_dir { self.custom_roll_dir } else { self.sum });
        self.sum *= delirium_data::kv!(60);
        self.slope = self.dir.vd(c.up).clamp(-1.0, 1.0).asin();
        if self.use_zero_pose && !self.helper_only {
            let k = crate::Fma::fma(-crate::nmdbm::slen(c.angvel, b"yxz").clamp(0.0, delirium_data::kv!(27)), delirium_data::kv!(73), 1.0);
            for e in c.ch.eff.iter_mut() {
                e.blend_to_zero(k);
            }
        }
        self.roll_force(c);
    }

    fn roll_force(&self, c: &mut Ctx) {
        let all: Vec<usize> = (0..c.b.build.parts.len()).collect();
        let mut touching = c.b.doll.collided(&all);
        let (w, up) = (c.angvel, c.up);
        let (spin, speed) = (crate::nmdbm::slen(w, b"yxz"), crate::nmdbm::slen(c.vel, b"yxz"));
        let fwd = (-spin).mul_add(delirium_data::kv!(63), speed).clamp(-1.0, 1.0);
        let axis = if self.slope.abs() > delirium_data::kv!(170) && !self.use_custom_roll_dir {
            if spin > 0.5 { -safe_normalize(w - up * up.vd(w)) } else { c.m.a }
        } else {
            safe_normalize(self.dir.vx(up))
        };
        let mut f = -delirium_data::kv!(76) * self.forcemag;
        if !touching && self.spin_when_in_air {
            touching = true;
            f = (1.0 - (spin * delirium_data::kv!(73)).clamp(0.0, 1.0)) * f * delirium_data::kv!(9);
        }
        let k = (fwd.abs() * (1.0 / self.mass) * f).clamp(-2.0, 2.0);
        if fwd > 0.0 && speed > 0.5 && touching {
            for p in [0, delirium_data::kv!(22 int), delirium_data::kv!(23 int), delirium_data::kv!(93 int), delirium_data::kv!(77 int)] {
                c.out.force(p, axis * (c.b.build.parts[p].mass * k * delirium_data::kv!(27)));
            }
            if self.z_axis_spin_reduction > 0.0 {
                crate::nmcf::spin_reduction(c, self.z_axis_spin_reduction);
            }
        }
    }
}

