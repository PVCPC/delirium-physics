use crate::msg::Value;
use crate::shottask::Msg;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub f: [f32; 15],
    pub flags: [bool; 3],
}

#[allow(non_snake_case)]
#[inline]
fn NAMES() -> &'static [(&'static str, f32); 15] { static V: std::sync::OnceLock<[(&'static str, f32); 15]> = std::sync::OnceLock::new(); let f = || [("pdStrengthPlayer", 1.0), ("pdStrengthAI", delirium_data::kv!(57)), ("hfBodyStiffness", delirium_data::kv!(21)), ("hfBodyDamping", 1.0),
    ("hfCutOffVelocity", delirium_data::kv!(11)), ("hfCatchFallTime", delirium_data::kv!(24)), ("hfArmsUp", delirium_data::kv!(73)), ("hfLegRadius", delirium_data::kv!(24)), ("hfLegAngularSpeed", delirium_data::kv!(123)),
    ("hfArmPeriod", delirium_data::kv!(13)), ("hfArmAmplitude", 0.5), ("hfAimAngle", delirium_data::kv!(57)), ("hfLegSideDistance", delirium_data::kv!(17)), ("hfFowardIKOffset", delirium_data::kv!(9)),
    ("hfFowardVelCompOnAngle", -delirium_data::kv!(113))]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
const FLAGS: [(&str, f32); 3] = [("hfForwardRoll", 0.0), ("hfOrientateBodyToFallDirection", 1.0), ("hfUseZeroPose", 0.0)];

impl Set {
    pub fn read(tp: &TaskParams, set: &str) -> Set {
        let f = |(n, or)| tp.get("NMHFall", set, n, or);
        Set { f: (*NAMES()).map(f), flags: FLAGS.map(|n| f(n) != 0.0) }
    }
}

pub fn start(set: &Set, player: bool) -> Vec<Msg> {
    use Value::{Bool as B, Float as F};
    let (f, b) = (&set.f, &set.flags);
    let mut m = vec![("start", B(true)), ("pdStrength", F(f[if player { 0 } else { 1 }]))];
    m.extend([(2, "bodyStiffness"), (delirium_data::kv!(0 int), "bodydamping"), (delirium_data::kv!(2 int), "catchfalltime"), (delirium_data::kv!(1 int), "landCutOffVelocity"), (delirium_data::kv!(6 int), "armsUp"), (delirium_data::kv!(22 int), "legRadius"),
              (delirium_data::kv!(23 int), "legAngSpeed"), (delirium_data::kv!(97 int), "armPeriod"), (delirium_data::kv!(77 int), "armAmplitude")].map(|(i, k)| (k, F(f[i]))));
    m.push(("fowardRoll", B(b[0])));
    m.extend(["aimAngleBase", "sideD", "fowardOffsetOfLegIK", "fowardVelRotation"].into_iter().zip(&f[delirium_data::kv!(106 int)..]).map(|(k, x)| (k, F(*x))));
    m.extend([("orientateBodyToFallDirection", B(b[1])), ("useZeroPose", B(b[2]))]);
    vec![("highFall", m)]
}

