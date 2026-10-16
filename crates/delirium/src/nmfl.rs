use crate::prelude::*;
use crate::Vx;
use crate::Fma;

use crate::nmdbm::Dbm;
use crate::nmhl::HeadLook;
use crate::nmrs::{Character, CLAVICLE, ELBOW, HIP, KNEE, NECK, SHOULDER, SPINE, WRIST};
use crate::nmshot::{limb, Ctx, Shot};
use crate::nmst::SpineTwist;

const SPINE3: usize = 10;

#[derive(Clone, Debug)]
pub struct Flinch {
    pub pos: Vec3,
    pub hand_distance_vertical: f32,
    pub hand_distance_left_right: f32,
    pub hand_distance_front_back: f32,
    pub body_stiffness: f32,
    pub body_damping: f32,
    pub back_bend_amount: f32,
    pub use_right_arm: bool,
    pub use_left_arm: bool,
    pub noise_scale: f32,
    pub new_hit: bool,
    pub turn_towards: i32,
    turn: bool,
    dir: Vec3,
    t_behind: f32,
    t_front: f32,
    leg_offset: f32,
    bearing: f32,
    noise_at: f32,
    t_hands: f32,
    t_ramp: f32,
    ramp: f32,
    ms: f32,
    pub active: bool,
    pub hands_off: bool,
    behind: bool,
    right: bool,
}

impl Default for Flinch {
    fn default() -> Flinch {
        Flinch { pos: Vec3::ZERO, hand_distance_vertical: 0.1, hand_distance_left_right: 0.1, hand_distance_front_back: 0.06,
                 body_stiffness: 11.0, body_damping: 1.0, back_bend_amount: -0.55, use_right_arm: true, use_left_arm: true,
                 noise_scale: 0.1, new_hit: true, turn_towards: 1, turn: true, dir: Vec3::ZERO, t_behind: 0.0, t_front: 0.0,
                 leg_offset: 0.0, bearing: 0.0, noise_at: 0.0, t_hands: 0.0, t_ramp: 0.0, ramp: 0.4, ms: 0.01, active: false,
                 hands_off: false, behind: false, right: false }
    }
}

const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37,
    240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177,
    33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146,
    158, 231, 83, 111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25,
    63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100,
    109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206,
    59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153,
    101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246,
    97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192,
    214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114,
    67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

const GRAD: [[f32; 3]; 16] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0], [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0], [0.0, 1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, 1.0], [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [0.0, -1.0, 1.0], [0.0, -1.0, -1.0],
];

fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let p = |i: usize| PERM[i & 255] as usize;
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = (fx as i32 as usize & 255, fy as i32 as usize & 255, fz as i32 as usize & 255);
    let (x, y, z) = (x - fx, y - fy, z - fz);
    let fade = |t: f32| t.mul_add(6.0, -15.0).mul_add(t, 10.0) * (t * (t * t));
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let g = |h: usize, x: f32, y: f32, z: f32| {
        let k = GRAD[h & 15];
        k[0] * x + k[1] * y + k[2] * z
    };
    let lerp = |t: f32, a: f32, b: f32| crate::Fma::fma(b - a, t, a);
    let (a, b) = (p(xi) + yi, p(xi + 1) + yi);
    let (aa, ab, ba, bb) = (p(a) + zi, p(a + 1) + zi, p(b) + zi, p(b + 1) + zi);
    lerp(w,
         lerp(v, lerp(u, g(p(aa), x, y, z), g(p(ba), x - 1.0, y, z)),
              lerp(u, g(p(ab), x, y - 1.0, z), g(p(bb), x - 1.0, y - 1.0, z))),
         lerp(v, lerp(u, g(p(aa + 1), x, y, z - 1.0), g(p(ba + 1), x - 1.0, y, z - 1.0)),
              lerp(u, g(p(ab + 1), x, y - 1.0, z - 1.0), g(p(bb + 1), x, y, z))))
}

