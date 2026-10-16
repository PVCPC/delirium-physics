use crate::prelude::*;
use crate::Vd;

use crate::nmdbm::Body;
use crate::nmrs::{Character, CLAVICLE, SPINE};

#[derive(Clone, Debug)]
pub struct SpineTwist {
    pub pos: Vec3,
    pub vel: Vec3,
    pub stiffness: f32,
    pub twist: f32,
    pub active: bool,
    pub arms: bool,
    pub always: bool,
    pub twisting: bool,
    range: [(f32, f32); 4],
    sum: (f32, f32),
    inv: (f32, f32),
}

impl Default for SpineTwist {
    fn default() -> SpineTwist {
        SpineTwist { pos: Vec3::ZERO, vel: Vec3::ZERO, stiffness: 12.0, twist: 0.0, active: false, arms: false, always: false,
                     twisting: false, range: [(0.0, 0.0); 4], sum: (0.0, 0.0), inv: (0.0, 0.0) }
    }
}

impl SpineTwist {
    pub fn activate(&mut self, ch: &mut Character) {
        if self.active {
            return;
        }
        (self.active, self.twist, self.twisting) = (true, 0.0, true);
        for (k, j) in SPINE.into_iter().enumerate() {
            let l = ch.limits(j);
            let f = if k == 0 { 1.35 } else { 0.35f32 + 1.0 };
            self.range[k] = (l[4] * f, l[5] * f);
        }
        let r = &self.range;
        self.sum = (((r[2].0 + r[1].0) + r[0].0) + r[3].0, ((r[3].1 + r[2].1) + r[1].1) + r[0].1);
        self.inv = (1.0 / self.sum.0, 1.0 / self.sum.1);
        ch.set_stiffness(self.stiffness, 2.0, "us", None);
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = SpineTwist::default();
        }
    }

    pub fn tick(&mut self, ch: &mut Character, b: &Body) {
        if !self.active {
            return;
        }
        let p = b.pmat(0);
        let t = (self.vel - p.v).mul_add(Vec3::splat(0.1), self.pos);
        let n = crate::rigid::nrm(t - p.m.d);
        self.twist = crate::nmdbm::atan2(p.m.b.vd(n), -p.m.c.vd(n));
        let lim = 2.513_274_2;
        if !(self.twist < lim && self.twist > -lim) && !self.always {
            self.twisting = false;
            return;
        }
        self.twisting = true;
        let up = crate::nmdbm::sdot(ch.block.unwrap_or_else(|| b.block()).m.b, ch.up, b"zyx").abs();
        let x = (up * self.twist).max(self.sum.0).min(self.sum.1);
        for (k, j) in SPINE.into_iter().enumerate() {
            let share = if x >= 0.0 { self.range[k].1 * self.inv.1 } else { self.range[k].0 * self.inv.0 };
            ch.eff[j].desired[2] = share * x;
        }
        if self.arms {
            ch.eff[CLAVICLE[1]].desired[0] = x;
            ch.eff[CLAVICLE[0]].desired[0] = -x;
        }
    }
}

