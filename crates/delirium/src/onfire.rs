use crate::falldown::Step;
use crate::msg::Value;
use crate::nm::Next;
use crate::prelude::*;
use crate::shottask::Msg;
use crate::{Fma, Vd};
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Debug, Default)]
pub struct Ped {
    pub side: Vec3,
    pub fwd: Vec3,
    pub vel: Vec3,
    pub health: f32,
    pub dying: bool,
    pub burning: bool,
    pub no_lie: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub f: [f32; 24],
    pub rethink: u32,
    pub variance: f32,
}

const NAMES: [&str; 24] = ["onFireStumbleLeanAmount",
    "writheStandingArmStiffness", "writheStandingArmDamping", "writheStandingArmPeriod", "writheStandingArmAmplitude",
    "writheStandingElbowAmplitude", "writheStandingBackStiffness", "writheStandingBackDamping", "writheStandingBackPeriod",
    "writheStandingBackAmplitude",
    "writheLyingArmStiffness", "writheLyingArmDamping", "writheLyingArmPeriod", "writheLyingArmAmplitude",
    "writheLyingElbowAmplitude", "writheLyingBackStiffness", "writheLyingBackDamping", "writheLyingBackPeriod",
    "writheLyingBackAmplitude", "writheLyingLegStiffness", "writheLyingLegDamping", "writheLyingLegPeriod",
    "writheLyingLegAmplitude", "writheLyingKneeAmplitude"];

const WRITHE: [&str; 14] = ["armStiffness", "armDamping", "armPeriod", "armAmplitude", "elbowAmplitude", "backStiffness",
    "backDamping", "backPeriod", "backAmplitude", "legStiffness", "legDamping", "legPeriod", "legAmplitude", "kneeAmplitude"];

