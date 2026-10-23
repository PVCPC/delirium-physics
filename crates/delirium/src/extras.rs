use crate::msg::{Error, Kind, Message, Param, Value};
use crate::nmshot::Ctx;
use bevy_transform::components::Transform;

pub fn messages() -> impl Iterator<Item = &'static Message> {
    MESSAGES.iter().chain(&crate::extras_fall::MESSAGES).chain(&crate::extras_held::MESSAGES)
}

pub static MESSAGES: [Message; 3] = [
    Message { name: "electrocute", params: &[
        Param { name: "stunMag", kind: Kind::Float, used: true, doc: "each jolt's kick, m/s on each part (0.25)" },
        Param { name: "initialMult", kind: Kind::Float, used: true, doc: "the first jolts' multiplier (1)" },
        Param { name: "largeMult", kind: Kind::Float, used: true, doc: "the big jolts' multiplier (1)" },
        Param { name: "largeMinTime", kind: Kind::Float, used: true, doc: "s between big jolts, at least (1)" },
        Param { name: "largeMaxTime", kind: Kind::Float, used: true, doc: "at most (2)" },
        Param { name: "movingMult", kind: Kind::Float, used: true, doc: "jolts times this while he moves (1)" },
        Param { name: "balancingMult", kind: Kind::Float, used: true, doc: "while he stands still (1)" },
        Param { name: "airborneMult", kind: Kind::Float, used: true, doc: "off the ground (1)" },
        Param { name: "movingThresh", kind: Kind::Float, used: true, doc: "m/s above which he's moving (1)" },
        Param { name: "stunInterval", kind: Kind::Float, used: true, doc: "s between jolts (0.07)" },
        Param { name: "directionRandomness", kind: Kind::Float, used: true, doc: "0: each jolt reverses the last, 1: all random (0.5)" },
        Param { name: "hipType", kind: Kind::Int, used: true, doc: "0: no pelvis jolt, 1: the pelvis jolts too (0)" },
        Param { name: "leftLeg", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "rightLeg", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "leftArm", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "rightArm", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "spine", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "neck", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "phasedLegs", kind: Kind::Bool, used: true, doc: "the legs take turns (false)" },
        Param { name: "applyStiffness", kind: Kind::Bool, used: true, doc: "stiff (12) while shocked (true)" },
        Param { name: "useTorques", kind: Kind::Bool, used: true, doc: "twist the parts, not push them (true)" },
    ] },
    Message { name: "hardConstraint", params: &[
        Param { name: "partIndex", kind: Kind::Str, used: true, doc: "the parts held, e.g. 16,20 (none)" },
        Param { name: "constrainOrientation", kind: Kind::Str, used: true, doc: "per part 1/0: hold its turn (all 1)" },
        Param { name: "constrainTranslation", kind: Kind::Str, used: true, doc: "per part 1/0: hold its place (all 1)" },
        Param { name: "blendWithPreviousFrame", kind: Kind::Str, used: true, doc: "per part 1/0: half last frame's target (all 0)" },
        Param { name: "applyTeleporation", kind: Kind::Bool, used: true, doc: "put the parts on their targets at the start (false)" },
        Param { name: "stabiliseHead", kind: Kind::Bool, used: true, doc: "hold the head's turn too (false)" },
        Param { name: "telportedVelOnDeactivation", kind: Kind::Bool, used: true, doc: "let go moving as the target was (false)" },
    ] },
    Message { name: "injury", params: &[
        Param { name: "part", kind: Kind::Int, used: true, doc: "the part hit: its arm or leg is hurt (a trunk or head part: none) (-1)" },
        Param { name: "mask", kind: Kind::Str, used: true, doc: "or these joints, e.g. ur (none)" },
        Param { name: "amount", kind: Kind::Float, used: true, doc: "how hurt, 0..1; 0 heals (0.5)" },
    ] },
];

fn injury(p: &mut P) -> Result<(Vec<usize>, f32), Error> {
    let (part, mask, amount) = (p.i("part", -1)?, p.s("mask", "")?, p.f("amount", 0.5)?.clamp(0.0, 1.0));
    use crate::nmrs::Character;
    let joints = if !mask.is_empty() {
        Character::mask(&mask)
    } else {
        ["ul", "ur", "ll", "lr"].iter().map(|m| Character::mask(m)).find(|js| part >= 1 && js.contains(&(part as usize - 1))).unwrap_or_default()
    };
    Ok((joints, amount))
}

