use crate::nmdbm::{sdot, slen};
use crate::Vx;
use crate::prelude::Vec3;
use crate::nmrs::{Character, NECK, SPINE};
use crate::nmshot::{limb, Ctx};
use crate::rigid::{nrm, q_scaled, qrot};
use crate::Fma;

const SPINE3: usize = 10;
const NECK_PART: usize = 11;

#[derive(Clone, Debug)]
pub struct RollUp {
    pub stiffness: f32,
    pub use_arm_to_slow_down: f32,
    pub arm_reach_amount: f32,
    pub leg_push: f32,
    pub asymmetrical_legs: f32,
    pub arm_reach_length: f32,
    pub hips_from_vel: bool,
    pub mask: &'static str,
    t: f32,
    shoulder: f32,
    damping: f32,
    reach: [f32; 2],
    still: f32,
    roll: f32,
    asym: f32,
    rate: f32,
    phase: f32,
    pub active: bool,
}

impl Default for RollUp {
    fn default() -> RollUp {
        RollUp { stiffness: 10.0, use_arm_to_slow_down: 1.3, arm_reach_amount: 1.4, leg_push: 0.0, asymmetrical_legs: 0.0,
                 arm_reach_length: 0.6, hips_from_vel: false, mask: "fb", t: 0.0, shoulder: 0.0, damping: 0.0, reach: [0.0; 2], still: 0.0,
                 roll: 0.0, asym: 0.0, rate: 0.0, phase: 0.0, active: false }
    }
}

impl RollUp {
    fn roll_asym(&mut self, ch: &mut Character) {
        let x = 2.0 * ch.random() - 1.0;
        self.asym = x.signum() * self.asymmetrical_legs - self.asymmetrical_legs * x * 0.3;
    }

    pub fn activate(&mut self, ch: &mut Character) {
        if self.active {
            return;
        }
        (self.t, self.active) = (0.0, true);
        let keep = ch.lock(self.mask);
        self.start(ch);
        ch.unlock(self.mask, keep);
    }

    fn start(&mut self, ch: &mut Character) {
        (self.roll, self.still, self.damping) = (0.0, 0.0, 0.5);
        for j in 0..ch.eff.len() {
            ch.eff[j].reset(&ch.joints[j]);
            ch.eff[j].desired = [0.0; 3];
        }
        for e in ch.eff.iter_mut() {
            e.muscle_stiffness *= 0.75;
        }
        for i in 2..4 {
            ch.eff[limb(i).e[3]].muscle_stiffness *= 2.0;
        }
        ch.set_stiffness(self.stiffness, self.damping, "fb", None);
        ch.set_stiffness(self.stiffness + 2.0, 1.0, "ua", None);
        ch.set_stiffness(self.stiffness + 2.0, 1.0, "uc", None);
        self.reach = [0.0; 2];
        self.roll_asym(ch);
        self.rate = 6.2 + 12.4 * ch.random();
        self.phase = 6.2 * ch.random();
        self.shoulder = if ch.mode == 1 { ch.random() - 1.0 } else { 0.0 };
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = RollUp::default();
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, brace: bool, dt: f32) {
        self.t += dt;
        if !self.active {
            return;
        }
        let keep = c.ch.lock(self.mask);
        self.roll(c, brace, dt);
        c.ch.unlock(self.mask, keep);
    }

