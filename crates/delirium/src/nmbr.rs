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
#[allow(non_snake_case)]
#[inline]
fn SPINE2() -> usize { delirium_data::kv!(93 int) }
#[allow(non_snake_case)]
#[inline]
fn SPINE3() -> usize { delirium_data::kv!(77 int) }

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
        BraceParams { body_stiffness: delirium_data::kv!(21), pos: Vec3::new(0.0, 1.0, -delirium_data::kv!(12)), instance_index: -1, brace_distance: delirium_data::kv!(8),
                      target_prediction_time: delirium_data::kv!(24), reach_absorbtion_time: delirium_data::kv!(224), look: Vec3::ZERO, grab_strength: delirium_data::kv!(11),
                      grab_distance: 2.0, grab_reach_angle: delirium_data::kv!(12), grab_hold_timer: delirium_data::kv!(30), max_grab_speed: delirium_data::kv!(225), behind_time: 1.0,
                      grab_dont_let_go: false, leg_stiffness: delirium_data::kv!(21) }
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
    c.b.doll.touch[k] & delirium_data::kv!(91 int) == delirium_data::kv!(7 int)
}

fn contact(c: &Ctx, i: usize) -> Option<(Vec3, Vec3, i32)> {
    let l = limb(i);
    let k = if touching(c, l.p[delirium_data::kv!(0 int)]) { l.p[delirium_data::kv!(0 int)] } else { l.p[2] };
    c.b.doll.hit[k]
}