pub trait Extra: Send + Sync {
    fn tick(&mut self, c: &mut Ctx, dt: f32);
    fn lines(&mut self) -> Vec<String> {
        vec![]
    }
    fn done(&self) -> bool {
        false
    }
    fn constraints(&self) -> Vec<(usize, Option<bevy_math::Vec3>, Option<bevy_math::Quat>)> {
        vec![]
    }
    fn fire(&mut self) {}
    fn let_go(&mut self, _hand: usize) {}
    fn objects(&mut self, _obs: &[crate::ragdoll::Obstacle]) {}
    fn stop(&mut self, _c: &mut Ctx) {}
}

#[derive(Default)]
pub struct Extras {
    pub on: Vec<(&'static str, Box<dyn Extra>)>,
    pub outbox: Vec<String>,
    pub hard: Option<HardConstraint>,
    release: Vec<usize>,
    want: Vec<(usize, Option<bevy_math::Vec3>, Option<bevy_math::Quat>)>,
    wanted: Vec<usize>,
    stopping: Vec<Box<dyn Extra>>,
    injure: Vec<(Vec<usize>, f32)>,
}

impl Extras {
    pub fn send(&mut self, msg: &str, params: &[(&str, Value)]) -> Option<Result<usize, Error>> {
        let name = messages().find(|m| m.name == msg)?.name;
        let mut p = P { params, used: vec![] };
        if name == "injury" {
            return Some(injury(&mut p).and_then(|i| p.done().map(|()| self.injure.push(i))).map(|()| 0));
        }
        let start = match p.b("start", false) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        if !start && !params.iter().any(|(n, _)| *n == "start") {
            if params.iter().any(|(n, v)| *n == "fireWeapon" && *v == Value::Bool(true)) {
                self.on.iter_mut().filter(|(n, _)| *n == name).for_each(|(_, e)| e.fire());
            }
            if let Some(h) = params.iter().find(|(n, _)| *n == "letGo").and_then(|(_, v)| v.i().ok()) {
                self.on.iter_mut().filter(|(n, _)| *n == name).for_each(|(_, e)| e.let_go(h.clamp(0, 1) as usize));
            }
            return Some(Ok(0));
        }
        if name == "hardConstraint" {
            let was = self.hard.take();
            if !start {
                self.hard = was.map(|h| HardConstraint { stop: true, ..h });
                return Some(Ok(0));
            }
            self.release.extend(was.iter().flat_map(HardConstraint::parts));
            return Some(HardConstraint::new(&mut p).and_then(|h| p.done().map(|()| self.hard = Some(h))).map(|()| 0));
        }
        self.take_off(|n, _| n == name);
        if !start {
            return Some(Ok(0));
        }
        let e: Result<Box<dyn Extra>, Error> = match name {
            "electrocute" => Electrocute::new(&mut p).map(|e| Box::new(e) as _),
            _ => match crate::extras_fall::build(name, &mut p) {
                Some(e) => e,
                None => crate::extras_held::build(name, &mut p).expect("every extra's message builds"),
            },
        };
        Some(e.and_then(|e| p.done().map(|()| self.on.push((name, e)))).map(|()| 0))
    }

    pub fn tick(&mut self, c: &mut Ctx, obs: &[crate::ragdoll::Obstacle], dt: f32) {
        for (joints, amount) in self.injure.drain(..) {
            for j in joints {
                c.ch.eff[j].injury = amount;
            }
        }
        for mut e in self.stopping.drain(..) {
            e.stop(c);
            self.outbox.extend(e.lines());
        }
        self.want.clear();
        for (_, e) in &mut self.on {
            e.objects(obs);
            e.tick(c, dt);
            self.outbox.extend(e.lines());
            self.want.extend(e.constraints());
        }
        self.take_off(|_, e| e.done());
    }

    fn take_off(&mut self, off: impl Fn(&str, &dyn Extra) -> bool) {
        let (gone, kept) = std::mem::take(&mut self.on).into_iter().partition(|(n, e)| off(n, e.as_ref()));
        self.on = kept;
        self.stopping.extend(gone.into_iter().map(|(_, e): (&str, Box<dyn Extra>)| e));
    }
}

impl Extras {
    pub fn stop_all(&mut self) {
        let held: Vec<usize> = self.wanted.iter().copied().chain(self.hard.iter().flat_map(HardConstraint::parts)).collect();
        *self = Default::default();
        self.release = held;
    }

