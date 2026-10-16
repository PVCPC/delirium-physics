use std::f32::consts::TAU;

use crate::nmrs::{Character, NECK, SPINE};
use crate::nmshot::{limb, Ctx};
use crate::Fma;

#[derive(Clone, Debug)]
pub struct Windmill {
    pub body_stiffness: f32,
    pub arm_stiffness: f32,
    pub period: f32,
    pub amplitude: f32,
    pub phase: f32,
    pub disable_on_impact: bool,
    t: f32,
    elbow: [(f32, f32, f32, f32); 2],
    pub active: bool,
    hit: [bool; 2],
    going: [bool; 2],
}

impl Default for Windmill {
    fn default() -> Windmill {
        Windmill { body_stiffness: 11.0, arm_stiffness: 14.142, period: 1.0, amplitude: 0.6, phase: 0.0, disable_on_impact: true,
                   t: 0.0, elbow: [(0.0, 0.0, 0.0, 0.0); 2], active: false, hit: [false; 2], going: [false; 2] }
    }
}

impl Windmill {
    fn stiffness(&self, ch: &mut Character) {
        ch.set_stiffness(self.body_stiffness, 0.5, "ub", None);
        ch.set_stiffness(self.arm_stiffness, 1.0, "ua", None);
    }

    pub fn activate(&mut self, ch: &mut Character) {
        if self.active {
            return;
        }
        self.active = true;
        self.t = 5000.0 * ch.random();
        for i in 0..2 {
            let l = ch.limits(limb(i).e[2]);
            self.elbow[i] = (l[0], l[1], 0.261_799_4, 15.0);
        }
        (self.hit, self.going) = ([false; 2], [true; 2]);
        self.stiffness(ch);
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = Windmill::default();
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if !self.active {
            return;
        }
        for i in 0..2 {
            if self.disable_on_impact && !self.hit[i] && c.b.doll.collided(&[limb(i).p[3]]) {
                self.hit[i] = true;
            }
            if self.disable_on_impact && self.hit[i] {
                self.going[i] = false;
            }
        }
        let ch = &mut *c.ch;
        if !self.going[0] && !self.going[1] {
            for e in ch.eff.iter_mut() {
                (e.strength, e.damping) = (e.strength * 0.975, e.damping * 0.975);
            }
            return;
        }
        self.stiffness(ch);
        self.t += dt;
        let w = TAU / self.period * if crate::nmdbm::sdot(c.angvel, c.m.a, b"yzx") >= 0.0 { -1.0 } else { 1.0 };
        let x = w * self.t;
        let amp = self.amplitude;
        for (i, at) in [(0, x), (1, self.phase + x)] {
            if !self.going[i] {
                continue;
            }
            let l = limb(i);
            let (s, co) = ((at as f64).sin() as f32, (at as f64).cos() as f32);
            ch.eff[l.e[0]].desired[0] = -(s.fma(0.75, 0.25) * amp);
            ch.eff[l.e[0]].desired[1] = amp.fma(co, -0.15);
            ch.eff[l.e[1]].desired[0] = -(amp * s) - 0.3;
            ch.eff[l.e[1]].desired[1] = amp * co;
            let (lo, hi, min, rate) = self.elbow[i];
            let now = ch.actual(l.e[2], c.b.frames)[0];
            if now < min {
                ch.eff[l.e[2]].desired[0] = rate.fma(dt, now).clamp(lo, hi);
            }
        }
        let sin = (x as f64).sin() as f32;
        for j in SPINE {
            ch.eff[j].desired = [0.0, 0.0, sin * 0.4 * 0.25];
        }
        ch.eff[NECK[0]].desired[2] = sin * -0.3;
    }
}

