use bevy::prelude::*;
use delirium::ragdoll::{Build, Drive, Obstacle, Rigid, Ragdoll};

use crate::Sim;

pub const KICK: f32 = 8.0;
const BLAST_KICK: f32 = 9.0;
const BLAST_CUT: f32 = 1.0;
pub const HAT: (f32, f32) = (0.14, 0.12);
const POP: f32 = 2.5;
const BOUNCE: f32 = 0.3;
const SCRUB: f32 = 4.0;
const HEAD: usize = 12;
const LEGS: [usize; 6] = [1, 2, 3, 4, 5, 6];

pub const CUTS: [(&str, usize); 5] = [("sever left arm", 14), ("sever right arm", 18), ("sever left leg", 1), ("sever right leg", 4), ("sever head", 11)];

pub struct Cut {
    parts: Vec<usize>,
    build: Build,
    doll: Ragdoll,
    rigid: Vec<Rigid>,
}

pub enum Hat {
    On,
    Off(Vec3, Quat, Vec3, Vec3),
}

fn cut_at(k: usize) -> Option<usize> {
    match k {
        1..=6 | 14..=16 | 18..=20 => Some(k),
        13 | 17 => Some(k + 1),
        11 | 12 => Some(11),
        _ => None,
    }
}

impl Sim {
    pub(crate) fn sever(&mut self, k: usize, at: Vec3, push: Vec3) -> bool {
        self.homer_out(false);
        let Some((build, mut doll, parts)) = self.agent.doll.sever(&self.build, k) else { return false };
        if push != Vec3::ZERO {
            doll.impulse(0, at, push);
        }
        self.say(format!("sever  {} cut off ({} parts)", delirium_data::nm_asset::PARTS.get(k).unwrap_or(&"?"), parts.len()));
        let leg = parts.iter().any(|p| LEGS.contains(p));
        let all = self.agent.limp_rage();
        let rigid = parts.iter().map(|&p| all.get(p).copied().unwrap_or_default()).collect();
        self.cuts.push(Cut { parts, build, doll, rigid });
        if leg {
            self.send_to(0, "catchFall start=true");
        }
        true
    }

    pub(crate) fn sever_shot(&mut self, k: usize, at: Vec3, dir: Vec3) -> usize {
        if matches!(k, 11 | 12) {
            self.hat_off(dir * POP);
        }
        match cut_at(k).filter(|_| self.dismember) {
            Some(c) if self.sever(c, at, dir.normalize_or_zero() * KICK) => self.build.parts[c].parent.unwrap_or(0),
            _ => k,
        }
    }

    pub(crate) fn sever_blast(&mut self, at: Vec3) -> bool {
        let frames: Vec<Transform> = self.agent.doll.frames().collect();
        if frames[HEAD].translation.distance(at) < BLAST_CUT * 2.0 {
            self.hat_off((frames[HEAD].translation - at).normalize_or(Vec3::Z) * POP * 2.0);
        }
        if !self.dismember {
            return false;
        }
        let mut any = false;
        for k in [1, 4, 14, 18, 11] {
            let p = frames[k].translation;
            let f = 1.0 - p.distance(at) / BLAST_CUT;
            if f > 0.0 && self.sever(k, p, Vec3::ZERO) {
                let c = self.cuts.last_mut().unwrap();
                let dir = ((p - at).normalize_or(Vec3::Z) + Vec3::Z * 0.5).normalize();
                for (i, x) in c.doll.frames().collect::<Vec<_>>().into_iter().enumerate() {
                    let m = c.doll.part_mass(i);
                    c.doll.impulse(i, x.translation, dir * m * BLAST_KICK * f);
                }
                any = true;
            }
        }
        any
    }

    pub(crate) fn hat_button(&mut self) {
        match self.hat {
            Some(Hat::On) => self.hat_off(Vec3::ZERO),
            _ => {
                self.hat = Some(Hat::On);
                self.say("sever  hat on".into());
            }
        }
    }

    fn hat_off(&mut self, push: Vec3) {
        if !matches!(self.hat, Some(Hat::On)) {
            return;
        }
        let Some(t) = self.hat_frame(&self.homer_frames()) else { return };
        let v = self.head_doll().map_or(Vec3::ZERO, |(d, i)| d.vel(i).0);
        self.hat = Some(Hat::Off(t.translation, t.rotation, v + push + Vec3::Z * POP, push.cross(Vec3::Z) * 4.0));
        self.say("sever  hat knocked off".into());
    }

    fn head_doll(&self) -> Option<(&Ragdoll, usize)> {
        match self.cuts.iter().find(|c| c.parts.contains(&HEAD)) {
            Some(c) => c.parts.iter().position(|&p| p == HEAD).map(|i| (&c.doll, i)),
            None => Some((&self.agent.doll, HEAD)),
        }
    }

    pub(crate) fn hat_frame(&self, f: &[Transform]) -> Option<Transform> {
        match self.hat.as_ref()? {
            Hat::Off(x, q, ..) => Some(Transform::from_translation(*x).with_rotation(*q)),
            Hat::On => {
                let (neck, head) = (f.get(11)?.translation, f.get(HEAD)?.translation);
                let up = (head - neck).normalize_or(Vec3::Z);
                Some(Transform::from_translation(head + up * (self.build.parts[HEAD].half + self.build.parts[HEAD].radius * 0.6 + HAT.1 * 0.5))
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, up)))
            }
        }
    }

    pub(crate) fn homer_frames(&self) -> Vec<Transform> {
        let mut f: Vec<Transform> = self.agent.doll.frames().collect();
        for c in &self.cuts {
            for (i, x) in c.doll.frames().enumerate() {
                f[c.parts[i]] = x;
            }
        }
        f
    }

    pub(crate) fn cut_capsules(&self) -> impl Iterator<Item = (usize, Transform)> + '_ {
        self.cuts.iter().flat_map(|c| c.doll.capsules().enumerate().map(|(i, t)| (c.parts[i], t)))
    }

    pub(crate) fn sever_frame(&mut self, dt: f32, obstacles: &[Obstacle]) {
        for c in &mut self.cuts {
            self.agent.doll.ghost(&c.parts);
            c.doll.step(&c.build, &self.world, dt, Drive { rigid: &c.rigid, ..Drive::default() }, obstacles, &[]);
        }
        if self.cuts.iter().any(|c| c.parts.iter().any(|p| LEGS.contains(p))) {
            self.rises[0] = None;
        }
        let Some(Hat::Off(x, q, v, w)) = self.hat.as_mut() else { return };
        v.z -= 9.8 * dt;
        *x += *v * dt;
        *q = (Quat::from_scaled_axis(*w * dt) * *q).normalize();
        let floor = self.world.ground(*x + Vec3::Z * 0.5, 3.0).unwrap_or(0.0);
        let low = HAT.1 * 0.5;
        if x.z < floor + low {
            x.z = floor + low;
            v.z = -v.z * BOUNCE;
            let k = (1.0 - SCRUB * dt).max(0.0);
            (v.x, v.y, *w) = (v.x * k, v.y * k, *w * k);
            *q = Quat::from_rotation_arc(*q * Vec3::Y, Vec3::Z).slerp(Quat::IDENTITY, 1.0 - 4.0 * dt) * *q;
        }
    }
}