    pub fn hard_step(&mut self, doll: &mut crate::ragdoll::Ragdoll, incoming: &[Transform], dt: f32) {
        for k in self.release.drain(..) {
            doll.constrain(k, None, None);
        }
        for &k in self.wanted.iter().filter(|&&k| !self.want.iter().any(|w| w.0 == k)) {
            doll.constrain(k, None, None);
        }
        for &(k, at, turn) in &self.want {
            doll.constrain(k, at, turn);
        }
        self.wanted = self.want.iter().map(|w| w.0).collect();
        let Some(h) = self.hard.as_mut() else { return };
        if h.stop {
            for k in h.parts() {
                doll.constrain(k, None, None);
            }
            if h.carry {
                let frames: Vec<Transform> = doll.frames().collect();
                for s in &h.slots {
                    let (Some(t0), Some(t1)) = (h.prev[s.part], h.last[s.part]) else { continue };
                    let w = (t1.rotation * t0.rotation.inverse()).to_scaled_axis() / dt;
                    let f = frames[s.part];
                    doll.set_body(s.part, f.translation, f.rotation, (t1.translation - t0.translation) / dt, w);
                }
            }
            self.hard = None;
            return;
        }
        let frames: Vec<Transform> = doll.frames().collect();
        let mut held: Vec<(usize, bool, bool, bool)> = h.slots.iter().map(|s| (s.part, s.orient, s.trans, s.blend)).collect();
        if h.head && !held.iter().any(|s| s.0 == HEAD) {
            held.push((HEAD, true, false, false));
        }
        for (k, orient, trans, blend) in held {
            let mut t = incoming.get(k).copied().or(h.last[k]).unwrap_or(frames[k]);
            if let (true, Some(p)) = (blend, h.last[k]) {
                t = Transform { translation: p.translation.lerp(t.translation, 0.5), rotation: p.rotation.slerp(t.rotation, 0.5), ..t };
            }
            if h.first && h.teleport {
                doll.set_body(k, t.translation, t.rotation, bevy_math::Vec3::ZERO, bevy_math::Vec3::ZERO);
            }
            doll.constrain(k, trans.then_some(t.translation), orient.then_some(t.rotation));
            (h.prev[k], h.last[k]) = (h.last[k].or(Some(t)), Some(t));
        }
        h.first = false;
    }
}

const HEAD: usize = 12;

pub struct HardConstraint {
    slots: Vec<Slot>,
    teleport: bool,
    head: bool,
    carry: bool,
    first: bool,
    stop: bool,
    prev: [Option<Transform>; 21],
    last: [Option<Transform>; 21],
}

struct Slot {
    part: usize,
    orient: bool,
    trans: bool,
    blend: bool,
}

impl HardConstraint {
    fn new(p: &mut P) -> Result<HardConstraint, Error> {
        let list = |s: String| -> Result<Vec<i32>, Error> {
            s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(|x| x.parse().map_err(|_| Error::Param)).collect()
        };
        let parts = list(p.s("partIndex", "")?)?;
        let (o, t, b) = (list(p.s("constrainOrientation", "")?)?, list(p.s("constrainTranslation", "")?)?, list(p.s("blendWithPreviousFrame", "")?)?);
        if parts.iter().any(|&k| !(0..21).contains(&k)) {
            return Err(Error::Param);
        }
        let slots = parts.iter().enumerate().map(|(i, &k)| Slot { part: k as usize, orient: o.get(i).is_none_or(|&x| x != 0),
                                                                  trans: t.get(i).is_none_or(|&x| x != 0), blend: b.get(i).is_some_and(|&x| x != 0) }).collect();
        Ok(HardConstraint { slots, teleport: p.b("applyTeleporation", false)?, head: p.b("stabiliseHead", false)?,
                            carry: p.b("telportedVelOnDeactivation", false)?, first: true, stop: false, prev: [None; 21], last: [None; 21] })
    }

    fn parts(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots.iter().map(|s| s.part).chain(self.head.then_some(HEAD))
    }
}

pub struct P<'a, 'b> {
    params: &'a [(&'b str, Value)],
    used: Vec<&'b str>,
}

