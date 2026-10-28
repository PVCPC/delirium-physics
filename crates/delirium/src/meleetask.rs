use crate::nm::Hit;
use crate::prelude::*;
use crate::shottask::Msg;
use crate::msg::Value;
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Debug)]
pub enum MeleeTask {
    Balance { set: String, started: bool },
    Flinch { set: String, started: bool, lean_until: u32 },
}

impl MeleeTask {
    pub fn update(&mut self, rand: &mut Rand, tp: &TaskParams, hit: &Hit, now: u32, failed: bool) -> Vec<Msg> {
        let first = match self {
            MeleeTask::Balance { started, .. } | MeleeTask::Flinch { started, .. } => !std::mem::replace(started, true),
        };
        let mut out = if first { vec![("stopAllBehaviours", vec![])] } else { vec![] };
        match self {
            MeleeTask::Balance { set, .. } if first => out.extend(balance_start(tp, set, hit)),
            MeleeTask::Balance { .. } => out.extend(balance_control(hit)),
            MeleeTask::Flinch { set, lean_until, .. } => {
                if first {
                    let (msgs, until) = flinch_start(rand, tp, set, hit, now);
                    out.extend(msgs);
                    *lean_until = until;
                }
                out.extend(flinch_control(tp, set, lean_until, hit, now, failed));
            }
        }
        out
    }
}

pub fn balance_start(tp: &TaskParams, set: &str, hit: &Hit) -> Vec<Msg> {
    use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
    let nb = |n: &str, or: f32| tp.get("NMBalance", set, n, or);
    let v3 = |n: &str, or: Vec3| {
        let v = tp.0.get("NMBalance").and_then(|t| t.get(set).and_then(|s| s.get(n)).or_else(|| t.get("*").and_then(|s| s.get(n))));
        v.filter(|v| v.len() == delirium_data::kv!(0 int)).map_or(or, |v| Vec3::new(v[0], v[1], v[2]))
    };
    let mut p = vec![("start", B(true)), ("bodyStiffness", F(nb("balanceStiffness", delirium_data::kv!(127)))), ("damping", F(nb("balanceDamping", delirium_data::kv!(70)))),
                     ("elbow", F(nb("balanceElbow", delirium_data::kv!(13)))), ("shoulder", F(nb("balanceShoulder", 1.0))),
                     ("somersaultAngle", F(nb("balanceSomersaultAngle", 1.0))), ("somersaultAngleThreshold", F(nb("balanceSomersaultAngleThreshold", delirium_data::kv!(17)))),
                     ("sideSomersaultAngle", F(nb("balanceSideSomersaultAngle", 1.0))),
                     ("sideSomersaultAngleThreshold", F(nb("balanceSideSomersaultAngleThreshold", delirium_data::kv!(17)))),
                     ("returningToBalanceArmsOut", F(nb("balanceReturningToBalanceArmsOut", 0.0))),
                     ("angVelMultiplier", V(v3("balanceAngVelMultiplier", Vec3::new(delirium_data::kv!(104), 1.0, delirium_data::kv!(104))))),
                     ("angVelThreshold", V(v3("balanceAngVelThreshold", Vec3::new(delirium_data::kv!(74), delirium_data::kv!(5), delirium_data::kv!(74))))),
                     ("elbowAngleOnContact", F(nb("balanceElbowAngleOnContact", delirium_data::kv!(178)))), ("bendElbowsTime", F(nb("balanceBendElbowsTime", delirium_data::kv!(24)))),
                     ("bendElbowsGait", F(nb("balanceBendElbowsGait", delirium_data::kv!(70)))), ("headLookAtVelProb", F(nb("balanceHeadLookAtVelProb", -1.0))),
                     ("turnOffProb", F(nb("balanceTurnOffProb", delirium_data::kv!(73)))), ("turn2VelProb", F(nb("balanceTurn2VelProb", delirium_data::kv!(24)))),
                     ("turnAwayProb", F(nb("balanceTurnAwayProb", delirium_data::kv!(9)))), ("turnLeftProb", F(nb("balanceTurnLeftProb", delirium_data::kv!(179)))),
                     ("turnRightProb", F(nb("balanceTurnRightProb", delirium_data::kv!(179)))), ("turn2TargetProb", F(nb("balanceTurn2TargetProb", delirium_data::kv!(57))))];
    if let Some(head) = hit.attacker_head {
        if nb("balanceArmsOutOnPush", 1.0) != 0.0 {
            p.extend([("armsOutOnPush", B(true)), ("armsOutStraightenElbows", F(nb("balanceArmsOutStraightenElbows", 0.0))),
                      ("armsOutMinLean2", F(nb("balanceArmsOutMinLean2", -delirium_data::kv!(180)))), ("armsOutOnPushMultiplier", F(nb("balanceArmsOutOnPushMultiplier", 1.0))),
                      ("armsOutOnPushTimeout", F(nb("balanceArmsOutOnPushTimeout", delirium_data::kv!(128))))]);
        }
        p.extend([("useHeadLook", B(true)), ("headLookPos", V(head)), ("headLookAlwaysTimeout", F(nb("balanceHeadLookTimeout", delirium_data::kv!(27))))]);
    }
    let look = hit.attacker_head.map(|h| (h, true)).or(hit.look);
    if let Some((at, turn)) = look {
        if hit.attacker_head.is_none() {
            p.extend([("useHeadLook", B(true)), ("headLookPos", V(at)), ("headLookAlwaysTimeout", F(nb("balanceHeadLookTimeout", delirium_data::kv!(27))))]);
        }
        if turn && nb("balanceUseBodyTurn", 1.0) != 0.0 {
            p.extend([("useBodyTurn", B(true)), ("bodyTurnTimeout", F(nb("balanceBodyTurnTimeout", 1.0)))]);
        }
    }
    if hit.few_steps {
        p.push(("maxSteps", I(delirium_data::kv!(1 int))));
    }
    let clamp = if hit.few_steps { delirium_data::kv!(24) } else { nb("balanceStepClampScale", 1.0) };
    vec![("bodyBalance", p), ("configureBalance", vec![("stepClampScale", F(clamp))])]
}

