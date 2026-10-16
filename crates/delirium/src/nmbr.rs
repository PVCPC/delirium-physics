use crate::Fma;
use crate::Vd;
use crate::Vx;
use bevy_math::Vec3;

use crate::nmdbm::Dbm;
use crate::nmgr::{Grab, GrabParams};
use crate::nmhl::HeadLook;
use crate::nmpd::Pedal;
use crate::nmrds::RollDownStairs;
use crate::nmrs::Character;
use crate::nmru::RollUp;
use crate::nmshot::{limb, Ctx, Limb};
use crate::nmst::SpineTwist;
use crate::ragdoll::Obstacle;

const PELVIS: usize = 0;
const SPINE2: usize = 9;
const SPINE3: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct BraceParams {
    pub body_stiffness: f32,
    pub pos: Vec3,
    pub instance_index: i32,
    pub brace_distance: f32,
    pub target_prediction_time: f32,
    pub reach_absorbtion_time: f32,
    pub look: Vec3,
    pub grab_strength: f32,
    pub grab_distance: f32,
    pub grab_reach_angle: f32,
    pub grab_hold_timer: f32,
    pub max_grab_speed: f32,
    pub behind_time: f32,
    pub grab_dont_let_go: bool,
    pub leg_stiffness: f32,
}

impl Default for BraceParams {
    fn default() -> BraceParams {
        BraceParams { body_stiffness: 12.0, pos: Vec3::new(0.0, 1.0, -1.5), instance_index: -1, brace_distance: 0.65,
                      target_prediction_time: 0.3, reach_absorbtion_time: 0.075, look: Vec3::ZERO, grab_strength: 40.0,
                      grab_distance: 2.0, grab_reach_angle: 1.5, grab_hold_timer: 2.5, max_grab_speed: 95.0, behind_time: 1.0,
                      grab_dont_let_go: false, leg_stiffness: 12.0 }
    }
}

pub struct Kids<'a> {
    pub head: &'a mut HeadLook,
    pub twist: &'a mut SpineTwist,
    pub pedal: &'a mut Pedal,
    pub rds: &'a mut RollDownStairs,
    pub ru: &'a mut RollUp,
    pub grab: &'a mut Grab,
    pub dbm: Option<&'a mut Dbm>,
    pub fallen: bool,
}

#[derive(Default)]
pub struct BraceOut {
    pub catch_fall: Option<crate::nmcf::Params>,
    pub stop_catch_fall: bool,
    pub stop_dbm: bool,
    pub leg_stiffness: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct Brace {
    pub p: BraceParams,
    target: Vec3,
    short: Vec3,
    tvel: Vec3,
    to: Vec3,
    reach: [Vec3; 2],
    normal: [Vec3; 2],
    contact: [Vec3; 2],
    clock: f32,
    dist: f32,
    lean: f32,
    lean2: f32,
    head_t: f32,
    delay: f32,
    side: [f32; 2],
    crouch: f32,
    split: f32,
    scale: f32,
    legs: f32,
    pub active: bool,
    braced: bool,
    fell: bool,
    on_target: [bool; 2],
    left_farther: bool,
    pick: bool,
    grabbed: bool,
    arm: [bool; 2],
    head_at: bool,
}

fn touching(c: &Ctx, k: usize) -> bool {
    c.b.doll.touch[k] & 0x28 == 0x20
}

fn contact(c: &Ctx, i: usize) -> Option<(Vec3, Vec3, i32)> {
    let l = limb(i);
    let k = if touching(c, l.p[3]) { l.p[3] } else { l.p[2] };
    c.b.doll.hit[k]
}

fn brace_wrist(c: &mut Ctx, l: Limb, n: Vec3, side: f32) {
    use crate::nmdbm::sdot;
    let mut w = c.b.eff3(l.e[3]);
    let (pa, pb, pc) = (w.m.a, w.m.c.vx(w.m.a), w.m.c);
    let b2 = pb * side;
    let (na, nb, npc) = (sdot(n, pa, b"yzx"), -sdot(n, b2, b"yxz"), sdot(n, pc, b"yzx"));
    let len = nb.fma(nb, na * na).sqrt() + 1e-6;
    let inv = 1.0 / len;
    let tw = ((inv * na) as f64).atan2((inv * nb) as f64) as f32;
    let tw = tw.clamp(-len, len);
    let (s, co) = ((tw as f64).sin() as f32, (tw as f64).cos() as f32);
    let pp = b2.mul_add(Vec3::splat(-co), pa.mul_add(Vec3::splat(s), Vec3::ZERO));
    let phi = (npc as f64).atan2(sdot(n, pp, b"zyx") as f64) as f32;
    w.desired = [-(phi * s), phi * co, tw];
    c.ch.eff[l.e[3]].desired = w.desired;
}

fn world(obstacles: &[Obstacle], pos: Vec3, inst: i32) -> Vec3 {
    Obstacle::find(obstacles, inst).map_or(pos, |o| o.to_world(pos))
}

fn flat(v: Vec3, up: Vec3) -> Vec3 {
    up * -crate::nmdbm::sdot(up, v, b"zyx") + v
}

impl Brace {

