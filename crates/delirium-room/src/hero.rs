use crate::getup::{self, Clips};
use crate::melee;
use bevy::prelude::*;
use delirium::ragdoll::Capsule;

pub const MELEE_PUSH: f32 = 30.0;
pub const BUMP_SPEED: f32 = 2.5;
const STAGGER_TIME: f32 = 2.5;
pub type Held = (f32, f32, f32);
const TURN: f32 = 10.0;
const EYE: f32 = 1.6;
const PELVIS: f32 = 1.0;
const BOUND: f32 = 0.3;
const SWIM_DEEP: f32 = 1.2;
const SWIM_ROOT: f32 = 1.35;
const SWIM_SPEEDS: [f32; 3] = [0.9, 1.4, 2.0];

pub struct Hero {
    pub feet: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    clip: usize,
    phase: f32,
    fight: melee::Melee,
    pad: melee::Pad,
    press: u32,
    pub keys: u32,
    pub scripted: u32,
    clock: f32,
    rand: delirium_data::rng::Rand,
    bones: Vec<(Vec3, Quat)>,
    speeds: [f32; 3],
    pub caps: Vec<Transform>,
    last: Vec<Transform>,
    dt: f32,
    stagger: f32,
    pub go: (Vec3, usize),
    pub hold: Option<(usize, f32, Vec3)>,
    pub hand: Vec3,
    pub grip: Quat,
    pub homer: Vec<Transform>,
    #[cfg(feature = "portal")]
    pub chell: Option<Quat>,
    #[cfg(feature = "portal")]
    pub fall: f32,
    pub foe: melee::Foe,
    pub swimming: bool,
}

pub enum Hit {
    Swing(f32, Vec3, Vec3),
    Bump(Vec3),
}