pub(crate) fn noise01(x: f32, y: f32, z: f32) -> f32 {
    (noise3(x, y, z) + 0.5).clamp(0.0, 1.0)
}

impl Flinch {
    pub fn activate(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist) {
        if self.active {
            return;
        }
        (self.active, self.right) = (true, true);
        let ch = &mut *c.ch;
        self.leg_offset = 0.1 * ch.random() - 0.1;
        self.noise_at = 4000.0 * ch.random();
        self.turn = ch.random_bits() & 1 != 0;
        for j in Character::mask("ub") {
            ch.eff[j].oppose_gravity = 1.0;
        }
        st.vel = Vec3::ZERO;
        st.activate(ch);
        head.vel = Vec3::ZERO;
        head.activate(ch, c.b);
        self.aim(c, head, st);
        self.stiffen(c.ch);
    }

    pub fn deactivate(&mut self, ch: &mut Character, head: &mut HeadLook, st: &mut SpineTwist) {
        if !self.active {
            return;
        }
        for j in Character::mask("ub") {
            ch.eff[j].reset(&ch.joints[j]);
        }
        *self = Flinch::default();
        st.deactivate();
        head.deactivate();
    }

    pub fn tick(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist, dbm: Option<&mut Dbm>, dbm_failed: bool, dt: f32) {
        if !self.active {
            return;
        }
        if self.hands_off {
            self.t_hands -= dt;
            if self.t_hands <= 0.0 {
                self.hands_off = false;
            }
        }
        self.aim(c, head, st);
        let dbm_on = dbm.is_some();
        if let Some(d) = dbm {
            let mut v = self.pos - c.com;
            if self.bearing.abs() > 2.8 {
                v = c.m.a * if self.bearing > 0.0 { 1.0 } else if self.bearing < 0.0 { -1.0 } else { 0.0 };
            }
            let n = v.normalize_or_zero();
            if self.turn && self.turn_towards != 0 {
                (d.pv.fixed_vel, d.pv.vel) = (true, (n * self.turn_towards as f32).normalize_or_zero());
            } else {
                (d.pv.fixed_vel, d.pv.vel) = (false, n);
            }
            d.leg_offset = if self.behind { self.leg_offset } else { 0.0 };
        }
        if self.new_hit {
            self.stiffen(c.ch);
            self.new_hit = false;
        } else {
            self.soften(c.ch, dt);
        }
        if self.behind {
            (self.t_behind, self.t_front) = (self.t_behind + dt, 0.0);
            self.turned_away(c, !dbm_on && dbm_failed);
        } else {
            (self.t_behind, self.t_front) = (0.0, self.t_front + dt);
            self.front(c);
        }
    }

    fn swap_side(&mut self) {
        (self.hands_off, self.t_hands) = (true, 1.5);
    }

    fn aim(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist) {
        let up = c.up;
        let r = self.pos - c.com;
        self.dir = crate::rigid::nrm(up * -crate::nmdbm::sdot(up, r, b"yzx") + r);
        let loc = crate::rigid::to_frame([c.m.a, c.m.b, c.m.c, c.m.d], self.pos);
        self.bearing = crate::nmdbm::atan2f(loc.x, -loc.z);
        if self.bearing.abs() < 1.5 {
            self.behind = false;
        } else if self.bearing.abs() > 1.6 {
            self.behind = true;
        }
        if self.bearing > 0.4 {
            if !self.right {
                self.swap_side();
            }
            self.right = true;
        } else if self.bearing < -0.4 {
            if self.right {
                self.swap_side();
            }
            self.right = false;
        }
        let wob = noise01(c.ch.agent as f32, (self.t_behind + self.t_front) * 0.75, self.noise_at * 0.5) - 0.5;
        let side = self.dir.vx(up) * (wob * 0.75);
        st.pos = if self.behind { c.b.at(SPINE3) - self.dir } else { self.pos } + side;
        st.stiffness = self.body_stiffness;
        let look = if self.behind { (c.b.at(limb(2).p[3]) + c.b.at(limb(3).p[3])) * 0.5 } else { self.pos };
        head.pos = look - side * 0.75;
        (head.stiffness, head.damping) = (self.body_stiffness, self.body_damping);
    }