    pub fn place(&mut self, obstacles: &[Obstacle]) {
        self.target = world(obstacles, self.p.pos, self.p.instance_index);
    }

    pub fn activate(&mut self, p: BraceParams, c: &mut Ctx, k: &mut Kids, obstacles: &[Obstacle]) -> BraceOut {
        let mut out = BraceOut::default();
        if self.active {
            return out;
        }
        *self = Brace { p, active: true, ..Default::default() };
        let ch = &mut *c.ch;
        for j in 0..ch.eff.len() {
            ch.eff[j].reset(&ch.joints[j]);
            ch.eff[j].oppose_gravity = 1.0;
        }
        self.target = world(obstacles, p.pos, p.instance_index);
        (self.dist, self.legs) = (100.0, p.leg_stiffness);
        out.leg_stiffness = Some(p.leg_stiffness);
        k.head.activate(c.ch, c.b);
        k.head.always_look = true;
        self.pick = true;
        self.draw_sides(c.ch);
        self.crouch = c.ch.random().fma(0.08, 0.02);
        self.split = c.ch.random() - 0.5;
        self.scale = c.ch.random().fma(0.4, 0.2);
        if self.crouch < 0.05 {
            self.scale = c.ch.random().fma(0.15, 0.2);
        }
        out
    }

    fn draw_sides(&mut self, ch: &mut Character) {
        let (a, b) = if ch.mode == 2 { (0.075, 0.155) } else { (0.17, 0.23) };
        self.side[0] = ch.random().fma(b, a);
        self.side[1] = ch.random().fma(b, a);
    }

    pub fn deactivate(&mut self, c: &mut Ctx, k: &mut Kids) -> BraceOut {
        let mut out = BraceOut::default();
        if !self.active {
            return out;
        }
        self.active = false;
        if let Some(d) = k.dbm.as_deref_mut() {
            (d.pv.fixed_vel, d.pv.vel, d.leg_offset) = (false, Vec3::ZERO, 0.0);
            out.stop_dbm = true;
        }
        k.head.deactivate();
        k.twist.deactivate();
        if self.fell {
            k.grab.deactivate(c.ch, k.head, k.twist, k.dbm.as_deref_mut());
            out.stop_catch_fall = true;
            k.rds.deactivate(k.ru);
            k.pedal.deactivate();
        }
        c.out.send(20, "brace", 12.0);
        out
    }