impl Hero {
    pub fn new(clips: &Clips, homer: Vec3, fwd: Vec3) -> Hero {
        let speed = |c: usize| (clips.pose(c, 1.0)[0].0 - clips.pose(c, 0.0)[0].0).truncate().length() / clips.duration(c).max(0.01);
        let feet = (homer + fwd * 1.5).with_z(0.0);
        let mut n = Hero { feet, yaw: (-fwd.y).atan2(-fwd.x), vel: Vec3::ZERO, clip: getup::IDLE, phase: 0.0, fight: Default::default(), pad: Default::default(),
                           press: 0, keys: 0, scripted: 0, clock: 0.0, rand: Default::default(), bones: vec![],
                           speeds: [speed(getup::WALK), speed(getup::WALK + 1), speed(getup::WALK + 2)],
                           caps: vec![], last: vec![], dt: 0.0, stagger: 0.0, go: (Vec3::ZERO, 0),
                           hold: None, hand: feet + Vec3::Z * 1.0, grip: Quat::IDENTITY, homer: vec![], foe: Default::default(),
                           #[cfg(feature = "portal")] chell: None, #[cfg(feature = "portal")] fall: 0.0, swimming: false };
        n.pose(clips);
        n
    }

    pub fn facing(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), self.yaw.sin(), 0.0)
    }

    pub fn held_axis(&self, (back, ahead, big): Held) -> (Vec3, Vec3, f32, f32) {
        let d = self.grip * Vec3::X;
        (self.hand - d * back, self.hand + d * ahead, big, back / (back + ahead))
    }

    pub fn playing(&self) -> bool {
        self.fight.mv.is_some()
    }

    pub fn as_foe(&self, clips: &Clips, weapon: Option<&delirium_data::weapons::Weapon>) -> melee::Foe {
        let f = melee::Foe { weapon: weapon.map(melee::held), ..Default::default() };
        clips.fight.as_ref().map_or(f, |d| f.with_move(&d.0, self.fight.mv.as_ref()))
    }

    pub fn disarmed(&self) -> bool {
        self.fight.disarm_foe
    }

    pub fn blocking(&self, clips: &Clips) -> bool {
        let r = clips.fight.as_ref().zip(self.fight.mv.as_ref()).map(|(f, m)| &f.0.table.results[m.result]);
        r.is_some_and(|r| r.is_a_block)
    }

    pub fn hit_by(&mut self, clips: &Clips, by: &Hero, weapon: &delirium_data::weapons::Weapon) -> Option<String> {
        let d = &clips.fight.as_ref()?.0;
        let from = melee::Foe { feet: by.feet, heading: by.yaw - std::f32::consts::FRAC_PI_2, health: 200.0, ..Default::default() }.with_move(d, by.fight.mv.as_ref());
        let (res, clip) = from.task.unwrap_or((0, None));
        let blow = melee::Blow { by: from, player: false, res, clip };
        let me = melee::Foe { feet: self.feet, heading: self.yaw - std::f32::consts::FRAC_PI_2, health: 200.0, ..Default::default() };
        let rand = &mut self.rand;
        let r = melee::react(d, &me, true, Some(weapon), self.fight.mv.as_ref(), &blow, false, [false; 2], false, &mut || rand.next());
        melee::play_result(&mut self.fight.mv, d, r.action?, true, &|k| clips.fight_clip(k).map(|c| c.1.duration));
        self.fight.on |= self.fight.mv.is_some();
        r.result.and_then(|r| d.clip(r))
    }

    pub fn head(&self) -> Vec3 {
        self.feet + Vec3::Z * EYE
    }

    pub fn punch(&mut self) {
        self.press |= melee::ATTACK_BIT;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, clips: &Clips, world: &delirium::collision::Collision, dt: f32, mv: Vec3, gait: usize, homer: Option<(Vec3, f32, f32, [Option<Vec3>; 5])>,
                  weapon: &delirium_data::weapons::Weapon, held: Option<Held>, body: &[Capsule], water: Option<f32>) -> Option<Hit> {
        if dt <= 0.0 {
            return None;
        }
        self.stagger -= dt;
        let mut hit = None;
        let fight = clips.fight.as_ref().map(|f| f.0.clone());
        let mut locked = false;
        let was = self.fight.mv.as_ref().and_then(|m| Some((clips.fight_clip(m.key.as_deref()?)?.0, m.phase())));
        if let Some(d) = fight.as_deref() {
            self.clock += dt;
            let bits = std::mem::take(&mut self.press) | self.keys | self.scripted;
            self.pad.feed(bits, [0x7F; 2], [if bits & 1 != 0 { 255 } else { 0 }, 0], (self.clock * 1000.0) as u32);
            let reach = if weapon.target_range > 0.0 { weapon.target_range } else { 10.0 };
            let foe = homer.map(|(p, heading, health, bones)| melee::Foe { feet: p.with_z(self.feet.z), heading, health, bones, ..self.foe })
                .filter(|f| f.feet.distance(self.feet) <= reach);
            locked = foe.is_some() && self.pad.value(melee::LOCK_LT) >= melee::LOCK_FULL;
            let c = melee::Ctx { pad: &self.pad, pos: self.feet + Vec3::Z, health: 200.0, weapon, foe, me: None, lock_on: locked };
            let mut heading = self.yaw - std::f32::consts::FRAC_PI_2;
            let rand = &mut self.rand;
            self.fight.step(d, &c, &mut heading, dt, &|k| clips.fight_clip(k).map(|c| c.1), &mut || rand.next());
            self.yaw = heading + std::f32::consts::FRAC_PI_2;
        }
        let stance = fight.as_deref().filter(|_| self.fight.on || locked).and_then(|d| {
            clips.fight_clip(&d.stance_key(-1, 0x0F)?).map(|c| c.0)
        });
        let playing = self.fight.mv.as_ref().and_then(|m| Some((clips.fight_clip(m.key.as_deref()?)?.0, m.phase())));
        if let Some((c, phase)) = playing {
            let heading = self.yaw - std::f32::consts::FRAC_PI_2;
            let from = was.filter(|w| w.0 == c && !self.fight.started).map_or(0.0, |w| w.1);
            let at = clips.mover(c, phase);
            let (anim, travel) = ((at - clips.mover(c, from)).truncate() / dt, (at - clips.mover(c, 0.0)).y);
            let v = self.fight.home.map_or(anim, |h| Vec2::new(anim.x, h.speed(self.feet + Vec3::Z, heading, anim, if h.push { 0.0 } else { travel }, dt).unwrap_or(anim.y)));
            (self.clip, self.phase, self.vel) = (c, phase, Quat::from_rotation_z(heading) * v.extend(0.0));
        } else if mv.length_squared() > 1e-4 {
            let want = mv.y.atan2(mv.x);
            let d = (want - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            self.yaw += d.clamp(-TURN * dt, TURN * dt);
            let walk = self.swim(clips).map_or(getup::WALK, |s| s + 1);
            let c = walk + gait.min(2);
            if self.clip != c && !(walk..walk + 3).contains(&self.clip) {
                self.phase = 0.0;
            }
            self.clip = c;
            self.phase = (self.phase + dt / clips.duration(c)).fract();
            self.vel = self.facing() * if self.swim(clips).is_some() { SWIM_SPEEDS[gait.min(2)] } else { self.speeds[gait.min(2)] };
        } else if let Some((c, phase, at)) = self.hold {
            let to = (at - self.feet).with_z(0.0);
            let d = (to.y.atan2(to.x) - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            self.yaw += d.clamp(-TURN * dt, TURN * dt);
            (self.clip, self.phase, self.vel) = (c, phase, Vec3::ZERO);
        } else {
            let c = self.swim(clips).or(stance).unwrap_or(getup::IDLE);
            if self.clip != c {
                self.phase = 0.0;
            }
            self.clip = c;
            self.phase = (self.phase + dt / clips.duration(c)).fract();
            self.vel = Vec3::ZERO;
        }
        self.feet += self.vel * dt;
        let ground = world.ground(self.feet.with_z(self.feet.z.max(water.unwrap_or(self.feet.z))) + Vec3::Z * 1.0, 3.5);
        self.swimming = water.zip(ground).is_some_and(|(w, g)| w - g > SWIM_DEEP);
        let ground = if self.swimming { water.map(|w| w - SWIM_ROOT + clips.mover(self.clip, 0.0).z) } else { ground };
        #[cfg(feature = "portal")]
        let ground = crate::portal::hero_fall(self, ground, dt);
        if let Some(z) = ground {
            self.feet.z = z;
        }
        for &(a, b, r, ..) in body {
            let (p, q) = closest(self.feet + Vec3::Z * 0.3, self.feet + Vec3::Z * 1.5, a, b);
            let out = (p - q).with_z(0.0);
            if out.length() < BOUND + r {
                self.feet += out.normalize_or(-self.facing()) * (BOUND + r - out.length()) * 0.5;
            }
        }
        if let Some((f, ..)) = homer {
            let into = (f - self.feet).with_z(0.0);
            if self.stagger <= 0.0 && into.length() < 0.7 && self.vel.length() > BUMP_SPEED && self.vel.dot(into) > 0.0 {
                self.stagger = STAGGER_TIME;
                hit = hit.or(Some(Hit::Bump(self.feet)));
            }
        }
        self.dt = dt;
        self.pose(clips);
        if let Some(d) = fight.as_deref().filter(|_| self.fight.on) {
            let caps: Vec<(Vec3, Vec3, f32, u16)> = body.iter().filter(|_| homer.is_some()).map(|c| (c.0, c.1, c.2, 0)).collect();
            let (bones, heading) = (&self.bones, self.yaw - std::f32::consts::FRAC_PI_2);
            let part = |tag: u32| melee::part(tag, &|t| clips.tagged(bones, t));
            let axis = held.map(|h| self.held_axis(h));
            for (.., at, dmg) in self.fight.strike(d, self.feet, heading, &part, axis, &[(0, &caps)]) {
                hit = hit.or(Some(Hit::Swing(dmg, at, self.facing() * MELEE_PUSH)));
            }
        }
        hit
    }

    fn swim(&self, clips: &Clips) -> Option<usize> {
        clips.swim.filter(|_| self.swimming)
    }

    fn pose(&mut self, clips: &Clips) {
        let Some(rig) = clips.hero.as_ref() else { return };
        let mut p = clips.play(self.clip, self.phase.clamp(0.0, 1.0));
        let body = Transform::from_translation(self.feet + Vec3::Z * PELVIS).with_rotation(Quat::from_rotation_z(self.yaw - std::f32::consts::FRAC_PI_2));
        if let Some((c, _, at)) = self.hold.filter(|h| h.0 == self.clip) {
            let chest = self.feet + Vec3::Z * 1.4;
            let pitch = (at.z - chest.z).atan2((at - chest).with_z(0.0).length().max(0.1));
            let _ = c;
            clips.bend(&mut p, body, self.facing().cross(Vec3::Z).normalize_or(Vec3::X), pitch.clamp(-1.0, 1.0));
            #[cfg(feature = "portal")]
            if self.chell.is_some() {
                self.chell = Some(crate::portal::chell_arms(clips, &mut p, body, pitch.clamp(-1.0, 1.0)));
            }
        }
        if let Some((at, q)) = clips.bone(&p, body, "Char_R_Hand") {
            (self.hand, self.grip) = (at, q);
        }
        self.bones = clips.world(&p, body);
        self.homer = clips.place(&p, body);
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

pub fn closest(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> (Vec3, Vec3) {
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
