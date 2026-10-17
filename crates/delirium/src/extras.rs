use crate::msg::{Error, Kind, Message, Param, Value};
use crate::nmshot::Ctx;

pub fn messages() -> impl Iterator<Item = &'static Message> {
    MESSAGES.iter().chain(&crate::extras_fall::MESSAGES).chain(&crate::extras_held::MESSAGES)
}

pub static MESSAGES: [Message; 1] = [
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
];

pub trait Extra {
    fn tick(&mut self, c: &mut Ctx, dt: f32);
    fn lines(&mut self) -> Vec<String> {
        vec![]
    }
    fn done(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct Extras {
    pub on: Vec<(&'static str, Box<dyn Extra>)>,
    pub outbox: Vec<String>,
}

impl Extras {
    pub fn send(&mut self, msg: &str, params: &[(&str, Value)]) -> Option<Result<usize, Error>> {
        let name = messages().find(|m| m.name == msg)?.name;
        let mut p = P { params, used: vec![] };
        let start = match p.b("start", false) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        if !start && !params.iter().any(|(n, _)| *n == "start") {
            return Some(Ok(0));
        }
        self.on.retain(|(n, _)| *n != name);
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

    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        for (_, e) in &mut self.on {
            e.tick(c, dt);
            self.outbox.extend(e.lines());
        }
        self.on.retain(|(_, e)| !e.done());
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