    pub fn tick(&mut self, c: &mut Ctx, k: &mut Kids, obstacles: &[Obstacle], dt: f32) -> BraceOut {
        let mut out = BraceOut::default();
        if !self.active {
            return out;
        }
        self.clock += dt;
        if !k.fallen {
            self.stand(c, k, obstacles, dt, &mut out);
            return out;
        }
        let p = self.p;
        if k.grab.grip.iter().any(|g| g.is_some()) {
            let pd = &mut *k.pedal;
            (pd.random_seed, pd.pedal_offset, pd.speed_asymmetry, pd.radius, pd.drag_multiplier) = (100, 0.02, 4.0, 0.3, 0.3);
            (pd.left, pd.right, pd.back_pedal, pd.adaptive) = (true, true, false, true);
            pd.leg_stiffness = (p.body_stiffness - 1.0).clamp(1.0, 20.0);
            pd.activate(c);
        } else {
            k.pedal.deactivate();
            if self.fell {
                k.grab.deactivate(c.ch, k.head, k.twist, k.dbm.as_deref_mut());
            }
        }
        if !self.fell {
            self.fell = true;
            if let Some(d) = k.dbm.as_deref_mut() {
                (d.pv.fixed_vel, d.pv.vel, d.leg_offset) = (false, Vec3::ZERO, 0.0);
            }
            out.stop_dbm = true;
            k.twist.deactivate();
            k.head.deactivate();
            let ub: Vec<usize> = Character::mask("ub").into_iter().map(|j| c.ch.joints[j].child).collect();
            let touched = c.b.doll.collided(&ub);
            let mut roll = c.ch.random_bits() & 1 != 0;
            if !touched {
                let (ll, rl) = (limb(2), limb(3));
                let feet = touching(c, ll.p[3]) || touching(c, rl.p[3]);
                let rest = [ll.p[2], rl.p[2], ll.p[1], rl.p[1], PELVIS].into_iter().any(|q| touching(c, q));
                if feet && !rest {
                    roll = false;
                }
            }
            if roll {
                let r = &mut *k.rds;
                (r.asymmetrical_legs, r.forcemag, r.spin_when_in_air, r.use_arm_to_slow_down) = (0.4, 0.4, true, -0.9);
                (r.asymmetry, r.arm_reach_amount, r.leg_push, r.arm_reach_length) = (0.0, 1.4, 0.2, 0.6);
                r.stiffness = (p.body_stiffness - 3.0).clamp(1.0, 15.0);
                r.asymmetrical_legs = 0.2 + 0.6 * c.ch.random();
                r.activate(c, k.ru);
            } else {
                let s = p.body_stiffness;
                out.catch_fall = Some(crate::nmcf::Params { legs_stiffness: s * 0.5, torso_stiffness: s * 0.75, arms_stiffness: s * 1.25,
                                                            forward_max_arm_offset: 0.35, backwards_min_arm_offset: -0.25,
                                                            z_axis_spin_reduction: 0.0, use_head_look: true });
            }
        }
        if self.grabbed {
            return out;
        }
        let speed = Obstacle::find(obstacles, p.instance_index).map_or(0.0, |o| o.vel_at(Vec3::ZERO).length());
        let mut on = [false; 2];
        for i in [1, 0] {
            let l = limb(i);
            if touching(c, l.p[3]) || touching(c, l.p[2]) {
                if let Some((at, n, inst)) = contact(c, i) {
                    (self.contact[i], self.normal[i]) = (at, n);
                    on[i] = inst == p.instance_index;
                }
            }
        }
        if (on[0] || on[1]) && speed < p.max_grab_speed && !k.grab.active {
            self.grabbed = true;
            let mut g = GrabParams { dont_let_go: p.grab_dont_let_go, grab_strength: p.grab_strength, grab_dist: p.grab_distance,
                                     reach_angle: p.grab_reach_angle, grab_hold_max_timer: p.grab_hold_timer, linegrab: false,
                                     use_left: on[0], use_right: on[1], instance: p.instance_index, ..k.grab.p };
            k.grab.p = g;
            k.grab.place(obstacles);
            if on[0] {
                (g.pos[1], g.normal_l) = (k.grab.to_local(self.contact[0]), self.normal[0]);
            }
            if on[1] {
                (g.pos[0], g.normal_r) = (k.grab.to_local(self.contact[1]), self.normal[1]);
            }
            k.grab.p = g;
            k.grab.activate(c, k.head, k.twist, k.dbm.as_deref_mut());
        }
        out
    }

