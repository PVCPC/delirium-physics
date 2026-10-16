use crate::Fma;
use crate::Vx;
use crate::prelude::*;
use bevy_math::Vec3Swizzles;

use crate::nmdbm::{asin, sdot, Body, M34};
use crate::nmrs::{Character, CLAVICLE, ELBOW, HIP, NECK, SHOULDER, SPINE, WRIST};

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub body_stiffness: f32,
    pub elbow: f32,
    pub shoulder: f32,
    pub damping: f32,
    pub max_steps: u32,
    pub head_look_pos: Vec3,
    pub head_look_instance_index: i32,
    pub head_look_always_timeout: f32,
    pub head_look_at_vel_prob: f32,
    pub somersault_angle: f32,
    pub somersault_angle_threshold: f32,
    pub side_somersault_angle: f32,
    pub side_somersault_angle_threshold: f32,
    pub ang_vel_multiplier: Vec3,
    pub ang_vel_threshold: Vec3,
    pub arms_out_on_push_multiplier: f32,
    pub arms_out_on_push_timeout: f32,
    pub returning_to_balance_arms_out: f32,
    pub arms_out_straighten_elbows: f32,
    pub arms_out_min_lean2: f32,
    pub body_turn_timeout: f32,
    pub turn_off_prob: f32,
    pub turn2_target_prob: f32,
    pub turn2_vel_prob: f32,
    pub turn_away_prob: f32,
    pub turn_left_prob: f32,
    pub turn_right_prob: f32,
    pub elbow_angle_on_contact: f32,
    pub bend_elbows_time: f32,
    pub bend_elbows_gait: f32,
    pub use_head_look: bool,
    pub arms_out_on_push: bool,
    pub use_body_turn: bool,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            body_stiffness: 9.0, elbow: 1.75, shoulder: 1.0, damping: 0.7, max_steps: 100, head_look_pos: Vec3::ZERO,
            head_look_instance_index: -1, head_look_always_timeout: 10.0, head_look_at_vel_prob: -1.0,
            somersault_angle: 1.0, somersault_angle_threshold: 0.25, side_somersault_angle: 1.0,
            side_somersault_angle_threshold: 0.25, ang_vel_multiplier: Vec3::new(4.0, 1.0, 4.0),
            ang_vel_threshold: Vec3::new(1.2, 3.0, 1.2), arms_out_on_push_multiplier: 0.0, arms_out_on_push_timeout: 1.0,
            returning_to_balance_arms_out: 0.0, arms_out_straighten_elbows: 0.0, arms_out_min_lean2: -9.9,
            body_turn_timeout: 1.0, turn_off_prob: 0.1, turn2_target_prob: 0.2, turn2_vel_prob: 0.3, turn_away_prob: 0.15,
            turn_left_prob: 0.125, turn_right_prob: 0.125, elbow_angle_on_contact: 1.9, bend_elbows_time: 0.5,
            bend_elbows_gait: 0.7, use_head_look: false, arms_out_on_push: false, use_body_turn: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Out {
    pub catch_fall: Option<[f32; 3]>,
    pub head_look: Option<(Vec3, Vec3)>,
    pub head_look_stiffness: f32,
    pub step_clamp: Option<f32>,
    pub turn: Option<Vec3>,
}

#[derive(Clone, Copy, Debug)]
pub struct BodyBalance {
    pub p: Params,
    arms_out: f32,
    floor: f32,
    euler: [f32; 3],
    bend: f32,
    cut: f32,
    stepping: u32,
    look_time: f32,
    look_rand: f32,
    turn_rand: f32,
    turn_time: f32,
    look_vel: bool,
    pub use_char_frame: bool,
    frame: [Vec3; 3],
}

impl BodyBalance {
    pub fn activate(p: Params, ch: &mut Character, b: &Body) -> BodyBalance {
        for (e, j) in ch.eff.iter_mut().zip(&ch.joints) {
            e.reset(j);
            e.desired = [0.0; 3];
        }
        for i in NECK {
            ch.eff[i].oppose_gravity = 1.0;
        }
        for i in [SPINE[0], SPINE[1], SPINE[2], CLAVICLE[0], CLAVICLE[1]] {
            ch.eff[i].oppose_gravity = 2.0;
        }
        for i in CLAVICLE {
            ch.eff[i].blend_to_zero(1.0);
        }
        let mut s = BodyBalance {
            p, arms_out: p.arms_out_on_push_timeout, floor: 0.03, euler: [0.0; 3], bend: 0.0, cut: ch.random() * 0.8,
            stepping: 0, look_time: 0.0, look_rand: 0.0, turn_rand: 0.0, turn_time: 0.0, look_vel: false,
            use_char_frame: false, frame: [Vec3::ZERO; 3],
        };
        s.look_rand = ch.random();
        s.turn_rand = ch.random();
        s.frame = s.frame(b);
        s.euler = s.angles();
        s
    }