pub fn balance_control(hit: &Hit) -> Vec<Msg> {
    hit.attacker_head.or(hit.attacker_at).map_or(vec![], |at| vec![("bodyBalance", vec![("useHeadLook", Value::Bool(true)), ("headLookPos", Value::Vec3(at))])])
}

pub fn balance_lean(amount: &mut f32, dir: Vec3, left: &mut f32, holding: bool, fwd: Vec3, dt: f32) -> Option<Msg> {
    use crate::Vd;
    if *amount <= 0.0 {
        return None;
    }
    let stop = holding || (*left > 0.0 && {
        *left -= dt;
        *left <= 0.0
    });
    if stop {
        *amount = 0.0;
        return Some(("leanInDirection", vec![("start", Value::Bool(false))]));
    }
    let d = dir.with_z(0.0);
    let d = if d.x.mul_add(d.x, d.y * d.y) > delirium_data::kv!(48) { d * crate::estimate::vrsqrtefp(d.vd(d)) } else { fwd };
    Some(("leanInDirection", vec![("start", Value::Bool(true)), ("applyAsForce", Value::Bool(true)), ("dir", Value::Vec3(d)),
        ("leanAmount", Value::Float(*amount))]))
}

pub fn grab_pedal(tp: &TaskParams, set: &str, pedalling: &mut bool, holding: bool, vel: Vec3) -> Option<Msg> {
    use Value::{Bool as B, Float as F};
    let nb = |n: &str| tp.get("NMBalance", set, n, 0.0);
    if !holding {
        return std::mem::take(pedalling).then(|| ("pedalLegs", vec![("start", B(false))]));
    }
    let mut p = if *pedalling { vec![] } else { vec![("start", B(true))] };
    *pedalling = true;
    let max = nb("balanceGrabPedalSpeedMax");
    let speed = if nb("balanceGrabPedalScaleWithLocalVelocity") != 0.0 {
        let s = vel.x.mul_add(vel.x, vel.y.mul_add(vel.y, vel.z * vel.z)).sqrt() * nb("balanceGrabPedalScaleWithLocalVelocityMultiplier");
        if s.max(0.0) >= max { max } else { s.max(0.0) }
    } else {
        max
    };
    let drag = nb("angSpeedMultiplier4Dragging");
    p.extend([("legStiffness", F(nb("balanceGrabPedalStiffness"))), ("radius", F(nb("balanceGrabPedalRadius"))), ("angularSpeed", F(speed)),
        ("pedalOffset", F(nb("balanceGrabPedalOffset"))), ("speedAsymmetry", F(nb("balanceGrabPedalAsymmetry"))),
        ("adaptivePedal4Dragging", B(drag >= 0.0)), ("angSpeedMultiplier4Dragging", F(drag)), ("backPedal", B(false))]);
    Some(("pedalLegs", p))
}