    fn stand(&mut self, c: &mut Ctx, k: &mut Kids, obstacles: &[Obstacle], dt: f32, out: &mut BraceOut) {
        let p = self.p;
        let up = c.up;
        let now = world(obstacles, p.pos, p.instance_index);
        use crate::nmdbm::{sdot, slen};
        self.tvel = (now - self.target) * (1.0 / dt);
        let len = slen(self.tvel, b"yxz");
        if len > 10.0 {
            self.tvel *= 10.0 / len;
        }
        self.target = now;
        let s2 = c.b.part(SPINE2).m.d;
        let rel = c.vel - self.tvel;
        let d = slen(self.target - s2, b"yzx");
        let dir = crate::rigid::nrm(self.target - s2);
        let close = sdot(rel, dir, b"yxz");
        let pred = (-close).fma(p.target_prediction_time, d).max(0.1);
        self.short = dir * -(close * p.reach_absorbtion_time).max(d - p.brace_distance).min(d - 0.3);
        self.dist = pred.min((1.0 - 2.0 * dt).fma(self.dist, 2.0 * dt * pred));
        self.braced = self.dist < p.brace_distance + if self.braced { 0.1 } else { 0.0 };
        if !self.braced {
            self.arm = [false; 2];
        } else {
            let b = flat(c.b.part(SPINE2).m.b, up);
            for i in 0..2 {
                let a = sdot(crate::rigid::nrm(flat(self.target - c.b.joint_pos(limb(i).e[1]), up)), b, b"zyx");
                let a = crate::nmdbm::acos(a);
                let x = if i == 0 { 2.891_592_7 - a } else { a - 0.25 };
                if !self.arm[i] {
                    if x > 0.2 && !c.ch.busy[i] {
                        self.arm[i] = true;
                    }
                } else if x < -0.2 {
                    self.arm[i] = false;
                }
            }
        }
        if self.braced && self.clock < p.behind_time && -sdot(c.m.c, flat(self.target - c.com, up), b"zyx") < 0.0 {
            self.arm = [false; 2];
            self.braced = false;
        }
        let lv = ((c.b.at(SPINE3) - c.b.at(PELVIS)) * 0.5 + (c.b.part(SPINE3).v - c.b.part(PELVIS).v) * 0.3) * 2.0;
        let lean = (crate::nmdbm::slen(lv, b"zyx") - 0.15).clamp(0.0, 1.0);
        for i in 0..2 {
            if self.arm[i] {
                continue;
            }
            self.guard(c, i, lv, lean);
        }
        if self.arm[1] && !self.arm[0] || !self.arm[1] && self.arm[0] {
            (self.delay, self.pick) = (0.0, false);
        }
        if self.arm[0] && self.arm[1] && self.pick {
            let dl = crate::nmdbm::slen(self.target - c.b.joint_pos(limb(0).e[3]), b"yxz");
            let dr = crate::nmdbm::slen(self.target - c.b.joint_pos(limb(1).e[3]), b"yxz");
            self.left_farther = dl > dr;
            self.delay = (c.ch.random_bits() & 0x7F_FFFF) as f32 * 1.192_092_9e-8;
        }
        self.to = crate::rigid::nrm(self.target - c.com);
        if let Some(d) = k.dbm.as_deref_mut() {
            (d.pv.fixed_vel, d.pv.vel) = (true, self.to);
        }
        self.spine(c, k.dbm.as_deref_mut(), dt);
        if self.braced {
            if let Some(d) = k.dbm.as_deref_mut() {
                d.leg_offset = -self.crouch;
            }
            self.pick = false;
            if p.leg_stiffness != self.legs {
                if k.dbm.is_some() {
                    out.leg_stiffness = Some(p.leg_stiffness);
                    self.legs = p.leg_stiffness;
                }
            }
            self.braced_tick(c, k, dt);
        } else {
            (self.delay, self.pick) = (0.0, true);
            self.draw_sides(c.ch);
            if let Some(d) = k.dbm.as_deref_mut() {
                d.leg_offset = 0.0;
            }
            k.twist.deactivate();
        }
        self.head_t -= dt;
        if self.head_t <= 0.0 {
            self.head_t = ((c.ch.random_bits() & 0x7F_FFFF) as f32).fma(1.490_116_1e-7, 0.25);
            self.head_at = c.ch.random_bits() & 1 != 0;
        }
        let at = world(obstacles, p.pos, p.instance_index);
        let behind = crate::nmdbm::sdot(c.m.c, crate::rigid::nrm(at - c.com), b"yzx") > 0.7;
        k.head.pos = if self.head_at && self.braced && (self.arm[0] || self.arm[1]) && !behind {
            world(obstacles, p.pos * 0.83 + p.look * 0.17, p.instance_index)
        } else {
            world(obstacles, p.look, p.instance_index)
        };
        k.head.stiffness = p.body_stiffness - 1.0;
        k.head.always_look = true;
    }

