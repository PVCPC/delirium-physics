use bevy_math::Vec3;

use crate::nmrs::Character;
use crate::nmshot::{limb, Ctx};

#[derive(Clone, Copy, Debug)]
pub struct PointArmParams {
    pub target: Vec3,
    pub twist: f32,
    pub arm_straightness: f32,
    pub arm_stiffness: f32,
    pub arm_damping: f32,
    #[allow(dead_code)]
    pub instance_index: i32,
    pub point_swing_limit: f32,
    pub use_left_arm: bool,
    pub use_zero_pose_when_not_pointing: bool,
}

#[derive(Clone, Debug)]
pub struct PointArm {
    pub p: PointArmParams,
    pub(crate) reach: f32,
    pub active: bool,
    pub(crate) pointing: bool,
}

impl Default for PointArmParams {
    fn default() -> PointArmParams {
        PointArmParams { target: Vec3::ZERO, twist: 0.3, arm_straightness: 0.8, arm_stiffness: 15.0, arm_damping: 1.0, instance_index: -1,
                         point_swing_limit: 1.5, use_left_arm: false, use_zero_pose_when_not_pointing: false }
    }
}

impl Default for PointArm {
    fn default() -> PointArm {
        PointArm { p: PointArmParams::default(), reach: 0.7, active: false, pointing: false }
    }
}

fn swing(ch: &Character, j: usize) -> f32 {
    let l = ch.limits(j);
    let d = ch.eff[j].desired;
    let (m0, h0) = ((l[0] + l[1]) * 0.5, (l[1] - l[0]) * 0.5);
    let (m1, h1) = ((l[2] + l[3]) * 0.5, (l[3] - l[2]) * 0.5);
    (((d[0] - m0) / h0).powi(2) + ((d[1] - m1) / h1).powi(2)).sqrt()
}

pub(crate) fn share(ch: &mut Character, frames: &[crate::prelude::Transform], clav: usize, shoulder: usize) {
    let (cl, sl) = (ch.limits(clav), ch.limits(shoulder));
    let now = ch.actual(clav, frames);
    for k in 0..2 {
        let t = now[k] + ch.eff[shoulder].desired[k];
        let (lo, hi) = (2 * k, 2 * k + 1);
        let i = if t >= 0.0 { hi } else { lo };
        let s = 1.0 / (cl[i] + sl[i]);
        ch.eff[clav].desired[k] = cl[i] * s * t;
        ch.eff[shoulder].desired[k] = sl[i] * s * t;
    }
}

impl PointArm {
    pub fn activate(&mut self, p: PointArmParams) {
        self.p = p;
        if !self.active {
            (self.active, self.pointing) = (true, true);
        }
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn tick(&mut self, c: &mut Ctx) {
        if !self.active {
            return;
        }
        let p = self.p;
        let (l, side, mask) = if p.use_left_arm { (limb(0), 1.0, "ul") } else { (limb(1), -1.0, "ur") };
        let at = c.b.joint_pos(l.e[1]);
        let t = (crate::rigid::nrm(p.target - at) * self.reach).mul_add(Vec3::splat(p.arm_straightness), at);
        let (clav, sh) = (c.ch.eff[l.e[0]].desired, c.ch.eff[l.e[1]].desired);
        c.ch.set_stiffness(p.arm_stiffness, p.arm_damping, mask, None);
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, p.twist, 1.0, side, 1.0, Vec3::ZERO, None);
        share(c.ch, c.b.frames, l.e[0], l.e[1]);
        let s = swing(c.ch, l.e[0]);
        if !self.pointing && s < p.point_swing_limit - 0.1 {
            self.pointing = true;
        }
        if self.pointing && s > p.point_swing_limit + 0.1 {
            self.pointing = false;
        }
        let ch = &mut *c.ch;
        if self.pointing {
            for j in l.e {
                ch.eff[j].oppose_gravity = 1.0;
            }
            ch.eff[l.e[3]].desired = [0.0; 3];
            ch.eff[l.e[0]].muscle_stiffness = 1.5;
            ch.eff[l.e[1]].muscle_stiffness = 0.5;
            ch.eff[l.e[2]].muscle_stiffness = 0.5;
            ch.eff[l.e[3]].desired[0] = ch.eff[l.e[2]].desired[0] * -0.5;
        } else if p.use_zero_pose_when_not_pointing {
            ch.eff[l.e[0]].desired = [0.0; 3];
            ch.eff[l.e[1]].desired = [0.0, 0.5, 0.0];
            ch.eff[l.e[2]].desired[0] = 0.5;
            for j in Character::mask(mask) {
                ch.eff[j].blend_to_zero(1.0);
            }
            ch.set_stiffness(8.0, 1.0, mask, None);
        } else {
            ch.eff[l.e[0]].desired = clav;
            ch.eff[l.e[1]].desired = sh;
        }
    }
}