    pub fn deactivate(ch: &mut Character) {
        ch.stabilise = None;
    }

    fn frame(&self, b: &Body) -> [Vec3; 3] {
        if self.use_char_frame {
            let m = b.snap(Vec3::Z).m;
            [m.b, m.a, m.c]
        } else {
            let m = b.pmat(b.ch.joints[SPINE[2]].child).m;
            [m.a, -m.b, m.c]
        }
    }

    fn angles(&self) -> [f32; 3] {
        let y = asin(self.frame[2].z);
        let k = 1.0 / (y as f64).cos() as f32;
        [asin(-self.frame[1].z * k), y, asin(-self.frame[2].y * k)]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, ch: &mut Character, b: &Body, obstacles: &[crate::ragdoll::Obstacle], stepping: u32, failed: bool, catching: bool, dt: f32) -> Out {
        let p = self.p;
        let mut out = Out::default();
        if failed {
            if !catching {
                ch.stabilise = None;
                let k = p.body_stiffness;
                out.catch_fall = Some([k * 0.611_111_1, k * 1.111_111_2, k * 1.666_666_6]);
            }
            return out;
        }
        if stepping != 0 && self.stepping != stepping {
            self.stepping = stepping;
            self.cut = ch.random() * 0.8;
            if p.arms_out_on_push_timeout > 998.9 && p.arms_out_on_push_timeout < 999.1 {
                out.step_clamp = Some(ch.random() * 1.2 + 0.3);
            }
        }
        for i in SPINE.into_iter().chain(NECK) {
            ch.eff[i].muscle_stiffness = 1.5;
        }
        ch.set_stiffness(p.head_look_always_timeout, p.body_turn_timeout, "ub", None);
        ch.set_stiffness(p.body_stiffness, p.damping, "ua", None);
        ch.set_stiffness(p.body_stiffness, 1.0, "un", None);
        ch.set_stiffness(9.0, 1.0, "uc", None);
        let k = p.body_stiffness * 0.9;
        for i in ELBOW {
            ch.eff[i].damping = p.damping * 0.8 * k * 2.0;
            ch.eff[i].strength = k * k;
        }
        let speed = (b.doll.kinetic() * 2.0).sqrt();
        let snap = b.snap(Vec3::Z);
        if p.use_head_look {
            out.head_look_stiffness = p.head_look_always_timeout + 1.0;
            let mut look = look_at(&p, b, obstacles);
            if stepping != 0 {
                self.look_time -= dt;
                if self.look_time < 0.0 {
                    self.look_time = ch.random() * 0.2 + 0.2;
                    if p.head_look_at_vel_prob < -0.001 {
                        self.p.head_look_at_vel_prob = self.look_rand;
                    }
                    self.look_vel = self.p.head_look_at_vel_prob > ch.random();
                }
                if self.look_vel {
                    let head = b.at(b.ch.joints[NECK[1]].child);
                    let chest = b.doll.motion(b.ch.joints[SPINE[3]].child).0;
                    look = (snap.vel.mul_add(Vec3::splat(crate::rigid::nrm_k(snap.vel)), head), chest);
                }
            } else {
                self.look_time = 0.0;
            }
            out.head_look = Some(look);
        }
        let f19 = speed.clamp(0.001, 1.0);
        if p.use_body_turn {
            self.turn_time -= dt;
            if self.turn_time < 0.0 {
                self.turn_time = ch.random() * 0.6 + 0.4;
                self.turn_rand = ch.random();
            }
            let probs = [p.turn2_target_prob, p.turn2_vel_prob, p.turn_away_prob, p.turn_left_prob, p.turn_right_prob];
            let sum = probs.iter().fold(p.turn_off_prob, |s, x| s + x);
            if sum > 1e-4 {
                let inv = 1.0 / sum;
                let mut edge = 0.0;
                let edges = probs.map(|x| { edge = inv.fma(x, edge); edge });
                let r = self.turn_rand;
                let root = b.part(b.ch.joints[HIP[0]].parent).m;
                let dir = if r <= edges[0] {
                    Some(p.head_look_pos - snap.com)
                } else if r <= edges[1] {
                    Some(snap.vel)
                } else if r <= edges[2] {
                    Some(-(p.head_look_pos - snap.com))
                } else if r <= edges[3] {
                    Some(root.d - root.a - snap.com)
                } else if r <= edges[4] {
                    Some(root.d + root.a - snap.com)
                } else {
                    None
                };
                if let Some(d) = dir.filter(|_| r < edges[4]) {
                    out.turn = Some(d.normalize_or_zero());
                }
            }
        }
        if f19 < 0.06 {
            out.turn = None;
        }
        for i in NECK {
            ch.eff[i].desired = [0.0; 3];
        }
        let m: M34 = snap.m;
        let side = m.b.vx(ch.up);
        let sin = |x: f32| (x as f64).sin() as f32;
        let pitch = sin(sdot(side, m.a, b"yzx")) * -2.0;
        let roll = sin(sdot(side, m.c, b"yzx")) * 2.0;
        for (i, l1) in SPINE.into_iter().zip([-0.07, 0.25, -0.07, 0.07]) {
            ch.eff[i].desired[0] = l1;
            ch.eff[i].desired[1] = 0.0;
        }
        let hip = |side: usize, des: bool| if des { ch.eff[HIP[side]].desired[0] } else { b.ch.actual(HIP[side], b.frames)[0] };
        let tw = (hip(0, true) - hip(1, true)).fma(0.15, hip(0, false) - hip(1, false));
        for (i, k) in SPINE.into_iter().zip([0.35, 0.3, 0.2, 0.15]) {
            ch.eff[i].desired[2] = tw * k;
        }
        let over = |x: f32, thr: f32| x.abs().max(thr).min(0.8) - thr;
        let (tilt, side_tilt) = (over(pitch, p.somersault_angle_threshold), over(roll, p.side_somersault_angle_threshold));
        self.frame = self.frame(b);
        let now = self.angles();
        let inv = 1.0 / dt;
        let rate = [0, 1, 2].map(|k| -((now[k] - self.euler[k]) * inv));
        self.euler = now;
        let f5 = if rate[1] * pitch < 0.0 { p.returning_to_balance_arms_out } else { 1.0 };
        let f4 = if rate[0] * roll < 0.0 { 1.0 } else { p.returning_to_balance_arms_out };
        let spin = |x: f32, thr: f32| x.abs().max(thr).min(7.0) - thr;
        let (t, m) = (p.ang_vel_threshold, p.ang_vel_multiplier);
        let f27 = (-(p.side_somersault_angle * f4)).fma(side_tilt, -(p.somersault_angle * f5 * tilt));
        let f28 = (-(spin(rate[1], t.x) * m.x)).fma(f5, f27);
        let f28 = (-spin(rate[2], t.y)).fma(m.y, f28);
        let f28 = (-(spin(rate[0], t.z) * m.z)).fma(f4, f28);
        let mut push = 0.0;
        if self.arms_out > 0.0 {
            self.arms_out -= dt;
            push = p.arms_out_on_push_multiplier;
        }
        if self.bend > 0.0 {
            self.bend -= dt;
        }
        if b.doll.collided(&Character::mask("ub").iter().map(|&i| b.ch.joints[i].child).collect::<Vec<_>>()) {
            self.bend = p.bend_elbows_time;
        }
        let foot = |side: usize| b.doll.touch[b.ch.joints[crate::nmrs::ANKLE[side]].child];
        let down = |t: u8| t & 0x20 != 0 && t & 0x08 == 0;
        let grounded = down(foot(0)) || down(foot(1));
        let mut f30 = 0.0;
        for side in 0..2 {
            if ch.busy[side] || !grounded {
                continue;
            }
            let other = 1 - side;
            let act = |j: usize| b.ch.actual(j, b.frames);
            let (ho_des, ho_act, hs_act) = (ch.eff[HIP[other]].desired, act(HIP[other]), act(HIP[side]));
            let x = (-ho_act[0]).fma(1.2, ho_des[0]);
            let x = (-ho_act[0].fma(1.1, -hs_act[0])).fma(0.6, x);
            ch.eff[SHOULDER[side]].desired[0] = x.fma(p.shoulder, act(SHOULDER[side])[0]);
            ch.eff[SHOULDER[side]].desired[2] = -0.35;
            let mut elbow = (ho_des[0] - ho_act[0]).fma(p.elbow, act(ELBOW[side])[0]);
            if p.arms_out_on_push {
                elbow = p.arms_out_straighten_elbows.fma(if side == 0 { f28 } else { f30 + f28 }, elbow);
            }
            let mut cut = 0.0;
            if self.bend > 0.0 {
                let v = crate::rigid::nrm(snap.vel);
                let (fwd, lat) = (sdot(self.frame[2], v, b"zyx"), sdot(self.frame[1], v, b"zyx"));
                let toward = if side == 0 { lat > 0.0 } else { lat < 0.0 };
                if (fwd > 0.0 && toward) || fwd > 0.5 {
                    elbow = elbow.max(p.elbow_angle_on_contact);
                } else {
                    cut = self.cut;
                }
            }
            ch.eff[ELBOW[side]].desired[0] = elbow.max(p.bend_elbows_gait).min(6.0);
            let hs_l2 = act(HIP[side])[1];
            let mut l2 = hs_l2.fma(0.3, 0.5);
            if p.arms_out_on_push {
                let max = l2 - cut;
                let c = |x: f32| x.abs().max(self.floor).min(10.0) - self.floor;
                let (a, bb) = (c(ch.eff[HIP[side]].desired[1]) * push, c(hs_l2) * push);
                let (cc, d) = (c(ch.eff[HIP[other]].desired[1]) * push, c(act(HIP[other])[1]) * push);
                let o = cc.fma(-0.5, -(a * 0.25));
                let o = (-d).fma(0.5, o);
                let o = (-bb).fma(0.25, o);
                if side == 0 {
                    f30 = o;
                }
                l2 = (l2 + o + f28).max(p.arms_out_min_lean2).min(max);
            }
            ch.eff[SHOULDER[side]].desired[1] = l2;
        }
        for i in WRIST {
            ch.eff[i].desired = [0.0; 3];
        }
        let k = (1.0 - f19).fma(0.8, 0.2);
        for i in Character::mask("ub") {
            ch.eff[i].blend_to_zero(k);
        }
        ch.stabilise = Some((2.0, 0.707));
        out
    }
}

