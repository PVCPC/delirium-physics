use std::f32::consts::{PI, TAU};

use crate::nmfl::noise01;
use crate::nmrs::{Character, NECK, SPINE};
use crate::nmshot::{limb, Ctx};
use crate::Fma;

#[derive(Clone, Debug)]
pub struct Writhe {
    pub arm_stiffness: f32,
    pub back_stiffness: f32,
    pub leg_stiffness: f32,
    pub arm_damping: f32,
    pub back_damping: f32,
    pub leg_damping: f32,
    pub arm_period: f32,
    pub back_period: f32,
    pub leg_period: f32,
    pub arm_amplitude: f32,
    pub back_amplitude: f32,
    pub leg_amplitude: f32,
    pub elbow_amplitude: f32,
    pub knee_amplitude: f32,
    pub roll_over: bool,
    pub mask: &'static str,
    pub(crate) t: f32,
    pub(crate) t_noise: f32,
    pub(crate) noise_at: f32,
    pub(crate) roll_rate: f32,
    pub(crate) roll_force: f32,
    pub(crate) roll_timer: f32,
    pub active: bool,
}

impl Writhe {
    pub fn new(ch: &mut Character) -> Writhe {
        Writhe { arm_stiffness: delirium_data::kv!(190), back_stiffness: delirium_data::kv!(190), leg_stiffness: delirium_data::kv!(190), arm_damping: 0.5, back_damping: 0.5,
                 leg_damping: 0.5, arm_period: 1.0, back_period: 1.0, leg_period: 1.0, arm_amplitude: 1.0, back_amplitude: 1.0,
                 leg_amplitude: 1.0, elbow_amplitude: 1.0, knee_amplitude: 1.0, roll_over: false, mask: "fb", t: 0.0, t_noise: 0.0,
                 noise_at: delirium_data::kv!(239) * ch.random(), roll_rate: delirium_data::kv!(12), roll_force: 1.0, roll_timer: delirium_data::kv!(12), active: false }
    }

    fn stiffness(&self, ch: &mut Character) {
        ch.set_stiffness(self.arm_stiffness, self.arm_damping, "ua", None);
        ch.set_stiffness(self.back_stiffness, self.back_damping, "uk", None);
        ch.set_stiffness(self.leg_stiffness, self.leg_damping, "lb", None);
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
        self.t_noise = delirium_data::kv!(239) * ch.random();
        for j in Character::mask("fb") {
            ch.eff[j].reset(&ch.joints[j]);
            ch.eff[j].desired = [0.0; 3];
        }
        for j in (*SPINE()) {
            ch.eff[j].muscle_stiffness = delirium_data::kv!(18);
        }
        self.stiffness(ch);
    }