impl Set {
    pub fn read(tp: &TaskParams, set: &str) -> Set {
        let mut f = [0.0; 24];
        for (x, n) in f.iter_mut().zip(NAMES) {
            *x = tp.get("NMOnFire", set, n, 0.0);
        }
        Set { f, rethink: tp.get("NMOnFire", set, "timePeriodBetweenDirectionRethink", 0.0) as u32,
              variance: tp.get("NMOnFire", set, "rangeOfDirectionVariance", 0.0) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OnFire {
    min: u32,
    max: u32,
    pub started: Option<u32>,
    pub flags: u32,
    lean: f32,
    dir: Vec3,
    free: bool,
    pub lying: bool,
    rethink: u32,
    roll: u32,
}

fn text(s: &str) -> Value {
    Value::Str(s.into())
}

fn heading(p: &Ped) -> Vec3 {
    let d = p.fwd.with_z(0.0);
    let d = if d.x.fma(d.x, d.y * d.y) > 0.01 { d } else { p.side.with_z(0.0) };
    d * crate::estimate::vrsqrtefp(d.vd(d))
}

impl OnFire {
    pub fn new(min: u32, max: u32, lean: f32, dir: Option<Vec3>) -> Self {
        OnFire { min, max, started: None, flags: 0, lean, dir: dir.unwrap_or(Vec3::ZERO), free: dir.is_none(), lying: false,
                 rethink: 0, roll: 0 }
    }

    pub fn feedback(&mut self, name: &str) {
        if name.starts_with("balance") {
            (self.flags, self.lying) = (self.flags | 2, true);
        }
    }

    pub fn update(&mut self, p: &Ped, set: &Set, rand: &mut Rand, now: u32) -> Step {
        let mut s = Step::default();
        let Some(t0) = self.started else {
            self.started = Some(now);
            s.msgs.push(("stopAllBehaviours", vec![]));
            self.start(p, set, rand, now, &mut s.msgs);
            return s;
        };
        self.control(p, set, rand, &mut s.msgs);
        (s.done, s.next) = self.finish(p, set, rand, now, t0, &mut s.msgs);
        s
    }

    fn start(&mut self, p: &Ped, set: &Set, rand: &mut Rand, now: u32, m: &mut Vec<Msg>) {
        m.push(("bodyBalance", vec![("start", Value::Bool(true))]));
        let mut w = vec![("start", Value::Bool(true))];
        w.extend(WRITHE[..9].iter().zip(&set.f[1..10]).map(|(k, x)| (*k, Value::Float(*x))));
        w.push(("mask", text("ub")));
        m.push(("bodyWrithe", w));
        self.flags |= 1;
        if self.free {
            self.rethink = now.wrapping_add(set.rethink);
        }
        self.control(p, set, rand, m);
    }

    fn control(&mut self, p: &Ped, set: &Set, rand: &mut Rand, m: &mut Vec<Msg>) {
        use Value::{Bool as B, Float as F};
        if self.flags & 2 != 0 {
            self.flags &= !3;
            m.push(("bodyBalance", vec![("start", B(false))]));
            let mut w = vec![("start", B(true))];
            w.extend(WRITHE.iter().zip(&set.f[10..24]).map(|(k, x)| (*k, F(*x))));
            w.push(("mask", text("fb")));
            w.push(("rollOverFlag", B(rand.next() & 3 != 0)));
            m.push(("bodyWrithe", w));
        } else if !(self.lean < 0.0) && self.flags & 1 != 0 {
            if self.lean == 0.0 {
                self.lean = set.f[0];
            }
            if self.dir == Vec3::ZERO {
                self.dir = heading(p);
            }
            m.push(("leanInDirection", vec![("start", B(true)), ("applyAsForce", B(false)), ("dir", Value::Vec3(self.dir)),
                                            ("leanAmount", F(self.lean))]));
        }
    }

    fn finish(&mut self, p: &Ped, set: &Set, rand: &mut Rand, now: u32, t0: u32, m: &mut Vec<Msg>) -> (bool, Option<Next>) {
        use Value::{Bool as B, Float as F};
        let relax = (true, Some(Next::Relax));
        let dead = !(p.health > 0.0) && p.dying;
        let past_min = now > t0.wrapping_add(self.min);
        if dead || !p.burning && past_min {
            return if dead || self.flags & 2 != 0 { relax } else { (true, Some(Next::Blend)) };
        }
        let v = p.vel;
        if past_min && v.x.fma(v.x, v.y.fma(v.y, v.z * v.z)) < 0.2f32 * 0.2 && !p.no_lie {
            if self.flags == 1 {
                (self.flags, self.lying) = (3, true);
            } else if !p.burning || p.health < 20.0 {
                return relax;
            }
        }
        if self.flags & 2 != 0 {
            if self.max != 0 && now > t0.wrapping_add(self.max) {
                return relax;
            }
        } else if self.max != 0 && now > t0.wrapping_add(self.max - self.max / 5) {
            (self.flags, self.lying) = (self.flags | 2, true);
        }
        if self.free && self.rethink != 0 && self.flags & 1 != 0 && now > self.rethink {
            self.rethink = set.rethink.wrapping_add(now);
            let half = set.variance * 0.017453292 * 0.5;
            let d = heading(p);
            let a = rand.float_in_range(0.0 - half, half);
            let (sin, cos) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
            self.dir = Vec3::new(d.x.fma(cos, -(d.y * sin)), d.y.fma(cos, d.x * sin), d.z);
            m.push(("leanInDirection", vec![("dir", Value::Vec3(self.dir))]));
        }
        if self.roll == 0 {
            if self.lying && rand.int_in_range(0, 32767) < 500 {
                self.roll = (rand.int_in_range(500, 1500) as u32).wrapping_add(now);
                m.push(("bodyWrithe", vec![("mask", text("lb"))]));
                m.push(("bodyRollUp", vec![("start", B(true)), ("mask", text("ub")), ("stiffness", F(13.0)), ("armReachAmount", F(1.5)),
                                           ("useArmToSlowDown", F(0.0))]));
            }
        } else if now > self.roll {
            m.push(("bodyWrithe", vec![("mask", text("fb"))]));
            m.push(("bodyRollUp", vec![("start", B(false))]));
            self.roll = 0;
        }
        (false, None)
    }
}