pub fn look_at(p: &Params, b: &Body, obstacles: &[crate::ragdoll::Obstacle]) -> (Vec3, Vec3) {
    let (i, x) = (p.head_look_instance_index, p.head_look_pos);
    if i >= 0 && i == b.ch.inst_id {
        return from_own(x, b);
    }
    crate::ragdoll::Obstacle::find(obstacles, i).map_or((x, Vec3::ZERO), |o| {
        let at = o.to_world(x);
        (at, o.vel_at(at))
    })
}

const OWN_AT: Vec3 = Vec3::new(-3e-5, -0.013_61, 0.003_67);
fn own(b: &Body) -> (Vec3, Quat) {
    let f = b.frames[0];
    (f.translation, f.rotation * crate::nmdbm::rage_link(b.build.nm, 0))
}

pub fn from_own(x: Vec3, b: &Body) -> (Vec3, Vec3) {
    let ((o, r), (v, w)) = (own(b), b.doll.motion(0));
    let at = match b.doll.inst {
        Some([a, bb, c, d]) => crate::vm4(a, bb, c, d, x),
        None => o + r * (x - OWN_AT),
    };
    let r = at - if b.doll.rigid.is_some() { b.at(0) } else { o };
    (at, (-r.yzx()).mul_add(w.zxy(), r.zxy().mul_add(w.yzx(), v)))
}

pub fn inst_place(doll: &crate::ragdoll::Ragdoll, nm: usize) -> Vec3 {
    match doll.inst {
        Some(m) => m[3],
        None => doll.frames().next().map_or(Vec3::ZERO, |f| f.translation - f.rotation * crate::nmdbm::rage_link(nm, 0) * OWN_AT),
    }
}

pub fn to_own(at: Vec3, b: &Body) -> Vec3 {
    if let Some(m) = b.doll.inst {
        return crate::rigid::to_frame(m, at);
    }
    let (o, r) = own(b);
    r.inverse() * (at - o) + OWN_AT
}

