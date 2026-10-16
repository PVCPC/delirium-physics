use crate::getup::{self, Clips};
use bevy::prelude::*;
use delirium::ragdoll::Capsule;

const DAMAGE: [f32; 3] = [7.0, 7.0, 9.0];
pub const MELEE_PUSH: f32 = 30.0;
pub const BUMP_SPEED: f32 = 2.5;
const STAGGER_TIME: f32 = 2.5;
const TURN: f32 = 10.0;
const EYE: f32 = 1.6;
const PELVIS: f32 = 1.0;
const BOUND: f32 = 0.3;

pub struct Hero {
    pub feet: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    clip: usize,
    phase: f32,
    swing: Option<(usize, f32, bool)>,
    combo: usize,
    speeds: [f32; 3],
    pub caps: Vec<Transform>,
    last: Vec<Transform>,
    dt: f32,
    stagger: f32,
    pub go: (Vec3, usize),
}

pub enum Hit {
    Swing(f32, Vec3, Vec3),
    Bump(Vec3),
}

impl Hero {
    pub fn new(clips: &Clips, homer: Vec3, fwd: Vec3) -> Hero {
        let speed = |c: usize| (clips.pose(c, 1.0)[0].0 - clips.pose(c, 0.0)[0].0).truncate().length() / clips.duration(c).max(0.01);
        let feet = (homer + fwd * 1.5).with_z(0.0);
        let mut n = Hero { feet, yaw: (-fwd.y).atan2(-fwd.x), vel: Vec3::ZERO, clip: getup::IDLE, phase: 0.0, swing: None, combo: 0,
                           speeds: [speed(getup::WALK), speed(getup::WALK + 1), speed(getup::WALK + 2)],
                           caps: vec![], last: vec![], dt: 0.0, stagger: 0.0, go: (Vec3::ZERO, 0) };
        n.pose(clips);
        n
    }