pub fn flinch_start(rand: &mut Rand, tp: &TaskParams, set: &str, hit: &Hit, now: u32) -> (Vec<Msg>, u32) {
    use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
    let f = |n: &str, or: f32| tp.get("NMFlinch", set, n, or);
    let at = hit.attacker_neck.unwrap_or(hit.pos);
    let left = rand.next() < delirium_data::kv!(46 int);
    let mut out = vec![("upperBodyFlinch", vec![("start", B(true)), ("pos", V(at)), ("bodyStiffness", F(f("bodyStiffness", delirium_data::kv!(21)))),
                                                  ("bodyDamping", F(f("damping", 1.0))), ("backBendAmount", F(f("backBendAmount", -delirium_data::kv!(123)))),
                                                  ("leftHanded", B(left)), ("rightHanded", B(!left)),
                                                  ("handDistanceVertical", F(f("handDistanceVertical", delirium_data::kv!(9)))),
                                                  ("handDistanceLeftRight", F(f("handDistanceLeftRight", delirium_data::kv!(64)))),
                                                  ("handDistanceFrontBack", F(f("handDistanceFrontBack", delirium_data::kv!(9)))), ("noiseScale", F(f("noiseScale", delirium_data::kv!(73)))),
                                                  ("turnTowards", I(f("turnTowards", 1.0) as i32))])];
    let mut bb = vec![("start", B(true))];
    if set == "EXPLOSION" {
        bb.push(("maxSteps", I(rand.int_in_range(f("flinchMaxStepsL", delirium_data::kv!(5)) as i32, f("flinchMaxStepsH", delirium_data::kv!(61)) as i32))));
    }
    out.push(("bodyBalance", bb));
    let mut until = 0;
    if f("leanAwayFromPos", 0.0) != 0.0 {
        out.push(("leanInDirection", vec![("start", B(true)), ("applyAsForce", B(false)), ("dir", V(crate::shottask::flat(hit.pos - at, hit.fwd))),
                                          ("leanAmount", F(f("leanAwayAmount", delirium_data::kv!(57))))]));
        until = (rand.int_in_range(f("leanMaxTimeL", delirium_data::kv!(78)) as i32, f("leanMaxTimeH", delirium_data::kv!(69)) as i32) as u32).wrapping_add(now);
    }
    (out, until)
}

pub fn flinch_control(tp: &TaskParams, set: &str, lean_until: &mut u32, hit: &Hit, now: u32, failed: bool) -> Vec<Msg> {
    let mut out = vec![];
    if let Some(at) = hit.attacker_neck {
        out.push(("upperBodyFlinch", vec![("pos", Value::Vec3(at))]));
    }
    if !failed && tp.get("NMFlinch", set, "leanAwayFromPos", 0.0) != 0.0 && *lean_until != 0 && now > *lean_until {
        out.push(("leanInDirection", vec![("start", Value::Bool(false))]));
        *lean_until = 0;
    }
    out
}

pub fn flinch_fails(failed: &mut bool, name: &str) -> Vec<Msg> {
    if *failed || !crate::nm::task_fails(crate::nm::Task::Flinch, name) {
        return vec![];
    }
    *failed = true;
    vec![("upperBodyFlinch", vec![("start", Value::Bool(false))])]
}

pub fn flinch_point(vec: Vec3, attacker: Option<(Vec3, Option<Vec3>)>) -> Vec3 {
    match attacker {
        None => vec,
        Some((place, neck)) => neck.unwrap_or(place),
    }
}