    fn roll(&mut self, c: &mut Ctx, brace: bool, dt: f32) {
        let (m, w, v, up) = (c.m, c.angvel, c.vel, c.up);
        let spin = (sdot(m.b, w, b"zyx") * 0.125).clamp(-1.0, 1.0);
        let speed = slen(w, b"zyx") + slen(v, b"yxz");
        let slow = ((self.use_arm_to_slow_down.abs() - sdot(up, m.c, b"zyx")) - 1.0).clamp(0.0, 10.0);
        let back = -sdot(c.b.part(SPINE3).v, m.c, b"zyx");
        let c2 = qrot(q_scaled(w * 0.1), m.c, false);
        let u = sdot(up, c2, b"zyx");
        let push = u * self.leg_push * -0.5;
        let (mut arm_l, mut arm_r) = (1.0, 1.0);
        if self.use_arm_to_slow_down * spin > 0.0 {
            arm_l = (-spin.abs()).fma(slow, 1.0);
        } else {
            arm_r = (-spin.abs()).fma(slow, 1.0);
        }
        let sp = speed * 0.5;
        let k = self.roll.fma(2.0, sp.clamp(0.0, 1.0)) * 0.333_333_34;
        self.roll = k;
        if sp < 0.2 {
            self.still += dt;
            if self.still > 0.5 {
                c.out.send(12, "Roll Up", 0.0);
            }
        }
        let slack = if brace { (slen(v, b"yxz").clamp(0.2, 2.2) - 0.2) * 0.5 } else { 1.0 };
        let s = (-(-slack).fma(k, 1.0)).fma(5.0, self.stiffness).clamp(1.0, 16.0);
        c.ch.set_stiffness(s, self.damping, "fb", None);
        if u > 0.0 && (u as f64) < 0.1 && (slen(w, b"yxz") as f64) > 2.7 {
            self.roll_asym(c.ch);
        }
        let hip = if self.hips_from_vel { (0.3 - back).clamp(0.0, 1.0) } else { 1.6 };
        let a = self.asym;
        let ch = &mut *c.ch;
        let (l, r) = (limb(2), limb(3));
        ch.eff[l.e[1]].desired[0] = ((hip - push) - 0.125).fma(k, a) + 0.125;
        ch.eff[r.e[1]].desired[0] = ((hip - push) - 0.125).fma(k, -a) + 0.125;
        let knee = (-push).fma(3.0, 1.25);
        let sin = |x: f32| (x as f64).sin() as f32;
        let rt = self.rate * self.t;
        let (sl, sr) = (sin(self.rate.fma(self.t, self.phase)), sin(rt.fma(1.13, self.phase) + 3.0));
        ch.eff[l.e[2]].desired[0] = ((-sl).fma(0.75, -knee) + 0.5).fma(k, -a) - 0.5;
        ch.eff[r.e[2]].desired[0] = ((-sr).fma(0.75, -knee) + 0.5).fma(k, a) - 0.5;
        let spread = if ch.mode == 2 { 0.1 } else { 0.35 };
        let (s4, s3) = (sin(rt.fma(0.4, self.phase)), sin(rt.fma(0.3, self.phase)));
        ch.eff[l.e[1]].desired[1] = (-((s4 + 1.0) * spread)).fma(0.5, a.fma(-0.5, -((1.3 - arm_l) * 0.3)));
        ch.eff[r.e[1]].desired[1] = (-((s3 + 1.0) * spread)).fma(0.5, a.fma(0.5, -((1.4 - arm_r) * 0.3)));
        for j in SPINE {
            ch.eff[j].desired = [(k + 0.5) * 0.25, 0.0, a * 0.25];
        }
        ch.eff[SPINE[0]].desired[0] = k * 0.35;
        ch.eff[NECK[1]].desired[0] = k.fma(0.3, 0.2);
        ch.eff[NECK[0]].desired[0] = k.fma(0.3, 0.2);
        for (i, arm) in [(0, arm_l), (1, arm_r)] {
            let e = &mut ch.eff[limb(i).e[1]];
            e.desired = [arm.fma(1.7, self.shoulder), -0.5, -1.0];
            ch.eff[limb(i).e[2]].desired[0] = k.fma(0.200_000_05, 1.5);
        }
        ch.eff[l.e[3]].desired[0] = -a - 1.5;
        ch.eff[r.e[3]].desired[0] = a - 1.5;
        if sp < 5.0 {
            let n = c.b.pmat(NECK_PART).m;
            for j in NECK {
                ch.eff[j].desired[0] = -sdot(n.c, up, b"xyz");
                ch.eff[j].desired[1] = -sdot(n.a, up, b"xyz");
            }
        }
        for (i, side, mask) in [(0, 1.0, "ul"), (1, -1.0, "ur")] {
            let out;
            (out, self.reach[i]) = self.arm(c, i, side, self.reach[i]);
            let s = ((-out).fma(2.0, 4.0).fma(k - 1.0, out * 3.0) + self.stiffness).clamp(0.5, 20.0);
            c.ch.set_stiffness(s, 1.0, mask, None);
        }
    }

    fn arm(&self, c: &mut Ctx, i: usize, side: f32, reach: f32) -> (f32, f32) {
        let l = limb(i);
        let (m, up) = (c.m, c.up);
        let at = c.b.eff3(l.e[1]).m.d;
        let s3 = c.b.pmat(SPINE3);
        let mut out = 1.0;
        let rise = sdot(up, s3.v, b"zyx");
        if rise > 0.0 {
            out = (-rise).fma(0.5, 1.0).clamp(0.0, 1.0);
        }
        let dir = nrm(up.mul_add(Vec3::splat(-2.45), s3.v) + (m.b - m.c));
        let p = s3.w.vx(up).mul_add(Vec3::splat(0.0125), dir.mul_add(Vec3::splat(0.5), at));
        let dir = nrm(p - at);
        let target = dir.mul_add(Vec3::splat(self.arm_reach_length), at);
        let n = nrm(s3.m.c * -2.0 + s3.m.b * side);
        let d = sdot(n, dir, b"zyx");
        let hi = ((d - ((1.3 - self.arm_reach_amount) - 0.3)) * 3.333_333_3).min(1.0);
        let lo = ((d - (0.7 - self.arm_reach_amount)) * 3.333_333_3).max(0.0);
        let reach = reach.max(hi).min(lo);
        out *= reach;
        if out > 0.0 {
            let vel = c.b.part(l.p[0]).v;
            crate::nmdbm::limb_ik_w(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], target, 0.0, 1.0, side, 1.0, vel, None, out, false);
            crate::nmcf::clav_follow(c.ch, l.e[0], l.e[1]);
            c.ch.eff[l.e[3]].desired[1] = out * -0.8;
        }
        (out, reach)
    }
}

