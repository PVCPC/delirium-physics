use crate::collision::Collision;
use crate::msg::{self, Value};
use crate::nm::{Agent, Hit};
use crate::prelude::*;
use crate::ragdoll::Build;
use crate::{Fma, Vd};
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;

pub type Msg = (&'static str, Vec<(&'static str, Value)>);

#[derive(Clone, Debug)]
pub struct ShotTask {
    pub part: usize,
    pub set: String,
    at: Vec3,
    vel: Vec3,
    normal: Vec3,
    pub started: Option<u32>,
    pub(crate) lean_until: u32,
    pub(crate) blind_until: u32,
    blind_next: u32,
    pub(crate) balance: bool,
    pub(crate) look: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Step {
    pub msgs: Vec<Msg>,
    pub fire: bool,
    pub done: bool,
}

impl ShotTask {
    pub fn new(hit: &Hit, set: &str, part: usize, at: Vec3, vel: Vec3, normal: Vec3) -> Self {
        ShotTask { part, set: set.into(), at: hit.root.map_or(Vec3::ZERO, |r| at - r), vel, normal, started: None, lean_until: 0,
                   blind_until: 0, blind_next: 0, balance: false, look: false }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn next_hit(&mut self, rand: &mut Rand, tp: &TaskParams, hit: &Hit, now: u32, set: &str, part: usize, at: Vec3, vel: Vec3,
                    normal: Vec3) -> Vec<Msg> {
        *self = ShotTask { started: Some(now), ..ShotTask::new(hit, set, part, at, vel, normal) };
        self.start(rand, tp, hit, now, false)
    }

    pub fn update(&mut self, rand: &mut Rand, tp: &TaskParams, hit: &Hit, now: u32) -> Step {
        let Some(t0) = self.started else {
            self.started = Some(now);
            let mut msgs = vec![("stopAllBehaviours", vec![])];
            msgs.extend(self.start(rand, tp, hit, now, true));
            return Step { msgs, fire: false, done: false };
        };
        let (msgs, fire) = self.control(hit, now);
        let v = hit.vel;
        let max = if v.x.fma(v.x, v.y.fma(v.y, v.z * v.z)) < 4.0 || hit.pos.z < 0.0 { 10000 } else { 100000 };
        Step { msgs, fire, done: now > t0.wrapping_add(max) }
    }

    fn start(&mut self, rand: &mut Rand, tp: &TaskParams, hit: &Hit, now: u32, first: bool) -> Vec<Msg> {
        use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        crate::bump::pain_line(hit.health, if hit.dying { 1 } else { 0 }, -1, true, rand);
        (self.lean_until, self.blind_until, self.blind_next, self.balance, self.look) = (0, 0, 0, false, false);
        let v = hit.vel;
        if hit.moving && v.z.fma(v.z, v.x.fma(v.x, v.y * v.y)).sqrt() > 1.0 {
            self.balance = true;
            let m = |n: &str, or: f32| tp.get("NMShot_IsMoving", "*", n, or);
            return vec![
                ("bodyBalance", vec![("start", B(true)), ("bodyStiffness", F(m("balanceBodyStiffness", 12.0))), ("damping", F(m("balanceBodyDamping", 0.9))),
                                     ("elbow", F(m("balanceElbow", 1.5))), ("shoulder", F(m("balanceShoulder", 1.5))),
                                     ("maxSteps", I(m("balanceMaxSteps", 3.0) as i32))]),
                ("configureBalance", vec![("balanceAbortThreshold", F(m("balanceAbortThreshold", 1.0)))]),
                ("leanInDirection", vec![("start", B(true)), ("applyAsForce", B(m("balanceLeanDirApplyAsForce", 0.0) != 0.0)),
                                         ("dir", V(flat(v, hit.fwd))), ("leanAmount", F(m("balanceLeanAmount", 0.4)))]),
            ];
        }
        let st = |n: &str, or: f32| tp.get("NMShot_Standing", &self.set, n, or);
        let melee = self.set.starts_with("MELEE");
        let mut p = if first { vec![("start", B(true))] } else { vec![] };
        p.extend([("bodyPart", I(self.part as i32)), ("hitPoint", V(hit.root.unwrap_or(hit.pos) + self.at)), ("bulletVel", V(self.vel)),
                  ("normal", V(self.normal))]);
        if let Some(look) = hit.attacker_head {
            p.push(("headLook", V(look)));
            self.look = true;
        }
        p.push(("grabHoldTime", F(st("shotGrabTime", 3.0))));
        if melee {
            p.extend([("useHeadLook", B(true)), ("melee", B(true))]);
        }
        p.extend([("crouching", B(false)), ("reachForWound", B(!melee)), ("fling", B(false)), ("chickenArms", B(false)),
                  ("maxSteps", I(if hit.few_steps { 4 } else { st("shotMaxSteps", 6.0) as i32 })), ("loosenessAmount", F(st("shotLooseness", 1.0))),
                  ("bodyStiffness", F(st("shotBodyStiffness", 13.5))), ("stableHandsAndNeck", B(st("shotStablizeHandsAndNeck", 0.0) != 0.0))]);
        let mut lh = |n: &str, l: f32, h: f32| rand.float_in_range(st(&format!("{n}L"), l), st(&format!("{n}H"), h));
        p.push(("timeBeforeReachForWound", F(lh("shotTimeBeforeReachForWound", 0.1, 0.2))));
        p.push(("timeBeforeCollapseWoundLeg", F(st("shotTimeBeforeCollapseWoundLeg", 1.0))));
        let pain = [lh("shotSpinePainTime", 0.5, 1.5), lh("shotSpinePainMultiplier", 1.0, 2.0), lh("shotSpinePainTwistMultiplier", 0.5, 1.5)];
        p.extend([("spinePainTime", F(pain[0])), ("spinePainMultiplier", F(pain[1])), ("spinePainTwistMultiplier", F(pain[2])),
                  ("useCatchFallOnFall", B(hit.health >= 100.0)), ("headLookAtWoundMinTimer", F(st("shotHeadLookAtWoundMinTimer", 0.1))),
                  ("headLookAtWoundMaxTimer", F(st("shotHeadLookAtWoundMaxTimer", 0.75))),
                  ("headLookAtHeadPosMinTimer", F(st("shotHeadLookAtShooterMinTimer", 0.75))),
                  ("headLookAtHeadPosMaxTimer", F(st("shotHeadLookAtShooterMaxTimer", 1.5))), ("addShockSpin", B(st("addShockSpin", 1.0) != 0.0)),
                  ("randomizeShockSpinDirection", B(st("randomizeShockSpinDirection", 0.0) != 0.0)),
                  ("alwaysAddShockSpin", B(st("alwaysAddShockSpin", 0.0) != 0.0)), ("shockSpinMin", F(st("shockSpinMin", 40.0))),
                  ("shockSpinMax", F(st("shockSpinMax", 80.0))), ("shockSpinLiftForceMult", F(st("shockSpinLiftForceMult", -1.0))),
                  ("shockSpinDecayMult", F(st("shockSpinDecayMult", 4.0))), ("shockSpinScalePerComponent", F(st("shockSpinScalePerComponent", 0.5)))]);
        let mut out = vec![("shot", p)];
        if hit.health >= 100.0 {
            out.push(("stayUpright", vec![("start", B(true)), ("stiffness", F(st("shotUprightStiffness", 3.0)))]));
        } else {
            let fatal = hit.health < 50.0;
            out.push(("shotRelax", vec![("relaxPeriod", F(st(if fatal { "shotRelaxPeriod_FatallyInjured" } else { "shotRelaxPeriod_Injured" },
                                                                 if fatal { 0.5 } else { 1.5 })))]));
        }
        if hit.gun && ((st("shotBlindFireProbability", 0.1) * 100.0) as i32) >= (rand.next() & 100) as i32 {
            let ms = rand.int_in_range(st("shotMaxBlindFireTimeL", 400.0) as i32, st("shotMaxBlindFireTimeH", 1000.0) as i32);
            (self.blind_until, self.blind_next) = ((ms as u32).wrapping_add(now), now);
        }
        let mut amount = st("shotLeanAmount", 0.3);
        if amount > 0.0 {
            let d = flat(Vec3::ZERO - self.normal, hit.fwd);
            let ms = rand.int_in_range(st("shotMaxTimeToStopLeanL", 600.0) as i32, st("shotMaxTimeToStopLeanH", 1200.0) as i32);
            self.lean_until = (ms as u32).wrapping_add(now);
            if hit.running {
                self.lean_until = self.lean_until.wrapping_add(st("additionalLeanTimeWhenRunning", 1000.0) as i32 as u32);
                amount *= st("balanceLeanAmountScaleWhenRunning", 1.5);
            }
            out.push(("leanInDirection", vec![("start", B(true)),
                                              ("applyAsForce", B(tp.get("NMShot_Standing", "*", "balanceLeanDirApplyAsForce", 1.0) != 0.0)),
                                              ("dir", V(flat(hit.vel + d, d))), ("leanAmount", F(amount))]));
        }
        out
    }

    fn control(&mut self, hit: &Hit, now: u32) -> (Vec<Msg>, bool) {
        use Value::{Bool as B, Vec3 as V};
        let mut out = vec![];
        if let (true, Some(look)) = (self.look, hit.attacker_head) {
            out.push(if self.balance {
                ("bodyBalance", vec![("useHeadLook", B(true)), ("headLookPos", V(look))])
            } else {
                ("shot", vec![("useHeadLook", B(true)), ("headLook", V(look))])
            });
        }
        if self.lean_until != 0 && now > self.lean_until {
            out.push(("leanInDirection", vec![("start", B(false))]));
            self.lean_until = 0;
        }
        let mut fire = false;
        if self.blind_until != 0 {
            if now >= self.blind_until {
                (self.blind_until, self.blind_next) = (0, 0);
            } else if now >= self.blind_next && hit.health >= 100.0 && hit.gun && !hit.weapon_busy {
                (fire, self.blind_next) = (true, now.wrapping_add(hit.fire_period));
            }
        }
        (out, fire)
    }
}

pub(crate) fn flat(v: Vec3, or: Vec3) -> Vec3 {
    let v = v.with_z(0.0);
    if v.x.fma(v.x, v.y * v.y) > 0.01 { v * crate::estimate::vrsqrtefp(v.vd(v)) } else { or }
}

pub fn bullet(hit: Vec3, from: Vec3, force: f32) -> Vec3 {
    let mut d = crate::rigid::nrm(hit - from);
    let l = d.x.fma(d.x, d.y * d.y).sqrt();
    if l > 0.05 {
        let s = 1.0 / l;
        (d.x, d.y) = (s * d.x, s * d.y);
    }
    d * force
}

pub fn apply(a: &mut Agent, build: &Build, world: &Collision, st: &mut msg::State, msgs: &[Msg]) {
    for (name, ps) in msgs {
        if let Some(p) = look_only(ps) {
            if *name == "shot" { a.shot_look(p) } else { a.body_balance_look(p) }
            continue;
        }
        let Some(m) = msg::find(name) else { continue };
        let ps: Vec<_> = ps.iter().filter(|(k, _)| *k == "start" || m.params.iter().any(|p| p.name == *k)).cloned().collect();
        let _ = msg::send(a, build, world, st, name, &ps);
    }
}

fn look_only(ps: &[(&'static str, Value)]) -> Option<Vec3> {
    let look = ps.iter().find_map(|(k, v)| match (*k, v) { ("headLook" | "headLookPos", Value::Vec3(p)) => Some(*p), _ => None });
    look.filter(|_| ps.iter().all(|(k, _)| matches!(*k, "useHeadLook" | "headLook" | "headLookPos")))
}

pub fn text((name, ps): &Msg) -> String {
    ps.iter().fold(name.to_string(), |s, (k, v)| match v {
        Value::Float(x) => format!("{s} {k}={x}"),
        Value::Int(i) => format!("{s} {k}={i}"),
        Value::Bool(b) => format!("{s} {k}={b}"),
        Value::Vec3(v) => format!("{s} {k}={},{},{}", v.x, v.y, v.z),
        Value::Str(t) => format!("{s} {k}={t}"),
    })
}

