use bevy_math::{Quat, Vec3};
use crate::ragdoll::Obstacle;
use crate::Vd;
use crate::Vx;

use crate::nmdbm::Dbm;
use crate::nmhl::HeadLook;
use crate::nmrs::Character;
use crate::nmshot::{limb, Ctx, Limb};
use crate::nmst::SpineTwist;
use crate::Fma;

const SPINE3: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct GrabParams {
    pub justbrace: bool,
    pub linegrab: bool,
    pub surface_grab: bool,
    pub use_right: bool,
    pub use_left: bool,
    pub dont_let_go: bool,
    pub pos: [Vec3; 4],
    pub normal_r: Vec3,
    pub normal_l: Vec3,
    pub grab_strength: f32,
    pub arm_stiffness: f32,
    pub grab_dist: f32,
    pub reach_angle: f32,
    pub one_side_reach_angle: f32,
    pub body_stiffness: f32,
    pub pull_up_time: f32,
    pub pull_up_strength: [f32; 2],
    pub grab_hold_max_timer: f32,
    pub max_reach: f32,
    pub hold_stiffness: f32,
    pub instance: i32,
    pub part: i32,
}

impl Default for GrabParams {
    fn default() -> GrabParams {
        GrabParams { justbrace: false, linegrab: false, surface_grab: false, use_right: false, use_left: false,
                     dont_let_go: false, pos: [Vec3::ZERO; 4], normal_r: Vec3::ZERO, normal_l: Vec3::ZERO, grab_strength: -1.0,
                     arm_stiffness: 14.0, grab_dist: 1.0, reach_angle: 2.8, one_side_reach_angle: 1.4, body_stiffness: 11.0,
                     pull_up_time: 1.0, pull_up_strength: [0.0; 2], grab_hold_max_timer: 100.0, max_reach: 0.7,
                     hold_stiffness: 4.0, instance: -1, part: 0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Grip {
    pub broken: bool,
    pub local: Vec3,
    pub to: Vec3,
    pub len: f32,
    on: Vec3,
}

#[derive(Clone, Debug)]
pub struct Grab {
    pub p: GrabParams,
    pub active: bool,
    pub grip: [Option<Grip>; 2],
    target: [Vec3; 2],
    normal: [Vec3; 2],
    held: [f32; 2],
    len: [f32; 2],
    told: [bool; 2],
    grabbing: bool,
    look_ahead: bool,
    frame: (Vec3, Quat),
    pub contact_off: u32,
}

impl Default for Grab {
    fn default() -> Grab {
        Grab { p: GrabParams::default(), active: false, grip: [None; 2], target: [Vec3::ZERO; 2], normal: [Vec3::ZERO; 2],
               held: [0.0; 2], len: [0.0; 2], told: [false; 2], grabbing: false, look_ahead: true, frame: (Vec3::ZERO, Quat::IDENTITY), contact_off: 0 }
    }
}

fn on_line(a: Vec3, b: Vec3, x: Vec3) -> Vec3 {
    let d = a - b;
    let n = crate::rigid::nrm(d);
    let len = crate::nmdbm::slen(d, b"yzx");
    let t = crate::nmdbm::sdot(x - b, n, b"yzx");
    let t = if t >= 0.0 { t } else { 0.0 };
    let t = if t - len >= 0.0 { len } else { t };
    n.mul_add(Vec3::splat(t), b)
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    let k = |p: f32, q: f32, r: f32, s: f32| -p.mul_add(q, -(r * s));
    Vec3::new(k(a.z, b.y, a.y, b.z), k(a.x, b.z, a.z, b.x), k(a.y, b.x, a.x, b.y))
}

fn on_quad(q: &[Vec3; 4], justbrace: bool, x: Vec3, edge: f32) -> (Vec3, Vec3) {
    use crate::nmdbm::{sdot, slen};
    use crate::rigid::nrm;
    let e = [on_line(q[0], q[1], x), on_line(q[1], q[2], x), on_line(q[2], q[3], x), on_line(q[3], q[0], x)];
    let n = cross(nrm(q[0] - q[1]), nrm(q[2] - q[1])) * -1.0;
    let rel = x - q[0];
    let h = sdot(n, rel, b"zxy");
    let p = (rel - n * h) + q[0];
    let d = [slen(p - e[0], b"xzy"), slen(p - e[1], b"yzx"), slen(p - e[2], b"yzx"), slen(p - e[3], b"yzx")];
    let (mut best, mut at, mut ends) = (d[0], e[0], (q[0], q[1]));
    for (k, ab) in [(1, (q[1], q[2])), (2, (q[2], q[3])), (3, (q[3], q[0]))] {
        if d[k] < best {
            (best, at, ends) = (d[k], e[k], ab);
        }
    }
    let u = nrm(ends.1 - ends.0);
    let w = nrm(at - p);
    let out = sdot(cross(w, u), n, b"zxy") < 0.0;
    let p = if out || (!justbrace && best < edge) { at } else { p };
    (p, if h < 0.0 { n * -1.0 } else { n })
}

fn apart(this: &mut Vec3, other: Vec3, a: Vec3, b: Vec3) {
    let u = crate::rigid::nrm(other - *this);
    let end = if crate::nmdbm::sdot(crate::rigid::nrm(a - b), u, b"zyx") > 0.0 { b } else { a };
    let len = crate::nmdbm::slen(other - end, b"yzx");
    *this -= u * if len - 0.15 >= 0.0 { 0.15 } else { len };
}

impl Grab {
    pub fn place(&mut self, obstacles: &[Obstacle]) {
        self.frame = Obstacle::find(obstacles, self.p.instance).map_or((Vec3::ZERO, Quat::IDENTITY), |o| match self.p.part {
            k @ 1.. => { let (at, m) = o.component(k as usize); (at, Quat::from_mat3(&m)) }
            _ => (o.at, o.rot),
        });
        let f = self.frame;
        for g in self.grip.iter_mut().flatten() {
            g.to = f.0 + f.1 * g.on;
        }
    }

    fn to_world(&self, x: Vec3) -> Vec3 {
        self.frame.0 + self.frame.1 * x
    }

    pub(crate) fn to_local(&self, x: Vec3) -> Vec3 {
        self.frame.1.inverse() * (x - self.frame.0)
    }

    fn dir_world(&self, n: Vec3) -> Vec3 {
        self.frame.1 * n
    }

    pub fn grips(&self) -> Vec<(usize, Vec3, Vec3, f32, f32)> {
        (0..2).filter_map(|i| self.grip[i].filter(|g| !g.broken).map(|g| (limb(i).p[3], g.local, g.to, g.len, self.p.grab_strength))).collect()
    }

    pub fn snapped(&mut self, parts: &[usize]) {
        for i in 0..2 {
            if let Some(g) = self.grip[i].as_mut().filter(|_| parts.contains(&limb(i).p[3])) {
                g.broken = true;
            }
        }
    }

    pub fn activate(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist, dbm: Option<&mut Dbm>) {
        if self.active {
            return;
        }
        (self.active, self.grip, self.look_ahead) = (true, [None; 2], true);
        for (i, on) in [(0, self.p.use_left), (1, self.p.use_right)] {
            if on && !self.p.justbrace {
                self.contact_off |= 1 << limb(i).p[3];
            }
        }
        self.tick(c, head, st, dbm, 0.0);
    }

    pub fn deactivate(&mut self, ch: &mut Character, head: &mut HeadLook, st: &mut SpineTwist, dbm: Option<&mut Dbm>) {
        if !self.active {
            return;
        }
        head.deactivate();
        st.deactivate();
        if let Some(d) = dbm {
            (d.pv.fixed_vel, d.pv.vel) = (false, self.target[1]);
        }
        *self = Grab::default();
        ch.set_stiffness(10.0, 1.0, "fb", Some(1.0));
    }

    pub fn tick(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist, dbm: Option<&mut Dbm>, dt: f32) {
        if !self.active {
            return;
        }
        let p = self.p;
        let (mut left, mut right) = (p.use_left, p.use_right);
        if left || right {
            self.decide(c, &mut left, &mut right);
        } else {
            self.grabbing = false;
        }
        if self.grabbing && (left || right) {
            for (i, on, a, b, n) in [(1, right, p.pos[0], p.pos[1], p.normal_r), (0, left, p.pos[1], p.pos[0], p.normal_l)] {
                if on && self.grip[i].is_none() {
                    let l = limb(i);
                    (self.target[i], self.normal[i]) = self.aim(c, a, b, n, self.to_local(c.b.part(l.p[3]).m.d), c.b.joint_pos(l.e[1]));
                }
            }
            if p.linegrab && (self.target[0] - self.target[1]).length() < 0.15 {
                let (l, r) = (self.target[0], self.target[1]);
                if left && self.grip[0].is_none() {
                    apart(&mut self.target[0], r, p.pos[0], p.pos[1]);
                } else if right && self.grip[1].is_none() {
                    apart(&mut self.target[1], l, p.pos[1], p.pos[0]);
                }
            }
            let w = [0, 1].map(|i| (self.to_world(self.target[i]), self.dir_world(self.normal[i])));
            for (i, on) in [(0, left), (1, right)] {
                if on && !p.justbrace {
                    self.contact_off |= 1 << limb(i).p[3];
                }
                if on {
                    self.arm(c, i, w[i], ["ul", "ur"][i], [-1.0, 1.0][i], dt);
                }
            }
            let mid = match (left, right) {
                (true, true) => (w[0].0 + w[1].0) * 0.5,
                (true, false) => w[0].0,
                _ => w[1].0,
            };
            let t = self.held[0].max(self.held[1]);
            if t > 1.6 {
                self.look_off(c, head);
            } else if t < 0.05 || head.active {
                if !head.active {
                    head.activate(c.ch, c.b);
                }
                (head.pos, head.local, head.always_look, head.stiffness, head.damping) = (mid, None, true, p.body_stiffness, 1.2);
                head.vel = Vec3::ZERO;
            }
            if !st.active {
                st.activate(c.ch);
            }
            (st.pos, st.stiffness, st.always, st.arms) = (mid, p.body_stiffness, true, false);
            if if p.justbrace { left } else { left && right } {
                if let Some(d) = dbm {
                    (d.pv.fixed_vel, d.pv.vel) = (true, (mid - c.com).normalize_or_zero());
                }
            }
        } else {
            self.look_off(c, head);
            st.deactivate();
            c.out.send(20, "GrabNotGrabbing", 0.0);
            if let Some(d) = dbm {
                (d.pv.fixed_vel, d.pv.vel) = (false, self.target[1]);
            }
        }
        for (i, on) in [(1, right), (0, left)] {
            if !on && self.grip[i].take().is_some() && p.grab_hold_max_timer > 0.0 {
                self.held[i] = p.grab_hold_max_timer + 0.2;
            }
            if !on {
                self.contact_off &= !(1 << limb(i).p[3]);
            }
        }
        if !p.use_left {
            self.held[0] = 0.0;
        }
        if !p.use_right {
            self.held[1] = 0.0;
        }
    }

    fn look_off(&mut self, c: &Ctx, head: &mut HeadLook) {
        if self.look_ahead {
            self.look_ahead = false;
            let m = c.b.pmat(SPINE3).m;
            head.local = Some(crate::nmbb::to_own((m.b * 0.4 - m.c) + c.b.at(SPINE3), c.b));
        }
    }

    fn decide(&mut self, c: &Ctx, left: &mut bool, right: &mut bool) {
        let p = self.p;
        let s3 = c.b.part(SPINE3).m;
        let at = self.to_local(s3.d);
        let dist = if p.justbrace { p.grab_dist } else { p.grab_dist * 1.75 };
        let g = if p.linegrab {
            on_line(p.pos[0], p.pos[1], at)
        } else if p.surface_grab {
            on_quad(&p.pos, p.justbrace, at, 0.0).0
        } else {
            match (p.use_left, p.use_right) {
                (true, true) => (p.pos[0] + p.pos[1]) * 0.5,
                (true, false) => p.pos[1],
                (false, true) => p.pos[0],
                _ => at,
            }
        };
        let (g, at) = (self.to_world(g), s3.d);
        let k = if p.justbrace { 0.94 } else { 0.45 };
        let d = g - at;
        let len = d.length();
        let closing = (c.vel.vd(d.normalize_or_zero()) * k).max(0.0).min(len);
        let left_over = len - closing;
        self.grabbing = left_over < if self.grabbing { dist + 0.2 } else { dist };
        let v = g - s3.d;
        let local = Vec3::new(s3.a.vd(v), s3.b.vd(v), s3.c.vd(v));
        let ang = local.y.atan2(-local.z);
        if ang < p.reach_angle && ang > -p.reach_angle {
            *right = if ang > p.reach_angle - p.one_side_reach_angle { false } else { p.use_right };
            *left = if ang < p.one_side_reach_angle - p.reach_angle { false } else { p.use_left };
        } else {
            (self.grabbing, *left, *right) = (false, false, false);
        }
        if p.dont_let_go && self.grip.iter().any(Option::is_some) {
            (self.grabbing, *right, *left) = (true, p.use_right, p.use_left);
        }
        if p.grab_hold_max_timer > 0.0 {
            if self.held[1] > p.grab_hold_max_timer {
                *right = false;
            }
            if self.held[0] > p.grab_hold_max_timer {
                *left = false;
            }
        }
        *left &= !c.ch.busy[0];
        *right &= !c.ch.busy[1];
    }

    fn aim(&self, _c: &Ctx, a: Vec3, b: Vec3, normal: Vec3, hand: Vec3, shoulder: Vec3) -> (Vec3, Vec3) {
        let p = self.p;
        let none = normal == Vec3::ZERO;
        let shoulder = self.to_local(shoulder);
        let (mut t, mut n) = (a, crate::rigid::nrm(if none { shoulder - a } else { normal }));
        if p.linegrab {
            t = on_line(a, b, hand);
            if none {
                let d = crate::rigid::nrm(a - b);
                n = -crate::rigid::nrm(shoulder - t).vx(d).vx(d);
            }
        }
        if p.surface_grab {
            (t, n) = on_quad(&p.pos, p.justbrace, hand, 0.8);
        }
        (t, n)
    }

    fn arm(&mut self, c: &mut Ctx, i: usize, (w, n): (Vec3, Vec3), mask: &str, side: f32, dt: f32) {
        let p = self.p;
        let l = limb(i);
        let pull = p.pull_up_strength[i] * 3.0;
        let speed = crate::nmdbm::slen(c.vel, b"yxz");
        let dir = crate::rigid::nrm(c.vel);
        let at = c.b.joint_pos(l.e[1]);
        let k = (speed.clamp(5.0, 20.0) - 5.0) * (1.0 / 15.0);
        let wc = w - c.com;
        let mut t = c.com + dir.mul_add(Vec3::splat(-((crate::nmdbm::sdot(dir, wc, b"yxz") * k) * -0.5)), wc);
        let all: Vec<usize> = (0..c.b.build.parts.len()).collect();
        let touching = c.b.doll.collided(&all);
        if !touching {
            t = c.up.mul_add(Vec3::splat(k), t);
        }
        let a = speed.clamp(0.0, 1.0);
        let reach = crate::nmdbm::slen(t - at, b"xzy").fma(1.0 - (1.0 - a), p.max_reach.clamp(0.1, 3.0) * (1.0 - a));
        let t = crate::rigid::nrm(t - at).mul_add(Vec3::splat(reach), at);
        let yaw = reach.clamp(0.0, 0.71);
        let ik_side = if i == 0 { 1.0 } else { -1.0 };
        if self.grip[i].is_none() {
            c.ch.set_stiffness(p.arm_stiffness, 1.0, mask, Some(1.0));
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, yaw, 0.75, ik_side, 1.0, Vec3::ZERO, None);
            crate::nmpa::share(c.ch, c.b.frames, l.e[0], l.e[1]);
            let hand = c.b.at(l.p[3]);
            if crate::nmdbm::sdot(n, crate::rigid::nrm(hand - at), b"zyx") > 0.85 {
                c.ch.eff[l.e[3]].desired[2] = 0.0;
                c.ch.eff[l.e[3]].desired[0] = 0.0;
            } else {
                crate::nmshot::Shot::wrist(c, l, t, n, ik_side);
            }
            let strength = if p.justbrace { 0.0 } else { 4.0 };
            c.ch.set_relative(l.e[3], 1, -1.0);
            self.pull(c, i, l, w, side, strength, !p.justbrace, dt);
            return;
        }
        self.held[i] += dt;
        let mut arm_k = p.hold_stiffness;
        if touching {
            arm_k *= 0.025;
        }
        let top = p.pull_up_strength[i] * 9.0;
        let s = (top / (p.pull_up_time + 0.01) * self.held[i] + 5.0).max(5.0).min(top);
        if !self.told[i] {
            self.told[i] = true;
            c.out.send(12, ["LeftArmGrab", "RightArmGrab"][i], 0.0);
        }
        c.ch.set_stiffness(s, 0.5, mask, Some(7.0));
        let wr = &mut c.ch.eff[l.e[3]];
        (wr.muscle_stiffness, wr.strength, wr.damping) = (20.0, 144.0, 1.2);
        self.pull(c, i, l, w, side, arm_k, !p.justbrace, dt);
        for j in [l.e[3], l.e[1]] {
            c.ch.eff[j].desired = c.ch.actual(j, c.b.frames);
        }
        self.turn_hand(c, l, n, s, dt);
        if pull > 0.0 {
            c.ch.eff[l.e[2]].desired[0] = pull;
            c.ch.eff[l.e[1]].desired[0] = (pull * 0.7).clamp(0.0, 2.0);
            c.ch.eff[l.e[0]].desired[0] = pull * 0.2;
        }
        let legs = |k: usize| c.b.doll.collided(&limb(k).p[1..]);
        let trunk = if !legs(2) && !legs(3) {
            c.ch.set_stiffness(4.0, 1.0, "us", Some(1.0));
            7.0
        } else {
            c.ch.set_stiffness(10.0, 1.0, "us", Some(1.0));
            10.0
        };
        c.ch.set_stiffness(trunk, 1.0, "uc", Some(1.0));
    }

    #[allow(clippy::too_many_arguments)]
    fn pull(&mut self, c: &mut Ctx, i: usize, l: Limb, w: Vec3, side: f32, strength: f32, can_grip: bool, dt: f32) {
        const DIST: f32 = 0.2;
        let hand = c.b.part(l.p[3]);
        let at = hand.m.d - hand.m.a * (side * 0.02);
        let d = (w - at).length();
        if d < 3.0 * DIST && self.grip[i].is_none() {
            let k = ((3.0 * DIST - d) / (2.0 * DIST)).clamp(0.0, 1.0) * strength;
            let mut imp = (w - at) * (k * k) - hand.v * (k * 0.2);
            if dt < 1.0 / 60.0 {
                imp *= dt * 60.0;
            }
            c.out.impulse.push((l.p[3], c.b.joint_pos(l.e[3]), imp));
        }
        if can_grip && d < DIST && self.grip[i].is_none() {
            self.len[i] = d;
            let local = crate::rigid::grip_local([Vec3::X, Vec3::Y, Vec3::Z, Vec3::ZERO], Some([hand.m.a, hand.m.b, hand.m.c, hand.m.d]), at);
            self.grip[i] = Some(Grip { broken: false, local, to: w, len: d, on: self.to_local(w) });
        } else if let Some(g) = self.grip[i].as_mut() {
            self.len[i] = (self.len[i] - dt * 0.6).max(0.0).min(d);
            g.len = self.len[i];
        }
    }

    fn turn_hand(&self, c: &mut Ctx, l: Limb, n: Vec3, k: f32, dt: f32) {
        let a = c.b.part(l.p[3]).m.a * k;
        let ang = crate::nmdbm::acos(n.vd(a));
        if let Some(axis) = unit_axis(a.vx(n)).filter(|_| dt > 0.0) {
            c.out.torque(l.p[3], axis * (ang / (dt * 60.0)));
        }
    }
}

fn unit_axis(v: Vec3) -> Option<Vec3> {
    (v.vd(v) > 1e-6).then(|| crate::rigid::nrm(v))
}

