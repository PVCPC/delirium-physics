use crate::msg::Value;
use crate::shottask::Msg;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub f: [f32; 15],
    pub flags: [bool; 3],
}

const NAMES: [(&str, f32); 15] = [("pdStrengthPlayer", 1.0), ("pdStrengthAI", 0.2), ("hfBodyStiffness", 12.0), ("hfBodyDamping", 1.0),
    ("hfCutOffVelocity", 40.0), ("hfCatchFallTime", 0.3), ("hfArmsUp", 0.1), ("hfLegRadius", 0.3), ("hfLegAngularSpeed", 3.5),
    ("hfArmPeriod", 1.75), ("hfArmAmplitude", 0.5), ("hfAimAngle", 0.2), ("hfLegSideDistance", 0.25), ("hfFowardIKOffset", 0.15),
    ("hfFowardVelCompOnAngle", -0.02)];
const FLAGS: [(&str, f32); 3] = [("hfForwardRoll", 0.0), ("hfOrientateBodyToFallDirection", 1.0), ("hfUseZeroPose", 0.0)];

impl Set {
    pub fn read(tp: &TaskParams, set: &str) -> Set {
        let f = |(n, or)| tp.get("NMHFall", set, n, or);
        Set { f: NAMES.map(f), flags: FLAGS.map(|n| f(n) != 0.0) }
    }
}

pub fn start(set: &Set, player: bool) -> Vec<Msg> {
    use Value::{Bool as B, Float as F};
    let (f, b) = (&set.f, &set.flags);
    let mut m = vec![("start", B(true)), ("pdStrength", F(f[if player { 0 } else { 1 }]))];
    m.extend([(2, "bodyStiffness"), (3, "bodydamping"), (5, "catchfalltime"), (4, "landCutOffVelocity"), (6, "armsUp"), (7, "legRadius"),
              (8, "legAngSpeed"), (9, "armPeriod"), (10, "armAmplitude")].map(|(i, k)| (k, F(f[i]))));
    m.push(("fowardRoll", B(b[0])));
    m.extend(["aimAngleBase", "sideD", "fowardOffsetOfLegIK", "fowardVelRotation"].into_iter().zip(&f[11..]).map(|(k, x)| (k, F(*x))));
    m.extend([("orientateBodyToFallDirection", B(b[1])), ("useZeroPose", B(b[2]))]);
    vec![("highFall", m)]
}