fn brace_wrist(c: &mut Ctx, l: Limb, n: Vec3, side: f32) {
    use crate::nmdbm::sdot;
    let mut w = c.b.eff3(l.e[delirium_data::kv!(0 int)]);
    let (pa, pb, pc) = (w.m.a, w.m.c.vx(w.m.a), w.m.c);
    let b2 = pb * side;
    let (na, nb, npc) = (sdot(n, pa, b"yzx"), -sdot(n, b2, b"yxz"), sdot(n, pc, b"yzx"));
    let len = nb.fma(nb, na * na).sqrt() + delirium_data::kv!(47);
    let inv = 1.0 / len;
    let tw = ((inv * na) as f64).atan2((inv * nb) as f64) as f32;
    let tw = tw.clamp(-len, len);
    let (s, co) = ((tw as f64).sin() as f32, (tw as f64).cos() as f32);
    let pp = b2.mul_add(Vec3::splat(-co), pa.mul_add(Vec3::splat(s), Vec3::ZERO));
    let phi = (npc as f64).atan2(sdot(n, pp, b"zyx") as f64) as f32;
    w.desired = [-(phi * s), phi * co, tw];
    c.ch.eff[l.e[delirium_data::kv!(0 int)]].desired = w.desired;
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
        (self.dist, self.legs) = (delirium_data::kv!(28), p.leg_stiffness);
        out.leg_stiffness = Some(p.leg_stiffness);
        k.head.activate(c.ch, c.b);
        k.head.always_look = true;
        self.pick = true;
        self.draw_sides(c.ch);
        self.crouch = c.ch.random().fma(delirium_data::kv!(121), delirium_data::kv!(109));
        self.split = c.ch.random() - 0.5;
        self.scale = c.ch.random().fma(delirium_data::kv!(106), delirium_data::kv!(57));
        if self.crouch < delirium_data::kv!(64) {
            self.scale = c.ch.random().fma(delirium_data::kv!(9), delirium_data::kv!(57));
        }
        out
    }

    fn draw_sides(&mut self, ch: &mut Character) {
        let (a, b) = if ch.mode == 2 { (delirium_data::kv!(224), delirium_data::kv!(226)) } else { (delirium_data::kv!(227), delirium_data::kv!(228)) };
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
        c.out.send(delirium_data::kv!(118 int), "brace", delirium_data::kv!(21));
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
            (pd.random_seed, pd.pedal_offset, pd.speed_asymmetry, pd.radius, pd.drag_multiplier) = (delirium_data::kv!(189 int), delirium_data::kv!(109), delirium_data::kv!(104), delirium_data::kv!(24), delirium_data::kv!(24));
            (pd.left, pd.right, pd.back_pedal, pd.adaptive) = (true, true, false, true);
            pd.leg_stiffness = (p.body_stiffness - 1.0).clamp(1.0, delirium_data::kv!(132));
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
                let (ll, rl) = (limb(2), limb(delirium_data::kv!(0 int)));
                let feet = touching(c, ll.p[delirium_data::kv!(0 int)]) || touching(c, rl.p[delirium_data::kv!(0 int)]);
                let rest = [ll.p[2], rl.p[2], ll.p[1], rl.p[1], PELVIS].into_iter().any(|q| touching(c, q));
                if feet && !rest {
                    roll = false;
                }
            }
            if roll {
                let r = &mut *k.rds;
                (r.asymmetrical_legs, r.forcemag, r.spin_when_in_air, r.use_arm_to_slow_down) = (delirium_data::kv!(106), delirium_data::kv!(106), true, -delirium_data::kv!(108));
                (r.asymmetry, r.arm_reach_amount, r.leg_push, r.arm_reach_length) = (0.0, delirium_data::kv!(170), delirium_data::kv!(57), delirium_data::kv!(110));
                r.stiffness = (p.body_stiffness - delirium_data::kv!(5)).clamp(1.0, delirium_data::kv!(26));
                r.asymmetrical_legs = delirium_data::kv!(57) + delirium_data::kv!(110) * c.ch.random();
                r.activate(c, k.ru);
            } else {
                let s = p.body_stiffness;
                out.catch_fall = Some(crate::nmcf::Params { legs_stiffness: s * 0.5, torso_stiffness: s * delirium_data::kv!(18), arms_stiffness: s * delirium_data::kv!(229),
                                                            forward_max_arm_offset: delirium_data::kv!(63), backwards_min_arm_offset: -delirium_data::kv!(17),
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
            if touching(c, l.p[delirium_data::kv!(0 int)]) || touching(c, l.p[2]) {
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
        if len > delirium_data::kv!(27) {
            self.tvel *= delirium_data::kv!(27) / len;
        }
        self.target = now;
        let s2 = c.b.part(SPINE2()).m.d;
        let rel = c.vel - self.tvel;
        let d = slen(self.target - s2, b"yzx");
        let dir = crate::rigid::nrm(self.target - s2);
        let close = sdot(rel, dir, b"yxz");
        let pred = (-close).fma(p.target_prediction_time, d).max(delirium_data::kv!(73));
        self.short = dir * -(close * p.reach_absorbtion_time).max(d - p.brace_distance).min(d - delirium_data::kv!(24));
        self.dist = pred.min((1.0 - 2.0 * dt).fma(self.dist, 2.0 * dt * pred));
        self.braced = self.dist < p.brace_distance + if self.braced { delirium_data::kv!(73) } else { 0.0 };
        if !self.braced {
            self.arm = [false; 2];
        } else {
            let b = flat(c.b.part(SPINE2()).m.b, up);
            for i in 0..2 {
                let a = sdot(crate::rigid::nrm(flat(self.target - c.b.joint_pos(limb(i).e[1]), up)), b, b"zyx");
                let a = crate::nmdbm::acos(a);
                let x = if i == 0 { delirium_data::kv!(230) - a } else { a - delirium_data::kv!(17) };
                if !self.arm[i] {
                    if x > delirium_data::kv!(57) && !c.ch.busy[i] {
                        self.arm[i] = true;
                    }
                } else if x < -delirium_data::kv!(57) {
                    self.arm[i] = false;
                }
            }
        }
        if self.braced && self.clock < p.behind_time && -sdot(c.m.c, flat(self.target - c.com, up), b"zyx") < 0.0 {
            self.arm = [false; 2];
            self.braced = false;
        }
        let lv = ((c.b.at(SPINE3()) - c.b.at(PELVIS)) * 0.5 + (c.b.part(SPINE3()).v - c.b.part(PELVIS).v) * delirium_data::kv!(24)) * 2.0;
        let lean = (crate::nmdbm::slen(lv, b"zyx") - delirium_data::kv!(9)).clamp(0.0, 1.0);
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
            let dl = crate::nmdbm::slen(self.target - c.b.joint_pos(limb(0).e[delirium_data::kv!(0 int)]), b"yxz");
            let dr = crate::nmdbm::slen(self.target - c.b.joint_pos(limb(1).e[delirium_data::kv!(0 int)]), b"yxz");
            self.left_farther = dl > dr;
            self.delay = (c.ch.random_bits() & delirium_data::kv!(182 int)) as f32 * delirium_data::kv!(231);
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
            self.head_t = ((c.ch.random_bits() & delirium_data::kv!(182 int)) as f32).fma(delirium_data::kv!(232), delirium_data::kv!(17));
            self.head_at = c.ch.random_bits() & 1 != 0;
        }
        let at = world(obstacles, p.pos, p.instance_index);
        let behind = crate::nmdbm::sdot(c.m.c, crate::rigid::nrm(at - c.com), b"yzx") > delirium_data::kv!(70);
        k.head.pos = if self.head_at && self.braced && (self.arm[0] || self.arm[1]) && !behind {
            world(obstacles, p.pos * delirium_data::kv!(233) + p.look * delirium_data::kv!(227), p.instance_index)
        } else {
            world(obstacles, p.look, p.instance_index)
        };
        k.head.stiffness = p.body_stiffness - 1.0;
        k.head.always_look = true;
    }

    fn guard(&self, c: &mut Ctx, i: usize, lv: Vec3, lean: f32) {
        use crate::nmdbm::sdot;
        let l = limb(i);
        c.ch.set_stiffness(lean.fma(delirium_data::kv!(5), delirium_data::kv!(29)), 1.0, if i == 0 { "ul" } else { "ur" }, None);
        let m = c.b.eff3(l.e[1]).m;
        let a = sdot(m.a, lv, b"zyx") * delirium_data::kv!(104);
        let side = if i == 0 { m.c - m.b } else { m.c + m.b } * delirium_data::kv!(218);
        let b = sdot(side, lv, b"zyx") * delirium_data::kv!(104);
        let (sin, cos) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
        let lift = (-lean).fma(0.5, cos + b.clamp(-delirium_data::kv!(61), delirium_data::kv!(61)));
        let ch = &mut *c.ch;
        let (sl, cl) = (ch.limits(l.e[1]), ch.limits(l.e[0]));
        ch.eff[l.e[1]].oppose_gravity = lean;
        ch.eff[l.e[0]].oppose_gravity = lean;
        let sh = &mut ch.eff[l.e[1]].desired;
        sh[0] = ((sl[0] + sl[1]) * 0.5 - sin) * lean;
        sh[1] = ((sl[2] + sl[delirium_data::kv!(0 int)]) * 0.5 + lift).fma(lean, (1.0 - lean) * delirium_data::kv!(57));
        sh[2] = if i == 0 { 0.0 } else { -delirium_data::kv!(73) };
        if i == 0 {
            ch.eff[l.e[0]].desired = [0.0, ((cl[2] + cl[delirium_data::kv!(0 int)]) * 0.5 + lift) * lean, 0.0];
            ch.eff[l.e[delirium_data::kv!(0 int)]].desired = [0.0; 3];
            ch.eff[l.e[2]].desired[0] = (1.0 - lean).fma(delirium_data::kv!(70), lean);
        } else {
            ch.eff[l.e[0]].desired = [0.0; 3];
            ch.eff[l.e[delirium_data::kv!(0 int)]].desired[2] = 0.0;
            ch.eff[l.e[2]].desired[0] = ((1.0 - lean) + lean) * delirium_data::kv!(20);
        }
        if ch.busy[i] {
            ch.eff[l.e[delirium_data::kv!(0 int)]].set_stiffness(self.p.body_stiffness, delirium_data::kv!(12), None);
        }
    }

    fn spine(&mut self, c: &mut Ctx, dbm: Option<&mut Dbm>, dt: f32) {
        let (l1, l2) = (self.lean * delirium_data::kv!(12), self.lean2);
        let ch = &mut *c.ch;
        let sp = (*crate::nmrs::SPINE());
        for (n, j) in sp.into_iter().enumerate() {
            let lo = n < 2;
            ch.eff[j].desired[0] = if lo { 2.0 * l1 / delirium_data::kv!(32) } else { l1 / delirium_data::kv!(32) };
            ch.eff[j].desired[1] = if lo { 0.5 * l2 } else { delirium_data::kv!(17) * l2 };
        }
        let q = self.lean * delirium_data::kv!(17);
        ch.eff[sp[0]].desired[0] = (self.split + delirium_data::kv!(12)) * q;
        ch.eff[sp[1]].desired[0] = (self.split + delirium_data::kv!(12)) * q;
        ch.eff[sp[2]].desired[0] = (delirium_data::kv!(12) - self.split) * q;
        ch.eff[sp[delirium_data::kv!(0 int)]].desired[0] = (delirium_data::kv!(12) - self.split) * q;
        if let Some(d) = dbm {
            d.acc = self.lean * -delirium_data::kv!(234);
        }
        self.lean /= dt.fma(delirium_data::kv!(235), 1.0);
    }

    fn braced_tick(&mut self, c: &mut Ctx, k: &mut Kids, dt: f32) {
        let up = c.up;
        k.twist.pos = self.target;
        k.twist.vel = self.tvel;
        use crate::nmdbm::sdot;
        let d = c.com - self.target;
        let away = crate::rigid::nrm(up * -sdot(up, d, b"zyx") + d);
        let h = sdot(up, c.b.at(SPINE3()), b"yzx") - sdot(up, self.target, b"zyx");
        self.lean = (h * self.scale).fma(sdot(c.m.c, away, b"zyx"), delirium_data::kv!(57));
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
        let s2 = c.b.part(SPINE2());
        let r = self.target + self.short - s2.m.d;
        let side = crate::rigid::nrm((self.target - s2.m.d).vx(up));
        let r2 = r + r.vx(s2.w) * delirium_data::kv!(73);
        let base = s2.m.d + crate::rigid::nrm(r2) * crate::nmdbm::slen(r, b"yxz");
        let (mut yaw, mut sense, mut most) = (0.0, 1.0, delirium_data::kv!(123));
        if !twisting && self.arm[0] && self.arm[1] {
            (yaw, sense, most) = (1.0, -1.0, delirium_data::kv!(8));
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
            c.ch.eff[l.e[2]].set_stiffness(s, delirium_data::kv!(18), None);
            c.ch.eff[l.e[delirium_data::kv!(0 int)]].set_stiffness(s - 1.0, delirium_data::kv!(13), None);
            let off = if i == 0 { self.side[0] * -sense * 0.5 } else { self.side[1] * sense };
            let at = c.b.joint_pos(l.e[1]);
            let d = base + side * off - at;
            let pad = if touching(c, l.p[delirium_data::kv!(0 int)]) { 0.0 } else { delirium_data::kv!(64) };
            let t = at + crate::rigid::nrm(d) * (crate::nmdbm::slen(d, b"yxz") + pad).min(most);
            self.reach[i] = t;
            let vel = c.b.part(l.p[0]).v;
            let hand_side = if i == 0 { 1.0 } else { -1.0 };
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], t, yaw, 1.0, hand_side, 1.0, vel, None);
            crate::nmcf::clav_follow(c.ch, l.e[0], l.e[1]);
            c.ch.eff[l.e[delirium_data::kv!(0 int)]].desired = [delirium_data::kv!(106), -delirium_data::kv!(106), 1.0];
            let hand = c.b.part(l.p[delirium_data::kv!(0 int)]).m.d;
            if !touching(c, l.p[delirium_data::kv!(0 int)]) && !touching(c, l.p[2]) {
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

