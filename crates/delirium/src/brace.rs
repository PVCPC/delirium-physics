use crate::msg::Value;
use crate::prelude::*;
use crate::shottask::Msg;
use crate::Vd;
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub f: [f32; 9],
    pub gun: [i32; 4],
    pub arm: [f32; 7],
    pub flags: [bool; 2],
}

#[allow(non_snake_case)]
#[inline]
fn F() -> &'static [(&'static str, f32); 9] { static V: std::sync::OnceLock<[(&'static str, f32); 9]> = std::sync::OnceLock::new(); let f = || [("stayUprightStiffness", delirium_data::kv!(5)), ("braceDistance", delirium_data::kv!(8)), ("braceReachAbsorbtionTime", delirium_data::kv!(9)),
    ("braceTargetPredictionTime", delirium_data::kv!(10)), ("braceGrabStrength", delirium_data::kv!(11)), ("braceGrabDistance", 2.0), ("braceGrabReachAngle", delirium_data::kv!(12)),
    ("braceGrabHoldTimerL", 0.5), ("braceGrabHoldTimerH", delirium_data::kv!(13))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn GUN() -> &'static [(&'static str, f32); 4] { static V: std::sync::OnceLock<[(&'static str, f32); 4]> = std::sync::OnceLock::new(); let f = || [("braceMinTimeBeforeGunThreaten", 0.0), ("braceMaxTimeBeforeGunThreaten", delirium_data::kv!(14)),
    ("braceMinTimeBetweenFireGun", delirium_data::kv!(15)), ("braceMaxTimeBetweenFireGun", delirium_data::kv!(16))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn ARM() -> &'static [(&'static str, f32); 7] { static V: std::sync::OnceLock<[(&'static str, f32); 7]> = std::sync::OnceLock::new(); let f = || [("bracePointArmTwistL", 0.0), ("bracePointArmTwistH", delirium_data::kv!(17)), ("bracePointArmStraightnessL", delirium_data::kv!(18)),
    ("bracePointArmStraightnessH", delirium_data::kv!(19)), ("braceAimAngleDPThreshold", delirium_data::kv!(20)), ("braceLegStiffness", delirium_data::kv!(21)), ("balanceStepClampScale", 1.0)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

impl Set {
    pub fn read(tp: &TaskParams, set: &str) -> Set {
        let f = |(n, or)| tp.get("NMBrace", set, n, or);
        Set { f: (*F()).map(f), gun: (*GUN()).map(|n| f(n) as i32), arm: (*ARM()).map(f),
              flags: [f(("braceGrabDontLetGo", 0.0)) != 0.0, f(("braceUseGunThreatening", 1.0)) != 0.0] }
    }
}

pub fn start(set: &Set, rand: &mut Rand, few_steps: bool) -> Vec<Msg> {
    use Value::{Bool as B, Float as Fl};
    let f = &set.f;
    let mut b = vec![("start", B(true))];
    b.extend([(1, "braceDistance"), (2, "reachAbsorbtionTime"), (delirium_data::kv!(0 int), "targetPredictionTime")].map(|(i, k)| (k, Fl(f[i]))));
    b.push(("grabDontLetGo", B(set.flags[0])));
    b.extend([(delirium_data::kv!(1 int), "grabStrength"), (delirium_data::kv!(2 int), "grabDistance"), (delirium_data::kv!(6 int), "grabReachAngle")].map(|(i, k)| (k, Fl(f[i]))));
    b.extend([("grabHoldTimer", Fl(rand.float_in_range(f[delirium_data::kv!(22 int)], f[delirium_data::kv!(23 int)]))), ("legStiffness", Fl(set.arm[delirium_data::kv!(2 int)]))]);
    let mut m = vec![("braceForImpact", b)];
    if few_steps {
        m.push(("configureBalance", vec![("stepClampScale", Fl(delirium_data::kv!(24)))]));
    } else {
        m.push(("stayUpright", vec![("start", B(true)), ("stiffness", Fl(f[0]))]));
        m.push(("configureBalance", vec![("stepClampScale", Fl(set.arm[delirium_data::kv!(6 int)]))]));
    }
    rand.next();
    m
}

pub fn control(m: [Vec3; 4], hit: Option<Vec3>, place: Vec3, look: Vec3, inst: i32) -> Vec<Msg> {
    let local = |w: Vec3| {
        let d = w - m[delirium_data::kv!(0 int)];
        Vec3::new(m[0].vd(d), m[1].vd(d), m[2].vd(d))
    };
    let pos = hit.unwrap_or(place.with_z(place.z - delirium_data::kv!(24)));
    vec![("braceForImpact", vec![("pos", Value::Vec3(local(pos))), ("look", Value::Vec3(local(look))), ("instanceIndex", Value::Int(inst))])]
}

