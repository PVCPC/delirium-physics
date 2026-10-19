use bevy::prelude::*;

use crate::{Sim, HERO};

const COMMIT: f32 = 4.0;
const LAUNCH: f32 = 5.5;
const LIFT: f32 = 1.6;
const TIP: f32 = 2.0;
const REACH: f32 = 0.3;
const SEEK: f32 = 0.06;
const GRIP: f32 = 0.3;
const ARM_MASS: f32 = 8.0;
const GRIP_MAX: f32 = 2500.0;
const IMPACT: f32 = 0.1;
const DRIVE: f32 = 500.0;
const PULL: f32 = 900.0;
const PULL_TIME: f32 = 1.0;
const WHIFF: f32 = 1.0;
const HOLD: f32 = 2.0;
const HANDS: [usize; 2] = [16, 20];
const CHEST: usize = 9;
const THIGHS: [usize; 2] = [1, 4];

pub struct Tackle {
    keep: bool,
    t: f32,
    dir: Vec3,
    grips: [(usize, Vec3, Option<f32>); 2],
}

impl Sim {
    pub(crate) fn tackle_start(&mut self, low: bool, keep: bool) {
        let Some(n) = self.hero.as_ref().filter(|_| self.hero_doll.is_none() && !self.driving) else { return self.say("tackle  play as the Hero, on his feet, first".into()) };
        let (feet, vel) = (n.feet, n.vel);
        let homer: Vec<Transform> = self.agent.doll.frames().collect();
        let aim = homer[if low { THIGHS[0] } else { CHEST }].translation;
        let gap = (aim - feet).with_z(0.0);
        if gap.length() > COMMIT {
            return self.say(format!("tackle  Homer's {:.1} m off: get within {COMMIT} m", gap.length()));
        }
        let lead = (aim + self.agent.doll.com().1 * gap.length() / LAUNCH - feet).with_z(0.0);
        let dir = lead.normalize_or(Vec3::X);
        if !self.hero_fall() {
            return self.say("tackle  needs Hero physics".into());
        }
        let Some(h) = self.hero_doll.as_mut() else { return };
        let doll = &mut h.agent.doll;
        let now: Vec<Transform> = doll.frames().collect();
        let face = self.hero.as_ref().map_or(dir, |n| n.facing().with_z(0.0).normalize_or(dir));
        let turn = Quat::from_rotation_arc(face, dir);
        let c = now[0].translation;
        let w = Vec3::Z.cross(dir) * TIP;
        for (k, f) in now.iter().enumerate() {
            let x = c + turn * (f.translation - c);
            doll.set_body(k, x, (turn * f.rotation).normalize(), vel.with_z(0.0) * 0.5 + dir * LAUNCH + Vec3::Z * LIFT + w.cross(x - c), w);
        }
        let side = dir.cross(Vec3::Z);
        let on = |k: usize, at: Vec3| (k, homer[k].rotation.inverse() * (at - homer[k].translation), None);
        let grips = if low {
            let near = |p: Vec3| if homer[THIGHS[0]].translation.distance(p) < homer[THIGHS[1]].translation.distance(p) { THIGHS[0] } else { THIGHS[1] };
            [-1.0, 1.0].map(|s: f32| { let k = near(c - side * s); on(k, homer[k].translation) })
        } else {
            [-1.0, 1.0].map(|s: f32| on(CHEST, homer[CHEST].translation - side * s * 0.15))
        };
        self.send_to(HERO, "catchFall start=true");
        self.tackle = Some(Tackle { keep, t: 0.0, dir, grips });
        self.say(format!("tackle  {} dive at {:.1} m", if low { "low" } else { "high" }, gap.length()));
    }

    pub(crate) fn tackle_frame(&mut self, dt: f32) {
        let Some(mut tk) = self.tackle.take() else { return };
        let Some(h) = self.hero_doll.as_mut().filter(|h| h.rise.is_none()) else { return };
        tk.t += dt;
        h.t = 0.0;
        let held = tk.grips.iter().filter_map(|g| g.2).fold(None, |a: Option<f32>, s| Some(a.map_or(s, |a| a.min(s))));
        if held.is_none() && tk.t > WHIFF {
            return self.say("tackle  whiff: no hold".into());
        }
        if held.is_some_and(|s| !tk.keep && tk.t - s > HOLD) {
            return self.say("tackle  let go".into());
        }
        if held.is_some() && self.homer.is_some() {
            self.homer_out(false);
        }
        let h = self.hero_doll.as_mut().unwrap();
        let (hero, homer) = (&mut h.agent.doll, &mut self.agent.doll);
        let mut said = vec![];
        for (i, g) in tk.grips.iter_mut().enumerate() {
            let (k, local, since) = *g;
            let f = homer.frames().nth(k).unwrap();
            let at = f.translation + f.rotation * local;
            let hand = hero.frames().nth(HANDS[i]).unwrap().translation;
            let gap = at - hand;
            if since.is_none() && gap.length() < REACH {
                g.2 = Some(tk.t);
                said.push(format!("tackle  {} hand holds at {:.2} s", ["left", "right"][i], tk.t));
            }
            let rel = speed_at(homer, k, at) - speed_at(hero, HANDS[i], hand);
            let m = 1.0 / (1.0 / ARM_MASS + 1.0 / homer.part_mass(k));
            let j = if g.2.is_some() { (gap * GRIP / dt + rel) * m } else { gap.normalize_or_zero() * SEEK / dt * m };
            if g.2.is_some_and(|s| tk.t - s > IMPACT) && j.length() > GRIP_MAX * dt {
                g.2 = None;
                said.push(format!("tackle  {} hand slips", ["left", "right"][i]));
                continue;
            }
            hero.impulse(HANDS[i], hand, j);
            homer.impulse(k, at, -j);
        }
        if held.is_some_and(|s| tk.t - s < PULL_TIME) {
            let chest = hero.frames().nth(CHEST).unwrap().translation;
            hero.impulse(CHEST, chest, tk.dir * DRIVE * dt);
            for &(k, local, since) in &tk.grips {
                if since.is_some() {
                    let f = homer.frames().nth(k).unwrap();
                    homer.impulse(k, f.translation + f.rotation * local, (tk.dir * 0.6 - Vec3::Z) * PULL * 0.5 * dt);
                }
            }
        }
        for s in said {
            self.say(s);
        }
        self.tackle = Some(tk);
    }
}

fn speed_at(doll: &delirium::ragdoll::Ragdoll, k: usize, at: Vec3) -> Vec3 {
    let (v, w) = doll.vel(k);
    v + w.cross(at - doll.centre(k))
}
