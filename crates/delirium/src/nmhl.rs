use std::f32::consts::FRAC_PI_2;
use crate::Vx;

use crate::prelude::*;

use crate::nmdbm::{acos, sdot, slen, Body};
use crate::rigid::{nrm, q_axis, q_scaled, qrot};
use crate::Fma;
use crate::nmrs::{Character, NECK, SPINE};

const SPINE3: usize = 10;
const HEAD: usize = 12;

#[derive(Clone, Debug)]
pub struct HeadLook {
    pub pos: Vec3,
    pub vel: Vec3,
    pub stiffness: f32,
    pub damping: f32,
    pub always_look: bool,
    pub always_eyes_horizontal: bool,
    pub active: bool,
    pub visible: bool,
    pub cone: f32,
    pub len: f32,
    pub local: Option<Vec3>,
}

impl Default for HeadLook {
    fn default() -> HeadLook {
        HeadLook { pos: Vec3::ZERO, vel: Vec3::ZERO, stiffness: 10.0, damping: 1.0, always_look: false, always_eyes_horizontal: true,
                   active: false, visible: false, cone: 0.0, len: 0.0, local: None }
    }
}

fn split(x: f32, a: f32, b: f32, c: f32, d: f32) -> (f32, f32) {
    let (l, h) = (1.0 / (a + c), 1.0 / (b + d));
    if x >= 0.0 { (h * b * x, h * d * x) } else { (l * a * x, l * c * x) }
}

impl HeadLook {
    pub fn activate(&mut self, ch: &mut Character, b: &Body) {
        if self.active {
            return;
        }
        self.active = true;
        self.cone = std::f32::consts::FRAC_PI_3;
        self.visible = false;
        self.len = slen(b.at(HEAD) - b.at(SPINE3), b"xyz");
        ch.set_stiffness(self.stiffness, self.damping, "un", None);
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = HeadLook::default();
        }
    }

    pub fn tick(&mut self, ch: &mut Character, b: &Body) {
        if !self.active {
            return;
        }
        if let Some(x) = self.local {
            (self.pos, self.vel) = crate::nmbb::from_own(x, b);
        }
        ch.set_stiffness(self.stiffness, self.damping, "un", None);
        let (neck, head) = (NECK[0], NECK[1]);
        let strength = (ch.eff[head].strength + ch.eff[neck].strength) * 0.5;
        let damping = (ch.eff[head].damping + ch.eff[neck].damping) * 0.25;
        let (nl, hl) = (ch.limits(neck), ch.limits(head));
        let sp = b.pmat(SPINE3);
        let m = sp.m;
        let q = q_scaled(m.b * -0.238_196_28);
        let (a, c) = (qrot(q, m.a, false), qrot(q, m.c, false));
        let h = a.mul_add(Vec3::splat(self.len), b.at(SPINE3));
        let t = (((self.pos - m.d).vx(sp.w) - sp.v) + self.vel).mul_add(Vec3::splat(damping / strength), self.pos);
        let dir = nrm(t - m.d);
        let x = sdot(dir, a, b"zyx");
        let k = if x > 0.0 { FRAC_PI_2 - self.cone } else { self.cone - FRAC_PI_2 };
        self.visible = sdot(c, dir, b"zyx") < (k.fma(x, self.cone) as f64).cos() as f32;
        if !self.visible && !self.always_look {
            return;
        }
        let mut n = nrm((t - h) * -1.0);
        let p = nrm(a.vx(n).vx(a));
        let tw = acos(sdot(p, c, b"zyx"));
        let max = (nl[5] + hl[5]) * 1.2;
        let mut over = 0.0;
        if tw > max {
            over = max - tw;
            if sdot(m.b, p, b"zyx") < 0.0 {
                over *= -1.0;
            }
            n = qrot(q_axis(a, -over), n, false);
        }
        let up = ch.up;
        let ua = sdot(up, a, b"zyx");
        let s = if ua * ua <= 0.25 && !self.always_eyes_horizontal {
            a.vx(n)
        } else if sdot(up, n, b"zyx").abs() < 0.9 {
            up.vx(n)
        } else {
            (Vec3::ONE - up) * b.pmat(HEAD).m.a
        };
        let s = nrm(s);
        let w = nrm(n.vx(s));
        let r = q_scaled(nrm(w.vx(a)) * acos(sdot(w, a, b"zyx")));
        let n2 = nrm(qrot(r, n, false));
        let sign = |x: f32| if x < 0.0 { -1.0 } else { 1.0 };
        let twist = acos(sdot(n2, c, b"zyx")) * sign(-sdot(n2, m.b, b"zyx"));
        let u1 = nrm(w.vx(m.b).vx(m.b) * -1.0);
        let lean1 = acos(sdot(u1, a, b"zyx")) * sign(-sdot(u1, c, b"zyx"));
        let u2 = nrm(w.vx(c).vx(c) * -1.0);
        let lean2 = acos(sdot(u2, a, b"zyx")) * sign(sdot(u2, m.b, b"zyx"));
        let (s2, s3) = (SPINE[2], SPINE[3]);
        let (l2, l3) = (ch.limits(s2), ch.limits(s3));
        let x3 = over.fma(0.65, ch.eff[s3].desired[2]);
        ch.eff[s3].desired[2] = x3.max(l3[4] * 1.35).min(l3[5] * 1.35);
        let x2 = over.fma(0.35, ch.eff[s2].desired[2]);
        ch.eff[s2].desired[2] = x2.max(l2[4] * 1.35).min(l2[5] * 1.35);
        for (n, x, lo, hi) in [(2, twist, 4, 5), (0, lean1, 0, 1), (1, lean2, 2, 3)] {
            let (xn, xh) = split(x, nl[lo], nl[hi], hl[lo], hl[hi]);
            ch.eff[neck].desired[n] = xn.clamp(-9.0, 9.0);
            ch.eff[head].desired[n] = xh.clamp(-9.0, 9.0);
        }
    }
}

