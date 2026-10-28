use bevy_math::{Mat3, Quat, Vec3};
use crate::Vd;
use crate::Vx;

use crate::collision::Collision;
use crate::ragdoll::Obstacle;
use crate::nmaw::Windmill;
use crate::nmdbm::{slen, Dbm};
use crate::nmhl::HeadLook;
use crate::nmpd::Pedal;
use crate::nmrds::RollDownStairs;
use crate::nmrs::{Character, NECK, SPINE};
use crate::nmru::RollUp;
use crate::nmshot::{limb, Ctx};

const PELVIS: usize = 0;

#[derive(Clone, Copy, Debug)]
pub struct HighFallParams {
    pub body_stiffness: f32,
    pub bodydamping: f32,
    pub catchfalltime: f32,
    pub crash_or_land_cut_off: f32,
    pub pd_strength: f32,
    pub catch_fall_cut_off: f32,
    pub arms_up: f32,
    pub leg_radius: f32,
    pub leg_ang_speed: f32,
    pub arm_period: f32,
    pub arm_amplitude: f32,
    pub aim_angle_base: f32,
    pub foward_vel_rotation: f32,
    #[allow(dead_code)]
    pub foot_vel_comp_scale: f32,
    pub leg_l: f32,
    pub side_d: f32,
    pub foward_offset_of_leg_ik: f32,
    pub foward_vel_cut_off: f32,
    pub leg_strength: f32,
    pub orientate_body_to_fall_direction: bool,
    pub foward_roll: bool,
    pub zero_pose_roll: bool,
    pub ignor_world_collisions: bool,
}

impl Default for HighFallParams {
    fn default() -> HighFallParams {
        HighFallParams { body_stiffness: delirium_data::kv!(107), bodydamping: 1.0, catchfalltime: delirium_data::kv!(24), crash_or_land_cut_off: delirium_data::kv!(240), pd_strength: delirium_data::kv!(24),
                         catch_fall_cut_off: 0.5, arms_up: 1.0, leg_radius: delirium_data::kv!(17), leg_ang_speed: delirium_data::kv!(195), arm_period: 1.0,
                         arm_amplitude: 2.0, aim_angle_base: delirium_data::kv!(131), foward_vel_rotation: -delirium_data::kv!(109), foot_vel_comp_scale: delirium_data::kv!(64),
                         leg_l: delirium_data::kv!(20), side_d: delirium_data::kv!(9), foward_offset_of_leg_ik: delirium_data::kv!(57), foward_vel_cut_off: delirium_data::kv!(60), leg_strength: delirium_data::kv!(21),
                         orientate_body_to_fall_direction: true, foward_roll: false, zero_pose_roll: false,
                         ignor_world_collisions: false }
    }
}

pub struct Kids<'a> {
    pub head: &'a mut HeadLook,
    pub pedal: &'a mut Pedal,
    pub windmill: &'a mut Windmill,
    pub ru: &'a mut RollUp,
    pub rds: &'a mut RollDownStairs,
    pub dbm: Option<&'a mut Dbm>,
    pub fallen: bool,
    pub balancing: bool,
    pub catching: bool,
}