    pub fn deactivate(&mut self, ch: &mut Character) {
        if self.active {
            *self = Writhe::new(ch);
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if !self.active {
            return;
        }
        let keep = c.ch.lock(self.mask);
        self.stiffness(c.ch);
        self.writhe(c, dt);
        c.ch.unlock(self.mask, keep);
        self.t_noise = dt.fma(delirium_data::kv!(70), self.t_noise);
        self.t += (noise01(self.t_noise, self.noise_at, delirium_data::kv!(133)) + delirium_data::kv!(18)) * dt;
    }

    fn writhe(&mut self, c: &mut Ctx, dt: f32) {
        let ch = &mut *c.ch;
        let s = self.t * delirium_data::kv!(24);
        let ph = |n: f32| n.fma(TAU, -PI);
        let (sin, cos) = (|x: f32| (x as f64).sin() as f32, |x: f32| (x as f64).cos() as f32);
        let agent = ch.agent as f32;
        let p16 = ph(noise01(s, self.noise_at, agent));
        let p15 = ph(noise01(s, s, agent));
        let p24 = ph(noise01(s, self.noise_at, s));
        let p23 = ph(noise01(s, s, self.noise_at));
        let p22 = ph(noise01(s, agent, s));
        let a = self.t / self.arm_period * TAU;
        let amp = noise01(self.noise_at, a, a) * self.arm_amplitude;
        let cl = |x: f32| x.clamp(-1.0, 1.0);
        let arms = [(0, [cl(sin(a + p16) * amp * delirium_data::kv!(20)), cl(cos(a + p15) * amp * 2.0), cl(-p24)], sin(a + p22)),
                    (1, [cl(sin(a + p24) * amp * delirium_data::kv!(20)), cl(cos(a + p23) * amp * 2.0), cl(-p22)], cos(a + p23))];
        for (i, x, elbow) in arms {
            let l = limb(i);
            for k in 0..delirium_data::kv!(0 int) {
                ch.set_relative(l.e[0], k, x[k] * delirium_data::kv!(17));
            }
            ch.eff[l.e[0]].blend_to_zero(0.5);
            for k in 0..delirium_data::kv!(0 int) {
                ch.set_relative(l.e[1], k, x[k]);
            }
            ch.set_relative(l.e[2], 0, cl(elbow * amp * self.elbow_amplitude));
        }
        let (sn, cs) = (sin(a + p16), cos(a + p23));
        for (j, k) in [((*NECK())[0], -1.0), ((*NECK())[1], 1.0)] {
            ch.set_relative(j, 0, sn * k);
            ch.set_relative(j, 1, cs * k);
            ch.set_relative(j, 2, sn * k);
        }
        let b = self.t / self.back_period * TAU;
        let back = [self.back_amplitude * sin((b + p23) * delirium_data::kv!(20)) * delirium_data::kv!(17), p24 * delirium_data::kv!(9), self.back_amplitude * sin(b + p16) * delirium_data::kv!(240 f32)];
        for j in (*SPINE()) {
            ch.eff[j].desired = back.map(|x| x * delirium_data::kv!(17));
        }
        let g = self.t / self.leg_period * delirium_data::kv!(241);
        let la = self.leg_amplitude;
        let (l, r) = (limb(2), limb(delirium_data::kv!(0 int)));
        ch.eff[l.e[1]].desired[0] = sin(g + p22 + 2.0) * la * 0.5;
        ch.eff[l.e[2]].desired[0] = (self.knee_amplitude * cos(g + p15)).fma(delirium_data::kv!(112), -0.5);
        ch.eff[l.e[1]].desired[1] = (-(sin(g * delirium_data::kv!(114)) * la)).fma(delirium_data::kv!(57), -delirium_data::kv!(73));
        ch.eff[l.e[1]].desired[2] = sin(g.fma(delirium_data::kv!(242), 1.0)) * la * delirium_data::kv!(24);
        ch.eff[r.e[1]].desired[0] = cos(g + p23) * la * -0.5;
        ch.eff[r.e[2]].desired[0] = (-(self.knee_amplitude * sin(g + p24))).fma(delirium_data::kv!(112), -0.5);
        ch.eff[r.e[1]].desired[1] = (-(cos(g * delirium_data::kv!(114)) * la)).fma(delirium_data::kv!(57), -delirium_data::kv!(73));
        ch.eff[r.e[1]].desired[2] = sin(g.fma(delirium_data::kv!(243), -1.0)) * la * delirium_data::kv!(24);
        if !self.roll_over {
            return;
        }
        let up = (c.b.joint_pos((*SPINE())[delirium_data::kv!(0 int)]) - c.b.joint_pos((*SPINE())[0])).normalize_or_zero();
        let k = ((self.roll_rate * self.t).sin() + 1.0) * 0.5 - delirium_data::kv!(70);
        let f = up * (k.clamp(0.0, 1.0) * self.roll_force * delirium_data::kv!(244));
        let down = c.b.doll.collided(&[r.p[delirium_data::kv!(0 int)], l.p[delirium_data::kv!(0 int)], delirium_data::kv!(97 int), 0]);
        if crate::nmdbm::slen(c.angvel, b"yxz") < delirium_data::kv!(136) && down {
            for k in [delirium_data::kv!(77 int), delirium_data::kv!(97 int), delirium_data::kv!(23 int), delirium_data::kv!(22 int)] {
                c.out.force(k, f);
            }
        }
        if self.roll_timer < 0.0 {
            self.roll_force = -self.roll_force;
            self.roll_timer = c.ch.random() * delirium_data::kv!(30) + 0.5;
        } else {
            self.roll_timer -= dt;
        }
    }
}