impl<'b> P<'_, 'b> {
    fn get(&mut self, name: &str) -> Option<&Value> {
        let (n, v) = self.params.iter().find(|(n, _)| *n == name)?;
        self.used.push(n);
        Some(v)
    }
    pub fn f(&mut self, name: &str, d: f32) -> Result<f32, Error> {
        self.get(name).map_or(Ok(d), Value::f)
    }
    pub fn b(&mut self, name: &str, d: bool) -> Result<bool, Error> {
        self.get(name).map_or(Ok(d), Value::b)
    }
    pub fn s(&mut self, name: &str, d: &'static str) -> Result<String, Error> {
        self.get(name).map_or(Ok(d.to_string()), |v| v.s().map(str::to_string))
    }
    pub fn v(&mut self, name: &str, d: bevy_math::Vec3) -> Result<bevy_math::Vec3, Error> {
        self.get(name).map_or(Ok(d), Value::v)
    }
    pub fn i(&mut self, name: &str, d: i32) -> Result<i32, Error> {
        self.get(name).map_or(Ok(d), Value::i)
    }
    fn done(&self) -> Result<(), Error> {
        match self.params.iter().all(|(n, _)| self.used.contains(n)) {
            true => Ok(()),
            false => Err(Error::Param),
        }
    }
}

pub struct Electrocute {
    mag: f32,
    initial: f32,
    large: (f32, f32, f32),
    mult: [f32; 3],
    moving: f32,
    interval: f32,
    random: f32,
    parts: Vec<usize>,
    phased: bool,
    stiff: bool,
    torques: bool,
    last: [bevy_math::Vec3; 21],
    t: f32,
    next_large: f32,
    jolts: u32,
}

impl Electrocute {
    fn new(p: &mut P) -> Result<Electrocute, Error> {
        let (mag, initial) = (p.f("stunMag", 0.25)?, p.f("initialMult", 1.0)?);
        let large = (p.f("largeMult", 1.0)?, p.f("largeMinTime", 1.0)?, p.f("largeMaxTime", 2.0)?);
        let mult = [p.f("movingMult", 1.0)?, p.f("balancingMult", 1.0)?, p.f("airborneMult", 1.0)?];
        let (moving, interval, random) = (p.f("movingThresh", 1.0)?, p.f("stunInterval", 0.07)?, p.f("directionRandomness", 0.5)?);
        let mut parts = vec![];
        for (on, ks) in [(p.i("hipType", 0)? != 0, 0..1), (p.b("leftLeg", true)?, 1..4), (p.b("rightLeg", true)?, 4..7),
                         (p.b("spine", true)?, 7..11), (p.b("neck", true)?, 11..13), (p.b("leftArm", true)?, 13..17),
                         (p.b("rightArm", true)?, 17..21)] {
            if on {
                parts.extend(ks);
            }
        }
        Ok(Electrocute { mag, initial, large, mult, moving, interval, random, parts, phased: p.b("phasedLegs", false)?,
                         stiff: p.b("applyStiffness", true)?, torques: p.b("useTorques", true)?, last: [bevy_math::Vec3::X; 21],
                         t: 0.0, next_large: large.1, jolts: 0 })
    }
}

impl Extra for Electrocute {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.stiff {
            c.ch.set_stiffness(12.0, 1.0, "fb", None);
        }
        self.next_large -= dt;
        self.t -= dt;
        if self.t > 0.0 {
            return;
        }
        self.t += self.interval.max(dt);
        self.jolts += 1;
        let state = if !c.b.doll.collided(&[3, 6]) { 2 } else if (c.vel * bevy_math::Vec3::new(1.0, 1.0, 0.0)).length() > self.moving { 0 } else { 1 };
        let mut m = self.mag * self.mult[state];
        if self.jolts <= 3 {
            m *= self.initial;
        }
        if self.next_large < 0.0 {
            m *= self.large.0;
            self.next_large = self.large.1 + (self.large.2 - self.large.1) * c.ch.random();
        }
        for &k in &self.parts {
            if self.phased && (1..7).contains(&k) && ((k < 4) == (self.jolts % 2 == 0)) {
                continue;
            }
            let r = bevy_math::Vec3::new(c.ch.random(), c.ch.random(), c.ch.random()) * 2.0 - 1.0;
            let d = (-self.last[k]).lerp(r.normalize_or(bevy_math::Vec3::Z), self.random).normalize_or(bevy_math::Vec3::Z);
            self.last[k] = d;
            let mass = c.b.doll.part_mass(k);
            match self.torques {
                true => c.out.torque(k, d * (mass * 0.01 * m * 80.0 / dt)),
                false => c.out.force(k, d * (mass * m * 4.0 / dt)),
            }
        }
    }
}