#[derive(Default)]
pub struct HighFallOut {
    pub catch_fall: Option<crate::nmcf::Params>,
    pub stop_catch_fall: bool,
    pub balance: Option<crate::nmbb::Params>,
    pub stop_balance: bool,
    pub taper_off: bool,
    pub dbm_set: Option<(f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct HighFall {
    pub p: HighFallParams,
    t: f32,
    land: f32,
    fwd: f32,
    timer: f32,
    speed: f32,
    s: u8,
    f: u8,
}

impl Default for HighFall {
    fn default() -> HighFall {
        HighFall { p: HighFallParams::default(), t: 0.0, land: delirium_data::kv!(76), fwd: 0.0, timer: 0.0, speed: 0.0, s: delirium_data::kv!(189 int), f: 0 }
    }
}

fn flat(v: Vec3, up: Vec3) -> Vec3 {
    v - up * up.vd(v)
}

impl HighFall {
    pub fn active(&self) -> bool {
        self.s & delirium_data::kv!(249 int) != 0
    }

    pub fn activate(&mut self, c: &mut Ctx, world: &Collision, obstacles: &[Obstacle], dt: f32) {
        if self.active() {
            return;
        }
        self.s |= delirium_data::kv!(249 int);
        self.orient(c, world, obstacles, dt);
    }

    pub fn deactivate(&mut self, k: &mut Kids) -> HighFallOut {
        let mut out = HighFallOut::default();
        if !self.active() {
            return out;
        }
        *self = HighFall::default();
        k.ru.deactivate();
        k.rds.deactivate(k.ru);
        out.stop_catch_fall = k.catching;
        out.stop_balance = k.balancing;
        k.head.deactivate();
        k.pedal.deactivate();
        k.windmill.deactivate();
        out
    }

    pub fn tick(&mut self, c: &mut Ctx, k: &mut Kids, world: &Collision, obstacles: &[Obstacle], dt: f32) -> HighFallOut {
        let mut out = HighFallOut::default();
        if !self.active() {
            return out;
        }
        self.t += dt;
        let all: Vec<usize> = (0..c.b.build.parts.len()).collect();
        let contact = c.b.doll.collided(&all);
        if self.f & delirium_data::kv!(251 int) != 0 {
            if self.timer > delirium_data::kv!(73) && self.f & delirium_data::kv!(51 int) != 0 {
                self.f = (self.f & delirium_data::kv!(250 int)) | delirium_data::kv!(249 int);
            }
            if self.s & delirium_data::kv!(51 int) != 0 {
                self.f = (self.f & delirium_data::kv!(369 int)) | delirium_data::kv!(7 int);
            }
        }
        if self.s & 1 != 0 {
            let slow = contact && self.fwd < self.p.foward_vel_cut_off;
            let crash = contact && (self.f & delirium_data::kv!(23 int) != 0 || self.s & delirium_data::kv!(23 int) != 0);
            let landed = if self.p.foward_roll {
                self.f |= delirium_data::kv!(251 int);
                true
            } else if crash {
                self.f |= delirium_data::kv!(7 int);
                true
            } else if slow {
                self.f |= delirium_data::kv!(355 int);
                true
            } else {
                false
            };
            if landed {
                self.s &= delirium_data::kv!(294 int);
            }
        }
        if self.s & delirium_data::kv!(1 int) != 0 {
            let late = self.t > self.land - self.p.catchfalltime - delirium_data::kv!(9);
            let go = (late && self.s & delirium_data::kv!(7 int) == 0) || (contact && self.t > delirium_data::kv!(106));
            if go && !self.p.ignor_world_collisions {
                self.s = (self.s & delirium_data::kv!(428 int)) | 2;
            } else if late {
                self.s = (self.s & delirium_data::kv!(36 int)) | 1;
            }
        }
        self.orient(c, world, obstacles, dt);
        self.flail(c, k);
        self.roll_up(c, k);
        if self.s & 1 != 0 {
            self.landing(c);
        }
        self.balance(c, k, &mut out, dt);
        self.roll(c, k, dt);
        self.catch(c, k, &mut out);
        out
    }

    fn orient(&mut self, c: &mut Ctx, world: &Collision, obstacles: &[Obstacle], dt: f32) {
        let p = self.p;
        if self.f & delirium_data::kv!(249 int) == 0 && self.f & delirium_data::kv!(7 int) == 0 && self.s & 2 == 0 {
            c.ch.set_stiffness(p.body_stiffness, p.bodydamping, "fb", None);
        }
        let (up, vel) = (c.up, c.vel);
        let vz = crate::nmdbm::sdot(up, vel, b"zyx");
        let (mut rot, mut base) = (p.foward_vel_rotation, p.aim_angle_base);
        if p.foward_roll {
            (rot, base) = (0.0, base + delirium_data::kv!(57));
        }
        let feet = (c.b.part(limb(2).p[delirium_data::kv!(0 int)]).m.d + c.b.part(limb(delirium_data::kv!(0 int)).p[delirium_data::kv!(0 int)]).m.d) * 0.5;
        let d = vel * delirium_data::kv!(60);
        let hit = crate::nmcf::cast(world, obstacles, feet, d.normalize_or_zero(), d.length())
            .filter(|&(_, n, _)| crate::nmdbm::sdot(n, up, b"zyx").abs() >= delirium_data::kv!(110));
        let h = hit.map(|(t, _, _)| {
            let v = d.normalize_or_zero() * t;
            crate::nmdbm::slen(v - crate::nmdbm::shz(v, up, b"zyx"), b"yxz")
        });
        let g = crate::ragdoll::GRAVITY();
        self.land = match h {
            Some(h) => ((vz.mul_add(vz, g * h * 2.0).sqrt() - vz.abs()) / g) + self.t,
            None => self.t + delirium_data::kv!(80),
        };
        let frac = 1.0 - (self.land - self.t) / self.land;
        let hv = crate::nmdbm::shz(vel, up, b"zyx");
        self.fwd = crate::nmdbm::sdot(c.m.c, -hv, b"yxz");
        let (b, cc) = if self.fwd.abs() >= 1.0 && p.orientate_body_to_fall_direction {
            let v = flat(-vel, up).normalize_or_zero();
            (-up.vx(v), v)
        } else {
            let ha = flat(c.m.a, up);
            (-ha, ha.vx(up))
        };
        let th = (rot * self.fwd + base).clamp(-0.5, delirium_data::kv!(70));
        let (sn, cs) = th.sin_cos();
        let (a, cc) = (up * cs - cc * sn, cc * cs + up * sn);
        let pm = c.b.part(PELVIS).m;
        let r = Mat3::from_cols(a, b, cc) * Mat3::from_cols(pm.a, pm.b, pm.c).transpose();
        let (mut axis, angle) = axis_angle(quat_of(&r));
        if self.fwd.abs() < delirium_data::kv!(70) {
            axis = up * up.vd(axis);
        }
        self.s |= delirium_data::kv!(7 int);
        if (angle * frac).abs() > p.crash_or_land_cut_off {
            self.s &= !delirium_data::kv!(7 int);
        }
        if angle > p.catch_fall_cut_off {
            self.s |= delirium_data::kv!(23 int);
        } else {
            self.s &= !delirium_data::kv!(23 int);
        }
        if self.s & delirium_data::kv!(251 int) != 0 {
            let k = (angle * delirium_data::kv!(507) - c.angvel.vd(axis) * delirium_data::kv!(193)) * p.pd_strength / (dt * delirium_data::kv!(193));
            c.out.torque(PELVIS, axis * k);
        }
    }

    fn flail(&mut self, c: &mut Ctx, k: &mut Kids) {
        let p = self.p;
        if self.s & delirium_data::kv!(1 int) == 0 {
            if k.head.active && self.f & delirium_data::kv!(249 int) == 0 && self.f & delirium_data::kv!(7 int) == 0 {
                k.head.deactivate();
            }
            k.pedal.deactivate();
            k.windmill.deactivate();
            return;
        }
        if !k.head.active {
            k.head.activate(c.ch, c.b);
            k.head.always_look = true;
        }
        if !k.pedal.active {
            let pd = &mut *k.pedal;
            (pd.random_seed, pd.pedal_offset, pd.left, pd.right, pd.speed_asymmetry) = (delirium_data::kv!(189 int), delirium_data::kv!(73), true, true, delirium_data::kv!(104));
            (pd.radius, pd.angular_speed, pd.back_pedal, pd.leg_stiffness, pd.adaptive) =
                (p.leg_radius, p.leg_ang_speed, false, p.body_stiffness, false);
            pd.activate(c);
        }
        if !k.windmill.active {
            k.windmill.activate(c.ch);
            k.windmill.disable_on_impact = true;
        }
        c.spine_lean(0.0, 0.0);
        let up = c.up;
        let h = &mut *k.head;
        h.pos = ((c.com + crate::nmdbm::shz(-c.m.c, up, b"yzx")) - up) - up;
        (h.stiffness, h.damping, h.vel) = (p.body_stiffness, p.bodydamping, c.vel);
        let w = &mut *k.windmill;
        (w.body_stiffness, w.arm_stiffness, w.period, w.amplitude, w.phase) =
            (p.body_stiffness, p.body_stiffness * delirium_data::kv!(508), p.arm_period, p.arm_amplitude, 1.0);
    }

    fn roll_up(&mut self, c: &mut Ctx, k: &mut Kids) {
        if self.s & 2 == 0 {
            k.ru.deactivate();
            return;
        }
        if !k.ru.active {
            k.ru.activate(c.ch);
            let r = &mut *k.ru;
            (r.use_arm_to_slow_down, r.arm_reach_amount, r.stiffness, r.leg_push, r.asymmetrical_legs, r.mask) =
                (delirium_data::kv!(178), delirium_data::kv!(509), self.p.body_stiffness, 0.0, delirium_data::kv!(18), "fb");
        }
        if slen(c.vel, b"zyx") < delirium_data::kv!(64) && self.s & delirium_data::kv!(51 int) == 0 {
            self.s |= delirium_data::kv!(51 int);
            c.out.send(delirium_data::kv!(23 int), "HighFall", 0.0);
        }
    }

    fn landing(&mut self, c: &mut Ctx) {
        let p = self.p;
        for i in 2..delirium_data::kv!(1 int) {
            c.ch.eff[limb(i).e[2]].muscle_stiffness = 1.0;
        }
        for i in 2..delirium_data::kv!(1 int) {
            c.ch.eff[limb(i).e[1]].muscle_stiffness = delirium_data::kv!(510);
        }
        let up = c.up.normalize_or_zero();
        c.ch.eff[(*NECK())[0]].desired[0] = delirium_data::kv!(106);
        c.ch.eff[(*NECK())[1]].desired[0] = delirium_data::kv!(24);
        c.spine_lean(0.0, 0.0);
        let pv = c.b.part(PELVIS).v;
        let head = c.b.part(c.ch.joints[(*NECK())[1]].child).m;
        let side = head.a.normalize_or_zero();
        let t = head.d + up * p.arms_up + side * delirium_data::kv!(24);
        let (l0, l1) = (limb(0), limb(1));
        crate::nmdbm::limb_ik(c.b, c.ch, l1.e[1], l1.e[2], l1.p[0], l1.p[delirium_data::kv!(0 int)], t, 0.0, 0.0, -1.0, 1.0, Vec3::ZERO, None);
        crate::nmdbm::limb_ik(c.b, c.ch, l0.e[1], l0.e[2], l0.p[0], l0.p[delirium_data::kv!(0 int)], t - side * delirium_data::kv!(110), 0.0, 0.0, 1.0, 1.0, Vec3::ZERO, None);
        for j in Character::mask("uw") {
            let jd = c.ch.joints[j].clone();
            c.ch.eff[j].reset(&jd);
            c.ch.eff[j].desired = [0.0; 3];
        }
        let cu = c.up;
        let side = flat(c.m.a, cu).normalize_or_zero();
        let front = flat(-c.m.c, cu).normalize_or_zero();
        let (l2, l3) = (limb(2), limb(delirium_data::kv!(0 int)));
        let mid = (c.b.joint_pos(l2.e[1]) + c.b.joint_pos(l3.e[1])) * 0.5;
        let at = (mid - flat(mid, cu)) + flat(c.com, cu);
        let ahead = at + c.vel.normalize_or_zero() * p.leg_l + front * p.foward_offset_of_leg_ik;
        let t = at + (ahead - at).normalize_or_zero() * p.leg_l - side * p.side_d;
        crate::nmdbm::limb_ik(c.b, c.ch, l2.e[1], l2.e[2], l2.p[0], l2.p[delirium_data::kv!(0 int)], t, 0.0, 0.0, 1.0, -1.0, pv, None);
        crate::nmdbm::limb_ik(c.b, c.ch, l3.e[1], l3.e[2], l3.p[0], l3.p[delirium_data::kv!(0 int)], t + side * (p.side_d * 2.0), 0.0, 0.0, -1.0, -1.0, pv,
                              None);
        let x = self.fwd.abs() * -delirium_data::kv!(73);
        c.ch.eff[l3.e[delirium_data::kv!(0 int)]].desired[0] = x;
        c.ch.eff[l2.e[delirium_data::kv!(0 int)]].desired[0] = x;
    }

    fn balance(&mut self, c: &mut Ctx, k: &mut Kids, out: &mut HighFallOut, dt: f32) {
        if self.f & delirium_data::kv!(249 int) == 0 {
            return;
        }
        if !k.balancing {
            out.taper_off = true;
            if let Some(d) = k.dbm.as_deref_mut() {
                d.p.taper = false;
            }
            let s = c.ch.random_bits() & 1 != 0;
            out.balance = Some(crate::nmbb::Params {
                body_stiffness: self.p.body_stiffness * delirium_data::kv!(511), elbow: delirium_data::kv!(74), shoulder: delirium_data::kv!(108), damping: delirium_data::kv!(70), max_steps: delirium_data::kv!(189 int),
                head_look_pos: Vec3::new(0.0, 0.0, -delirium_data::kv!(104)), head_look_instance_index: c.ch.inst_id, head_look_always_timeout: delirium_data::kv!(27),
                head_look_at_vel_prob: delirium_data::kv!(108), somersault_angle: 1.0, somersault_angle_threshold: delirium_data::kv!(17), side_somersault_angle: 1.0,
                side_somersault_angle_threshold: delirium_data::kv!(17), ang_vel_multiplier: Vec3::new(delirium_data::kv!(30), delirium_data::kv!(108), delirium_data::kv!(30)),
                ang_vel_threshold: Vec3::new(delirium_data::kv!(74), delirium_data::kv!(5), delirium_data::kv!(74)), arms_out_on_push_multiplier: 1.0, arms_out_on_push_timeout: 1.0,
                returning_to_balance_arms_out: delirium_data::kv!(73), arms_out_straighten_elbows: 0.0, arms_out_min_lean2: -delirium_data::kv!(27),
                body_turn_timeout: 1.0, turn_off_prob: delirium_data::kv!(64), turn2_target_prob: delirium_data::kv!(64), turn2_vel_prob: delirium_data::kv!(70), turn_away_prob: 0.0,
                turn_left_prob: delirium_data::kv!(73), turn_right_prob: delirium_data::kv!(73), elbow_angle_on_contact: delirium_data::kv!(70), bend_elbows_time: 0.0,
                bend_elbows_gait: delirium_data::kv!(70), use_head_look: true, arms_out_on_push: true, use_body_turn: s,
            });
            self.timer = 0.0;
        }
        self.timer += dt;
        if !k.fallen || out.balance.is_some() {
            out.taper_off = true;
            let acc = (self.timer - 0.5).clamp(-0.5, 0.0);
            let (ls, t) = (self.p.leg_strength.clamp(1.0, delirium_data::kv!(21)) * delirium_data::kv!(512), self.timer + 1.0);
            let knee = (ls * crate::Fma::fma(t, t, -0.5)).clamp(0.5, 1.0);
            match k.dbm.as_deref_mut() {
                Some(d) => (d.p.taper, d.acc, d.knee_scale) = (false, acc, knee),
                None => out.dbm_set = Some((acc, knee)),
            }
        }
        if self.s & delirium_data::kv!(51 int) == 0 && slen(c.vel, b"zyx") < delirium_data::kv!(9) {
            self.s |= delirium_data::kv!(51 int);
            c.out.send(if k.fallen && out.balance.is_none() { delirium_data::kv!(23 int) } else { delirium_data::kv!(52 int) }, "HighFall", 0.0);
        }
    }

    fn roll(&mut self, c: &mut Ctx, k: &mut Kids, dt: f32) {
        let p = self.p;
        if self.f & delirium_data::kv!(251 int) == 0 {
            k.rds.deactivate(k.ru);
            return;
        }
        self.timer += dt;
        let ch = &mut *c.ch;
        for j in (*SPINE()).into_iter().rev() {
            ch.eff[j].desired[0] = 1.0;
        }
        for (i, elbow) in [(1, delirium_data::kv!(170)), (0, delirium_data::kv!(485))] {
            let l = limb(i);
            ch.eff[l.e[0]].desired = [0.0; 3];
            ch.eff[l.e[1]].desired = [2.0, -2.0, -delirium_data::kv!(20)];
            ch.eff[l.e[2]].desired[0] = elbow;
        }
        let (l2, l3) = (limb(2), limb(delirium_data::kv!(0 int)));
        ch.eff[l2.e[1]].desired[0] = delirium_data::kv!(130);
        ch.eff[l3.e[1]].desired[0] = delirium_data::kv!(494);
        ch.eff[l2.e[2]].desired[0] = -2.0;
        ch.eff[l3.e[2]].desired[0] = -2.0;
        for j in (*NECK()) {
            ch.eff[j].desired = [delirium_data::kv!(5), 0.0, 0.0];
        }
        let s = (p.body_stiffness - self.timer * delirium_data::kv!(21)).clamp(delirium_data::kv!(61), delirium_data::kv!(21));
        self.speed = self.speed.mul_add(delirium_data::kv!(127), slen(c.vel, b"zyx")) * delirium_data::kv!(73);
        c.ch.set_stiffness(s, p.bodydamping, "fb", None);
        let x = c.up.vd(c.m.b);
        c.ch.eff[l2.e[2]].desired[0] = x - delirium_data::kv!(13);
        c.ch.eff[l3.e[2]].desired[0] = x - delirium_data::kv!(513);
        if p.zero_pose_roll {
            c.relax(1.0, "fb");
        }
        if !k.rds.active && self.f & delirium_data::kv!(51 int) == 0 {
            (k.rds.forcemag, k.rds.spin_when_in_air, k.rds.helper_only) = (1.0, false, true);
            k.rds.activate(c, k.ru);
        }
        if self.timer > delirium_data::kv!(485) && self.s & delirium_data::kv!(51 int) == 0 {
            self.s |= delirium_data::kv!(51 int);
            c.out.send(delirium_data::kv!(23 int), "HighFall", 0.0);
        }
    }

    fn catch(&mut self, c: &mut Ctx, k: &mut Kids, out: &mut HighFallOut) {
        if self.f & delirium_data::kv!(7 int) == 0 {
            return;
        }
        if !k.catching {
            let s = self.p.body_stiffness;
            out.catch_fall = Some(crate::nmcf::Params { legs_stiffness: s * delirium_data::kv!(110), torso_stiffness: s * delirium_data::kv!(115), arms_stiffness: s * delirium_data::kv!(514),
                                                        forward_max_arm_offset: delirium_data::kv!(63), backwards_min_arm_offset: -delirium_data::kv!(17),
                                                        z_axis_spin_reduction: 0.0, use_head_look: true });
        }
        let (n, up) = (c.b.part(c.ch.joints[(*NECK())[0]].child).m, c.up);
        for j in (*NECK()) {
            c.ch.eff[j].desired[0] = -2.0 * n.c.vd(up);
            c.ch.eff[j].desired[1] = -2.0 * n.a.vd(up);
        }
        if self.s & delirium_data::kv!(51 int) == 0 {
            c.out.send(delirium_data::kv!(23 int), "HighFall", 0.0);
        }
        self.s |= delirium_data::kv!(51 int);
    }
}

pub(crate) fn axis_angle(q: Quat) -> (Vec3, f32) {
    let v = Vec3::new(q.x, q.y, q.z);
    let axis = if v.vd(v) > delirium_data::kv!(47) { crate::nmdbm::vnorm(v) } else { Vec3::ZERO };
    let angle = if q.w <= -1.0 {
        std::f32::consts::PI
    } else if q.w >= 1.0 {
        0.0
    } else {
        ((q.w as f64).acos() as f32 * 2.0).abs()
    };
    (axis, angle)
}

pub(crate) fn quat_of(r: &Mat3) -> Quat {
    let (a, b, c) = (r.x_axis, r.y_axis, r.z_axis);
    let t = a.x + b.y + c.z;
    let case = if t >= a.x && t >= b.y && t >= c.z {
        0
    } else if a.x >= b.y && a.x >= c.z {
        1
    } else if b.y >= c.z {
        2
    } else {
        delirium_data::kv!(0 int)
    };
    let big = |d: f32| (d.mul_add(2.0, -t) + 1.0).sqrt() * 0.5;
    match case {
        0 => {
            let w = (t + 1.0).sqrt() * 0.5;
            let f = delirium_data::kv!(17) / w;
            Quat::from_xyzw((b.z - c.y) * f, (c.x - a.z) * f, (a.y - b.x) * f, w)
        }
        1 => {
            let x = big(a.x);
            let f = delirium_data::kv!(17) / x;
            Quat::from_xyzw(x, (b.x + a.y) * f, (c.x + a.z) * f, (b.z - c.y) * f)
        }
        2 => {
            let y = big(b.y);
            let f = delirium_data::kv!(17) / y;
            Quat::from_xyzw((b.x + a.y) * f, y, (c.y + b.z) * f, (c.x - a.z) * f)
        }
        _ => {
            let z = big(c.z);
            let f = delirium_data::kv!(17) / z;
            Quat::from_xyzw((c.x + a.z) * f, (c.y + b.z) * f, z, (a.y - b.x) * f)
        }
    }
}

