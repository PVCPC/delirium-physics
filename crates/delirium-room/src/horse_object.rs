use super::{spread, Horse, LEGS, MID, WELD, WELD_PLAY};
use bevy::prelude::*;
use delirium::ragdoll::{Limit, Muscle, Rules, GRAVITY};

const GIVE: f32 = 0.12;
const HZ: f32 = 1.5;
const SWING: f32 = 0.2;
const ROCK: f32 = 0.06;
const ROCK_K: [f32; 2] = [3.0, 0.6];
const YAW: f32 = 1.0;
const EASE: f32 = 0.5;
const AHEAD: f32 = -1.0;

impl Horse {
    pub(super) fn step_object(&mut self, dt: f32) {
        if self.held {
            self.held = false;
            self.doll.unpin();
            for (k, at, imp) in std::mem::take(&mut self.pushes) {
                self.doll.impulse(k, at, imp);
            }
        }
        let n = self.build.parts.len();
        let fresh = self.obj_free.is_empty();
        if fresh {
            self.obj_free = self.build.parts.iter().map(|p| (p.rest, p.limit)).collect();
        }
        let fr: Vec<Transform> = self.doll.frames().collect();
        let (stiff, lock) = crate::animal_fit::legs(self.animal);
        let mut targets = self.targets.clone();
        let mut torque = vec![Vec3::ZERO; n];
        let fwd = (fr[MID].rotation * self.fwd).with_z(0.0).normalize_or(Vec3::X);
        if self.walk != Vec3::ZERO {
            self.obj_way = if self.walk.dot(fwd) < 0.0 { -1.0 } else { 1.0 };
        }
        let on = self.walk != Vec3::ZERO && !self.dead;
        self.obj_amp = (self.obj_amp + (if on { dt } else { -dt }) / EASE).clamp(0.0, 1.0);
        let (way, amp) = (self.obj_way, self.obj_amp);
        if amp > 0.0 && !self.dead {
            self.waddle += dt * HZ;
            let ph = self.waddle * std::f32::consts::TAU;
            let a = way * amp * SWING * -ph.cos();
            for (l, leg) in LEGS.iter().enumerate() {
                let s = if l == 0 || l == 3 { a } else { -a };
                let parent = self.build.parts[leg[0]].parent.unwrap_or(MID);
                targets[leg[0]] = Quat::from_axis_angle(self.lefts[parent], AHEAD * s) * self.targets[leg[0]];
            }
            let (up, down) = if ph.sin() > 0.0 { ([0, 3], [1, 2]) } else { ([1, 2], [0, 3]) };
            let foot = |l: usize| fr[LEGS[l][2]].translation;
            let axis = (foot(down[1]) - foot(down[0])).with_z(0.0).normalize_or_zero();
            let r = (foot(up[0]) + foot(up[1])) * 0.5 - (foot(down[0]) + foot(down[1])) * 0.5;
            let lift = axis.cross(r).z.signum();
            let h = (fr[MID].translation.z - (0..4).map(|l| foot(l).z).fold(f32::MAX, f32::min)).max(0.1);
            let tilt = lift * axis.dot(Vec3::Z.cross(fr[MID].rotation * self.stand));
            let rate = if self.obj_tilt.is_nan() { 0.0 } else { (tilt - self.obj_tilt) / dt.max(1e-4) };
            self.obj_tilt = tilt;
            let want = amp * ph.sin().abs() * ROCK;
            let yaw = if on { YAW * fwd.cross(self.walk.with_z(0.0).normalize_or(fwd) * way).z.clamp(-1.0, 1.0).asin() } else { 0.0 };
            spread(&mut torque, (axis * lift * (ROCK_K[0] * (want - tilt) - ROCK_K[1] * rate) + Vec3::Z * yaw) * self.doll.mass() * GRAVITY() * h);
        } else {
            self.waddle = 0.0;
            self.obj_tilt = f32::NAN;
        }
        let knee = if lock { WELD_PLAY } else { WELD_PLAY + (GIVE - WELD_PLAY) * (1.0 - stiff) };
        for k in 0..n {
            let p = &mut self.build.parts[k];
            if self.dead || p.parent.is_none() {
                (p.rest, p.limit) = self.obj_free[k];
                continue;
            }
            let play = if LEGS.iter().any(|l| l[1..].contains(&k)) { knee } else { WELD_PLAY };
            (p.rest, p.limit) = (targets[k].inverse(), Limit::Cone { swing: play, twist: play });
        }
        if fresh {
            self.remake();
        }
        if std::env::var_os("HORSE_OBJLOG").is_some() {
            let off = |k: usize| self.build.parts[k].parent.map_or(0.0, |p| (fr[p].rotation.inverse() * fr[k].rotation).angle_between(self.targets[k]).to_degrees());
            let hips = LEGS.iter().map(|l| off(l[0])).fold(0.0, f32::max);
            let (worst, all) = (0..n).map(|k| (k, off(k))).fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
            let p = fr[MID].translation;
            eprintln!("obj hips {hips:.1} all {all:.1} ({worst}) up {:.2} at {:.3} {:.3} {:.3} fwd {:.2} {:.2} way {way} tilt {:.3}", (fr[MID].rotation * self.stand).z, p.x, p.y, p.z, fwd.x, fwd.y, self.obj_tilt);
        }
        self.kept.force = vec![Vec3::ZERO; n];
        self.kept.torque = torque;
        let muscles: Vec<Muscle> = (0..n).map(|k| Muscle { target: targets[k], stiffness: WELD, damping: 1.0 }).collect();
        let most = self.doll.mass() / 618.0;
        self.rage_drive(&muscles, 1.0, most, self.dead, dt);
        self.doll.rules = Rules::Rdr2;
        self.doll.limit_row = vec![None; n];
        self.doll.sleep_tol = [0.0; 3];
        self.kept.muscles = muscles;
    }

    pub(super) fn end_object(&mut self) {
        for (p, &(rest, limit)) in self.build.parts.iter_mut().zip(&std::mem::take(&mut self.obj_free)) {
            (p.rest, p.limit) = (rest, limit);
        }
        self.remake();
    }
}