    pub fn facing(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), self.yaw.sin(), 0.0)
    }

    pub fn head(&self) -> Vec3 {
        self.feet + Vec3::Z * EYE
    }

    pub fn punch(&mut self) {
        if self.swing.is_none() {
            self.swing = Some((getup::SWINGS + self.combo % 3, 0.0, false));
            self.combo += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, clips: &Clips, world: &delirium::collision::Collision, dt: f32, mv: Vec3, gait: usize, homer: Option<Vec3>,
                  range: f32, body: &[Capsule]) -> Option<Hit> {
        if dt <= 0.0 {
            return None;
        }
        self.stagger -= dt;
        let mut hit = None;
        let (facing, feet) = (self.facing(), self.feet);
        if let Some((c, t, done)) = self.swing.as_mut() {
            *t += dt;
            let dur = clips.duration(*c);
            (self.clip, self.phase, self.vel) = (*c, *t / dur, Vec3::ZERO);
            if !*done && *t >= dur * 0.45 {
                *done = true;
                let off = homer.map(|f| (f - feet).with_z(0.0));
                if let Some(off) = off.filter(|o| o.length() < range + 0.5 && o.normalize_or_zero().dot(facing) > 0.5) {
                    hit = Some(Hit::Swing(DAMAGE[*c - getup::SWINGS], feet + off + Vec3::Z * 1.4, facing * MELEE_PUSH));
                }
            }
            if *t >= dur {
                self.swing = None;
            }
        } else if mv.length_squared() > 1e-4 {
            let want = mv.y.atan2(mv.x);
            let d = (want - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            self.yaw += d.clamp(-TURN * dt, TURN * dt);
            let c = getup::WALK + gait.min(2);
            if self.clip != c && !(getup::WALK..getup::SWINGS).contains(&self.clip) {
                self.phase = 0.0;
            }
            self.clip = c;
            self.phase = (self.phase + dt / clips.duration(c)).fract();
            self.vel = self.facing() * self.speeds[gait.min(2)];
        } else {
            if self.clip != getup::IDLE {
                self.phase = 0.0;
            }
            self.clip = getup::IDLE;
            self.phase = (self.phase + dt / clips.duration(getup::IDLE)).fract();
            self.vel = Vec3::ZERO;
        }
        self.feet += self.vel * dt;
        if let Some(z) = world.ground(self.feet + Vec3::Z * 1.0, 3.0) {
            self.feet.z = z;
        }
        for &(a, b, r, ..) in body {
            let (p, q) = closest(self.feet + Vec3::Z * 0.3, self.feet + Vec3::Z * 1.5, a, b);
            let out = (p - q).with_z(0.0);
            if out.length() < BOUND + r {
                self.feet += out.normalize_or(-self.facing()) * (BOUND + r - out.length()) * 0.5;
            }
        }
        if let Some(f) = homer {
            let into = (f - self.feet).with_z(0.0);
            if self.stagger <= 0.0 && into.length() < 0.7 && self.vel.length() > BUMP_SPEED && self.vel.dot(into) > 0.0 {
                self.stagger = STAGGER_TIME;
                hit = hit.or(Some(Hit::Bump(self.feet)));
            }
        }
        self.dt = dt;
        self.pose(clips);
        hit
    }

    fn pose(&mut self, clips: &Clips) {
        let Some(rig) = clips.hero.as_ref() else { return };
        let p = clips.play(self.clip, self.phase.clamp(0.0, 1.0));
        let body = Transform::from_translation(self.feet + Vec3::Z * PELVIS).with_rotation(Quat::from_rotation_z(self.yaw - std::f32::consts::FRAC_PI_2));
        let frames = rig.place(&p, body);
        let doll = delirium::ragdoll::Ragdoll::new(&rig.build, &frames, Vec3::ZERO);
        self.last = std::mem::replace(&mut self.caps, doll.capsules().collect());
    }

    pub fn capsules(&self, clips: &Clips) -> Vec<Capsule> {
        let Some(rig) = clips.hero.as_ref() else { return vec![] };
        self.caps.iter().zip(&rig.build.parts).enumerate().map(|(k, (t, p))| {
            let a = t.rotation * Vec3::Y * p.half;
            let v = self.last.get(k).filter(|_| self.dt > 0.0).map_or(Vec3::ZERO, |l| (t.translation - l.translation) / self.dt);
            (t.translation - a, t.translation + a, p.radius, v, false)
        }).chain([(self.feet + Vec3::Z * 0.3, self.feet + Vec3::Z * 1.5, BOUND, self.vel, true)]).collect()
    }
}

fn closest(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (b - a, d - c, a - c);
    let (aa, ee, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (s, t) = if aa <= 1e-9 && ee <= 1e-9 {
        (0.0, 0.0)
    } else if aa <= 1e-9 {
        (0.0, (f / ee).clamp(0.0, 1.0))
    } else {
        let cc = d1.dot(r);
        if ee <= 1e-9 {
            ((-cc / aa).clamp(0.0, 1.0), 0.0)
        } else {
            let bb = d1.dot(d2);
            let den = aa * ee - bb * bb;
            let mut s = if den > 1e-9 { ((bb * f - cc * ee) / den).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (bb * s + f) / ee;
            if t < 0.0 {
                (t, s) = (0.0, (-cc / aa).clamp(0.0, 1.0));
            } else if t > 1.0 {
                (t, s) = (1.0, ((bb - cc) / aa).clamp(0.0, 1.0));
            }
            (s, t)
        }
    };
    (a + d1 * s, c + d2 * t)
}

#[test]
fn closest_points() {
    let (p, q) = closest(Vec3::ZERO, Vec3::Z * 2.0, Vec3::new(1.0, -1.0, 1.0), Vec3::new(1.0, 1.0, 1.0));
    assert!(p.distance(Vec3::Z) < 1e-5 && q.distance(Vec3::new(1.0, 0.0, 1.0)) < 1e-5);
}