    fn guard(&self, c: &mut Ctx, i: usize, lv: Vec3, lean: f32) {
        use crate::nmdbm::sdot;
        let l = limb(i);
        c.ch.set_stiffness(lean.fma(3.0, 7.0), 1.0, if i == 0 { "ul" } else { "ur" }, None);
        let m = c.b.eff3(l.e[1]).m;
        let a = sdot(m.a, lv, b"zyx") * 4.0;
        let side = if i == 0 { m.c - m.b } else { m.c + m.b } * 0.707;
        let b = sdot(side, lv, b"zyx") * 4.0;
        let (sin, cos) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
        let lift = (-lean).fma(0.5, cos + b.clamp(-8.0, 8.0));
        let ch = &mut *c.ch;
        let (sl, cl) = (ch.limits(l.e[1]), ch.limits(l.e[0]));
        ch.eff[l.e[1]].oppose_gravity = lean;
        ch.eff[l.e[0]].oppose_gravity = lean;
        let sh = &mut ch.eff[l.e[1]].desired;
        sh[0] = ((sl[0] + sl[1]) * 0.5 - sin) * lean;
        sh[1] = ((sl[2] + sl[3]) * 0.5 + lift).fma(lean, (1.0 - lean) * 0.2);
        sh[2] = if i == 0 { 0.0 } else { -0.1 };
        if i == 0 {
            ch.eff[l.e[0]].desired = [0.0, ((cl[2] + cl[3]) * 0.5 + lift) * lean, 0.0];
            ch.eff[l.e[3]].desired = [0.0; 3];
            ch.eff[l.e[2]].desired[0] = (1.0 - lean).fma(0.7, lean);
        } else {
            ch.eff[l.e[0]].desired = [0.0; 3];
            ch.eff[l.e[3]].desired[2] = 0.0;
            ch.eff[l.e[2]].desired[0] = ((1.0 - lean) + lean) * 0.8;
        }
        if ch.busy[i] {
            ch.eff[l.e[3]].set_stiffness(self.p.body_stiffness, 1.5, None);
        }
    }

    fn spine(&mut self, c: &mut Ctx, dbm: Option<&mut Dbm>, dt: f32) {
        let (l1, l2) = (self.lean * 1.5, self.lean2);
        let ch = &mut *c.ch;
        let sp = crate::nmrs::SPINE;
        for (n, j) in sp.into_iter().enumerate() {
            let lo = n < 2;
            ch.eff[j].desired[0] = if lo { 2.0 * l1 / 6.0 } else { l1 / 6.0 };
            ch.eff[j].desired[1] = if lo { 0.5 * l2 } else { 0.25 * l2 };
        }
        let q = self.lean * 0.25;
        ch.eff[sp[0]].desired[0] = (self.split + 1.5) * q;
        ch.eff[sp[1]].desired[0] = (self.split + 1.5) * q;
        ch.eff[sp[2]].desired[0] = (1.5 - self.split) * q;
        ch.eff[sp[3]].desired[0] = (1.5 - self.split) * q;
        if let Some(d) = dbm {
            d.acc = self.lean * -0.666_666_7;
        }
        self.lean /= dt.fma(3.18, 1.0);
    }

