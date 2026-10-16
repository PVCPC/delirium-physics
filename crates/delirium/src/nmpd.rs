use bevy_math::{Quat, Vec3};
use crate::Vd;
use crate::Vx;

use crate::nmdbm::M34;
use crate::nmfl::noise01;
use crate::Fma;
use crate::nmshot::{limb, Ctx};

#[derive(Clone, Debug)]
pub struct Pedal {
    pub radius: f32,
    pub radius_variance: f32,
    pub random_seed: u32,
    pub pedal_offset: f32,
    pub leg_stiffness: f32,
    pub angular_speed: f32,
    pub speed_asymmetry: f32,
    pub leg_angle_variance: f32,
    pub drag_multiplier: f32,
    pub centre_forwards: f32,
    pub centre_up: f32,
    pub left: bool,
    pub right: bool,
    pub back_pedal: bool,
    pub adaptive: bool,
    t: f32,
    t2: f32,
    damping: f32,
    speed: [f32; 2],
    axis: [Vec3; 2],
    at: [Vec3; 2],
    centre: [Vec3; 2],
    pub active: bool,
}

impl Default for Pedal {
    fn default() -> Pedal {
        Pedal { radius: 0.25, radius_variance: 0.4, random_seed: 100, pedal_offset: 0.15, leg_stiffness: 10.0, angular_speed: 10.0,
                speed_asymmetry: 8.0, leg_angle_variance: 0.5, drag_multiplier: 0.3, centre_forwards: 0.0, centre_up: 0.0,
                left: true, right: true, back_pedal: false, adaptive: false, t: 0.0, t2: 0.0, damping: 0.5, speed: [0.0; 2],
                axis: [Vec3::ZERO; 2], at: [Vec3::ZERO; 2], centre: [Vec3::ZERO; 2], active: false }
    }
}

fn apply(m: &M34, v: Vec3) -> Vec3 {
    crate::vm4(m.a, m.b, m.c, Vec3::ZERO, v) + m.d
}

impl Pedal {
    fn stiffness(&self, c: &mut Ctx) {
        for i in 2..4 {
            let l = limb(i);
            c.ch.eff[l.e[1]].set_stiffness(self.leg_stiffness, self.damping, None);
            c.ch.eff[l.e[2]].set_stiffness(self.leg_stiffness, self.damping, None);
            c.ch.eff[l.e[3]].set_stiffness(self.leg_stiffness * 1.5, self.damping, None);
        }
    }

    fn speeds(&mut self, w: f32, agent: f32) {
        let a = (noise01(self.t, agent, self.t2) - 0.5) * 2.0;
        let b = (noise01(self.t2, agent, self.t) - 0.5) * 2.0;
        self.speed = [a, b].map(|n| match n.fma(self.speed_asymmetry, w) {
            s if s < 0.0 => 0.2,
            s => s,
        });
    }

    fn centres(&mut self, c: &Ctx) {
        let r = self.radius + (noise01(self.t + 10.0, self.t2, c.ch.agent as f32) - 0.5) * self.radius_variance - 0.88;
        let p = c.b.part(0).m;
        for (i, k) in [(0, -self.pedal_offset - self.centre_up), (1, self.pedal_offset - self.centre_up)] {
            let m = c.b.eff3(limb(i + 2).e[1]).m;
            let v = p.c.mul_add(Vec3::splat(-self.centre_forwards), p.a * r);
            self.centre[i] = c.up.mul_add(Vec3::splat(k), Vec3::new(m.a.vd(v), m.b.vd(v), m.c.vd(v)));
        }
    }

    pub fn activate(&mut self, c: &mut Ctx) {
        if self.active {
            return;
        }
        self.active = true;
        self.t = 5000.0 * c.ch.random();
        self.t2 = 5000.0 * c.ch.random();
        self.radius = self.radius.clamp(0.0, 0.45);
        self.centres(c);
        self.axis = [Vec3::NEG_Y; 2];
        self.speeds(self.angular_speed, c.ch.agent as f32);
        let n = -c.b.part(0).m.c;
        let l = limb(3);
        let foot = c.b.part(l.p[3]).m.d - apply(&c.b.eff3(l.e[1]).m, self.centre[1]);
        let r = if foot.vd(n) >= 0.0 { self.radius } else { -self.radius };
        self.at = [Vec3::new(r, 0.0, 0.0), Vec3::new(-r, 0.0, 0.0)];
        self.stiffness(c);
    }

    pub fn deactivate(&mut self) {
        if self.active {
            *self = Pedal::default();
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if !self.active {
            return;
        }
        self.t += dt;
        self.stiffness(c);
        self.centres(c);
        let (mut back, mut legs) = (self.back_pedal, [self.left, self.right]);
        let mut w = self.angular_speed;
        if self.adaptive {
            let s = c.b.part(7);
            let v = s.v;
            w = (v.x.fma(v.x, v.y.fma(v.y, v.z * v.z)).sqrt() / self.radius * self.drag_multiplier).clamp(0.0, 13.0);
            if crate::nmdbm::sdot(crate::rigid::nrm(v).vx(c.up), s.m.b, b"zyx") > 0.0 {
                back = !self.back_pedal;
            }
            if crate::nmdbm::sdot(c.up, s.m.a, b"zyx") < -0.4 {
                legs = [false; 2];
            }
        }
        self.speeds(w, c.ch.agent as f32);
        for i in 0..2 {
            if !legs[i] {
                continue;
            }
            let yaw = (noise01(c.ch.agent as f32, self.t * 0.5, self.t2) - 0.5) * self.leg_angle_variance;
            let h = (self.speed[i] * dt * 0.5) as f64;
            let (s, co) = (h.sin() as f32, h.cos() as f32);
            let q = Quat::from_xyzw(self.axis[i].x * s, self.axis[i].y * s, self.axis[i].z * s, co);
            self.at[i] = crate::rigid::qrot(q, self.at[i], back);
            let l = limb(i + 2);
            let t = apply(&c.b.eff3(l.e[1]).m, self.at[i] + self.centre[i]);
            let side = if i == 0 { 1.0 } else { -1.0 };
            crate::nmdbm::limb_ik_w(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, yaw, 0.0, side, -1.0, Vec3::ZERO, None, 1.0, true);
        }
    }
}

