#![allow(dead_code)]
use crate::nmrs::{Character, NECK, SPINE};
use crate::nmshot::limb;

#[derive(Clone, Debug)]
pub struct Foetal {
    pub stiffness: f32,
    pub damping_factor: f32,
    pub asymmetry: f32,
    pub back_twist: f32,
    pub random_seed: u32,
    pub mask: &'static str,
    pub active: bool,
}

impl Default for Foetal {
    fn default() -> Foetal {
        Foetal { stiffness: 9.0, damping_factor: 1.4, asymmetry: 0.0, back_twist: 0.0, random_seed: 100, mask: "fb", active: false }
    }
}

struct Rng(u32, u32);

impl Rng {
    fn new(s: u32) -> Rng {
        Rng(s + (s == 0) as u32, s.rotate_left(16) ^ s)
    }

    fn next(&mut self) -> f32 {
        let n = self.0 as u64 * 0x5cdc_faa7 + self.1 as u64;
        (self.0, self.1) = (n as u32, (n >> 32) as u32);
        (self.0 & 0x7F_FFFF) as f32 * 1.192_092_9e-7
    }
}

impl Foetal {
    pub fn activate(&mut self, ch: &mut Character) {
        if !self.active {
            self.active = true;
            self.pose(ch);
        }
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = Foetal::default();
        }
    }

    pub fn tick(&mut self, ch: &mut Character) {
        if self.active {
            self.pose(ch);
        }
    }

    fn pose(&self, ch: &mut Character) {
        let keep = ch.lock(self.mask);
        self.curl(ch);
        ch.unlock(self.mask, keep);
    }

    fn curl(&self, ch: &mut Character) {
        let mut r = Rng::new(self.random_seed);
        if self.asymmetry == 0.0 {
            ch.set_stiffness(self.stiffness, self.damping_factor, "fb", None);
        } else {
            let a = self.asymmetry * 0.3;
            let (s0, s1) = ((1.0 - self.asymmetry * 0.5) * self.stiffness, (a + 1.0) * self.stiffness);
            let (d0, d1) = ((1.0 - a) * self.damping_factor, (a + 1.0) * self.damping_factor);
            for e in ch.eff.iter_mut() {
                let s = r.next() * (s1 - s0) + s0;
                let d = r.next() * (d1 - d0) + d0;
                (e.strength, e.damping) = (s * s, d * s * 2.0);
            }
        }
        let twist = r.next() * self.back_twist;
        for i in 2..4 {
            let l = limb(i);
            ch.eff[l.e[1]].desired[0] = 2.0;
            ch.eff[l.e[2]].desired[0] = -2.0;
        }
        for j in SPINE {
            ch.eff[j].desired = [0.5, 0.0, twist * 0.25];
        }
        ch.eff[NECK[0]].desired[0] = 0.5;
        for i in [1, 0] {
            let l = limb(i);
            ch.eff[l.e[1]].desired[2] = -0.3;
            ch.eff[l.e[1]].desired[0] = 1.57;
            ch.eff[l.e[2]].desired[0] = 2.0;
        }
        for i in 2..4 {
            let e = &mut ch.eff[limb(i).e[3]];
            (e.strength, e.damping, e.desired[0]) = (20.0, 5.0, -1.0);
        }
    }
}