    fn braced_tick(&mut self, c: &mut Ctx, k: &mut Kids, dt: f32) {
        let up = c.up;
        k.twist.pos = self.target;
        k.twist.vel = self.tvel;
        use crate::nmdbm::sdot;
        let d = c.com - self.target;
        let away = crate::rigid::nrm(up * -sdot(up, d, b"zyx") + d);
        let h = sdot(up, c.b.at(SPINE3), b"yzx") - sdot(up, self.target, b"zyx");
        self.lean = (h * self.scale).fma(sdot(c.m.c, away, b"zyx"), 0.2);
        if self.arm[0] && self.arm[1] {
            k.twist.stiffness = self.p.body_stiffness;
            k.twist.arms = false;
            k.twist.activate(c.ch);
        } else {
            k.twist.deactivate();
        }
        self.reach_arms(c, k.twist.twisting, dt);
    }

    fn reach_arms(&mut self, c: &mut Ctx, twisting: bool, dt: f32) {
        let up = c.up;
        let s2 = c.b.part(SPINE2);
        let r = self.target + self.short - s2.m.d;
        let side = crate::rigid::nrm((self.target - s2.m.d).vx(up));
        let r2 = r + r.vx(s2.w) * 0.1;
        let base = s2.m.d + crate::rigid::nrm(r2) * crate::nmdbm::slen(r, b"yxz");
        let (mut yaw, mut sense, mut most) = (0.0, 1.0, 0.55);
        if !twisting && self.arm[0] && self.arm[1] {
            (yaw, sense, most) = (1.0, -1.0, 0.65);
        }
        if !(self.arm[0] && self.arm[1]) {
            sense = 0.0;
        }
        for i in 0..2 {
            if !self.arm[i] {
                continue;
            }
            let wait = if i == 0 { self.left_farther } else { !self.left_farther };
            if self.delay >= 0.0 && wait && !c.ch.busy[1 - i] {
                continue;
            }
            let l = limb(i);
            let s = self.p.body_stiffness;
            c.ch.set_stiffness(s, 1.0, if i == 0 { "ul" } else { "ur" }, None);
            c.ch.eff[l.e[2]].set_stiffness(s, 0.75, None);
            c.ch.eff[l.e[3]].set_stiffness(s - 1.0, 1.75, None);
            let off = if i == 0 { self.side[0] * -sense * 0.5 } else { self.side[1] * sense };
            let at = c.b.joint_pos(l.e[1]);
            let d = base + side * off - at;
            let pad = if touching(c, l.p[3]) { 0.0 } else { 0.05 };
            let t = at + crate::rigid::nrm(d) * (crate::nmdbm::slen(d, b"yxz") + pad).min(most);
            self.reach[i] = t;
            let vel = c.b.part(l.p[0]).v;
            let hand_side = if i == 0 { 1.0 } else { -1.0 };
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, yaw, 1.0, hand_side, 1.0, vel, None);
            crate::nmcf::clav_follow(c.ch, l.e[0], l.e[1]);
            c.ch.eff[l.e[3]].desired = [0.4, -0.4, 1.0];
            let hand = c.b.part(l.p[3]).m.d;
            if !touching(c, l.p[3]) && !touching(c, l.p[2]) {
                if crate::nmdbm::slen(hand - self.target, b"yzx") < 0.5 {
                    self.normal[0] = crate::rigid::nrm(hand - self.target);
                    brace_wrist(c, l, self.normal[0], hand_side);
                }
            } else if let Some((pt, n, inst)) = contact(c, i) {
                self.normal[i] = n;
                brace_wrist(c, l, n, hand_side);
                (self.contact[i], self.reach[i]) = (pt, pt);
                self.on_target[i] = inst == self.p.instance_index;
            }
        }
        if self.arm[0] && self.delay >= 0.0 && self.left_farther {
            self.delay -= dt;
        }
        if self.delay >= 0.0 && !self.left_farther {
            self.delay -= dt;
        }
    }
}