    fn stiffen(&mut self, ch: &mut Character) {
        self.t_ramp = 0.0;
        let s = self.body_stiffness * 0.5;
        ch.set_stiffness(s, 0.5, "us", Some(0.01));
        ch.set_stiffness(s, 0.5, "ua", Some(0.01));
        ch.set_stiffness(s, 0.3, "uw", Some(0.01));
        ch.set_stiffness(s, 0.3, "un", Some(0.01));
    }

    fn soften(&mut self, ch: &mut Character, dt: f32) {
        self.t_ramp = (self.t_ramp + dt).min(self.ramp);
        let x = (1.0 - (self.t_ramp + 1e-5) / (self.ramp + 1e-5)).clamp(0.0, 1.0);
        for j in SPINE {
            ch.eff[j].oppose_gravity = x + 1.0;
        }
        for j in NECK.into_iter().chain([0, 1].into_iter().flat_map(|i| [CLAVICLE[i], SHOULDER[i], ELBOW[i]])) {
            ch.eff[j].oppose_gravity = 1.0;
        }
        let f29 = (self.ms + 1.0) * 0.5;
        let s = (1.0 - x * 0.5) * self.body_stiffness;
        let f25 = (1.0 - f29) * 0.5 + f29;
        let cap = 10.0 / (dt * 60.0) + 10.0;
        ch.set_stiffness(s, f25 * 2.0, "us", Some(f29 * 1.2));
        ch.set_stiffness(s, f25 * 2.0, "ua", Some(f29 * 0.5));
        ch.set_stiffness(s, f25 * 2.0, "uc", Some(f25 * 1.2));
        ch.set_stiffness(s.min(cap), 2.0, "uw", Some(f25 * 0.5));
        ch.set_stiffness(s.min(cap), f29 * 2.0, "un", Some(f29 * 0.5));
        self.ms = dt.fma(1.666_666_6, self.ms).min(1.0);
    }

    fn front(&mut self, c: &mut Ctx) {
        let up = c.up;
        let t = self.t_front * 0.75;
        let h = (crate::nmdbm::sdot(self.pos, up, b"yzx") - crate::nmdbm::sdot(c.com, up, b"yzx")).clamp(-0.5, 2.0);
        let agent = c.ch.agent as f32;
        let n1 = (noise01(t, self.noise_at, agent) - 0.5) * self.noise_scale;
        let n2 = (noise01(t, agent, self.noise_at) - 0.5) * self.noise_scale;
        let n3 = (noise01(agent, t, self.noise_at) - 0.5) * self.noise_scale;
        let side = self.dir.vx(up) * (self.hand_distance_left_right + n1);
        let rise = up * h;
        let fwd = crate::rigid::nrm(self.dir + rise) * 0.38;
        let mut vert = crate::rigid::nrm(rise) * (self.hand_distance_vertical + n2);
        let mut fb = fwd * (n3 - self.hand_distance_front_back);
        let centre = c.b.at(SPINE3) + fwd;
        if self.right {
            (vert, fb) = (-vert, -fb);
        }
        let (tl, tr) = (centre + side + fb + vert, centre - side - fb - vert);
        let ch = &mut *c.ch;
        let b = self.bearing.clamp(-1.0, 1.0);
        let (l1, l2) = ((1.0 - b.abs()) * self.back_bend_amount, self.back_bend_amount * b * -0.5);
        for (k, j) in SPINE.into_iter().enumerate() {
            ch.eff[j].desired[0] = l1 * [0.1, 0.2, 0.3, 0.4][k];
            ch.eff[j].desired[1] = l2 * [0.2, 0.3, 0.3, 0.2][k];
        }
        if self.t_front < 0.1 {
            for (i, on, l1, elbow, w1) in [(1, self.use_right_arm, 0.85, 0.3, 0.1), (0, self.use_left_arm, 0.995, 0.6, 0.2)] {
                if on {
                    let e = &mut ch.eff[SHOULDER[i]].desired;
                    (e[2], e[0], e[1]) = (-1.5, l1, -0.5);
                    ch.eff[ELBOW[i]].desired[0] = elbow;
                    (ch.eff[WRIST[i]].desired[1], ch.eff[WRIST[i]].desired[0]) = (0.0, w1);
                    ch.eff[WRIST[i]].blend_to_zero(0.35);
                }
            }
            return;
        }
        let n = crate::rigid::nrm_or_y(c.b.at(SPINE3) - self.pos);
        let mut yaw = 0.05;
        if !c.ch.busy[0] && self.use_left_arm {
            let l2 = if self.right {
                yaw = -0.1;
                -0.3
            } else {
                -0.5
            };
            Self::reach(c, 0, tl, n, 0.05, l2, 1.0);
        }
        if !c.ch.busy[1] && self.use_right_arm {
            let (yaw, l2) = if self.right { (yaw, -0.5) } else { (-0.1, -0.3) };
            Self::reach(c, 1, tr, n, yaw, l2, -1.0);
            c.ch.eff[WRIST[1]].blend_to_zero(0.25);
        }
    }

