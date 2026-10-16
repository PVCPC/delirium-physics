use crate::msg::Value;
use crate::prelude::*;
use crate::shottask::Msg;
use crate::Vd;
use delirium_data::taskparams::TaskParams;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Set {
    pub push: [f32; 4],
    pub roll_after: u32,
    pub roll: [f32; 12],
    pub flags: [bool; 2],
}

const PUSH: [&str; 4] = ["velocityMultiplier", "velocityMaxMag", "initialUpwardsLift", "catchFallBodyStiffness"];
const ROLL: [&str; 12] = ["rollUpStiffness", "rollUpDamping", "rollUpForceMag", "rollUpAsymmetry", "rollUpArmsToSlow", "rollUpLegPush",
    "rollUpArmReachLength", "rollUpArmReachAmount", "rollUpDecayTime", "rollUpDecayTarget", "rollUpAsymmetricalLegs", "zAxisSpinReduction"];

impl Set {
    pub fn read(tp: &TaskParams) -> Set {
        let f = |n| tp.get("NMJumpRollFromRoadVehicle", "*", n, 0.0);
        Set { push: PUSH.map(f), roll_after: f("timeTillRollUp") as u32, roll: ROLL.map(f),
              flags: [f("rollUpUseZeroPose") != 0.0, f("rollUpSpinInAir") != 0.0] }
    }
}

pub fn start(set: &Set) -> Vec<Msg> {
    use Value::{Bool as B, Float as F};
    let (k, spin) = (set.push[3], set.roll[11]);
    vec![("catchFall", vec![("start", B(true)), ("legsStiffness", F(k)), ("armsStiffness", F(k)), ("torsoStiffness", F(k)),
                            ("zAxisSpinReduction", F(spin))])]
}

pub fn roll_up(set: &Set, v0: Vec3) -> Vec<Msg> {
    use Value::{Bool as B, Float as F};
    let r = &set.roll;
    let mut rds = vec![("start", B(true))];
    rds.extend([(0, "stiffness"), (1, "damping"), (3, "asymmetry"), (2, "forcemag"), (4, "m_useArmToSlowDown")].map(|(i, k)| (k, F(r[i]))));
    rds.extend([("useZeroPose", B(set.flags[0])), ("spinWhenInAir", B(set.flags[1]))]);
    rds.extend(["m_legPush", "armReachLength", "m_armReachAmount", "stiffnessDecayTime", "stiffnessDecayTarget", "asymmetricalLegs",
                "zAxisSpinReduction"].into_iter().zip(&r[5..]).map(|(k, x)| (k, F(*x))));
    let d = v0.with_z(0.0);
    if d.x.mul_add(d.x, d.y * d.y) > 0.01 {
        rds.extend([("useCustomRollDir", B(true)), ("customRollDir", Value::Vec3(d * crate::estimate::vrsqrtefp(d.vd(d))))]);
    }
    vec![("catchFall", vec![("start", B(false))]), ("rollDownStairs", rds),
         ("pedalLegs", vec![("start", B(true)), ("adaptivePedal4Dragging", B(true)), ("angSpeedMultiplier4Dragging", F(0.6)),
                            ("backPedal", B(false)), ("legStiffness", F(8.0)), ("radius", F(0.4)), ("pedalOffset", F(0.1)),
                            ("speedAsymmetry", F(0.5)), ("angularSpeed", F(5.0))])]
}

pub fn push(set: &Set, v: Vec3, v0: Vec3, rolled: bool) -> Option<Vec3> {
    if v0 == Vec3::ZERO || v.x.mul_add(v.x, v.y.mul_add(v.y, v.z * v.z)) <= 25.0 {
        return None;
    }
    let a = (v.max(Vec3::ZERO).min(v0) * crate::gjk::recip(set.push[1])).min(Vec3::ONE) * set.push[0];
    Some(a.with_z(if rolled { 0.0 } else { set.push[2] }))
}

