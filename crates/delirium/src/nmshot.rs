use crate::prelude::*;
use crate::Vd;
use crate::Vx;
use crate::Fma;

use crate::nmdbm::{sdot, shz, slen, vnorm, Body, M34};
use crate::nmrs::{Character, ANKLE, CLAVICLE, ELBOW, HIP, KNEE, SHOULDER, SPINE, WRIST};

const SP: [usize; 7] = [0, 7, 8, 9, 10, 11, 12];
const SE: [usize; 6] = [6, 7, 8, 9, 10, 11];
const HEAD: usize = 12;

#[derive(Clone, Copy)]
pub(crate) struct Limb {
    pub e: [usize; 4],
    pub p: [usize; 4],
}

pub(crate) fn limb(i: usize) -> Limb {
    if i < 2 {
        let c = CLAVICLE[i];
        Limb { e: [c, SHOULDER[i], ELBOW[i], WRIST[i]], p: [c + 1, SHOULDER[i] + 1, ELBOW[i] + 1, WRIST[i] + 1] }
    } else {
        let k = i - 2;
        Limb { e: [usize::MAX, HIP[k], KNEE[k], ANKLE[k]], p: [0, HIP[k] + 1, KNEE[k] + 1, ANKLE[k] + 1] }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub normal: Vec3,
    pub head_look: Vec3,
    pub hit_point: Vec3,
    pub bullet_vel: Vec3,
    pub spine_pain_twist_multiplier: f32,
    pub fling_width: f32,
    pub time_before_collapse_wound_leg: f32,
    pub grab_hold_time: f32,
    pub spine_pain_time: f32,
    pub body_stiffness: f32,
    pub looseness_amount: f32,
    pub spine_pain_multiplier: f32,
    pub time_before_reach_for_wound: f32,
    pub p84: f32,
    pub head_look_at_wound_min_timer: f32,
    pub head_look_at_wound_max_timer: f32,
    pub head_look_at_head_pos_max_timer: f32,
    pub head_look_at_head_pos_min_timer: f32,
    pub body_part: usize,
    pub max_steps: i32,
    pub shock_spin_min: f32,
    pub shock_spin_max: f32,
    pub shock_spin_lift_force_mult: f32,
    pub shock_spin_decay_mult: f32,
    pub shock_spin_scale_per_component: f32,
    pub chicken_arms: bool,
    pub always_add_shock_spin: bool,
    pub randomize_shock_spin_direction: bool,
    pub add_shock_spin: bool,
    pub use_head_look: bool,
    pub crouching: bool,
    pub fling: bool,
    pub melee: bool,
    pub no_reach: [bool; 2],
    pub stable_hands_and_neck: bool,
    pub use_catch_fall_on_fall: bool,
    pub reach_for_wound: bool,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            normal: Vec3::new(0.0, 0.0, -1.0), head_look: Vec3::ZERO, hit_point: Vec3::ZERO, bullet_vel: Vec3::new(0.0, 0.0, 20.0),
            spine_pain_twist_multiplier: 0.5, fling_width: 0.5, time_before_collapse_wound_leg: 0.3, grab_hold_time: 2.0,
            spine_pain_time: 1.5, body_stiffness: 11.0, looseness_amount: 1.0, spine_pain_multiplier: 1.0,
            time_before_reach_for_wound: 0.2, p84: 1.0, head_look_at_wound_min_timer: 0.25, head_look_at_wound_max_timer: 0.8,
            head_look_at_head_pos_max_timer: 1.7, head_look_at_head_pos_min_timer: 0.6, body_part: 0, max_steps: 100,
            shock_spin_min: 50.0, shock_spin_max: 90.0, shock_spin_lift_force_mult: 0.0, shock_spin_decay_mult: 4.0,
            shock_spin_scale_per_component: 0.5, chicken_arms: false, always_add_shock_spin: false,
            randomize_shock_spin_direction: false, add_shock_spin: false, use_head_look: false, crouching: false, fling: false,
            melee: false, no_reach: [false; 2], stable_hands_and_neck: false, use_catch_fall_on_fall: false, reach_for_wound: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feedback {
    pub slot: u8,
    pub name: &'static str,
    pub value: f32,
}

#[derive(Default, Debug)]
pub struct Out {
    pub dbm: Option<bool>,
    pub dbm_steps: Option<i32>,
    pub dbm_leg_offset: Option<f32>,
    pub dbm_knee_scale: Option<f32>,
    pub dbm_acc: Option<f32>,
    pub dbm_turn: Option<Vec3>,
    pub hold_stiffness: bool,
    pub feedback: Vec<Feedback>,
    pub head_look_start: Option<(f32, f32)>,
    pub head_look_target: Option<Vec3>,
    pub head_look_activate: bool,
    pub head_look_stop: bool,
    pub catch_fall: Option<[f32; 3]>,
    pub roll_up: Option<f32>,
    pub fall_stop: bool,
    pub spine_twist: Option<(Vec3, f32)>,
    pub spine_twist_stop: bool,
    pub torque: Vec<Vec3>,
    pub force: Vec<Vec3>,
    pub impulse: Vec<(usize, Vec3, Vec3)>,
}

impl Out {
    pub(crate) fn send(&mut self, slot: u8, name: &'static str, value: f32) {
        self.feedback.push(Feedback { slot, name, value });
    }

    pub(crate) fn torque(&mut self, k: usize, t: Vec3) {
        if self.torque.len() <= k {
            self.torque.resize(k + 1, Vec3::ZERO);
        }
        self.torque[k] += t;
    }

    pub(crate) fn force(&mut self, k: usize, f: Vec3) {
        if self.force.len() <= k {
            self.force.resize(k + 1, Vec3::ZERO);
        }
        self.force[k] += f;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Reach {
    pos: Vec3,
    normal: Vec3,
    arm: usize,
    part: usize,
    off: f32,
    sign: f32,
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub p: Params,
    new_hit: bool,
    no_balance: bool,
    since_hit: f32,
    reach_t: [f32; 2],
    time: f32,
    s150: f32,
    s154: f32,
    s158: f32,
    s15c: f32,
    s160: f32,
    s164: f32,
    s168: f32,
    s16c: f32,
    s170: f32,
    t174: f32,
    head_t: f32,
    fall: [f32; 2],
    spin: f32,
    spin_blend: f32,
    t190: f32,
    pub t194: f32,
    behind: f32,
    side: f32,
    reach_part: [Option<usize>; 2],
    loose: f32,
    wound: Vec3,
    hit_local: Vec3,
    normal_local: Vec3,
    reach_pos: [Vec3; 2],
    reach_normal: [Vec3; 2],
    chicken_t: f32,
    react_t: f32,
    react_elbow: f32,
    react_phase: f32,
    reach: [Reach; 2],
    cur_reach: usize,
    cur_arm: usize,
    m228: f32,
    m248: f32,
    m24c: f32,
    m250: f32,
    m254: f32,
    f258: f32,
    f25c: f32,
    f260: f32,
    f264: f32,
    f268: f32,
    f26c: u32,
    f270: f32,
    f274: f32,
    f278: [bool; 2],
    flags: u8,
    grab_front: bool,
    sub: [bool; 15],
}

const LOOSE_L: usize = 0;
const LOOSE_R: usize = 1;
const COLLAPSE_L: usize = 2;
const COLLAPSE_R: usize = 3;
const REACT: usize = 4;
const HIT: usize = 5;
const REACH_L: usize = 6;
const REACH_R: usize = 7;
const STIFF: usize = 8;
const LOOK: usize = 9;
const FALL: usize = 10;
const CHICKEN: usize = 11;
const FLING: usize = 12;
const MELEE: usize = 13;
const CROUCH: usize = 14;

pub struct Ctx<'a, 'b> {
    pub ch: &'a mut Character,
    pub b: &'a Body<'b>,
    pub out: &'a mut Out,
    pub dbm_failed: bool,
    pub(crate) m: M34,
    pub(crate) com: Vec3,
    pub(crate) vel: Vec3,
    pub(crate) angvel: Vec3,
    pub(crate) up: Vec3,
}

impl<'a, 'b> Ctx<'a, 'b> {
    pub fn new(ch: &'a mut Character, b: &'a Body<'b>, out: &'a mut Out, dbm_failed: bool) -> Ctx<'a, 'b> {
        let up = ch.up;
        let s = b.snap(up);
        Ctx { ch, b, out, dbm_failed, m: s.m, com: s.com, vel: s.vel, angvel: s.angvel, up }
    }

    fn rand(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.ch.random()
    }

    fn actual(&self, j: usize) -> [f32; 3] {
        self.ch.actual(j, self.b.frames)
    }

    fn limits(&self, j: usize) -> [f32; 6] {
        self.ch.limits(j)
    }

    fn og(&mut self, j: usize, x: f32) {
        self.ch.eff[j].oppose_gravity = x;
    }

    fn spine_all(&mut self, l1: f32, l2: f32, tw: f32) {
        for j in SPINE {
            self.ch.eff[j].desired = [l1 * 0.25, l2 * 0.25, tw * 0.25];
        }
    }

    pub(crate) fn spine_lean(&mut self, l1: f32, l2: f32) {
        for (k, j) in SPINE.into_iter().enumerate() {
            let lo = k < 2;
            self.ch.eff[j].desired[0] = if lo { 2.0 * l1 / 6.0 } else { l1 / 6.0 };
            self.ch.eff[j].desired[1] = if lo { 0.5 * l2 } else { 0.25 * l2 };
        }
    }

    pub(crate) fn relax(&mut self, x: f32, mask: &str) {
        for j in Character::mask(mask) {
            self.ch.eff[j].blend_to_zero(x);
        }
    }
}

fn world(m: &M34, v: Vec3) -> Vec3 {
    crate::vm4(m.a, m.b, m.c, m.d, v)
}

fn rot(m: &M34, v: Vec3) -> Vec3 {
    crate::vm4(m.a, m.b, m.c, Vec3::ZERO, v)
}

fn local(m: &M34, v: Vec3) -> Vec3 {
    let d = v - m.d;
    Vec3::new(m.a.vd(d), m.b.vd(d), m.c.vd(d))
}

fn horizontal(v: Vec3, up: Vec3) -> Vec3 {
    v - up * up.vd(v)
}

fn deadzone(x: f32, d: f32) -> f32 {
    if x > d { x - d } else if x < -d { x + d } else { 0.0 }
}

type Check = fn(&Shot, &Ctx) -> bool;
type Act = fn(&mut Shot, &mut Ctx);
type Tick = fn(&mut Shot, &mut Ctx, f32);

impl Shot {
    pub fn activate(p: Params, c: &mut Ctx, dt: f32) -> Shot {
        let bs = p.body_stiffness;
        let mut s = Shot {
            p, new_hit: false, no_balance: false, since_hit: 0.0, reach_t: [0.0; 2], time: 0.0,
            s150: 11.0, s154: bs, s158: 1.0, s15c: bs * 0.909_090_94, s160: 1.0, s164: bs / 11.0 * 20.0, s168: 30.0 * dt,
            s16c: bs * 1.818_181_9, s170: 30.0 * dt, t174: if p.fling { 1.8 } else { 3.6 }, head_t: p.head_look_at_wound_max_timer,
            fall: [1.0; 2], spin: 0.0, spin_blend: -1.0, t190: 0.0, t194: 0.0, behind: 0.0, side: 0.0, reach_part: [None; 2],
            loose: 0.01, wound: Vec3::ZERO, hit_local: Vec3::ZERO, normal_local: Vec3::ZERO, reach_pos: [Vec3::ZERO; 2],
            reach_normal: [Vec3::ZERO; 2], chicken_t: 0.0, react_t: 0.0, react_elbow: 0.0, react_phase: 0.0,
            reach: [Reach::default(); 2], cur_reach: 0, cur_arm: 0, m228: 0.0, m248: 0.0, m24c: 0.0, m250: 0.0, m254: 0.0,
            f258: 0.0, f25c: 0.0, f260: 0.0, f264: 0.0, f268: 0.0, f26c: 0, f270: 0.0, f274: 0.0, f278: [false; 2],
            flags: 0x80, grab_front: false, sub: [false; 15],
        };
        for j in 0..c.ch.eff.len() {
            let jd = c.ch.joints[j].clone();
            c.ch.eff[j].reset(&jd);
            c.ch.eff[j].desired = [0.0; 3];
            c.ch.eff[j].oppose_gravity = 1.0;
        }
        if !s.no_balance {
            c.out.dbm = Some(true);
            if !p.chicken_arms {
                c.out.dbm_leg_offset = Some(-0.03);
            }
            c.out.dbm_steps = Some(p.max_steps + 1);
            if p.crouching {
                c.out.dbm_knee_scale = Some(0.5);
            }
        }
        if p.use_catch_fall_on_fall {
            if s.flags & 1 == 0 {
                c.out.hold_stiffness = true;
            }
        }
        if c.ch.busy == [true; 2] {
            c.relax(1.0, "ua");
            c.ch.set_stiffness(12.0, 1.0, "ua", None);
        }
        s.hit(p);
        s
    }

    pub fn hit(&mut self, p: Params) {
        self.p = p;
        self.new_hit = true;
    }

    pub fn subs(&self) -> [bool; 15] {
        self.sub
    }

    pub fn failed(&self) -> bool {
        self.flags & 1 != 0
    }

    fn crouch_check(&self, _: &Ctx) -> bool {
        self.p.crouching && !self.failed()
    }

    fn look_check(&self, _: &Ctx) -> bool {
        (self.grab_front && self.since_hit > 0.19) || self.p.use_head_look
    }

    fn look_done(&self, _: &Ctx) -> bool {
        !self.p.use_head_look && (!self.grab_front || self.new_hit)
    }

    #[allow(clippy::too_many_arguments)]
    fn run(&mut self, c: &mut Ctx, k: usize, enter_check: Option<Check>, enter: Option<Act>, tick: Option<Tick>,
           exit_check: Option<Check>, exit: Option<Act>, dt: f32) {
        if self.sub[k] && exit_check.is_some_and(|f| f(self, c)) {
            self.sub[k] = false;
            if let Some(f) = exit {
                f(self, c);
            }
        }
        if !self.sub[k] && enter_check.is_some_and(|f| f(self, c)) {
            self.sub[k] = true;
            if let Some(f) = enter {
                f(self, c);
            }
        }
        if self.sub[k] {
            if let Some(f) = tick {
                f(self, c, dt);
            }
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        self.t190 = (self.t190 + dt).min(self.t194);
        self.fall[0] = 1.0 - self.t190 / (self.t194 + 1e-5);
        self.fall[1] = if self.t190 <= 0.1 * self.t194 { 1.0 } else { 0.0 };
        (self.s170, self.s168) = (30.0 * dt, 30.0 * dt);
        self.time += dt;
        self.since_hit += dt;
        self.reach_t[0] += dt;
        self.reach_t[1] += dt;
        if self.t190 == self.t194 && self.t194 != 0.0 {
            c.out.send(12, "shot", 0.0);
            self.flags |= 1;
        }
        if c.dbm_failed {
            self.flags |= 1;
        }
        if p.spine_pain_time > 0.0 && !p.melee {
            let k = (p.spine_pain_time - self.since_hit) / p.spine_pain_time;
            if k > 0.0 {
                let mut lean = p.spine_pain_multiplier * k;
                if self.behind > 0.0 {
                    lean *= 0.75;
                }
                if [HIP[0] + 1, KNEE[0] + 1, HIP[1] + 1, KNEE[1] + 1].contains(&p.body_part) {
                    lean *= 1.25;
                }
                let tw = self.side * p.spine_pain_twist_multiplier * k * -10.0;
                c.spine_all(lean, tw, tw);
                for j in SPINE {
                    c.ch.eff[j].blend_to_zero(1.0 - lean);
                }
            }
        }
        if self.failed() || self.no_balance {
            c.out.dbm = Some(false);
        }
        self.run(c, HIT, Some(|s, _| s.new_hit), Some(Shot::hit_enter), Some(Shot::hit_tick), Some(|s, _| !s.new_hit), None, dt);
        self.wound = world(&c.b.part(p.body_part).m, self.hit_local);
        self.shock_spin(c, dt);
        if self.head_t < 0.0 && p.use_head_look {
            self.head_t += dt;
            c.out.head_look_target = Some(p.head_look);
            if self.head_t > 0.0 {
                self.head_t = c.rand(p.head_look_at_wound_min_timer, p.head_look_at_wound_max_timer);
            }
        } else {
            self.head_t -= dt;
            c.out.head_look_target = Some(self.wound);
            if self.head_t < 0.0 {
                self.head_t = -c.rand(p.head_look_at_head_pos_max_timer, p.head_look_at_head_pos_min_timer);
                c.out.send(20, "shot", 0.0);
            }
        }
        self.run(c, CROUCH, Some(Shot::crouch_check), None, Some(Shot::crouch_tick), Some(|s, _| s.failed()), None, dt);
        self.run(c, STIFF, Some(|s, _| s.new_hit && !s.failed()), Some(Shot::stiff_enter), Some(Shot::stiff_tick),
                 Some(|s, _| s.new_hit || s.failed()), Some(Shot::stiff_exit), dt);
        self.run(c, REACT, Some(|_, c| c.ch.busy != [true; 2]), Some(Shot::react_enter), Some(Shot::react_tick),
                 Some(|s, _| s.new_hit || s.failed()), None, dt);
        self.cur_arm = 0;
        self.run(c, LOOSE_L, Some(|s, _| s.time > 1.0 && s.flags & 0x10 != 0 && !s.p.melee), None, Some(Shot::loose_tick),
                 Some(|s, _| s.failed()), None, dt);
        self.cur_arm = 1;
        self.run(c, LOOSE_R, Some(|s, _| s.time > 1.0 && s.flags & 0x08 != 0 && !s.p.melee), None, Some(Shot::loose_tick),
                 Some(|s, _| s.failed()), None, dt);
        self.run(c, FALL, Some(|s, _| s.failed()), Some(Shot::fall_enter), Some(Shot::fall_tick), None, None, dt);
        self.run(c, CHICKEN, Some(|s, _| s.p.chicken_arms && s.new_hit), Some(Shot::chicken_enter), Some(Shot::chicken_tick),
                 Some(|s, _| s.since_hit > s.t174 || s.new_hit), None, dt);
        self.run(c, FLING, Some(|s, _| s.p.fling && s.new_hit), Some(Shot::fling_enter), Some(Shot::fling_tick),
                 Some(|s, _| s.t174 < s.since_hit || s.new_hit), Some(Shot::fling_exit), dt);
        self.cur_reach = 0;
        self.run(c, REACH_L, Some(|s, c| s.reach_check(c, 0)), Some(|s, _| s.reach_enter(0)), Some(Shot::reach_tick),
                 Some(|s, c| s.reach_done(c, 0)), Some(Shot::reach_exit), dt);
        self.cur_reach = 1;
        self.run(c, REACH_R, Some(|s, c| s.reach_check(c, 1)), Some(|s, _| s.reach_enter(1)), Some(Shot::reach_tick),
                 Some(|s, c| s.reach_done(c, 1)), Some(Shot::reach_exit), dt);
        self.run(c, COLLAPSE_L, Some(|s, _| s.flags & 4 != 0 && !s.p.melee && s.since_hit > s.p.time_before_collapse_wound_leg),
                 None, Some(|_, c, _| Shot::collapse(c, "ll")), None, None, dt);
        self.run(c, COLLAPSE_R, Some(|s, _| s.flags & 2 != 0 && !s.p.melee && s.since_hit > s.p.time_before_collapse_wound_leg),
                 None, Some(|_, c, _| Shot::collapse(c, "lr")), None, None, dt);
        self.run(c, LOOK, Some(Shot::look_check), Some(Shot::look_enter), None,
                 Some(Shot::look_done), Some(|_, c| c.out.head_look_stop = true), dt);
        self.run(c, MELEE, Some(|s, _| s.p.melee && s.new_hit), Some(Shot::melee_enter), Some(Shot::melee_tick),
                 Some(|s, _| s.failed() || s.new_hit), Some(|_, c| c.out.spine_twist_stop = true), dt);
        self.new_hit = false;
    }

    pub fn deactivate(&mut self, c: &mut Ctx) {
        if self.flags & 0x80 == 0 {
            return;
        }
        let exits: [(usize, Act); 7] = [
            (REACH_L, Shot::reach_exit), (REACH_R, Shot::reach_exit), (STIFF, Shot::stiff_exit),
            (LOOK, |_, c| c.out.head_look_stop = true), (FALL, |_, c| c.out.fall_stop = true), (FLING, Shot::fling_exit),
            (MELEE, |_, c| c.out.spine_twist_stop = true),
        ];
        for (k, f) in exits {
            if self.sub[k] {
                self.sub[k] = false;
                f(self, c);
            }
        }
        self.flags &= 0x7f;
    }

    fn shock_spin(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        let there = SP.contains(&p.body_part) || [CLAVICLE[0] + 1, SHOULDER[0] + 1, CLAVICLE[1] + 1, SHOULDER[1] + 1].contains(&p.body_part);
        if !(p.add_shock_spin && (p.always_add_shock_spin || there)) {
            return;
        }
        if self.new_hit && self.spin_blend < 0.0 {
            if p.randomize_shock_spin_direction {
                self.spin = c.rand(p.shock_spin_min, p.shock_spin_max);
                if (c.ch.random() * 8_388_608.0) as u32 & 1 != 0 {
                    self.spin = -self.spin;
                }
            } else {
                let mut d = horizontal(p.hit_point - c.com, c.up);
                if p.body_part == HEAD && p.melee {
                    d = d.normalize_or_zero();
                }
                let r = c.rand(p.shock_spin_min, p.shock_spin_max);
                self.spin = r * d.vx(p.bullet_vel.normalize_or_zero()).vd(c.up);
            }
            self.spin_blend = 1.0;
        }
        if self.spin_blend >= 0.0 {
            let axis = (c.b.joint_pos(SPINE[3]) - c.b.joint_pos(SPINE[0])).normalize_or_zero() * self.spin * self.spin_blend;
            let k = p.shock_spin_scale_per_component;
            c.out.torque(SP[3], axis);
            c.out.torque(SP[2], axis * k);
            c.out.torque(SP[4], axis * k);
            c.out.torque(SP[1], axis * k * k);
            c.out.torque(p.body_part, axis * k * k);
            if p.shock_spin_lift_force_mult > 0.0 {
                let f = (axis * k * k).abs() * p.shock_spin_lift_force_mult;
                c.out.force(SP[3], f);
                c.out.force(SP[4], f * k);
                c.out.force(SP[1], f * k * k);
                c.out.force(p.body_part, f * k * k);
            }
            self.spin_blend += p.shock_spin_decay_mult * dt * (-1.0 - self.spin_blend);
        }
    }

    fn hit_enter(&mut self, c: &mut Ctx) {
        self.since_hit = 0.0;
        self.behind = -sdot(self.p.bullet_vel, c.m.c, b"yzx");
        let (mut at, mut normal) = (self.p.hit_point, self.p.normal);
        if self.behind >= 0.0 {
            let far = at + self.p.bullet_vel.normalize_or_zero() * 0.4;
            if let Some((k, x, n)) = c.b.doll.probe(c.b.build, far, at) {
                (self.p.body_part, at, normal) = (k, x, n);
            }
        }
        let p = self.p;
        let m = c.b.part(p.body_part).m;
        self.hit_local = local(&m, at);
        self.normal_local = Vec3::new(m.a.vd(normal), m.b.vd(normal), m.c.vd(normal));
        let from = if SP.contains(&p.body_part) { c.b.at(p.body_part) } else { c.com };
        self.side = sdot(at - from, c.m.a, b"zyx").clamp(-2.0, 2.0);
    }

    fn hit_tick(&mut self, c: &mut Ctx, _dt: f32) {
        let p = self.p;
        let part = p.body_part;
        self.grab_front = p.reach_for_wound && self.behind < 0.0 && part != SP[6] && part != SP[5];
        let (ll, rl, la, ra) = (limb(2), limb(3), limb(0), limb(1));
        if ll.p[1..].contains(&part) {
            self.flags |= 4;
        }
        if rl.p[1..].contains(&part) {
            self.flags |= 2;
        }
        c.out.dbm_steps = Some(p.max_steps + 1);
        let lhit = la.p.contains(&part);
        let rhit = ra.p.contains(&part);
        let thigh = (part == ll.p[1] || part == rl.p[1]) && self.reach_part == [None; 2];
        if lhit && part != la.p[0] && self.reach_part[0].is_none() {
            self.flags |= 0x10;
        }
        if rhit && part != ra.p[0] && self.reach_part[1].is_none() {
            self.flags |= 0x08;
        }
        let mut r7 = self.side > -0.05 && !lhit;
        let mut r8 = self.side <= 0.05 && !rhit;
        if c.ch.busy[0] && !lhit {
            r8 = true;
        }
        if c.ch.busy[1] && !rhit {
            r7 = true;
        }
        if p.no_reach[0] {
            r7 = false;
        }
        if p.no_reach[1] {
            r8 = false;
        }
        if (rhit || thigh || r7) && self.flags & 0x10 == 0 {
            self.reach_part[0] = Some(part);
            (self.reach_pos[0], self.reach_normal[0], self.reach_t[0]) = (self.hit_local, self.normal_local, 0.0);
            self.flags |= 0x40;
        }
        if (lhit || thigh || r8) && self.flags & 0x08 == 0 {
            self.reach_part[1] = Some(part);
            (self.reach_pos[1], self.reach_normal[1], self.reach_t[1]) = (self.hit_local, self.normal_local, -0.1);
            self.flags |= 0x20;
        }
    }

    fn crouch_tick(&mut self, c: &mut Ctx, _dt: f32) {
        if slen(c.vel, b"zyx") < 0.05 && self.time > 0.4 {
            c.out.send(12, "shot", 0.0);
        }
    }

    fn stiff_enter(&mut self, c: &mut Ctx) {
        c.ch.set_stiffness(6.0, 0.5, "lb", None);
        c.ch.set_stiffness(self.s154 * 0.5, 0.5, "us", Some(0.01));
        c.ch.set_stiffness(self.s15c * 0.5, 0.5, "ua", Some(0.01));
        c.ch.set_stiffness(self.s16c * 0.5, 0.3, "uw", Some(0.01));
        c.ch.set_stiffness(self.s164 * 0.5, 0.3, "un", Some(0.01));
    }

    fn stiff_tick(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        let og = (self.fall[0] + 1.0) * 0.5;
        let both = c.ch.busy == [true; 2];
        for i in 0..2 {
            let l = limb(i);
            for j in &l.e[..3] {
                c.og(*j, if both { 1.0 } else { og });
            }
            c.og(l.e[3], og);
        }
        for j in SPINE {
            c.og(j, self.fall[0] + 1.0);
        }
        for j in [SE[4], SE[5]] {
            c.og(j, 1.0);
        }
        for i in 0..2 {
            c.og(HIP[i], og);
            c.og(KNEE[i], og);
            c.og(ANKLE[i], self.fall[1] * 2.0);
        }
        c.ch.set_stiffness(self.fall[1].fma(7.0, 5.0), (self.fall[1] + 1.0) * 0.5, "lb", None);
        for j in ANKLE {
            c.ch.eff[j].muscle_stiffness = 10.0;
            c.ch.eff[j].set_stiffness(self.fall[1].fma(9.0, 3.0), 1.0, None);
        }
        let f27 = 10.0 / (60.0 * dt) + 10.0;
        let f29 = p.looseness_amount.fma(self.loose, 1.0 - p.looseness_amount);
        c.ch.set_stiffness(self.s154 * og, self.s158 * f29, "us", Some(f29));
        c.ch.set_stiffness(self.s15c * og, self.s160 * f29, "ua", Some(f29 * 0.5));
        c.ch.set_stiffness(self.s154 * og, self.s158 * f29, "uc", Some(f29));
        c.ch.set_stiffness((self.s16c * og).min(f27), self.s170, "uw", Some(f29 * 0.5));
        c.ch.set_stiffness((self.s164 * og).min(f27), self.s168 * f29, "un", Some(f29 * 0.5));
        if p.stable_hands_and_neck {
            c.ch.set_stiffness(p.body_stiffness + 1.0, 1.0, "uw", None);
            c.ch.set_stiffness(p.body_stiffness + 1.0, 1.0, "un", None);
        }
        if p.reach_for_wound && self.reach_t[0] > p.grab_hold_time && self.reach_t[1] > p.grab_hold_time {
            c.ch.set_stiffness(p.body_stiffness + 1.0, 1.5, "un", Some(0.5));
            for j in [SE[4], SE[5]] {
                c.ch.eff[j].desired = [0.0; 3];
            }
        }
        self.loose = dt.fma(2.5, self.loose).min(1.0);
        if self.fall[0] < 0.3 {
            for j in KNEE {
                c.ch.eff[j].muscle_stiffness = self.fall[0] * 2.0 + 0.5;
            }
        }
        if both {
            c.ch.set_stiffness(12.0, 1.0, "ua", None);
        }
    }

    fn stiff_exit(&mut self, _c: &mut Ctx) {
        if !(self.new_hit && self.loose < 1.0) {
            self.loose = 0.0;
        }
    }

    fn react_enter(&mut self, c: &mut Ctx) {
        self.react_t = 0.0;
        self.react_elbow = ((c.ch.random_bits() & 0x7F_FFFF) as f32).fma(f32::from_bits(0x33CC_CCCE), 0.4);
        self.react_phase = c.ch.random() - 0.5;
        if self.behind > 0.0 {
            self.react_phase = c.ch.random() * 4.0 - 2.0;
        }
    }

    fn react_tick(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        self.react_t += dt;
        let v = c.b.part(SP[4]).v;
        let ratio = (self.t190 + 1e-4) / (self.t194 + 1e-4);
        let f10 = ((1.0 - ratio) + 1.0) * 0.5;
        let h = c.up * -sdot(c.m.b, c.up, b"yzx") + c.m.b;
        let f13 = (1.0 - slen(h, b"yzx")) * p.p84 * f10;
        let (va, vc) = (sdot(v, c.m.a, b"zyx"), sdot(v, c.m.c, b"zyx"));
        let f28 = deadzone(va - vc, 0.05);
        let f27 = deadzone(-vc - va, 0.05);
        let mut f25 = (f28.abs() * f13 * 0.5).min(1.0);
        let mut f23 = (f27.abs() * f13 * 0.5).min(1.0);
        let mut ph = self.react_t.fma(5.0, self.react_phase);
        ph += if self.behind < 0.0 { 0.785 } else { -1.9625 };
        let f24 = -((ph as f64).sin() as f32) * 1.3;
        let s = -1.3 * ((ph as f64).cos() as f32);
        let f22 = if f28 < 0.0 { -s } else { s };
        let f12 = if f27 < 0.0 { -s } else { s };
        if p.melee {
            f25 *= 0.7;
            f23 *= 0.8;
        }
        for (i, (f, k)) in [(f22, f25), (f12, f23)].into_iter().enumerate() {
            let l = limb(i);
            let tw = if i == 0 { f24.fma(0.6, -0.6) * k } else { f24.fma(0.5, -0.7) * k };
            c.ch.eff[l.e[0]].desired[0] = f * k * 0.5;
            c.ch.eff[l.e[0]].desired[1] = f24.fma(k, -1.4) * 0.4;
            c.ch.eff[l.e[1]].desired = [f * k * 1.2, (f24 - 2.0).fma(k, 1.2) * 0.6, tw];
            c.ch.eff[l.e[2]].desired[0] = ((f + 2.0) * k).fma(0.5, self.react_elbow);
            for &j in &l.e[..3] {
                c.og(j, 0.0);
            }
            if k < 0.5 {
                let w = 2.0 * k;
                for j in [l.e[1], l.e[0], l.e[2]] {
                    let a = c.actual(j);
                    let e = &mut c.ch.eff[j];
                    for n in 0..3 {
                        e.desired[n] = (1.0 - w).fma(a[n], e.desired[n] * w);
                    }
                }
            }
            let wr = &mut c.ch.eff[l.e[3]];
            wr.desired = [wr.desired[0] * 0.5, wr.desired[1] * 0.5, 0.0];
        }
    }

    fn loose_tick(&mut self, c: &mut Ctx, _dt: f32) {
        let l = limb(self.cur_arm);
        let lim = c.limits(l.e[0]);
        c.ch.eff[l.e[0]].desired[1] = lim[2] * 0.5;
        c.ch.eff[l.e[1]].desired = [1.0, 0.7, 0.5];
        c.ch.eff[l.e[2]].desired[0] = 1.0;
    }

    fn fall_enter(&mut self, c: &mut Ctx) {
        for j in 0..c.ch.eff.len() {
            c.ch.eff[j].reset(&c.ch.joints[j]);
        }
        c.ch.set_stiffness(5.0, 0.75, "lb", None);
        c.ch.set_stiffness(9.0, 0.75, "ub", None);
        for j in 0..c.ch.eff.len() {
            c.ch.eff[j].desired = c.actual(j);
        }
    }

    fn fall_tick(&mut self, c: &mut Ctx, _dt: f32) {
        let p = self.p;
        let k = p.body_stiffness / self.s150;
        if self.new_hit {
            for j in 0..c.ch.eff.len() {
                c.ch.eff[j].reset(&c.ch.joints[j]);
            }
            c.ch.set_stiffness(k * 5.0, 0.75, "lb", None);
            c.ch.set_stiffness(k * 9.0, 0.75, "ub", None);
        }
        for j in SE {
            c.og(j, 2.0);
        }
        for j in HIP {
            c.og(j, 2.0);
        }
        if !p.use_catch_fall_on_fall {
            c.out.roll_up = Some(k * 9.0);
        } else {
            let bs = p.body_stiffness;
            let (mut legs, mut torso, mut arms) = (bs * 0.3, bs * 0.7, bs * 0.9);
            if !p.melee {
                let a = (self.fall[0] + 1.0) * 0.5;
                arms *= a;
                torso *= a;
                legs *= (self.fall[1] + 1.0) * 0.5;
            }
            c.out.catch_fall = Some([legs, torso, arms]);
        }
    }

    fn chicken_enter(&mut self, _c: &mut Ctx) {
        let k = (1.0 / self.s150) * self.p.body_stiffness;
        self.chicken_t = 0.0;
        (self.s158, self.s160) = (1.0, 1.0);
        (self.s154, self.s15c) = (k * 14.0, k * 14.0);
        (self.s16c, self.s164) = (k * 20.0, k * 20.0);
    }

    fn chicken_tick(&mut self, c: &mut Ctx, dt: f32) {
        let (l, r) = (limb(0), limb(1));
        let e = &mut c.ch.eff;
        e[l.e[0]].desired = [-0.4, -0.15, 0.0];
        e[l.e[1]].desired = [0.0, 1.15, -0.35];
        e[l.e[2]].desired[0] = 1.0;
        e[l.e[3]].desired = [-0.4, 0.15, 0.4];
        e[r.e[0]].desired = [-0.5, -0.15, 0.0];
        e[r.e[1]].desired = [0.05, 1.15, -0.4];
        e[r.e[2]].desired[0] = 1.5;
        e[r.e[3]].desired = [-0.25, 0.25, 0.4];
        for j in &SE[..3] {
            e[*j].desired[0] = 0.05;
        }
        e[SE[3]].desired[0] = -0.3;
        e[SE[5]].desired[0] = -0.3;
        e[SE[5]].desired[1] = -0.2;
        e[SE[4]].desired[0] = -0.3;
        self.chicken_t += dt;
        let t = self.chicken_t;
        let k = 1.0 - (t / self.t174).min(1.0);
        let cos = |x: f32| (x as f64).cos() as f32;
        e[l.e[1]].desired[2] = cos(13.0 * t).fma(k, e[l.e[1]].desired[2]);
        e[r.e[1]].desired[2] = cos(t.fma(10.0, 0.5)).fma(k, e[r.e[1]].desired[2]);
        self.s15c = ((2.0 * t).min(1.0) + 1.0) * 7.0;
        for j in ELBOW {
            e[j].set_stiffness(self.s15c, 1.0, None);
        }
    }

    fn fling_enter(&mut self, c: &mut Ctx) {
        let p = self.p;
        (self.f258, self.f264, self.f25c, self.f260) = (0.0, 0.0, 0.0, 0.0);
        self.f278 = [true; 2];
        self.f26c = 0;
        if self.failed() {
            c.ch.set_stiffness(15.0, 0.5, "uk", None);
            self.f278 = [false; 2];
            self.f264 = 3.0;
        } else {
            let part = p.body_part;
            let (ll, rl, la, ra) = (limb(2), limb(3), limb(0), limb(1));
            self.f26c = if part == ll.p[3] || part == ll.p[2] {
                1
            } else if part == rl.p[2] || part == rl.p[3] {
                2
            } else if [la.p[3], ra.p[3], la.p[2], ra.p[2]].contains(&part) {
                3
            } else {
                (self.f264, self.f25c, self.f260) = (1.0, 1.0, 1.0);
                4
            };
            if self.f26c <= 2 {
                self.no_balance = true;
            }
        }
        let v = p.bullet_vel.normalize_or_zero();
        let m = c.b.part(SP[3]).m;
        let k = (1.0 / self.s150) * p.body_stiffness;
        self.f268 = self.t174 * f32::from_bits(0x3eaa_aaab);
        (self.s164, self.s16c) = (k * 10.0, k * 10.0);
        self.s154 = k * 12.0;
        self.s15c = k * 14.0;
        self.f270 = sdot(v, m.b, b"zyx");
        self.f274 = sdot(v, m.c, b"zyx");
        for i in 0..2 {
            let l = limb(i);
            for j in [l.e[1], l.e[0], l.e[2]] {
                c.og(j, 1.0);
            }
        }
    }

    fn fling_tick(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        self.f258 += dt;
        if self.failed() {
            c.ch.set_stiffness(16.0, 0.5, "uk", Some(0.5));
            c.ch.set_stiffness(12.0, 0.5, "ua", Some(0.5));
            c.spine_lean(self.f274 * 7.0, -self.f270);
            return;
        }
        let a = vnorm(shz(c.m.a, c.up, b"zyx"));
        let d = vnorm(shz(self.wound - c.com, c.up, b"zyx"));
        let f27 = (-(a.x * d.x)).fma(0.25, (a.z * d.z).fma(-0.25, -((a.y * d.y) * 0.25)));
        let cos = |x: f32| (x as f64).cos() as f32;
        let sin = |x: f32| (x as f64).sin() as f32;
        c.ch.set_stiffness(14.0, 0.5, "ua", Some(0.25));
        c.ch.set_stiffness(12.0, 0.5, "uw", None);
        let mut f30 = 0.0;
        let pi = std::f32::consts::PI;
        if self.f278[0] {
            let mut x = (self.f25c.abs() * ((self.f258 / self.f268).fma(pi, -f27) + 0.7)).min(5.0);
            if x == 5.0 {
                self.f278[0] = false;
            }
            x = x.clamp(0.0, 2.841_592_8);
            f30 = x;
            let l = limb(0);
            let e = &mut c.ch.eff;
            e[l.e[2]].desired[0] = self.f25c.abs() * (4.0 - x).min(0.32) * 3.5;
            e[l.e[0]].desired[0] = sin(x).fma(0.6, p.fling_width.fma(0.375, -0.2));
            e[l.e[0]].desired[1] = cos(x) - 0.5;
            e[l.e[1]].desired[0] = sin(x) * 0.5;
            e[l.e[1]].desired[1] = (1.5 - p.fling_width) * cos(x);
            e[l.e[1]].desired[2] = sin(x).fma(-2.5, -1.0);
        } else {
            c.ch.set_stiffness(9.0, 0.5, "ul", None);
        }
        if self.f278[1] {
            let mut x = (self.f260.abs() * ((self.f258 / self.f268).fma(pi, f27) + 0.7)).min(4.5);
            if x == 4.5 {
                self.f278[1] = false;
            }
            x = x.clamp(0.0, 3.041_592_8);
            f30 = x;
            let r = limb(1);
            let e = &mut c.ch.eff;
            e[r.e[2]].desired[0] = self.f260.abs() * (4.0 - x).min(0.32) * 2.5;
            e[r.e[0]].desired[0] = sin(x).fma(0.6, p.fling_width.fma(0.45, -0.25));
            e[r.e[0]].desired[1] = cos(x) - 0.5;
            e[r.e[1]].desired[0] = sin(x) * 0.5;
            e[r.e[1]].desired[1] = (1.65 - p.fling_width) * cos(x);
            e[r.e[1]].desired[2] = sin(x).fma(-2.5, -1.0);
        } else {
            c.ch.set_stiffness(9.0, 0.5, "ur", None);
        }
        c.ch.eff[SE[4]].desired[2] = sin(f30) * -0.3;
        c.spine_lean(self.f274 * self.f264 * -0.3, self.f270 * 0.15);
        for j in [SE[4], SE[5]] {
            c.ch.eff[j].desired[0] = -(self.f274.abs() * self.f264);
            c.ch.eff[j].desired[1] = self.f270;
        }
        for (n, i) in [(1, 0), (2, 1)] {
            if self.f26c == n && self.f278[i] {
                let l = limb(2 + i);
                let t = c.up.mul_add(Vec3::splat(0.6), c.b.part(l.p[3]).m.d);
                let side = [1.0, -1.0][i];
                crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, 0.0, 1.0, side, -1.0, Vec3::ZERO, None);
            }
        }
        if self.f258 > 0.2 && (self.f26c == 1 || self.f26c == 2) {
            self.no_balance = false;
        }
    }

    fn fling_exit(&mut self, c: &mut Ctx) {
        for j in [SE[5], SE[4]] {
            c.ch.eff[j].desired = [0.0; 3];
        }
        c.spine_lean(0.0, 0.0);
        (self.s154, self.s15c) = (12.0, 12.0);
        (self.s164, self.s16c) = (17.0, 17.0);
    }

    fn reach_check(&self, c: &Ctx, i: usize) -> bool {
        let p = &self.p;
        c.ch.busy != [true; 2] && p.reach_for_wound && self.since_hit > p.time_before_reach_for_wound
            && self.flags & [0x40, 0x20][i] != 0 && self.reach_t[i] < p.grab_hold_time && !c.ch.busy[i]
    }

    fn reach_enter(&mut self, i: usize) {
        self.reach[i] = Reach { pos: self.reach_pos[i], normal: self.reach_normal[i], arm: i, part: self.reach_part[i].unwrap_or(0),
                                off: [0.2, -0.2][i], sign: 1.0 };
        self.flags &= ![0x40, 0x20][i];
    }

    fn reach_done(&self, c: &Ctx, i: usize) -> bool {
        self.flags & [0x40, 0x20][i] != 0 || self.reach_t[i] > self.p.grab_hold_time || c.ch.busy[i] || !self.p.reach_for_wound
    }

    fn reach_exit(&mut self, _c: &mut Ctx) {
        let g = self.p.grab_hold_time;
        if self.reach_t[0] > g && self.reach_t[1] > g {
            self.grab_front = false;
        }
    }

    fn reach_tick(&mut self, c: &mut Ctx, dt: f32) {
        let i = self.cur_reach;
        let r = self.reach[i];
        let l = limb(r.arm);
        let lim = c.limits(l.e[0]);
        if [SP[0], SP[1], SP[2]].contains(&r.part) {
            c.ch.eff[l.e[0]].desired[0] = lim[1];
            c.ch.eff[l.e[0]].desired[1] = lim[3] * 0.5;
        } else {
            c.ch.eff[l.e[0]].desired[0] = lim[1] * 0.5;
            c.ch.eff[l.e[0]].desired[1] = lim[2] * 0.6;
        }
        for j in [l.e[1], l.e[0], l.e[2]] {
            c.og(j, 1.0);
        }
        let m = c.b.part(r.part).m;
        let t = world(&m, r.pos);
        let n = rot(&m, r.normal) * r.sign;
        let v = vnorm(n - c.m.c);
        let mut dh = c.b.at(l.p[3]) - t;
        let s = sdot(dh, v, b"zyx");
        dh -= v * if -s >= 0.0 { 0.0 } else { s };
        let len = slen(dh, b"zyx");
        let t2 = v.mul_add(Vec3::splat(len.fma(0.5, -0.05).min(0.25)), t);
        let pelvis = c.b.at(SP[0]);
        let (a, b) = (t - pelvis, c.b.at(SP[4]) - pelvis);
        let along = (sdot(a, b, b"zyx") / sdot(b, b, b"zyx")).clamp(0.0, 0.8);
        let mut yaw = (-along).fma(2.5, 1.0);
        if yaw < 0.0 {
            yaw *= 1.0 - (len * 2.5).min(1.0);
        }
        if [limb(2).p[1], limb(3).p[1], limb(2).p[2], limb(3).p[2]].contains(&r.part) {
            yaw = 1.0;
        }
        let side = if i == 0 { 1.0 } else { -1.0 };
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t2, yaw + 0.1 * side, 0.0, side, 1.0, Vec3::ZERO, None);
        let ret = Self::wrist(c, l, t, n, side);
        if ret < 0.0 && r.part != HEAD {
            self.reach[i].sign = -self.reach[i].sign;
        }
        let strength = if self.t190 >= self.t194 && self.t194 != 0.0 { 0.5 } else { 4.0 };
        self.pull(c, l, t, self.reach[i].sign, strength, 0.1, r.part, dt);
        c.ch.eff[l.e[0]].blend_to_zero(0.5);
    }

    pub(crate) fn wrist(c: &mut Ctx, l: Limb, t: Vec3, n: Vec3, side: f32) -> f32 {
        let (mut sh, el, mut w) = (c.b.eff3(l.e[1]), c.b.eff1(l.e[2]), c.b.eff3(l.e[3]));
        sh.desired = c.ch.eff[l.e[1]].desired;
        let e = crate::nmdbm::predict_knee(&sh, &el, side);
        use crate::rigid::nrm;
        let fore = t - e;
        let c1 = fore.vx(sh.m.d - e);
        let (pa, pb, pc);
        if slen(c1, b"zyx") > 0.05 {
            let (d, cn) = (nrm(fore), nrm(c1));
            (pa, pb, pc) = (cn.vx(d), cn, d);
        } else {
            (pa, pb, pc) = (w.m.a, w.m.c.vx(w.m.a), w.m.c);
        }
        let b2 = pb * side;
        let (na, nb, npc) = (sdot(n, pa, b"yzx"), -sdot(n, b2, b"yxz"), sdot(n, pc, b"yzx"));
        let len = nb.fma(nb, na * na).sqrt() + 1e-6;
        let (inv, max) = (1.0 / len, len * 2.4);
        let tw = ((inv * na) as f64).atan2((inv * nb) as f64) as f32;
        let tw = tw.clamp(-max, max);
        let (s, co) = ((tw as f64).sin() as f32, (tw as f64).cos() as f32);
        let pp = b2.mul_add(Vec3::splat(-co), pa.mul_add(Vec3::splat(s), Vec3::ZERO));
        let phi = (npc as f64).atan2(sdot(n, pp, b"zyx") as f64) as f32;
        w.desired = [-(phi * s), phi * co, tw];
        c.ch.eff[l.e[3]].desired = w.desired;
        -sdot(n, b2, b"zyx")
    }

    #[allow(clippy::too_many_arguments)]
    fn pull(&self, c: &mut Ctx, l: Limb, t: Vec3, sign: f32, strength: f32, dist: f32, part: usize, dt: f32) {
        let hand = c.b.part(l.p[3]);
        let at = hand.m.d - hand.m.a * (sign * 0.02);
        let d = (t - at).length();
        let wrist = c.b.joint_pos(l.e[3]);
        if d >= 3.0 * dist {
            return;
        }
        let k = ((3.0 * dist - d) / (2.0 * dist)).clamp(0.0, 1.0) * strength;
        let wound = c.b.part(part);
        let wv = wound.v;
        let mut imp = (t - at) * k * k + (wv - hand.v) * (k * 0.2);
        if dt < 1.0 / 60.0 {
            imp *= dt * 60.0;
        }
        c.out.impulse.push((l.p[3], wrist, imp));
        c.out.impulse.push((part, t, -imp));
    }

    fn collapse(c: &mut Ctx, mask: &str) {
        c.ch.set_stiffness(5.0, 1.0, mask, None);
        c.out.dbm_knee_scale = Some(0.416_666_66);
        c.out.dbm_acc = Some(-0.5);
    }

    fn look_enter(&mut self, c: &mut Ctx) {
        c.out.head_look_start = Some((self.p.body_stiffness + 1.0, 1.5));
    }

    fn melee_enter(&mut self, c: &mut Ctx) {
        let (a, b) = c.ch.stance_yaw;
        (self.m228, self.m250, self.m254) = (0.0, b, 0.4);
        (self.m248, self.m24c) = (a * 2.0, a * 2.0);
        c.spine_all(0.0, 0.0, 0.0);
    }

    fn melee_tick(&mut self, c: &mut Ctx, dt: f32) {
        let p = self.p;
        let target = if self.m254 > 0.0 {
            self.m254 -= dt;
            let m = c.b.part(SP[6]).m;
            m.c.mul_add(Vec3::splat(-5.0), m.d)
        } else {
            p.head_look
        };
        c.out.head_look_target = Some(target);
        c.out.head_look_activate = true;
        let to = vnorm(shz(vnorm(p.head_look - c.com), c.up, b"zyx"));
        let turn = |a: f32| {
            let sc = |a: f32| if a == 0.0 { (0.0, 1.0) } else { ((a as f64).sin() as f32, (a as f64).cos() as f32) };
            let ((sx, cx), (sy, cy), (sz, cz)) = (sc(c.up.x * a), sc(c.up.y * a), sc(c.up.z * a));
            let (czsy, szsy) = (cz * sy, sz * sy);
            let m = [Vec3::new(cz * cy, sz * cy, -sy),
                     Vec3::new(czsy.fma(sx, -(sz * cx)), szsy.fma(sx, cz * cx), cy * sx),
                     Vec3::new(czsy.fma(cx, sz * sx), szsy.fma(cx, -(cz * sx)), cy * cx)];
            Vec3::new(to.vd(Vec3::new(m[0].x, m[1].x, m[2].x)), to.vd(Vec3::new(m[0].y, m[1].y, m[2].y)), to.vd(Vec3::new(m[0].z, m[1].z, m[2].z)))
        };
        c.out.dbm_turn = Some(turn(self.m248 - self.m250));
        (c.out.dbm_leg_offset, c.out.dbm_acc) = (Some(0.0), Some(0.0));
        c.out.spine_twist = Some((turn(self.m24c - self.m250) + c.com, p.body_stiffness));
        self.m228 = ((-self.m228).mul_add(0.1, slen(c.vel, b"yxz")) * (dt * 120.0)).min(1.0);
        if self.m228 < 0.1 {
            self.m228 = 0.001;
        }
        c.spine_lean(0.0, 0.0);
        c.relax((-self.m228).fma(0.5, 1.0), "us");
        let x = (((self.since_hit.clamp(0.0, 4.9) * 0.321_428_57) as f64).sin()) as f32;
        if self.since_hit < 0.7 {
            c.relax(x.clamp(0.0, 1.0), "ua");
        } else {
            c.relax((-self.m228).fma(0.65, 1.0), "ua");
        }
    }
}

