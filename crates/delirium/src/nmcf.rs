use crate::prelude::*;
use crate::Vd;
use crate::Vx;

use crate::collision::Collision;
use crate::nmhl::HeadLook;
use crate::nmrs::{Character, ANKLE, HIP, KNEE, NECK, SPINE};
use crate::nmshot::{limb, Ctx, Limb};
use crate::nmst::SpineTwist;
use crate::ragdoll::Obstacle;
use crate::nmdbm::{sdot, slen};
use crate::rigid::nrm;
use crate::Fma;

pub(crate) fn cast(world: &Collision, obstacles: &[Obstacle], from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3, Vec3)> {
    let mut hit = world.cast(from, dir, len).map(|(t, n)| (t, n, Vec3::ZERO));
    for ob in obstacles {
        let r = ob.rot;
        if let Some((t, n)) = ray_box(r.inverse() * (from - ob.centre), r.inverse() * dir, ob.half).filter(|h| h.0 <= len && hit.is_none_or(|b| h.0 < b.0)) {
            hit = Some((t, r * n, ob.vel_at(from + dir * t)));
        }
    }
    hit
}

fn ray_box(o: Vec3, d: Vec3, half: Vec3) -> Option<(f32, Vec3)> {
    let (mut near, mut far, mut n) = (0.0f32, f32::MAX, Vec3::ZERO);
    for k in 0..3 {
        if d[k].abs() < 1e-8 {
            if o[k].abs() > half[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((-half[k] - o[k]) / d[k], (half[k] - o[k]) / d[k]);
        let (t0, t1) = (a.min(b), a.max(b));
        if t0 > near {
            near = t0;
            n = Vec3::ZERO;
            n[k] = -d[k].signum();
        }
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    (near > 0.0).then_some((near, n))
}

const SPINE3: usize = 10;
const NECK_PART: usize = 11;
const HEAD: usize = 12;
pub(crate) const REACH: f32 = 0.6;
pub(crate) const PROBE: f32 = f32::from_bits(0x3f33_3334);
pub(crate) const FALL_G: f32 = 1.0;
pub(crate) const SOFTEN: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub legs_stiffness: f32,
    pub torso_stiffness: f32,
    pub arms_stiffness: f32,
    pub forward_max_arm_offset: f32,
    pub backwards_min_arm_offset: f32,
    pub z_axis_spin_reduction: f32,
    pub use_head_look: bool,
}

impl Default for Params {
    fn default() -> Params {
        Params { legs_stiffness: 6.0, torso_stiffness: 9.0, arms_stiffness: 15.0, forward_max_arm_offset: 0.35,
                 backwards_min_arm_offset: -0.25, z_axis_spin_reduction: 0.0, use_head_look: true }
    }
}

#[derive(Clone, Copy, Debug)]
struct Arm {
    vel: Vec3,
    k: f32,
    clav_lean1: f32,
    clav_lean2: f32,
    elbow: f32,
    left: bool,
}

#[derive(Clone, Debug)]
pub struct CatchFall {
    pub p: Params,
    dir: Vec3,
    forward: f32,
    k: f32,
    upright: f32,
    leg: [f32; 2],
    lean2: f32,
    arm: [Arm; 2],
    touched: bool,
    pub active: bool,
}

impl CatchFall {
    pub fn start(p: Params, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist) -> CatchFall {
        let ch = &mut *c.ch;
        for (e, j) in ch.eff.iter_mut().zip(&ch.joints) {
            e.reset(j);
            e.desired = [0.0; 3];
        }
        let (k, h) = (p.torso_stiffness, 0.5);
        ch.set_stiffness(k, h, "us", None);
        if p.use_head_look {
            ch.set_stiffness(k, h, "un", None);
        }
        ch.set_stiffness(p.arms_stiffness - 1.0, 1.0, "ua", None);
        ch.set_stiffness(p.legs_stiffness, 0.5, "lb", None);
        for i in 0..2 {
            ch.eff[limb(i).e[3]].strength = 10.0;
        }
        for e in ch.eff.iter_mut() {
            e.oppose_gravity = 1.0;
        }
        let leg = [0.5 + 0.5 * ch.random(), 0.5 + 0.5 * ch.random()];
        let lean2 = ch.random().fma(0.2, -0.1);
        for i in 0..2 {
            ch.eff[ANKLE[i]].desired[0] = -leg[i];
            ch.eff[HIP[i]].desired[0] = leg[i];
            ch.eff[KNEE[i]].desired[0] = leg[i] * -2.0;
        }
        let arm = [Arm::init(ch, true), Arm::init(ch, false)];
        *st = SpineTwist::default();
        (st.stiffness, st.arms) = (p.torso_stiffness, true);
        st.activate(ch);
        if p.use_head_look {
            *head = HeadLook { stiffness: p.torso_stiffness, ..HeadLook::default() };
            head.activate(ch, c.b);
        }
        CatchFall { p, dir: Vec3::ZERO, forward: 0.0, k: 1.0, upright: 0.0, leg, lean2, arm, touched: false, active: true }
    }

    pub fn deactivate(&mut self, head: &mut HeadLook, st: &mut SpineTwist) {
        if self.active {
            self.active = false;
            if self.p.use_head_look {
                head.deactivate();
            }
            st.deactivate();
        }
    }

    pub fn tick(&mut self, c: &mut Ctx, world: &Collision, obstacles: &[Obstacle], head: &mut HeadLook, st: &mut SpineTwist, dt: f32) {
        if !self.active {
            return;
        }
        let p = self.p;
        if p.z_axis_spin_reduction > 0.0 {
            spin_reduction(c, p.z_axis_spin_reduction);
        }
        let (fl, fr) = (self.arm[0].k.max(0.3), self.arm[1].k.max(0.3));
        let ke = c.b.doll.kinetic();
        self.k = (((2.0 * ke).sqrt().fma(2.0, -self.k) * dt).fma(5.0, self.k)).max(0.2).min(1.0);
        let k = self.k;
        let weak = fl.min(fr);
        if weak == 0.3 {
            c.out.send(16, "catchFall", 0.0);
        }
        if weak < 0.9 && k < 1.0 {
            c.out.send(12, "catchFall", 0.0);
        }
        let ch = &mut *c.ch;
        ch.set_stiffness(p.torso_stiffness * k, 0.5, "us", None);
        if p.use_head_look {
            ch.set_stiffness(p.torso_stiffness * k, 0.5, "un", None);
        }
        ch.set_stiffness(fl * p.arms_stiffness, 1.0, "ul", None);
        ch.set_stiffness(fr * p.arms_stiffness, 1.0, "ur", None);
        ch.set_stiffness(p.legs_stiffness * k, 0.5, "lb", None);
        for (i, f) in [fl, fr].into_iter().enumerate() {
            ch.eff[limb(i).e[3]].set_stiffness((p.arms_stiffness + 13.0) * f * 0.5, 1.0, None);
            ch.eff[ANKLE[i]].set_stiffness(10.0, 1.0, None);
        }
        let under = (self.arm[0].vel + self.arm[1].vel) * 0.5;
        let s3 = c.b.part(SPINE3);
        let (d, up) = (s3.v - under, c.up);
        let du = sdot(up, d, b"yzx");
        self.dir = nrm(up * ((-FALL_G).fma(4.9, du) - du) + d);
        let m = c.m;
        self.forward = -sdot(m.c, self.dir, b"yzx");
        let back = -sdot(m.c, c.vel - under, b"yzx");
        self.upright = sdot(m.b, up, b"zyx").max(0.0).min(1.0);
        let on_back = sdot(m.c, up, b"zyx").max(0.0).min(1.0);
        let ch = &mut *c.ch;
        for i in 0..2 {
            ch.eff[ANKLE[i]].desired[0] = -self.leg[i];
        }
        let curl = (back * -2.0).max(0.0).min(1.0).fma(self.upright, -(self.forward * 0.5)).max(0.0).min(1.0);
        for j in SPINE {
            ch.eff[j].desired[0] = curl * k * 0.2;
            ch.eff[j].desired[1] = self.lean2;
        }
        if !st.twisting {
            let decay = 1.0 / dt.fma(12.0, 1.0);
            for j in SPINE {
                ch.eff[j].desired[2] *= decay;
            }
        }
        for i in 0..2 {
            ch.eff[HIP[i]].blend_to_zero(1.0);
        }
        for i in 0..2 {
            ch.eff[HIP[i]].desired[0] = (self.leg[i] + curl) * k;
        }
        let bend = (on_back + 1.0).fma(k, -on_back);
        for i in 0..2 {
            ch.eff[KNEE[i]].desired[0] = self.leg[i] * bend * -2.0;
        }
        let look = c.b.part(HEAD).m.d + self.dir;
        if p.use_head_look {
            (head.pos, head.vel) = (look, s3.v);
            head.stiffness = (p.torso_stiffness + 2.0) * if fl >= fr { fl } else { fr };
        }
        st.pos = look;
        for i in 0..2 {
            let mut a = self.arm[i];
            a.tick(self, c, world, obstacles, st.twist, i, dt);
            self.arm[i] = a;
        }
        let arms: Vec<usize> = (0..2).flat_map(|i| limb(i).p).collect();
        if c.b.doll.collided(&arms) {
            self.touched = true;
        }
        if self.touched {
            let n = c.b.pmat(NECK_PART).m;
            let v = -self.dir;
            for j in NECK {
                c.ch.eff[j].desired[0] = -sdot(n.c, v, b"xyz");
                c.ch.eff[j].desired[1] = -sdot(n.a, v, b"xyz");
            }
        }
    }
}

pub(crate) fn spin_reduction(c: &mut Ctx, k: f32) {
    let t = spin_torque(c.up, c.angvel, c.b.doll.mass(), k);
    for k in [0, 7, 8, 9, 10] {
        c.out.torque(k, t);
    }
}

fn spin_torque(up: Vec3, w: Vec3, mass: f32, k: f32) -> Vec3 {
    let along = up.x.mul_add(w.x, w.y.mul_add(up.y, w.z * up.z));
    crate::nmdbm::vnorm(up) * (mass * k.clamp(0.0, 1.0) * along * -0.2)
}

impl Arm {
    fn init(ch: &mut Character, left: bool) -> Arm {
        let mut r = || (ch.random_bits() & 0x7F_FFFF) as f32;
        let clav_lean1 = r().fma(f32::from_bits(0x334c_cccd), -0.4);
        let clav_lean2 = r() * f32::from_bits(0x3380_0000);
        let elbow = r().fma(f32::from_bits(0x3399_9999), 0.7);
        Arm { vel: Vec3::ZERO, k: 1.0, clav_lean1, clav_lean2, elbow, left }
    }

    fn tick(&mut self, cf: &CatchFall, c: &mut Ctx, world: &Collision, obstacles: &[Obstacle], twist: f32, i: usize, dt: f32) {
        let l: Limb = limb(i);
        let m = c.m;
        let mut s = m.b.vx(c.up);
        if sdot(s, s, b"zxy") > 1e-10 {
            s = nrm(s);
        }
        let out = if cf.forward > 0.0 {
            nrm(m.b + m.b.vx(s)) * cf.upright.max(0.0).min(1.0).fma(0.6, 0.4)
        } else {
            m.b
        };
        let p = cf.p;
        let from = out.mul_add(Vec3::splat(cf.forward.max(p.backwards_min_arm_offset).min(p.forward_max_arm_offset)), c.b.joint_pos(l.e[1]));
        let to = cf.dir.mul_add(Vec3::splat(PROBE), from);
        let hand = c.b.part(l.p[3]).m.d;
        let dir = (to - from).normalize_or_zero();
        #[allow(unused_mut)]
        let mut hit = cast(world, obstacles, from, dir, PROBE).map(|(t, n, v)| (from + dir * t, n, v));
        let (at, normal) = match hit {
            Some((a, n, _)) => (a, n),
            None => (to, c.up),
        };
        self.vel = hit.map_or(Vec3::ZERO, |h| h.2);
        let touching = c.b.doll.collided(&l.p);
        self.k = if touching { cf.k.max((-SOFTEN).fma(dt, self.k)) } else { dt.fma(2.0, self.k).min((3.0 * cf.k).min(1.0)) };
        let shoulder = c.b.joint_pos(l.e[1]);
        let aim = if touching { (at + hand) * 0.5 } else { at };
        let mut d = aim - shoulder;
        let len = slen(d, b"zyx");
        if len > REACH {
            d *= REACH / len;
        } else if len < 0.4 {
            d *= 0.4 / (len + 1e-4);
        }
        let target = shoulder + d;
        let side = if self.left { 1.0 } else { -1.0 };
        let yaw = 1.0 - cf.forward;
        let (t, lead, vel) = match (touching, hit.is_some()) {
            (true, true) => (target, 0.0, Vec3::ZERO),
            (true, false) => (hand, 0.0, Vec3::ZERO),
            (false, _) => (target, 0.5, c.b.part(l.p[0]).v),
        };
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], t, yaw, lead, side, 1.0, vel, Some((200.0, 0.4)));
        let ch = &mut *c.ch;
        if cf.forward < -0.3 {
            let e = &mut ch.eff[l.e[2]].desired[0];
            *e = e.min(self.elbow);
        }
        let clav = l.e[0];
        ch.eff[clav].desired[2] = 0.0;
        if cf.forward >= 0.0 && touching {
            ch.eff[clav].desired[1] = 0.0;
            ch.eff[SPINE[3]].desired[0] = -1.0;
            ch.eff[SPINE[2]].desired[0] = -1.0;
            ch.eff[clav].desired[2] = ch.actual(clav, c.b.frames)[2];
        } else {
            let saved = ch.eff[clav].desired[0];
            clav_follow(ch, clav, l.e[1]);
            if twist.abs() < 2.0 {
                ch.eff[clav].desired[0] = saved;
            } else {
                ch.eff[clav].desired = [self.clav_lean1, self.clav_lean2, 0.3];
            }
        }
        if hit.is_some() {
            crate::nmshot::Shot::wrist(c, l, target, normal, side);
        } else {
            c.ch.eff[l.e[3]].desired = [0.3, -0.5, 1.0];
        }
    }
}

pub(crate) fn clav_follow(ch: &mut Character, clav: usize, shoulder: usize) {
    let (cl, sl) = (ch.limits(clav), ch.limits(shoulder));
    let [l1, l2, _] = ch.eff[shoulder].desired;
    ch.eff[clav].desired[0] = l1 * if l1 >= 0.0 { cl[1] / sl[1] } else { cl[0] / sl[0] };
    ch.eff[clav].desired[1] = l2 * if l2 >= 0.0 { cl[3] / sl[3] } else { cl[2] / sl[2] };
}