    fn reach(c: &mut Ctx, i: usize, t: Vec3, n: Vec3, yaw: f32, clav_lean2: f32, side: f32) {
        let l = limb(i);
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, yaw, 0.0, side, 1.0, Vec3::ZERO, None);
        crate::nmcf::clav_follow(c.ch, l.e[0], l.e[1]);
        c.ch.eff[l.e[0]].desired[1] = clav_lean2;
        Shot::wrist(c, l, t, n, side);
        let tw = c.ch.actual(l.e[3], c.b.frames)[2];
        (c.ch.eff[l.e[3]].desired[0], c.ch.eff[l.e[3]].desired[1]) = ((tw as f64).sin() as f32, -((tw as f64).cos() as f32));
    }

    fn turned_away(&mut self, c: &mut Ctx, fallen: bool) {
        let ch = &mut *c.ch;
        for (i, on) in [(1, self.use_right_arm), (0, self.use_left_arm)] {
            if on {
                (ch.eff[CLAVICLE[i]].desired[0], ch.eff[CLAVICLE[i]].desired[1]) = (-0.4, -0.5);
            }
        }
        let s = (crate::nmdbm::sdot(c.m.a, crate::rigid::nrm(self.pos - c.com), b"xyz") as f64).sin() as f32;
        for (k, j) in SPINE.into_iter().enumerate() {
            ch.eff[j].desired[0] = [f32::from_bits(0x3df5_c290), f32::from_bits(0x3e75_c290), 0.36, f32::from_bits(0x3ef5_c290)][k];
            ch.eff[j].desired[1] = s * [0.2, 0.3, 0.3, 0.2][k];
        }
        let late = self.t_behind > 0.14;
        let arms = [(1, self.use_right_arm, if late { (2.15, 2.3) } else { (1.0, 0.3) }, (0.7, 0.5)),
                    (0, self.use_left_arm, if late { (1.995, 2.6) } else { (1.0, 0.6) }, (0.6, 0.4))];
        for (i, on, (l1, elbow), (w2, w1)) in arms {
            if on {
                ch.eff[SHOULDER[i]].desired = [l1, -0.5, -1.5];
                ch.eff[ELBOW[i]].desired[0] = elbow;
                (ch.eff[WRIST[i]].desired[1], ch.eff[WRIST[i]].desired[0]) = (w2, w1);
            }
        }
        if fallen {
            for i in 0..2 {
                ch.eff[HIP[i]].desired[0] = 1.5;
                ch.eff[KNEE[i]].desired[0] = -2.0;
            }
        }
    }
}

