#![allow(clippy::all)]
use crate::collision::Collision;
use crate::nm::Agent;
use crate::prelude::*;
use crate::ragdoll::Build;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float,
    Int,
    Bool,
    Vec3,
    Str,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Int(i32),
    Bool(bool),
    Vec3(Vec3),
    Str(String),
}

impl Value {
    pub(crate) fn f(&self) -> Result<f32, Error> {
        match self {
            Value::Float(x) => Ok(*x),
            Value::Int(i) => Ok(*i as f32),
            _ => Err(Error::Type),
        }
    }
    pub(crate) fn i(&self) -> Result<i32, Error> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Float(x) if x.fract() == 0.0 => Ok(*x as i32),
            Value::Bool(b) => Ok(*b as i32),
            _ => Err(Error::Type),
        }
    }
    pub(crate) fn b(&self) -> Result<bool, Error> {
        match self {
            Value::Bool(b) => Ok(*b),
            Value::Int(i) => Ok(*i != 0),
            _ => Err(Error::Type),
        }
    }
    pub(crate) fn v(&self) -> Result<Vec3, Error> {
        match self {
            Value::Vec3(v) => Ok(*v),
            _ => Err(Error::Type),
        }
    }
    pub(crate) fn s(&self) -> Result<&str, Error> {
        match self {
            Value::Str(s) => Ok(s),
            _ => Err(Error::Type),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Message,
    Param,
    Type,
}

pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    pub used: bool,
    pub doc: &'static str,
}

pub struct Message {
    pub name: &'static str,
    pub params: &'static [Param],
}

pub static MESSAGES: [Message; 30] = [
    Message { name: "activePose", params: &[
        Param { name: "mask", kind: Kind::Str, used: true, doc: "" },
    ] },
    Message { name: "applyImpulse", params: &[
        Param { name: "partIndex", kind: Kind::Int, used: true, doc: "" },
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "impulse", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "equalizeAmount", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "armsWindmillAdaptive", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "amplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "phase", kind: Kind::Float, used: true, doc: "" },
        Param { name: "disableOnImpact", kind: Kind::Bool, used: true, doc: "+144 0x40: an arm stops once its hand hits something (init's true; the game never sends it)." },
        Param { name: "period", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "bodyBalance", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookAtVelProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armsOutStraightenElbows", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bendElbowsTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "elbow", kind: Kind::Float, used: true, doc: "" },
        Param { name: "angVelMultiplier", kind: Kind::Vec3, used: true, doc: "angVelMultiplier and angVelThreshold, x y z." },
        Param { name: "angVelThreshold", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "returningToBalanceArmsOut", kind: Kind::Float, used: true, doc: "" },
        Param { name: "somersaultAngleThreshold", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armsOutMinLean2", kind: Kind::Float, used: true, doc: "" },
        Param { name: "sideSomersaultAngleThreshold", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bendElbowsGait", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armsOutOnPush", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "elbowAngleOnContact", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bodyTurnTimeout", kind: Kind::Float, used: true, doc: "The upper body's (\"ub\") damping as the tick uses it." },
        Param { name: "turnOffProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "turn2VelProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookPos", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "sideSomersaultAngle", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookAlwaysTimeout", kind: Kind::Float, used: true, doc: "The upper body's (\"ub\") stiffness as the tick uses it." },
        Param { name: "turnAwayProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "shoulder", kind: Kind::Float, used: true, doc: "" },
        Param { name: "turnRightProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "turnLeftProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "somersaultAngle", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armsOutOnPushMultiplier", kind: Kind::Float, used: true, doc: "" },
        Param { name: "turn2TargetProb", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useBodyTurn", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "headLookInstanceIndex", kind: Kind::Int, used: true, doc: "" },
        Param { name: "armsOutOnPushTimeout", kind: Kind::Float, used: true, doc: "" },
        Param { name: "maxSteps", kind: Kind::Int, used: true, doc: "" },
    ] },
    Message { name: "bodyFoetal", params: &[
        Param { name: "mask", kind: Kind::Str, used: true, doc: "+80: the effectors it may drive (the rest locked while it runs)." },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "+68 stiffness, +72 damping, +64 asymmetry, +60 the back's random twist, +76 the seed." },
        Param { name: "asymmetry", kind: Kind::Float, used: true, doc: "" },
        Param { name: "randomSeed", kind: Kind::Int, used: true, doc: "" },
        Param { name: "dampingFactor", kind: Kind::Float, used: true, doc: "" },
        Param { name: "backTwist", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "bodyRelax", params: &[
        Param { name: "mask", kind: Kind::Str, used: true, doc: "" },
        Param { name: "holdPose", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "relaxation", kind: Kind::Float, used: true, doc: "" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "bodyRollUp", params: &[
        Param { name: "mask", kind: Kind::Str, used: true, doc: "" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legPush", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armReachAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useArmToSlowDown", kind: Kind::Float, used: true, doc: "" },
        Param { name: "asymmetricalLegs", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "bodyWrithe", params: &[
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legPeriod", kind: Kind::Float, used: true, doc: "" },
        Param { name: "elbowAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "kneeAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "backDamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "backPeriod", kind: Kind::Float, used: true, doc: "" },
        Param { name: "rollOverFlag", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "backAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "backStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armDamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legDamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "mask", kind: Kind::Str, used: true, doc: "" },
        Param { name: "armAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armPeriod", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "braceForImpact", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "+20 bodyStiffness, +24 pos, +36 instanceIndex, +40 braceDistance, +44 targetPredictionTime, +48 reachAbsorbtionTime, +52 look, +64 grabStrength, +68 grabDistance, +72 grabReachAngle, +76 grabHoldTimer, +80 the car's top speed to grab (hash unnamed), +84 how long it braces at a target behind it (hash unnamed), +88 grabDontLetGo, +92 legStiffness." },
        Param { name: "look", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "grabReachAngle", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabHoldTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "braceDistance", kind: Kind::Float, used: true, doc: "" },
        Param { name: "targetPredictionTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabDistance", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "reachAbsorbtionTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabDontLetGo", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabStrength", kind: Kind::Float, used: true, doc: "" },
        Param { name: "instanceIndex", kind: Kind::Int, used: true, doc: "" },
        Param { name: "hash_4f7c0f0b", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "catchFall", params: &[
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "armsStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "torsoStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legsStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "forwardMaxArmOffset", kind: Kind::Float, used: true, doc: "" },
        Param { name: "zAxisSpinReduction", kind: Kind::Float, used: true, doc: "" },
        Param { name: "backwardsMinArmOffset", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "configureBalance", params: &[
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "stepHeight", kind: Kind::Float, used: true, doc: "" },
        Param { name: "stepClampScale", kind: Kind::Float, used: true, doc: "" },
        Param { name: "balanceAbortThreshold", kind: Kind::Float, used: true, doc: "" },
        Param { name: "taperKneeStrength", kind: Kind::Bool, used: true, doc: "DBM+0xBFC bit 0x10: on from init; the game never sends it, but highFall clears it for good." },
    ] },
    Message { name: "configureBalanceReset", params: &[
    ] },
    Message { name: "defineAttachedObject", params: &[
        Param { name: "partIndex", kind: Kind::Int, used: true, doc: "" },
        Param { name: "worldPos", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "objectMass", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "fallOverWall", params: &[
        Param { name: "bodyTwist", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useSpecifiedPoint", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "maxDistanceFromPelToHitPoint", kind: Kind::Float, used: true, doc: "" },
        Param { name: "fallOverEdge", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "fallOverPoint", kind: Kind::Vec3, used: true, doc: "+32 fallOverPoint, +48 fallOverEdge, +64 damping, +68 bodyStiffness, +72 magOfForce, +76 the time it pushes (1, no message field), +80 how far it tips before it's done (-0.2, none), +84 maxDistanceFromPelToHitPoint, +88 / +92 the twist's range (bodyTwist x 45 / x 85, clamped 0..1), +96 useSpecifiedPoint." },
        Param { name: "magOfForce", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "grab", params: &[
        Param { name: "instanceIndex", kind: Kind::Int, used: true, doc: "the obstacle (a step's car box, by its id) the points, normals and grips are in the frame of; -1 the world" },
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabStrength", kind: Kind::Float, used: true, doc: "+132 grabStrength, +136 armStiffness, +140 grabDist, +144 reachAngle, +148 oneSideReachAngle, +152 bodyStiffness, +156 pullUpTime, +160 / +164 pullUpStrengthLeft / Right, +168 grabHoldMaxTimer; +0x124 reachDistance (hash 0x34629c68): how far the arm reaches out when moving slowly; +0x128 the arm's stiffness while held (4, no message field)." },
        Param { name: "useLeft", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "normalR", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "pos3", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "useRight", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "pos1", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "pos2", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "pos4", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "surfaceGrab", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "oneSideReachAngle", kind: Kind::Float, used: true, doc: "" },
        Param { name: "normalL", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "instancePartIndex", kind: Kind::Int, used: true, doc: "+300: from 1, that child of the obstacle's composite (its frame, ) instead of the obstacle's" },
        Param { name: "linegrab", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "grabDist", kind: Kind::Float, used: true, doc: "" },
        Param { name: "dontLetGo", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "justbrace", kind: Kind::Bool, used: true, doc: "+20 justbrace, +21 linegrab, +22 surfaceGrab, +23 useRight, +24 useLeft, +25 dontLetGo." },
        Param { name: "reachAngle", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pullUpStrengthRight", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pullUpTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pullUpStrengthLeft", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabHoldMaxTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "reachDistance", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "headLook", params: &[
        Param { name: "instanceIndex", kind: Kind::Int, used: false, doc: "a game object index (the SDK has no level objects: positions are world coordinates)" },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "alwaysEyesHorizontal", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "alwaysLook", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "vel", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "highFall", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "+20 bodyStiffness, +24 bodydamping, +28 catchfalltime, +32 crashOrLandCutOff (landCutOffVelocity lands here too), +36 pdStrength, +40 catchFallCutOff, +44 armsUp, +48 legRadius, +52 legAngSpeed, +56 armPeriod, +60 armAmplitude, +64 aimAngleBase, +68 fowardVelRotation, +72 footVelCompScale, +76 legL, +80 sideD, +84 fowardOffsetOfLegIK, +88 fowardVelCutOff, +92 legStrength, +96 orientateBodyToFallDirection, +97 fowardRoll, +98 the zero pose rolling (hash 0xf411b4e7, unnamed), +99 ignorWorldCollisions. (useZeroPose is in the handler's table but stored nowhere.)" },
        Param { name: "armAmplitude", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armPeriod", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pdStrength", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useZeroPose", kind: Kind::Bool, used: false, doc: "the game handler stores it nowhere" },
        Param { name: "armsUp", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legRadius", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bodydamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legAngSpeed", kind: Kind::Float, used: true, doc: "" },
        Param { name: "crashOrLandCutOff", kind: Kind::Float, used: true, doc: "" },
        Param { name: "fowardOffsetOfLegIK", kind: Kind::Float, used: true, doc: "" },
        Param { name: "catchfalltime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "landCutOffVelocity", kind: Kind::Float, used: true, doc: "" },
        Param { name: "ignorWorldCollisions", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "fowardRoll", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "orientateBodyToFallDirection", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "aimAngleBase", kind: Kind::Float, used: true, doc: "" },
        Param { name: "fowardVelRotation", kind: Kind::Float, used: true, doc: "" },
        Param { name: "sideD", kind: Kind::Float, used: true, doc: "" },
        Param { name: "catchFallCutOff", kind: Kind::Float, used: true, doc: "" },
        Param { name: "footVelCompScale", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legL", kind: Kind::Float, used: true, doc: "" },
        Param { name: "fowardVelCutOff", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legStrength", kind: Kind::Float, used: true, doc: "" },
        Param { name: "zeroPoseRoll", kind: Kind::Bool, used: true, doc: "" },
    ] },
    Message { name: "incomingTransforms", params: &[
    ] },
    Message { name: "leanInDirection", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "applyAsForce", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "dir", kind: Kind::Vec3, used: true, doc: "" },
    ] },
    Message { name: "leanToPosition", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "applyAsForce", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "" },
    ] },
    Message { name: "leanTowardsObject", params: &[
        Param { name: "instanceIndex", kind: Kind::Int, used: false, doc: "a game object index (the SDK has no level objects: positions are world coordinates)" },
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "applyAsForce", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "offset", kind: Kind::Vec3, used: true, doc: "" },
    ] },
    Message { name: "pedalLegs", params: &[
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pedalRightLeg", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "speedAsymmetry", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pedalLeftLeg", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "radius", kind: Kind::Float, used: true, doc: "" },
        Param { name: "angularSpeed", kind: Kind::Float, used: true, doc: "" },
        Param { name: "pedalOffset", kind: Kind::Float, used: true, doc: "" },
        Param { name: "angSpeedMultiplier4Dragging", kind: Kind::Float, used: true, doc: "" },
        Param { name: "adaptivePedal4Dragging", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "backPedal", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "radiusVariance", kind: Kind::Float, used: true, doc: "" },
        Param { name: "centreUp", kind: Kind::Float, used: true, doc: "" },
        Param { name: "centreForwards", kind: Kind::Float, used: true, doc: "" },
        Param { name: "legAngleVariance", kind: Kind::Float, used: true, doc: "" },
        Param { name: "randomSeed", kind: Kind::Int, used: true, doc: "" },
    ] },
    Message { name: "pointArm", params: &[
        Param { name: "instanceIndex", kind: Kind::Int, used: true, doc: "" },
        Param { name: "useLeftArm", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "twist", kind: Kind::Float, used: true, doc: "" },
        Param { name: "target", kind: Kind::Vec3, used: true, doc: "+32 target, +48 twist, +52 armStraightness, +56 armStiffness, +60 armDamping, +64 instanceIndex, +68 pointSwingLimit, +72 useLeftArm, +73 useZeroPoseWhenNotPointing." },
        Param { name: "pointSwingLimit", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armStraightness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useZeroPoseWhenNotPointing", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "armDamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "rollDownStairs", params: &[
        Param { name: "forcemag", kind: Kind::Float, used: true, doc: "" },
        Param { name: "armReachLength", kind: Kind::Float, used: true, doc: "" },
        Param { name: "m_legPush", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useCustomRollDir", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "stiffnessDecayTarget", kind: Kind::Float, used: true, doc: "" },
        Param { name: "spinWhenInAir", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "m_armReachAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "customRollDir", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "stiffnessDecayTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "m_useArmToSlowDown", kind: Kind::Float, used: true, doc: "" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "The message's: +20 stiffness, +24 damping, +28 forcemag, +32 asymmetry, +36 asymmetricalLegs, +40 m_useArmToSlowDown, +44 m_armReachAmount, +48 useZeroPose, +49 spinWhenInAir, +52 m_legPush, +60 armReachLength, +64 stiffnessDecayTime, +68 stiffnessDecayTarget, +72 useCustomRollDir, +76 zAxisSpinReduction, +80 customRollDir." },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useZeroPose", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "zAxisSpinReduction", kind: Kind::Float, used: true, doc: "" },
        Param { name: "asymmetricalLegs", kind: Kind::Float, used: true, doc: "" },
        Param { name: "asymmetry", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "setStiffness", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "mask", kind: Kind::Str, used: true, doc: "" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "shot", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "grabHoldTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "" },
        Param { name: "loosenessAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "shockSpinDecayMult", kind: Kind::Float, used: true, doc: "" },
        Param { name: "timeBeforeReachForWound", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookAtWoundMaxTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "reachForWound", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "spinePainTwistMultiplier", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "shockSpinMin", kind: Kind::Float, used: true, doc: "" },
        Param { name: "stableHandsAndNeck", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "maxSteps", kind: Kind::Int, used: true, doc: "" },
        Param { name: "shockSpinMax", kind: Kind::Float, used: true, doc: "" },
        Param { name: "timeBeforeCollapseWoundLeg", kind: Kind::Float, used: true, doc: "" },
        Param { name: "melee", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "shockSpinScalePerComponent", kind: Kind::Float, used: true, doc: "" },
        Param { name: "randomizeShockSpinDirection", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "useCatchFallOnFall", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "crouching", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "headLookAtWoundMinTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "shockSpinLiftForceMult", kind: Kind::Float, used: true, doc: "" },
        Param { name: "chickenArms", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "fling", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "normal", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "spinePainTime", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookAtHeadPosMinTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "bulletVel", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "addShockSpin", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "spinePainMultiplier", kind: Kind::Float, used: true, doc: "" },
        Param { name: "headLookAtHeadPosMaxTimer", kind: Kind::Float, used: true, doc: "" },
        Param { name: "alwaysAddShockSpin", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "flingWidth", kind: Kind::Float, used: true, doc: "+0x64 flingWidth: fling's clavicle lift." },
        Param { name: "headLook", kind: Kind::Vec3, used: true, doc: "+16 headLook: the point useHeadLook looks at." },
    ] },
    Message { name: "shotRelax", params: &[
        Param { name: "relaxPeriod", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "stayUpright", params: &[
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "" },
        Param { name: "dampingScale", kind: Kind::Float, used: true, doc: "" },
    ] },
    Message { name: "stopAllBehaviours", params: &[
    ] },
    Message { name: "upperBodyFlinch", params: &[
        Param { name: "handDistanceFrontBack", kind: Kind::Float, used: true, doc: "" },
        Param { name: "noiseScale", kind: Kind::Float, used: true, doc: "" },
        Param { name: "handDistanceVertical", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useRightArm", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "handDistanceLeftRight", kind: Kind::Float, used: true, doc: "" },
        Param { name: "rightHanded", kind: Kind::Bool, used: true, doc: "" },
        Param { name: "turnTowards", kind: Kind::Int, used: true, doc: "" },
        Param { name: "backBendAmount", kind: Kind::Float, used: true, doc: "" },
        Param { name: "newHit", kind: Kind::Bool, used: true, doc: "+68: restart the flinch's timing; set only when sent (else the behaviour's own stands)." },
        Param { name: "leftHanded", kind: Kind::Bool, used: true, doc: "+32 / +33: stored, read by nothing sends one of them, a coin flip; both arms still flinch)." },
        Param { name: "bodyDamping", kind: Kind::Float, used: true, doc: "" },
        Param { name: "useLeftArm", kind: Kind::Bool, used: true, doc: "+61 / +60: the arms that flinch." },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "" },
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "" },
    ] },
];

#[derive(Clone, Debug)]
pub struct Impulse {
    pub part_index: i32,
    pub hit_point: Option<Vec3>,
    pub impulse: Vec3,
    pub equalize_amount: f32,
}
impl Default for Impulse {
    fn default() -> Impulse {
        Impulse { part_index: 0, hit_point: None, impulse: Vec3::new(0.0, -50.0, 0.0), equalize_amount: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct BodyRelax {
    pub mask: Option<String>,
    pub hold_pose: bool,
    pub relaxation: f32,
    pub damping: Option<f32>,
}
impl Default for BodyRelax {
    fn default() -> BodyRelax {
        BodyRelax { mask: None, hold_pose: false, relaxation: 50.0, damping: None }
    }
}

#[derive(Clone, Debug)]
pub struct Attached {
    pub part_index: i32,
    pub world_pos: Vec3,
    pub object_mass: f32,
}
impl Default for Attached {
    fn default() -> Attached {
        Attached { part_index: -1, world_pos: Vec3::ZERO, object_mass: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct HeadLookMsg {
    pub pos: Vec3,
    pub vel: Vec3,
    pub stiffness: f32,
    pub damping: f32,
    pub always_look: bool,
    pub always_eyes_horizontal: bool,
}
impl Default for HeadLookMsg {
    fn default() -> HeadLookMsg {
        HeadLookMsg { pos: Vec3::ZERO, vel: Vec3::ZERO, stiffness: 10.0, damping: 1.0, always_look: false, always_eyes_horizontal: true }
    }
}

#[derive(Clone, Debug)]
pub struct Lean {
    pub lean_amount: f32,
    pub apply_as_force: bool,
    pub dir: Vec3,
}
impl Default for Lean {
    fn default() -> Lean {
        Lean { lean_amount: 0.2, apply_as_force: false, dir: Vec3::Z }
    }
}

#[derive(Clone, Debug)]
pub struct LeanTo {
    pub lean_amount: f32,
    pub apply_as_force: bool,
    pub pos: Vec3,
}
impl Default for LeanTo {
    fn default() -> LeanTo {
        LeanTo { lean_amount: 0.2, apply_as_force: false, pos: Vec3::ZERO }
    }
}

#[derive(Clone, Debug)]
pub struct LeanTowards {
    pub lean_amount: f32,
    pub apply_as_force: bool,
    pub offset: Vec3,
}
impl Default for LeanTowards {
    fn default() -> LeanTowards {
        LeanTowards { lean_amount: 0.2, apply_as_force: false, offset: Vec3::ZERO }
    }
}

#[derive(Clone, Debug)]
pub struct Stiffness {
    pub body_stiffness: f32,
    pub damping: f32,
    pub mask: String,
}
impl Default for Stiffness {
    fn default() -> Stiffness {
        Stiffness { body_stiffness: 12.0, damping: 1.0, mask: "fb".into() }
    }
}

#[derive(Clone, Debug)]
pub struct ActivePose {
    pub mask: Option<String>,
}
impl Default for ActivePose {
    fn default() -> ActivePose {
        ActivePose { mask: None }
    }
}

#[derive(Clone, Debug)]
pub struct ShotRelax {
    pub relax_period: f32,
}
impl Default for ShotRelax {
    fn default() -> ShotRelax {
        ShotRelax { relax_period: 2.0 }
    }
}

#[derive(Clone, Debug)]
pub struct StayUpright {
    pub stiffness: f32,
    pub damping_scale: f32,
}
impl Default for StayUpright {
    fn default() -> StayUpright {
        StayUpright { stiffness: 3.0, damping_scale: 0.5 }
    }
}

#[derive(Clone, Default)]
pub struct State {
    pub active_pose: ActivePose,
    pub apply_impulse: Impulse,
    pub arms_windmill_adaptive: crate::nm::ArmsWindmill,
    pub body_balance: crate::nmbb::Params,
    pub body_foetal: crate::nmbf::Foetal,
    pub body_relax: BodyRelax,
    pub body_roll_up: crate::nm::BodyRollUp,
    pub body_writhe: crate::nm::BodyWrithe,
    pub brace_for_impact: crate::nmbr::BraceParams,
    pub catch_fall: crate::nmcf::Params,
    pub configure_balance: crate::nm::ConfigureBalance,
    pub define_attached_object: Attached,
    pub fall_over_wall: crate::nmfow::FallOverWallParams,
    pub grab: crate::nmgr::GrabParams,
    pub head_look: HeadLookMsg,
    pub high_fall: crate::nmhf::HighFallParams,
    pub lean_in_direction: Lean,
    pub lean_to_position: LeanTo,
    pub lean_towards_object: LeanTowards,
    pub pedal_legs: crate::nm::PedalLegs,
    pub point_arm: crate::nmpa::PointArmParams,
    pub roll_down_stairs: crate::nmrds::RollDownStairs,
    pub set_stiffness: Stiffness,
    pub shot: crate::nmshot::Params,
    pub shot_relax: ShotRelax,
    pub stay_upright: StayUpright,
    pub upper_body_flinch: crate::nm::UpperBodyFlinch,
}

fn intern(s: &str) -> &'static str {
    static SEEN: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap();
    if let Some(&k) = seen.iter().find(|k| **k == s) {
        return k;
    }
    let k: &'static str = Box::leak(s.into());
    seen.push(k);
    k
}

pub fn find(name: &str) -> Option<&'static Message> {
    MESSAGES.iter().chain(crate::extras::messages()).find(|m| m.name == name)
}

pub fn send(a: &mut Agent, build: &Build, world: &Collision, st: &mut State, msg: &str, params: &[(&str, Value)]) -> Result<usize, Error> {
    if let Some(r) = a.extras.send(msg, params) {
        return r;
    }
    let m = find(msg).ok_or(Error::Message)?;
    let start = params.iter().find(|(n, _)| *n == "start").map(|(_, v)| v.b()).transpose()?;
    let one_shot = matches!(msg, "activePose" | "applyImpulse" | "bodyRelax" | "defineAttachedObject" | "leanInDirection"
                                 | "leanToPosition" | "leanTowardsObject" | "setStiffness" | "shotRelax");
    if start == Some(true) && matches!(msg, "bodyBalance" | "shot" | "pointArm") {
        reset(st, msg);
    } else if !one_shot {
        a.held_params(st, msg);
    }
    let has = |n: &str| params.iter().any(|(p, _)| *p == n);
    let ang_vel = has("angVelMultiplier") && has("angVelThreshold");
    let mut unused = 0;
    for (name, v) in params {
        if *name == "start" || (msg == "bodyBalance" && !ang_vel && matches!(*name, "angVelMultiplier" | "angVelThreshold")) {
            continue;
        }
        let prm = m.params.iter().find(|p| p.name == *name).ok_or(Error::Param)?;
        if !prm.used {
            match prm.kind {
                Kind::Float => drop(v.f()?),
                Kind::Int => drop(v.i()?),
                Kind::Bool => drop(v.b()?),
                Kind::Vec3 => drop(v.v()?),
                Kind::Str => drop(v.s()?),
            }
            unused += 1;
            continue;
        }
        set(st, msg, name, v)?;
    }
    let on = start == Some(true);
    match msg {
        "activePose" => a.active_pose(st.active_pose.mask.as_deref()),
        "incomingTransforms" => if let Some(s) = start { a.incoming_state = if s { 2 } else { 4 } },
        "applyImpulse" => { let p = &st.apply_impulse; a.doll.equalized_impulse(p.part_index.max(0) as usize, p.impulse, p.hit_point, p.equalize_amount); }
        "bodyRelax" => { let p = &st.body_relax; a.body_relax_full(p.relaxation, p.damping, p.mask.as_deref(), p.hold_pose); }
        "configureBalance" => a.configure_balance(st.configure_balance.clone()),
        "configureBalanceReset" => { st.configure_balance = Default::default(); a.configure_balance(Default::default()); }
        "defineAttachedObject" => { let p = &st.define_attached_object; if p.part_index >= 0 { a.define_attached_object(p.part_index as usize, p.world_pos, p.object_mass); } }
        "leanInDirection" => { let p = &st.lean_in_direction; a.lean_in_direction(p.dir, p.lean_amount, f32::INFINITY, p.apply_as_force); }
        "leanToPosition" => { let p = &st.lean_to_position; a.lean_in_direction(p.pos - a.doll.com().0, p.lean_amount, f32::INFINITY, p.apply_as_force); }
        "leanTowardsObject" => { let p = &st.lean_towards_object; a.lean_in_direction(p.offset - a.doll.com().0, p.lean_amount, f32::INFINITY, p.apply_as_force); }
        "setStiffness" => { let p = &st.set_stiffness; a.set_stiffness_mask(p.body_stiffness, p.damping, &p.mask); }
        "shotRelax" => a.shot_relax(st.shot_relax.relax_period),
        "stopAllBehaviours" => a.stop_all_behaviours(),
        "stayUpright" => { let p = &st.stay_upright; a.stay_upright(start != Some(false), p.stiffness, p.damping_scale); }
        _ if start.is_none() => {
            a.update_params(build, world, st, msg, params.iter().any(|(n, _)| *n == "bodyPart"));
            st.upper_body_flinch.new_hit = None;
        }
        "armsWindmillAdaptive" => if on { a.arms_windmill(st.arms_windmill_adaptive.clone()) },
        "bodyBalance" => if on { a.body_balance_full(build, world, st.body_balance) } else { a.body_balance_stop() },
        "bodyFoetal" => a.body_foetal(on.then(|| st.body_foetal.clone())),
        "bodyRollUp" => if on { a.body_roll_up(st.body_roll_up.clone()) } else { a.stop_roll_up() },
        "bodyWrithe" => if on { a.body_writhe(st.body_writhe.clone()) },
        "braceForImpact" => if on { a.brace_for_impact(build, st.brace_for_impact.clone()) },
        "catchFall" => if on { a.catch_fall_full(build, st.catch_fall.clone()) } else { a.stop_catch_fall() },
        "fallOverWall" => if on { a.fall_over_wall(st.fall_over_wall.clone()) },
        "grab" => a.grab(build, on.then(|| st.grab.clone())),
        "headLook" => { let p = &st.head_look; a.head_look(build, on.then_some((p.pos, p.vel)), p.stiffness, p.damping, p.always_look, p.always_eyes_horizontal); }
        "highFall" => if on { a.high_fall(st.high_fall.clone()) },
        "pedalLegs" => if on { a.pedal_legs(st.pedal_legs.clone()) } else { a.stop_pedal() },
        "pointArm" => a.point_arm(on.then_some(st.point_arm)),
        "rollDownStairs" => if on { a.roll_down_stairs(st.roll_down_stairs.clone()) } else { a.stop_roll_down_stairs() },
        "shot" => if on { a.shot_start(st.shot) },
        "upperBodyFlinch" => if on { a.upper_body_flinch(build, st.upper_body_flinch.clone()); st.upper_body_flinch.new_hit = None; },
        _ => {}
    }
    if one_shot {
        reset(st, msg);
    }
    Ok(unused)
}

fn reset(st: &mut State, msg: &str) {
    match msg {
        "activePose" => st.active_pose = Default::default(),
        "applyImpulse" => st.apply_impulse = Default::default(),
        "bodyBalance" => st.body_balance = Default::default(),
        "bodyRelax" => st.body_relax = Default::default(),
        "defineAttachedObject" => st.define_attached_object = Default::default(),
        "leanInDirection" => st.lean_in_direction = Default::default(),
        "leanToPosition" => st.lean_to_position = Default::default(),
        "leanTowardsObject" => st.lean_towards_object = Default::default(),
        "pointArm" => st.point_arm = Default::default(),
        "setStiffness" => st.set_stiffness = Default::default(),
        "shot" => st.shot = Default::default(),
        "shotRelax" => st.shot_relax = Default::default(),
        _ => {}
    }
}

fn set(st: &mut State, msg: &str, name: &str, v: &Value) -> Result<(), Error> {
    match (msg, name) {
        ("activePose", "mask") => { let p = &mut st.active_pose; p.mask = Some(v.s()?.to_string()); }
        ("applyImpulse", "partIndex") => { let p = &mut st.apply_impulse; p.part_index = v.i()?; }
        ("applyImpulse", "hitPoint") => { let p = &mut st.apply_impulse; p.hit_point = Some(v.v()?); }
        ("applyImpulse", "impulse") => { let p = &mut st.apply_impulse; p.impulse = v.v()?; }
        ("applyImpulse", "equalizeAmount") => { let p = &mut st.apply_impulse; p.equalize_amount = v.f()?; }
        ("armsWindmillAdaptive", "bodyStiffness") => { let p = &mut st.arms_windmill_adaptive; p.body_stiffness = v.f()?; }
        ("armsWindmillAdaptive", "amplitude") => { let p = &mut st.arms_windmill_adaptive; p.amplitude = v.f()?; }
        ("armsWindmillAdaptive", "phase") => { let p = &mut st.arms_windmill_adaptive; p.phase = v.f()?; }
        ("armsWindmillAdaptive", "disableOnImpact") => { let p = &mut st.arms_windmill_adaptive; p.disable_on_impact = v.b()?; }
        ("armsWindmillAdaptive", "period") => { let p = &mut st.arms_windmill_adaptive; p.period = v.f()?; }
        ("armsWindmillAdaptive", "armStiffness") => { let p = &mut st.arms_windmill_adaptive; p.arm_stiffness = v.f()?; }
        ("bodyBalance", "bodyStiffness") => { let p = &mut st.body_balance; p.body_stiffness = v.f()?; }
        ("bodyBalance", "headLookAtVelProb") => { let p = &mut st.body_balance; p.head_look_at_vel_prob = v.f()?; }
        ("bodyBalance", "armsOutStraightenElbows") => { let p = &mut st.body_balance; p.arms_out_straighten_elbows = v.f()?; }
        ("bodyBalance", "bendElbowsTime") => { let p = &mut st.body_balance; p.bend_elbows_time = v.f()?; }
        ("bodyBalance", "elbow") => { let p = &mut st.body_balance; p.elbow = v.f()?; }
        ("bodyBalance", "angVelMultiplier") => { let p = &mut st.body_balance; p.ang_vel_multiplier = v.v()?; }
        ("bodyBalance", "angVelThreshold") => { let p = &mut st.body_balance; p.ang_vel_threshold = v.v()?; }
        ("bodyBalance", "returningToBalanceArmsOut") => { let p = &mut st.body_balance; p.returning_to_balance_arms_out = v.f()?; }
        ("bodyBalance", "somersaultAngleThreshold") => { let p = &mut st.body_balance; p.somersault_angle_threshold = v.f()?; }
        ("bodyBalance", "armsOutMinLean2") => { let p = &mut st.body_balance; p.arms_out_min_lean2 = v.f()?; }
        ("bodyBalance", "sideSomersaultAngleThreshold") => { let p = &mut st.body_balance; p.side_somersault_angle_threshold = v.f()?; }
        ("bodyBalance", "bendElbowsGait") => { let p = &mut st.body_balance; p.bend_elbows_gait = v.f()?; }
        ("bodyBalance", "armsOutOnPush") => { let p = &mut st.body_balance; p.arms_out_on_push = v.b()?; }
        ("bodyBalance", "elbowAngleOnContact") => { let p = &mut st.body_balance; p.elbow_angle_on_contact = v.f()?; }
        ("bodyBalance", "bodyTurnTimeout") => { let p = &mut st.body_balance; p.body_turn_timeout = v.f()?; }
        ("bodyBalance", "turnOffProb") => { let p = &mut st.body_balance; p.turn_off_prob = v.f()?; }
        ("bodyBalance", "turn2VelProb") => { let p = &mut st.body_balance; p.turn2_vel_prob = v.f()?; }
        ("bodyBalance", "damping") => { let p = &mut st.body_balance; p.damping = v.f()?; }
        ("bodyBalance", "headLookPos") => { let p = &mut st.body_balance; p.head_look_pos = v.v()?; }
        ("bodyBalance", "sideSomersaultAngle") => { let p = &mut st.body_balance; p.side_somersault_angle = v.f()?; }
        ("bodyBalance", "headLookAlwaysTimeout") => { let p = &mut st.body_balance; p.head_look_always_timeout = v.f()?; }
        ("bodyBalance", "turnAwayProb") => { let p = &mut st.body_balance; p.turn_away_prob = v.f()?; }
        ("bodyBalance", "shoulder") => { let p = &mut st.body_balance; p.shoulder = v.f()?; }
        ("bodyBalance", "turnRightProb") => { let p = &mut st.body_balance; p.turn_right_prob = v.f()?; }
        ("bodyBalance", "turnLeftProb") => { let p = &mut st.body_balance; p.turn_left_prob = v.f()?; }
        ("bodyBalance", "somersaultAngle") => { let p = &mut st.body_balance; p.somersault_angle = v.f()?; }
        ("bodyBalance", "armsOutOnPushMultiplier") => { let p = &mut st.body_balance; p.arms_out_on_push_multiplier = v.f()?; }
        ("bodyBalance", "turn2TargetProb") => { let p = &mut st.body_balance; p.turn2_target_prob = v.f()?; }
        ("bodyBalance", "useBodyTurn") => { let p = &mut st.body_balance; p.use_body_turn = v.b()?; }
        ("bodyBalance", "useHeadLook") => { let p = &mut st.body_balance; p.use_head_look = v.b()?; }
        ("bodyBalance", "headLookInstanceIndex") => { let p = &mut st.body_balance; p.head_look_instance_index = v.i()?; }
        ("bodyBalance", "armsOutOnPushTimeout") => { let p = &mut st.body_balance; p.arms_out_on_push_timeout = v.f()?; }
        ("bodyBalance", "maxSteps") => { let p = &mut st.body_balance; p.max_steps = v.i()?.max(0) as u32; }
        ("bodyFoetal", "mask") => { let p = &mut st.body_foetal; p.mask = intern(v.s()?); }
        ("bodyFoetal", "stiffness") => { let p = &mut st.body_foetal; p.stiffness = v.f()?; }
        ("bodyFoetal", "asymmetry") => { let p = &mut st.body_foetal; p.asymmetry = v.f()?; }
        ("bodyFoetal", "randomSeed") => { let p = &mut st.body_foetal; p.random_seed = v.i()?.max(0) as u32; }
        ("bodyFoetal", "dampingFactor") => { let p = &mut st.body_foetal; p.damping_factor = v.f()?; }
        ("bodyFoetal", "backTwist") => { let p = &mut st.body_foetal; p.back_twist = v.f()?; }
        ("bodyRelax", "mask") => { let p = &mut st.body_relax; p.mask = Some(v.s()?.to_string()); }
        ("bodyRelax", "holdPose") => { let p = &mut st.body_relax; p.hold_pose = v.b()?; }
        ("bodyRelax", "relaxation") => { let p = &mut st.body_relax; p.relaxation = v.f()?; }
        ("bodyRelax", "damping") => { let p = &mut st.body_relax; p.damping = Some(v.f()?); }
        ("bodyRollUp", "mask") => { let p = &mut st.body_roll_up; p.mask = Some(intern(v.s()?)); }
        ("bodyRollUp", "stiffness") => { let p = &mut st.body_roll_up; p.stiffness = v.f()?; }
        ("bodyRollUp", "legPush") => { let p = &mut st.body_roll_up; p.leg_push = v.f()?; }
        ("bodyRollUp", "armReachAmount") => { let p = &mut st.body_roll_up; p.arm_reach_amount = v.f()?; }
        ("bodyRollUp", "useArmToSlowDown") => { let p = &mut st.body_roll_up; p.use_arm_to_slow_down = v.f()?; }
        ("bodyRollUp", "asymmetricalLegs") => { let p = &mut st.body_roll_up; p.asymmetrical_legs = v.f()?; }
        ("bodyWrithe", "armStiffness") => { let p = &mut st.body_writhe; p.arm_stiffness = v.f()?; }
        ("bodyWrithe", "legStiffness") => { let p = &mut st.body_writhe; p.leg_stiffness = v.f()?; }
        ("bodyWrithe", "legPeriod") => { let p = &mut st.body_writhe; p.leg_period = v.f()?; }
        ("bodyWrithe", "elbowAmplitude") => { let p = &mut st.body_writhe; p.elbow_amplitude = v.f()?; }
        ("bodyWrithe", "legAmplitude") => { let p = &mut st.body_writhe; p.leg_amplitude = v.f()?; }
        ("bodyWrithe", "kneeAmplitude") => { let p = &mut st.body_writhe; p.knee_amplitude = v.f()?; }
        ("bodyWrithe", "backDamping") => { let p = &mut st.body_writhe; p.back_damping = v.f()?; }
        ("bodyWrithe", "backPeriod") => { let p = &mut st.body_writhe; p.back_period = v.f()?; }
        ("bodyWrithe", "rollOverFlag") => { let p = &mut st.body_writhe; p.roll_over = v.b()?; }
        ("bodyWrithe", "backAmplitude") => { let p = &mut st.body_writhe; p.back_amplitude = v.f()?; }
        ("bodyWrithe", "backStiffness") => { let p = &mut st.body_writhe; p.back_stiffness = v.f()?; }
        ("bodyWrithe", "armDamping") => { let p = &mut st.body_writhe; p.arm_damping = v.f()?; }
        ("bodyWrithe", "legDamping") => { let p = &mut st.body_writhe; p.leg_damping = v.f()?; }
        ("bodyWrithe", "mask") => { let p = &mut st.body_writhe; p.mask = intern(v.s()?); }
        ("bodyWrithe", "armAmplitude") => { let p = &mut st.body_writhe; p.arm_amplitude = v.f()?; }
        ("bodyWrithe", "armPeriod") => { let p = &mut st.body_writhe; p.arm_period = v.f()?; }
        ("braceForImpact", "bodyStiffness") => { let p = &mut st.brace_for_impact; p.body_stiffness = v.f()?; }
        ("braceForImpact", "look") => { let p = &mut st.brace_for_impact; p.look = v.v()?; }
        ("braceForImpact", "grabReachAngle") => { let p = &mut st.brace_for_impact; p.grab_reach_angle = v.f()?; }
        ("braceForImpact", "grabHoldTimer") => { let p = &mut st.brace_for_impact; p.grab_hold_timer = v.f()?; }
        ("braceForImpact", "braceDistance") => { let p = &mut st.brace_for_impact; p.brace_distance = v.f()?; }
        ("braceForImpact", "targetPredictionTime") => { let p = &mut st.brace_for_impact; p.target_prediction_time = v.f()?; }
        ("braceForImpact", "grabDistance") => { let p = &mut st.brace_for_impact; p.grab_distance = v.f()?; }
        ("braceForImpact", "pos") => { let p = &mut st.brace_for_impact; p.pos = v.v()?; }
        ("braceForImpact", "reachAbsorbtionTime") => { let p = &mut st.brace_for_impact; p.reach_absorbtion_time = v.f()?; }
        ("braceForImpact", "grabDontLetGo") => { let p = &mut st.brace_for_impact; p.grab_dont_let_go = v.b()?; }
        ("braceForImpact", "legStiffness") => { let p = &mut st.brace_for_impact; p.leg_stiffness = v.f()?; }
        ("braceForImpact", "grabStrength") => { let p = &mut st.brace_for_impact; p.grab_strength = v.f()?; }
        ("braceForImpact", "instanceIndex") => { let p = &mut st.brace_for_impact; p.instance_index = v.i()?; }
        ("braceForImpact", "hash_4f7c0f0b") => { let p = &mut st.brace_for_impact; p.behind_time = v.f()?; }
        ("catchFall", "useHeadLook") => { let p = &mut st.catch_fall; p.use_head_look = v.b()?; }
        ("catchFall", "armsStiffness") => { let p = &mut st.catch_fall; p.arms_stiffness = v.f()?; }
        ("catchFall", "torsoStiffness") => { let p = &mut st.catch_fall; p.torso_stiffness = v.f()?; }
        ("catchFall", "legsStiffness") => { let p = &mut st.catch_fall; p.legs_stiffness = v.f()?; }
        ("catchFall", "forwardMaxArmOffset") => { let p = &mut st.catch_fall; p.forward_max_arm_offset = v.f()?; }
        ("catchFall", "zAxisSpinReduction") => { let p = &mut st.catch_fall; p.z_axis_spin_reduction = v.f()?; }
        ("catchFall", "backwardsMinArmOffset") => { let p = &mut st.catch_fall; p.backwards_min_arm_offset = v.f()?; }
        ("configureBalance", "legStiffness") => { let p = &mut st.configure_balance; p.leg_stiffness = v.f()?; }
        ("configureBalance", "stepHeight") => { let p = &mut st.configure_balance; p.step_height = v.f()?; }
        ("configureBalance", "stepClampScale") => { let p = &mut st.configure_balance; p.step_clamp_scale = v.f()?; }
        ("configureBalance", "balanceAbortThreshold") => { let p = &mut st.configure_balance; p.balance_abort_threshold = v.f()?; }
        ("configureBalance", "taperKneeStrength") => { let p = &mut st.configure_balance; p.taper_knee_strength = v.b()?; }
        ("defineAttachedObject", "partIndex") => { let p = &mut st.define_attached_object; p.part_index = v.i()?; }
        ("defineAttachedObject", "worldPos") => { let p = &mut st.define_attached_object; p.world_pos = v.v()?; }
        ("defineAttachedObject", "objectMass") => { let p = &mut st.define_attached_object; p.object_mass = v.f()?; }
        ("fallOverWall", "bodyTwist") => { let p = &mut st.fall_over_wall; p.body_twist(v.f()?); }
        ("fallOverWall", "useSpecifiedPoint") => { let p = &mut st.fall_over_wall; p.use_specified_point = v.b()?; }
        ("fallOverWall", "maxDistanceFromPelToHitPoint") => { let p = &mut st.fall_over_wall; p.max_distance_from_pel_to_hit_point = v.f()?; }
        ("fallOverWall", "fallOverEdge") => { let p = &mut st.fall_over_wall; p.fall_over_edge = v.v()?; }
        ("fallOverWall", "fallOverPoint") => { let p = &mut st.fall_over_wall; p.fall_over_point = v.v()?; }
        ("fallOverWall", "magOfForce") => { let p = &mut st.fall_over_wall; p.mag_of_force = v.f()?; }
        ("fallOverWall", "bodyStiffness") => { let p = &mut st.fall_over_wall; p.body_stiffness = v.f()?; }
        ("fallOverWall", "damping") => { let p = &mut st.fall_over_wall; p.damping = v.f()?; }
        ("grab", "bodyStiffness") => { let p = &mut st.grab; p.body_stiffness = v.f()?; }
        ("grab", "instanceIndex") => { let p = &mut st.grab; p.instance = v.i()?; }
        ("grab", "instancePartIndex") => { let p = &mut st.grab; p.part = v.i()?; }
        ("grab", "armStiffness") => { let p = &mut st.grab; p.arm_stiffness = v.f()?; }
        ("grab", "grabStrength") => { let p = &mut st.grab; p.grab_strength = v.f()?; }
        ("grab", "useLeft") => { let p = &mut st.grab; p.use_left = v.b()?; }
        ("grab", "normalR") => { let p = &mut st.grab; p.normal_r = v.v()?; }
        ("grab", "pos3") => { let p = &mut st.grab; p.pos[2] = v.v()?; }
        ("grab", "useRight") => { let p = &mut st.grab; p.use_right = v.b()?; }
        ("grab", "pos1") => { let p = &mut st.grab; p.pos[0] = v.v()?; }
        ("grab", "pos2") => { let p = &mut st.grab; p.pos[1] = v.v()?; }
        ("grab", "pos4") => { let p = &mut st.grab; p.pos[3] = v.v()?; }
        ("grab", "surfaceGrab") => { let p = &mut st.grab; p.surface_grab = v.b()?; }
        ("grab", "oneSideReachAngle") => { let p = &mut st.grab; p.one_side_reach_angle = v.f()?; }
        ("grab", "normalL") => { let p = &mut st.grab; p.normal_l = v.v()?; }
        ("grab", "linegrab") => { let p = &mut st.grab; p.linegrab = v.b()?; }
        ("grab", "grabDist") => { let p = &mut st.grab; p.grab_dist = v.f()?; }
        ("grab", "dontLetGo") => { let p = &mut st.grab; p.dont_let_go = v.b()?; }
        ("grab", "justbrace") => { let p = &mut st.grab; p.justbrace = v.b()?; }
        ("grab", "reachAngle") => { let p = &mut st.grab; p.reach_angle = v.f()?; }
        ("grab", "pullUpStrengthRight") => { let p = &mut st.grab; p.pull_up_strength[1] = v.f()?; }
        ("grab", "pullUpTime") => { let p = &mut st.grab; p.pull_up_time = v.f()?; }
        ("grab", "pullUpStrengthLeft") => { let p = &mut st.grab; p.pull_up_strength[0] = v.f()?; }
        ("grab", "grabHoldMaxTimer") => { let p = &mut st.grab; p.grab_hold_max_timer = v.f()?; }
        ("grab", "reachDistance") => { let p = &mut st.grab; p.max_reach = v.f()?; }
        ("headLook", "pos") => { let p = &mut st.head_look; p.pos = v.v()?; }
        ("headLook", "stiffness") => { let p = &mut st.head_look; p.stiffness = v.f()?; }
        ("headLook", "alwaysEyesHorizontal") => { let p = &mut st.head_look; p.always_eyes_horizontal = v.b()?; }
        ("headLook", "alwaysLook") => { let p = &mut st.head_look; p.always_look = v.b()?; }
        ("headLook", "vel") => { let p = &mut st.head_look; p.vel = v.v()?; }
        ("headLook", "damping") => { let p = &mut st.head_look; p.damping = v.f()?; }
        ("highFall", "bodyStiffness") => { let p = &mut st.high_fall; p.body_stiffness = v.f()?; }
        ("highFall", "armAmplitude") => { let p = &mut st.high_fall; p.arm_amplitude = v.f()?; }
        ("highFall", "armPeriod") => { let p = &mut st.high_fall; p.arm_period = v.f()?; }
        ("highFall", "pdStrength") => { let p = &mut st.high_fall; p.pd_strength = v.f()?; }
        ("highFall", "armsUp") => { let p = &mut st.high_fall; p.arms_up = v.f()?; }
        ("highFall", "legRadius") => { let p = &mut st.high_fall; p.leg_radius = v.f()?; }
        ("highFall", "bodydamping") => { let p = &mut st.high_fall; p.bodydamping = v.f()?; }
        ("highFall", "legAngSpeed") => { let p = &mut st.high_fall; p.leg_ang_speed = v.f()?; }
        ("highFall", "crashOrLandCutOff") => { let p = &mut st.high_fall; p.crash_or_land_cut_off = v.f()?; }
        ("highFall", "fowardOffsetOfLegIK") => { let p = &mut st.high_fall; p.foward_offset_of_leg_ik = v.f()?; }
        ("highFall", "catchfalltime") => { let p = &mut st.high_fall; p.catchfalltime = v.f()?; }
        ("highFall", "landCutOffVelocity") => { let p = &mut st.high_fall; p.crash_or_land_cut_off = v.f()?; }
        ("highFall", "ignorWorldCollisions") => { let p = &mut st.high_fall; p.ignor_world_collisions = v.b()?; }
        ("highFall", "fowardRoll") => { let p = &mut st.high_fall; p.foward_roll = v.b()?; }
        ("highFall", "orientateBodyToFallDirection") => { let p = &mut st.high_fall; p.orientate_body_to_fall_direction = v.b()?; }
        ("highFall", "aimAngleBase") => { let p = &mut st.high_fall; p.aim_angle_base = v.f()?; }
        ("highFall", "fowardVelRotation") => { let p = &mut st.high_fall; p.foward_vel_rotation = v.f()?; }
        ("highFall", "sideD") => { let p = &mut st.high_fall; p.side_d = v.f()?; }
        ("highFall", "catchFallCutOff") => { let p = &mut st.high_fall; p.catch_fall_cut_off = v.f()?; }
        ("highFall", "footVelCompScale") => { let p = &mut st.high_fall; p.foot_vel_comp_scale = v.f()?; }
        ("highFall", "legL") => { let p = &mut st.high_fall; p.leg_l = v.f()?; }
        ("highFall", "fowardVelCutOff") => { let p = &mut st.high_fall; p.foward_vel_cut_off = v.f()?; }
        ("highFall", "legStrength") => { let p = &mut st.high_fall; p.leg_strength = v.f()?; }
        ("highFall", "zeroPoseRoll") => { let p = &mut st.high_fall; p.zero_pose_roll = v.b()?; }
        ("leanInDirection", "leanAmount") => { let p = &mut st.lean_in_direction; p.lean_amount = v.f()?; }
        ("leanInDirection", "applyAsForce") => { let p = &mut st.lean_in_direction; p.apply_as_force = v.b()?; }
        ("leanInDirection", "dir") => { let p = &mut st.lean_in_direction; p.dir = v.v()?; }
        ("leanToPosition", "leanAmount") => { let p = &mut st.lean_to_position; p.lean_amount = v.f()?; }
        ("leanToPosition", "applyAsForce") => { let p = &mut st.lean_to_position; p.apply_as_force = v.b()?; }
        ("leanToPosition", "pos") => { let p = &mut st.lean_to_position; p.pos = v.v()?; }
        ("leanTowardsObject", "leanAmount") => { let p = &mut st.lean_towards_object; p.lean_amount = v.f()?; }
        ("leanTowardsObject", "applyAsForce") => { let p = &mut st.lean_towards_object; p.apply_as_force = v.b()?; }
        ("leanTowardsObject", "offset") => { let p = &mut st.lean_towards_object; p.offset = v.v()?; }
        ("pedalLegs", "legStiffness") => { let p = &mut st.pedal_legs; p.leg_stiffness = v.f()?; }
        ("pedalLegs", "pedalRightLeg") => { let p = &mut st.pedal_legs; p.right = v.b()?; }
        ("pedalLegs", "speedAsymmetry") => { let p = &mut st.pedal_legs; p.speed_asymmetry = v.f()?; }
        ("pedalLegs", "pedalLeftLeg") => { let p = &mut st.pedal_legs; p.left = v.b()?; }
        ("pedalLegs", "radius") => { let p = &mut st.pedal_legs; p.radius = v.f()?; }
        ("pedalLegs", "angularSpeed") => { let p = &mut st.pedal_legs; p.angular_speed = v.f()?; }
        ("pedalLegs", "pedalOffset") => { let p = &mut st.pedal_legs; p.pedal_offset = v.f()?; }
        ("pedalLegs", "angSpeedMultiplier4Dragging") => { let p = &mut st.pedal_legs; p.drag_multiplier = v.f()?; }
        ("pedalLegs", "adaptivePedal4Dragging") => { let p = &mut st.pedal_legs; p.adaptive = v.b()?; }
        ("pedalLegs", "backPedal") => { let p = &mut st.pedal_legs; p.back_pedal = v.b()?; }
        ("pedalLegs", "radiusVariance") => { let p = &mut st.pedal_legs; p.radius_variance = v.f()?; }
        ("pedalLegs", "centreUp") => { let p = &mut st.pedal_legs; p.centre_up = v.f()?; }
        ("pedalLegs", "centreForwards") => { let p = &mut st.pedal_legs; p.centre_forwards = v.f()?; }
        ("pedalLegs", "legAngleVariance") => { let p = &mut st.pedal_legs; p.leg_angle_variance = v.f()?; }
        ("pedalLegs", "randomSeed") => { let p = &mut st.pedal_legs; p.random_seed = v.i()?.max(0) as u32; }
        ("pointArm", "instanceIndex") => { let p = &mut st.point_arm; p.instance_index = v.i()?; }
        ("pointArm", "useLeftArm") => { let p = &mut st.point_arm; p.use_left_arm = v.b()?; }
        ("pointArm", "twist") => { let p = &mut st.point_arm; p.twist = v.f()?; }
        ("pointArm", "target") => { let p = &mut st.point_arm; p.target = v.v()?; }
        ("pointArm", "pointSwingLimit") => { let p = &mut st.point_arm; p.point_swing_limit = v.f()?; }
        ("pointArm", "armStraightness") => { let p = &mut st.point_arm; p.arm_straightness = v.f()?; }
        ("pointArm", "useZeroPoseWhenNotPointing") => { let p = &mut st.point_arm; p.use_zero_pose_when_not_pointing = v.b()?; }
        ("pointArm", "armDamping") => { let p = &mut st.point_arm; p.arm_damping = v.f()?; }
        ("pointArm", "armStiffness") => { let p = &mut st.point_arm; p.arm_stiffness = v.f()?; }
        ("rollDownStairs", "forcemag") => { let p = &mut st.roll_down_stairs; p.forcemag = v.f()?; }
        ("rollDownStairs", "armReachLength") => { let p = &mut st.roll_down_stairs; p.arm_reach_length = v.f()?; }
        ("rollDownStairs", "m_legPush") => { let p = &mut st.roll_down_stairs; p.leg_push = v.f()?; }
        ("rollDownStairs", "useCustomRollDir") => { let p = &mut st.roll_down_stairs; p.use_custom_roll_dir = v.b()?; }
        ("rollDownStairs", "stiffnessDecayTarget") => { let p = &mut st.roll_down_stairs; p.stiffness_decay_target = v.f()?; }
        ("rollDownStairs", "spinWhenInAir") => { let p = &mut st.roll_down_stairs; p.spin_when_in_air = v.b()?; }
        ("rollDownStairs", "m_armReachAmount") => { let p = &mut st.roll_down_stairs; p.arm_reach_amount = v.f()?; }
        ("rollDownStairs", "customRollDir") => { let p = &mut st.roll_down_stairs; p.custom_roll_dir = v.v()?; }
        ("rollDownStairs", "stiffnessDecayTime") => { let p = &mut st.roll_down_stairs; p.stiffness_decay_time = v.f()?; }
        ("rollDownStairs", "m_useArmToSlowDown") => { let p = &mut st.roll_down_stairs; p.use_arm_to_slow_down = v.f()?; }
        ("rollDownStairs", "stiffness") => { let p = &mut st.roll_down_stairs; p.stiffness = v.f()?; }
        ("rollDownStairs", "damping") => { let p = &mut st.roll_down_stairs; p.damping = v.f()?; }
        ("rollDownStairs", "useZeroPose") => { let p = &mut st.roll_down_stairs; p.use_zero_pose = v.b()?; }
        ("rollDownStairs", "zAxisSpinReduction") => { let p = &mut st.roll_down_stairs; p.z_axis_spin_reduction = v.f()?; }
        ("rollDownStairs", "asymmetricalLegs") => { let p = &mut st.roll_down_stairs; p.asymmetrical_legs = v.f()?; }
        ("rollDownStairs", "asymmetry") => { let p = &mut st.roll_down_stairs; p.asymmetry = v.f()?; }
        ("setStiffness", "bodyStiffness") => { let p = &mut st.set_stiffness; p.body_stiffness = v.f()?; }
        ("setStiffness", "mask") => { let p = &mut st.set_stiffness; p.mask = v.s()?.to_string(); }
        ("setStiffness", "damping") => { let p = &mut st.set_stiffness; p.damping = v.f()?; }
        ("shot", "bodyStiffness") => { let p = &mut st.shot; p.body_stiffness = v.f()?; }
        ("shot", "grabHoldTime") => { let p = &mut st.shot; p.grab_hold_time = v.f()?; }
        ("shot", "bodyPart") => { let p = &mut st.shot; p.body_part = v.i()?.max(0) as usize; }
        ("shot", "loosenessAmount") => { let p = &mut st.shot; p.looseness_amount = v.f()?; }
        ("shot", "shockSpinDecayMult") => { let p = &mut st.shot; p.shock_spin_decay_mult = v.f()?; }
        ("shot", "timeBeforeReachForWound") => { let p = &mut st.shot; p.time_before_reach_for_wound = v.f()?; }
        ("shot", "headLookAtWoundMaxTimer") => { let p = &mut st.shot; p.head_look_at_wound_max_timer = v.f()?; }
        ("shot", "reachForWound") => { let p = &mut st.shot; p.reach_for_wound = v.b()?; }
        ("shot", "spinePainTwistMultiplier") => { let p = &mut st.shot; p.spine_pain_twist_multiplier = v.f()?; }
        ("shot", "useHeadLook") => { let p = &mut st.shot; p.use_head_look = v.b()?; }
        ("shot", "shockSpinMin") => { let p = &mut st.shot; p.shock_spin_min = v.f()?; }
        ("shot", "stableHandsAndNeck") => { let p = &mut st.shot; p.stable_hands_and_neck = v.b()?; }
        ("shot", "maxSteps") => { let p = &mut st.shot; p.max_steps = v.i()?; }
        ("shot", "shockSpinMax") => { let p = &mut st.shot; p.shock_spin_max = v.f()?; }
        ("shot", "timeBeforeCollapseWoundLeg") => { let p = &mut st.shot; p.time_before_collapse_wound_leg = v.f()?; }
        ("shot", "melee") => { let p = &mut st.shot; p.melee = v.b()?; }
        ("shot", "shockSpinScalePerComponent") => { let p = &mut st.shot; p.shock_spin_scale_per_component = v.f()?; }
        ("shot", "randomizeShockSpinDirection") => { let p = &mut st.shot; p.randomize_shock_spin_direction = v.b()?; }
        ("shot", "useCatchFallOnFall") => { let p = &mut st.shot; p.use_catch_fall_on_fall = v.b()?; }
        ("shot", "crouching") => { let p = &mut st.shot; p.crouching = v.b()?; }
        ("shot", "headLookAtWoundMinTimer") => { let p = &mut st.shot; p.head_look_at_wound_min_timer = v.f()?; }
        ("shot", "hitPoint") => { let p = &mut st.shot; p.hit_point = v.v()?; }
        ("shot", "shockSpinLiftForceMult") => { let p = &mut st.shot; p.shock_spin_lift_force_mult = v.f()?; }
        ("shot", "chickenArms") => { let p = &mut st.shot; p.chicken_arms = v.b()?; }
        ("shot", "fling") => { let p = &mut st.shot; p.fling = v.b()?; }
        ("shot", "normal") => { let p = &mut st.shot; p.normal = v.v()?; }
        ("shot", "spinePainTime") => { let p = &mut st.shot; p.spine_pain_time = v.f()?; }
        ("shot", "headLookAtHeadPosMinTimer") => { let p = &mut st.shot; p.head_look_at_head_pos_min_timer = v.f()?; }
        ("shot", "bulletVel") => { let p = &mut st.shot; p.bullet_vel = v.v()?; }
        ("shot", "addShockSpin") => { let p = &mut st.shot; p.add_shock_spin = v.b()?; }
        ("shot", "spinePainMultiplier") => { let p = &mut st.shot; p.spine_pain_multiplier = v.f()?; }
        ("shot", "headLookAtHeadPosMaxTimer") => { let p = &mut st.shot; p.head_look_at_head_pos_max_timer = v.f()?; }
        ("shot", "alwaysAddShockSpin") => { let p = &mut st.shot; p.always_add_shock_spin = v.b()?; }
        ("shot", "flingWidth") => { let p = &mut st.shot; p.fling_width = v.f()?; }
        ("shot", "headLook") => { let p = &mut st.shot; p.head_look = v.v()?; }
        ("shotRelax", "relaxPeriod") => { let p = &mut st.shot_relax; p.relax_period = v.f()?; }
        ("stayUpright", "stiffness") => { let p = &mut st.stay_upright; p.stiffness = v.f()?; }
        ("stayUpright", "dampingScale") => { let p = &mut st.stay_upright; p.damping_scale = v.f()?; }
        ("upperBodyFlinch", "handDistanceFrontBack") => { let p = &mut st.upper_body_flinch; p.hand_distance_front_back = v.f()?; }
        ("upperBodyFlinch", "noiseScale") => { let p = &mut st.upper_body_flinch; p.noise_scale = v.f()?; }
        ("upperBodyFlinch", "handDistanceVertical") => { let p = &mut st.upper_body_flinch; p.hand_distance_vertical = v.f()?; }
        ("upperBodyFlinch", "useRightArm") => { let p = &mut st.upper_body_flinch; p.use_right_arm = v.b()?; }
        ("upperBodyFlinch", "handDistanceLeftRight") => { let p = &mut st.upper_body_flinch; p.hand_distance_left_right = v.f()?; }
        ("upperBodyFlinch", "rightHanded") => { let p = &mut st.upper_body_flinch; p.right_handed = v.b()?; }
        ("upperBodyFlinch", "turnTowards") => { let p = &mut st.upper_body_flinch; p.turn_towards = v.i()?; }
        ("upperBodyFlinch", "backBendAmount") => { let p = &mut st.upper_body_flinch; p.back_bend_amount = v.f()?; }
        ("upperBodyFlinch", "newHit") => { let p = &mut st.upper_body_flinch; p.new_hit = Some(v.b()?); }
        ("upperBodyFlinch", "leftHanded") => { let p = &mut st.upper_body_flinch; p.left_handed = v.b()?; }
        ("upperBodyFlinch", "bodyDamping") => { let p = &mut st.upper_body_flinch; p.body_damping = v.f()?; }
        ("upperBodyFlinch", "useLeftArm") => { let p = &mut st.upper_body_flinch; p.use_left_arm = v.b()?; }
        ("upperBodyFlinch", "pos") => { let p = &mut st.upper_body_flinch; p.pos = v.v()?; }
        ("upperBodyFlinch", "bodyStiffness") => { let p = &mut st.upper_body_flinch; p.body_stiffness = v.f()?; }
        _ => return Err(Error::Param),
    }
    Ok(())
}

pub fn get(st: &State, msg: &str, name: &str) -> Option<Value> {
    Some(match (msg, name) {
        ("activePose", "mask") => { let p = &st.active_pose; Value::Str(p.mask.clone().unwrap_or_else(|| "fb".into())) }
        ("applyImpulse", "partIndex") => { let p = &st.apply_impulse; Value::Int(p.part_index) }
        ("applyImpulse", "hitPoint") => { let p = &st.apply_impulse; Value::Vec3(p.hit_point.unwrap_or(Vec3::ZERO)) }
        ("applyImpulse", "impulse") => { let p = &st.apply_impulse; Value::Vec3(p.impulse) }
        ("applyImpulse", "equalizeAmount") => { let p = &st.apply_impulse; Value::Float(p.equalize_amount) }
        ("armsWindmillAdaptive", "bodyStiffness") => { let p = &st.arms_windmill_adaptive; Value::Float(p.body_stiffness) }
        ("armsWindmillAdaptive", "amplitude") => { let p = &st.arms_windmill_adaptive; Value::Float(p.amplitude) }
        ("armsWindmillAdaptive", "phase") => { let p = &st.arms_windmill_adaptive; Value::Float(p.phase) }
        ("armsWindmillAdaptive", "disableOnImpact") => { let p = &st.arms_windmill_adaptive; Value::Bool(p.disable_on_impact) }
        ("armsWindmillAdaptive", "period") => { let p = &st.arms_windmill_adaptive; Value::Float(p.period) }
        ("armsWindmillAdaptive", "armStiffness") => { let p = &st.arms_windmill_adaptive; Value::Float(p.arm_stiffness) }
        ("bodyBalance", "bodyStiffness") => { let p = &st.body_balance; Value::Float(p.body_stiffness) }
        ("bodyBalance", "headLookAtVelProb") => { let p = &st.body_balance; Value::Float(p.head_look_at_vel_prob) }
        ("bodyBalance", "armsOutStraightenElbows") => { let p = &st.body_balance; Value::Float(p.arms_out_straighten_elbows) }
        ("bodyBalance", "bendElbowsTime") => { let p = &st.body_balance; Value::Float(p.bend_elbows_time) }
        ("bodyBalance", "elbow") => { let p = &st.body_balance; Value::Float(p.elbow) }
        ("bodyBalance", "angVelMultiplier") => { let p = &st.body_balance; Value::Vec3(p.ang_vel_multiplier) }
        ("bodyBalance", "angVelThreshold") => { let p = &st.body_balance; Value::Vec3(p.ang_vel_threshold) }
        ("bodyBalance", "returningToBalanceArmsOut") => { let p = &st.body_balance; Value::Float(p.returning_to_balance_arms_out) }
        ("bodyBalance", "somersaultAngleThreshold") => { let p = &st.body_balance; Value::Float(p.somersault_angle_threshold) }
        ("bodyBalance", "armsOutMinLean2") => { let p = &st.body_balance; Value::Float(p.arms_out_min_lean2) }
        ("bodyBalance", "sideSomersaultAngleThreshold") => { let p = &st.body_balance; Value::Float(p.side_somersault_angle_threshold) }
        ("bodyBalance", "bendElbowsGait") => { let p = &st.body_balance; Value::Float(p.bend_elbows_gait) }
        ("bodyBalance", "armsOutOnPush") => { let p = &st.body_balance; Value::Bool(p.arms_out_on_push) }
        ("bodyBalance", "elbowAngleOnContact") => { let p = &st.body_balance; Value::Float(p.elbow_angle_on_contact) }
        ("bodyBalance", "bodyTurnTimeout") => { let p = &st.body_balance; Value::Float(p.body_turn_timeout) }
        ("bodyBalance", "turnOffProb") => { let p = &st.body_balance; Value::Float(p.turn_off_prob) }
        ("bodyBalance", "turn2VelProb") => { let p = &st.body_balance; Value::Float(p.turn2_vel_prob) }
        ("bodyBalance", "damping") => { let p = &st.body_balance; Value::Float(p.damping) }
        ("bodyBalance", "headLookPos") => { let p = &st.body_balance; Value::Vec3(p.head_look_pos) }
        ("bodyBalance", "sideSomersaultAngle") => { let p = &st.body_balance; Value::Float(p.side_somersault_angle) }
        ("bodyBalance", "headLookAlwaysTimeout") => { let p = &st.body_balance; Value::Float(p.head_look_always_timeout) }
        ("bodyBalance", "turnAwayProb") => { let p = &st.body_balance; Value::Float(p.turn_away_prob) }
        ("bodyBalance", "shoulder") => { let p = &st.body_balance; Value::Float(p.shoulder) }
        ("bodyBalance", "turnRightProb") => { let p = &st.body_balance; Value::Float(p.turn_right_prob) }
        ("bodyBalance", "turnLeftProb") => { let p = &st.body_balance; Value::Float(p.turn_left_prob) }
        ("bodyBalance", "somersaultAngle") => { let p = &st.body_balance; Value::Float(p.somersault_angle) }
        ("bodyBalance", "armsOutOnPushMultiplier") => { let p = &st.body_balance; Value::Float(p.arms_out_on_push_multiplier) }
        ("bodyBalance", "turn2TargetProb") => { let p = &st.body_balance; Value::Float(p.turn2_target_prob) }
        ("bodyBalance", "useBodyTurn") => { let p = &st.body_balance; Value::Bool(p.use_body_turn) }
        ("bodyBalance", "useHeadLook") => { let p = &st.body_balance; Value::Bool(p.use_head_look) }
        ("bodyBalance", "headLookInstanceIndex") => { let p = &st.body_balance; Value::Int(p.head_look_instance_index) }
        ("bodyBalance", "armsOutOnPushTimeout") => { let p = &st.body_balance; Value::Float(p.arms_out_on_push_timeout) }
        ("bodyBalance", "maxSteps") => { let p = &st.body_balance; Value::Int(p.max_steps as i32) }
        ("bodyFoetal", "mask") => { let p = &st.body_foetal; Value::Str(p.mask.to_string()) }
        ("bodyFoetal", "stiffness") => { let p = &st.body_foetal; Value::Float(p.stiffness) }
        ("bodyFoetal", "asymmetry") => { let p = &st.body_foetal; Value::Float(p.asymmetry) }
        ("bodyFoetal", "randomSeed") => { let p = &st.body_foetal; Value::Int(p.random_seed as i32) }
        ("bodyFoetal", "dampingFactor") => { let p = &st.body_foetal; Value::Float(p.damping_factor) }
        ("bodyFoetal", "backTwist") => { let p = &st.body_foetal; Value::Float(p.back_twist) }
        ("bodyRelax", "mask") => { let p = &st.body_relax; Value::Str(p.mask.clone().unwrap_or_else(|| "fb".into())) }
        ("bodyRelax", "holdPose") => { let p = &st.body_relax; Value::Bool(p.hold_pose) }
        ("bodyRelax", "relaxation") => { let p = &st.body_relax; Value::Float(p.relaxation) }
        ("bodyRelax", "damping") => { let p = &st.body_relax; Value::Float(p.damping.unwrap_or(1.0)) }
        ("bodyRollUp", "mask") => { let p = &st.body_roll_up; Value::Str(p.mask.unwrap_or("").to_string()) }
        ("bodyRollUp", "stiffness") => { let p = &st.body_roll_up; Value::Float(p.stiffness) }
        ("bodyRollUp", "legPush") => { let p = &st.body_roll_up; Value::Float(p.leg_push) }
        ("bodyRollUp", "armReachAmount") => { let p = &st.body_roll_up; Value::Float(p.arm_reach_amount) }
        ("bodyRollUp", "useArmToSlowDown") => { let p = &st.body_roll_up; Value::Float(p.use_arm_to_slow_down) }
        ("bodyRollUp", "asymmetricalLegs") => { let p = &st.body_roll_up; Value::Float(p.asymmetrical_legs) }
        ("bodyWrithe", "armStiffness") => { let p = &st.body_writhe; Value::Float(p.arm_stiffness) }
        ("bodyWrithe", "legStiffness") => { let p = &st.body_writhe; Value::Float(p.leg_stiffness) }
        ("bodyWrithe", "legPeriod") => { let p = &st.body_writhe; Value::Float(p.leg_period) }
        ("bodyWrithe", "elbowAmplitude") => { let p = &st.body_writhe; Value::Float(p.elbow_amplitude) }
        ("bodyWrithe", "legAmplitude") => { let p = &st.body_writhe; Value::Float(p.leg_amplitude) }
        ("bodyWrithe", "kneeAmplitude") => { let p = &st.body_writhe; Value::Float(p.knee_amplitude) }
        ("bodyWrithe", "backDamping") => { let p = &st.body_writhe; Value::Float(p.back_damping) }
        ("bodyWrithe", "backPeriod") => { let p = &st.body_writhe; Value::Float(p.back_period) }
        ("bodyWrithe", "rollOverFlag") => { let p = &st.body_writhe; Value::Bool(p.roll_over) }
        ("bodyWrithe", "backAmplitude") => { let p = &st.body_writhe; Value::Float(p.back_amplitude) }
        ("bodyWrithe", "backStiffness") => { let p = &st.body_writhe; Value::Float(p.back_stiffness) }
        ("bodyWrithe", "armDamping") => { let p = &st.body_writhe; Value::Float(p.arm_damping) }
        ("bodyWrithe", "legDamping") => { let p = &st.body_writhe; Value::Float(p.leg_damping) }
        ("bodyWrithe", "mask") => { let p = &st.body_writhe; Value::Str(p.mask.to_string()) }
        ("bodyWrithe", "armAmplitude") => { let p = &st.body_writhe; Value::Float(p.arm_amplitude) }
        ("bodyWrithe", "armPeriod") => { let p = &st.body_writhe; Value::Float(p.arm_period) }
        ("braceForImpact", "bodyStiffness") => { let p = &st.brace_for_impact; Value::Float(p.body_stiffness) }
        ("braceForImpact", "look") => { let p = &st.brace_for_impact; Value::Vec3(p.look) }
        ("braceForImpact", "grabReachAngle") => { let p = &st.brace_for_impact; Value::Float(p.grab_reach_angle) }
        ("braceForImpact", "grabHoldTimer") => { let p = &st.brace_for_impact; Value::Float(p.grab_hold_timer) }
        ("braceForImpact", "braceDistance") => { let p = &st.brace_for_impact; Value::Float(p.brace_distance) }
        ("braceForImpact", "targetPredictionTime") => { let p = &st.brace_for_impact; Value::Float(p.target_prediction_time) }
        ("braceForImpact", "grabDistance") => { let p = &st.brace_for_impact; Value::Float(p.grab_distance) }
        ("braceForImpact", "pos") => { let p = &st.brace_for_impact; Value::Vec3(p.pos) }
        ("braceForImpact", "reachAbsorbtionTime") => { let p = &st.brace_for_impact; Value::Float(p.reach_absorbtion_time) }
        ("braceForImpact", "grabDontLetGo") => { let p = &st.brace_for_impact; Value::Bool(p.grab_dont_let_go) }
        ("braceForImpact", "legStiffness") => { let p = &st.brace_for_impact; Value::Float(p.leg_stiffness) }
        ("braceForImpact", "grabStrength") => { let p = &st.brace_for_impact; Value::Float(p.grab_strength) }
        ("braceForImpact", "instanceIndex") => { let p = &st.brace_for_impact; Value::Int(p.instance_index) }
        ("braceForImpact", "hash_4f7c0f0b") => { let p = &st.brace_for_impact; Value::Float(p.behind_time) }
        ("catchFall", "useHeadLook") => { let p = &st.catch_fall; Value::Bool(p.use_head_look) }
        ("catchFall", "armsStiffness") => { let p = &st.catch_fall; Value::Float(p.arms_stiffness) }
        ("catchFall", "torsoStiffness") => { let p = &st.catch_fall; Value::Float(p.torso_stiffness) }
        ("catchFall", "legsStiffness") => { let p = &st.catch_fall; Value::Float(p.legs_stiffness) }
        ("catchFall", "forwardMaxArmOffset") => { let p = &st.catch_fall; Value::Float(p.forward_max_arm_offset) }
        ("catchFall", "zAxisSpinReduction") => { let p = &st.catch_fall; Value::Float(p.z_axis_spin_reduction) }
        ("catchFall", "backwardsMinArmOffset") => { let p = &st.catch_fall; Value::Float(p.backwards_min_arm_offset) }
        ("configureBalance", "legStiffness") => { let p = &st.configure_balance; Value::Float(p.leg_stiffness) }
        ("configureBalance", "stepHeight") => { let p = &st.configure_balance; Value::Float(p.step_height) }
        ("configureBalance", "stepClampScale") => { let p = &st.configure_balance; Value::Float(p.step_clamp_scale) }
        ("configureBalance", "balanceAbortThreshold") => { let p = &st.configure_balance; Value::Float(p.balance_abort_threshold) }
        ("configureBalance", "taperKneeStrength") => { let p = &st.configure_balance; Value::Bool(p.taper_knee_strength) }
        ("defineAttachedObject", "partIndex") => { let p = &st.define_attached_object; Value::Int(p.part_index) }
        ("defineAttachedObject", "worldPos") => { let p = &st.define_attached_object; Value::Vec3(p.world_pos) }
        ("defineAttachedObject", "objectMass") => { let p = &st.define_attached_object; Value::Float(p.object_mass) }
        ("fallOverWall", "bodyTwist") => { let p = &st.fall_over_wall; Value::Float(p.twist_max / 85.0) }
        ("fallOverWall", "useSpecifiedPoint") => { let p = &st.fall_over_wall; Value::Bool(p.use_specified_point) }
        ("fallOverWall", "maxDistanceFromPelToHitPoint") => { let p = &st.fall_over_wall; Value::Float(p.max_distance_from_pel_to_hit_point) }
        ("fallOverWall", "fallOverEdge") => { let p = &st.fall_over_wall; Value::Vec3(p.fall_over_edge) }
        ("fallOverWall", "fallOverPoint") => { let p = &st.fall_over_wall; Value::Vec3(p.fall_over_point) }
        ("fallOverWall", "magOfForce") => { let p = &st.fall_over_wall; Value::Float(p.mag_of_force) }
        ("fallOverWall", "bodyStiffness") => { let p = &st.fall_over_wall; Value::Float(p.body_stiffness) }
        ("fallOverWall", "damping") => { let p = &st.fall_over_wall; Value::Float(p.damping) }
        ("grab", "bodyStiffness") => { let p = &st.grab; Value::Float(p.body_stiffness) }
        ("grab", "instanceIndex") => { let p = &st.grab; Value::Int(p.instance) }
        ("grab", "instancePartIndex") => { let p = &st.grab; Value::Int(p.part) }
        ("grab", "armStiffness") => { let p = &st.grab; Value::Float(p.arm_stiffness) }
        ("grab", "grabStrength") => { let p = &st.grab; Value::Float(p.grab_strength) }
        ("grab", "useLeft") => { let p = &st.grab; Value::Bool(p.use_left) }
        ("grab", "normalR") => { let p = &st.grab; Value::Vec3(p.normal_r) }
        ("grab", "pos3") => { let p = &st.grab; Value::Vec3(p.pos[2]) }
        ("grab", "useRight") => { let p = &st.grab; Value::Bool(p.use_right) }
        ("grab", "pos1") => { let p = &st.grab; Value::Vec3(p.pos[0]) }
        ("grab", "pos2") => { let p = &st.grab; Value::Vec3(p.pos[1]) }
        ("grab", "pos4") => { let p = &st.grab; Value::Vec3(p.pos[3]) }
        ("grab", "surfaceGrab") => { let p = &st.grab; Value::Bool(p.surface_grab) }
        ("grab", "oneSideReachAngle") => { let p = &st.grab; Value::Float(p.one_side_reach_angle) }
        ("grab", "normalL") => { let p = &st.grab; Value::Vec3(p.normal_l) }
        ("grab", "linegrab") => { let p = &st.grab; Value::Bool(p.linegrab) }
        ("grab", "grabDist") => { let p = &st.grab; Value::Float(p.grab_dist) }
        ("grab", "dontLetGo") => { let p = &st.grab; Value::Bool(p.dont_let_go) }
        ("grab", "justbrace") => { let p = &st.grab; Value::Bool(p.justbrace) }
        ("grab", "reachAngle") => { let p = &st.grab; Value::Float(p.reach_angle) }
        ("grab", "pullUpStrengthRight") => { let p = &st.grab; Value::Float(p.pull_up_strength[1]) }
        ("grab", "pullUpTime") => { let p = &st.grab; Value::Float(p.pull_up_time) }
        ("grab", "pullUpStrengthLeft") => { let p = &st.grab; Value::Float(p.pull_up_strength[0]) }
        ("grab", "grabHoldMaxTimer") => { let p = &st.grab; Value::Float(p.grab_hold_max_timer) }
        ("grab", "reachDistance") => { let p = &st.grab; Value::Float(p.max_reach) }
        ("headLook", "pos") => { let p = &st.head_look; Value::Vec3(p.pos) }
        ("headLook", "stiffness") => { let p = &st.head_look; Value::Float(p.stiffness) }
        ("headLook", "alwaysEyesHorizontal") => { let p = &st.head_look; Value::Bool(p.always_eyes_horizontal) }
        ("headLook", "alwaysLook") => { let p = &st.head_look; Value::Bool(p.always_look) }
        ("headLook", "vel") => { let p = &st.head_look; Value::Vec3(p.vel) }
        ("headLook", "damping") => { let p = &st.head_look; Value::Float(p.damping) }
        ("highFall", "bodyStiffness") => { let p = &st.high_fall; Value::Float(p.body_stiffness) }
        ("highFall", "armAmplitude") => { let p = &st.high_fall; Value::Float(p.arm_amplitude) }
        ("highFall", "armPeriod") => { let p = &st.high_fall; Value::Float(p.arm_period) }
        ("highFall", "pdStrength") => { let p = &st.high_fall; Value::Float(p.pd_strength) }
        ("highFall", "armsUp") => { let p = &st.high_fall; Value::Float(p.arms_up) }
        ("highFall", "legRadius") => { let p = &st.high_fall; Value::Float(p.leg_radius) }
        ("highFall", "bodydamping") => { let p = &st.high_fall; Value::Float(p.bodydamping) }
        ("highFall", "legAngSpeed") => { let p = &st.high_fall; Value::Float(p.leg_ang_speed) }
        ("highFall", "crashOrLandCutOff") => { let p = &st.high_fall; Value::Float(p.crash_or_land_cut_off) }
        ("highFall", "fowardOffsetOfLegIK") => { let p = &st.high_fall; Value::Float(p.foward_offset_of_leg_ik) }
        ("highFall", "catchfalltime") => { let p = &st.high_fall; Value::Float(p.catchfalltime) }
        ("highFall", "landCutOffVelocity") => { let p = &st.high_fall; Value::Float(p.crash_or_land_cut_off) }
        ("highFall", "ignorWorldCollisions") => { let p = &st.high_fall; Value::Bool(p.ignor_world_collisions) }
        ("highFall", "fowardRoll") => { let p = &st.high_fall; Value::Bool(p.foward_roll) }
        ("highFall", "orientateBodyToFallDirection") => { let p = &st.high_fall; Value::Bool(p.orientate_body_to_fall_direction) }
        ("highFall", "aimAngleBase") => { let p = &st.high_fall; Value::Float(p.aim_angle_base) }
        ("highFall", "fowardVelRotation") => { let p = &st.high_fall; Value::Float(p.foward_vel_rotation) }
        ("highFall", "sideD") => { let p = &st.high_fall; Value::Float(p.side_d) }
        ("highFall", "catchFallCutOff") => { let p = &st.high_fall; Value::Float(p.catch_fall_cut_off) }
        ("highFall", "footVelCompScale") => { let p = &st.high_fall; Value::Float(p.foot_vel_comp_scale) }
        ("highFall", "legL") => { let p = &st.high_fall; Value::Float(p.leg_l) }
        ("highFall", "fowardVelCutOff") => { let p = &st.high_fall; Value::Float(p.foward_vel_cut_off) }
        ("highFall", "legStrength") => { let p = &st.high_fall; Value::Float(p.leg_strength) }
        ("highFall", "zeroPoseRoll") => { let p = &st.high_fall; Value::Bool(p.zero_pose_roll) }
        ("leanInDirection", "leanAmount") => { let p = &st.lean_in_direction; Value::Float(p.lean_amount) }
        ("leanInDirection", "applyAsForce") => { let p = &st.lean_in_direction; Value::Bool(p.apply_as_force) }
        ("leanInDirection", "dir") => { let p = &st.lean_in_direction; Value::Vec3(p.dir) }
        ("leanToPosition", "leanAmount") => { let p = &st.lean_to_position; Value::Float(p.lean_amount) }
        ("leanToPosition", "applyAsForce") => { let p = &st.lean_to_position; Value::Bool(p.apply_as_force) }
        ("leanToPosition", "pos") => { let p = &st.lean_to_position; Value::Vec3(p.pos) }
        ("leanTowardsObject", "leanAmount") => { let p = &st.lean_towards_object; Value::Float(p.lean_amount) }
        ("leanTowardsObject", "applyAsForce") => { let p = &st.lean_towards_object; Value::Bool(p.apply_as_force) }
        ("leanTowardsObject", "offset") => { let p = &st.lean_towards_object; Value::Vec3(p.offset) }
        ("pedalLegs", "legStiffness") => { let p = &st.pedal_legs; Value::Float(p.leg_stiffness) }
        ("pedalLegs", "pedalRightLeg") => { let p = &st.pedal_legs; Value::Bool(p.right) }
        ("pedalLegs", "speedAsymmetry") => { let p = &st.pedal_legs; Value::Float(p.speed_asymmetry) }
        ("pedalLegs", "pedalLeftLeg") => { let p = &st.pedal_legs; Value::Bool(p.left) }
        ("pedalLegs", "radius") => { let p = &st.pedal_legs; Value::Float(p.radius) }
        ("pedalLegs", "angularSpeed") => { let p = &st.pedal_legs; Value::Float(p.angular_speed) }
        ("pedalLegs", "pedalOffset") => { let p = &st.pedal_legs; Value::Float(p.pedal_offset) }
        ("pedalLegs", "angSpeedMultiplier4Dragging") => { let p = &st.pedal_legs; Value::Float(p.drag_multiplier) }
        ("pedalLegs", "adaptivePedal4Dragging") => { let p = &st.pedal_legs; Value::Bool(p.adaptive) }
        ("pedalLegs", "backPedal") => { let p = &st.pedal_legs; Value::Bool(p.back_pedal) }
        ("pedalLegs", "radiusVariance") => { let p = &st.pedal_legs; Value::Float(p.radius_variance) }
        ("pedalLegs", "centreUp") => { let p = &st.pedal_legs; Value::Float(p.centre_up) }
        ("pedalLegs", "centreForwards") => { let p = &st.pedal_legs; Value::Float(p.centre_forwards) }
        ("pedalLegs", "legAngleVariance") => { let p = &st.pedal_legs; Value::Float(p.leg_angle_variance) }
        ("pedalLegs", "randomSeed") => { let p = &st.pedal_legs; Value::Int(p.random_seed as i32) }
        ("pointArm", "instanceIndex") => { let p = &st.point_arm; Value::Int(p.instance_index) }
        ("pointArm", "useLeftArm") => { let p = &st.point_arm; Value::Bool(p.use_left_arm) }
        ("pointArm", "twist") => { let p = &st.point_arm; Value::Float(p.twist) }
        ("pointArm", "target") => { let p = &st.point_arm; Value::Vec3(p.target) }
        ("pointArm", "pointSwingLimit") => { let p = &st.point_arm; Value::Float(p.point_swing_limit) }
        ("pointArm", "armStraightness") => { let p = &st.point_arm; Value::Float(p.arm_straightness) }
        ("pointArm", "useZeroPoseWhenNotPointing") => { let p = &st.point_arm; Value::Bool(p.use_zero_pose_when_not_pointing) }
        ("pointArm", "armDamping") => { let p = &st.point_arm; Value::Float(p.arm_damping) }
        ("pointArm", "armStiffness") => { let p = &st.point_arm; Value::Float(p.arm_stiffness) }
        ("rollDownStairs", "forcemag") => { let p = &st.roll_down_stairs; Value::Float(p.forcemag) }
        ("rollDownStairs", "armReachLength") => { let p = &st.roll_down_stairs; Value::Float(p.arm_reach_length) }
        ("rollDownStairs", "m_legPush") => { let p = &st.roll_down_stairs; Value::Float(p.leg_push) }
        ("rollDownStairs", "useCustomRollDir") => { let p = &st.roll_down_stairs; Value::Bool(p.use_custom_roll_dir) }
        ("rollDownStairs", "stiffnessDecayTarget") => { let p = &st.roll_down_stairs; Value::Float(p.stiffness_decay_target) }
        ("rollDownStairs", "spinWhenInAir") => { let p = &st.roll_down_stairs; Value::Bool(p.spin_when_in_air) }
        ("rollDownStairs", "m_armReachAmount") => { let p = &st.roll_down_stairs; Value::Float(p.arm_reach_amount) }
        ("rollDownStairs", "customRollDir") => { let p = &st.roll_down_stairs; Value::Vec3(p.custom_roll_dir) }
        ("rollDownStairs", "stiffnessDecayTime") => { let p = &st.roll_down_stairs; Value::Float(p.stiffness_decay_time) }
        ("rollDownStairs", "m_useArmToSlowDown") => { let p = &st.roll_down_stairs; Value::Float(p.use_arm_to_slow_down) }
        ("rollDownStairs", "stiffness") => { let p = &st.roll_down_stairs; Value::Float(p.stiffness) }
        ("rollDownStairs", "damping") => { let p = &st.roll_down_stairs; Value::Float(p.damping) }
        ("rollDownStairs", "useZeroPose") => { let p = &st.roll_down_stairs; Value::Bool(p.use_zero_pose) }
        ("rollDownStairs", "zAxisSpinReduction") => { let p = &st.roll_down_stairs; Value::Float(p.z_axis_spin_reduction) }
        ("rollDownStairs", "asymmetricalLegs") => { let p = &st.roll_down_stairs; Value::Float(p.asymmetrical_legs) }
        ("rollDownStairs", "asymmetry") => { let p = &st.roll_down_stairs; Value::Float(p.asymmetry) }
        ("setStiffness", "bodyStiffness") => { let p = &st.set_stiffness; Value::Float(p.body_stiffness) }
        ("setStiffness", "mask") => { let p = &st.set_stiffness; Value::Str(p.mask.clone()) }
        ("setStiffness", "damping") => { let p = &st.set_stiffness; Value::Float(p.damping) }
        ("shot", "bodyStiffness") => { let p = &st.shot; Value::Float(p.body_stiffness) }
        ("shot", "grabHoldTime") => { let p = &st.shot; Value::Float(p.grab_hold_time) }
        ("shot", "bodyPart") => { let p = &st.shot; Value::Int(p.body_part as i32) }
        ("shot", "loosenessAmount") => { let p = &st.shot; Value::Float(p.looseness_amount) }
        ("shot", "shockSpinDecayMult") => { let p = &st.shot; Value::Float(p.shock_spin_decay_mult) }
        ("shot", "timeBeforeReachForWound") => { let p = &st.shot; Value::Float(p.time_before_reach_for_wound) }
        ("shot", "headLookAtWoundMaxTimer") => { let p = &st.shot; Value::Float(p.head_look_at_wound_max_timer) }
        ("shot", "reachForWound") => { let p = &st.shot; Value::Bool(p.reach_for_wound) }
        ("shot", "spinePainTwistMultiplier") => { let p = &st.shot; Value::Float(p.spine_pain_twist_multiplier) }
        ("shot", "useHeadLook") => { let p = &st.shot; Value::Bool(p.use_head_look) }
        ("shot", "shockSpinMin") => { let p = &st.shot; Value::Float(p.shock_spin_min) }
        ("shot", "stableHandsAndNeck") => { let p = &st.shot; Value::Bool(p.stable_hands_and_neck) }
        ("shot", "maxSteps") => { let p = &st.shot; Value::Int(p.max_steps) }
        ("shot", "shockSpinMax") => { let p = &st.shot; Value::Float(p.shock_spin_max) }
        ("shot", "timeBeforeCollapseWoundLeg") => { let p = &st.shot; Value::Float(p.time_before_collapse_wound_leg) }
        ("shot", "melee") => { let p = &st.shot; Value::Bool(p.melee) }
        ("shot", "shockSpinScalePerComponent") => { let p = &st.shot; Value::Float(p.shock_spin_scale_per_component) }
        ("shot", "randomizeShockSpinDirection") => { let p = &st.shot; Value::Bool(p.randomize_shock_spin_direction) }
        ("shot", "useCatchFallOnFall") => { let p = &st.shot; Value::Bool(p.use_catch_fall_on_fall) }
        ("shot", "crouching") => { let p = &st.shot; Value::Bool(p.crouching) }
        ("shot", "headLookAtWoundMinTimer") => { let p = &st.shot; Value::Float(p.head_look_at_wound_min_timer) }
        ("shot", "hitPoint") => { let p = &st.shot; Value::Vec3(p.hit_point) }
        ("shot", "shockSpinLiftForceMult") => { let p = &st.shot; Value::Float(p.shock_spin_lift_force_mult) }
        ("shot", "chickenArms") => { let p = &st.shot; Value::Bool(p.chicken_arms) }
        ("shot", "fling") => { let p = &st.shot; Value::Bool(p.fling) }
        ("shot", "normal") => { let p = &st.shot; Value::Vec3(p.normal) }
        ("shot", "spinePainTime") => { let p = &st.shot; Value::Float(p.spine_pain_time) }
        ("shot", "headLookAtHeadPosMinTimer") => { let p = &st.shot; Value::Float(p.head_look_at_head_pos_min_timer) }
        ("shot", "bulletVel") => { let p = &st.shot; Value::Vec3(p.bullet_vel) }
        ("shot", "addShockSpin") => { let p = &st.shot; Value::Bool(p.add_shock_spin) }
        ("shot", "spinePainMultiplier") => { let p = &st.shot; Value::Float(p.spine_pain_multiplier) }
        ("shot", "headLookAtHeadPosMaxTimer") => { let p = &st.shot; Value::Float(p.head_look_at_head_pos_max_timer) }
        ("shot", "alwaysAddShockSpin") => { let p = &st.shot; Value::Bool(p.always_add_shock_spin) }
        ("shot", "flingWidth") => { let p = &st.shot; Value::Float(p.fling_width) }
        ("shot", "headLook") => { let p = &st.shot; Value::Vec3(p.head_look) }
        ("shotRelax", "relaxPeriod") => { let p = &st.shot_relax; Value::Float(p.relax_period) }
        ("stayUpright", "stiffness") => { let p = &st.stay_upright; Value::Float(p.stiffness) }
        ("stayUpright", "dampingScale") => { let p = &st.stay_upright; Value::Float(p.damping_scale) }
        ("upperBodyFlinch", "handDistanceFrontBack") => { let p = &st.upper_body_flinch; Value::Float(p.hand_distance_front_back) }
        ("upperBodyFlinch", "noiseScale") => { let p = &st.upper_body_flinch; Value::Float(p.noise_scale) }
        ("upperBodyFlinch", "handDistanceVertical") => { let p = &st.upper_body_flinch; Value::Float(p.hand_distance_vertical) }
        ("upperBodyFlinch", "useRightArm") => { let p = &st.upper_body_flinch; Value::Bool(p.use_right_arm) }
        ("upperBodyFlinch", "handDistanceLeftRight") => { let p = &st.upper_body_flinch; Value::Float(p.hand_distance_left_right) }
        ("upperBodyFlinch", "rightHanded") => { let p = &st.upper_body_flinch; Value::Bool(p.right_handed) }
        ("upperBodyFlinch", "turnTowards") => { let p = &st.upper_body_flinch; Value::Int(p.turn_towards) }
        ("upperBodyFlinch", "backBendAmount") => { let p = &st.upper_body_flinch; Value::Float(p.back_bend_amount) }
        ("upperBodyFlinch", "newHit") => { let p = &st.upper_body_flinch; Value::Bool(p.new_hit.unwrap_or(true)) }
        ("upperBodyFlinch", "leftHanded") => { let p = &st.upper_body_flinch; Value::Bool(p.left_handed) }
        ("upperBodyFlinch", "bodyDamping") => { let p = &st.upper_body_flinch; Value::Float(p.body_damping) }
        ("upperBodyFlinch", "useLeftArm") => { let p = &st.upper_body_flinch; Value::Bool(p.use_left_arm) }
        ("upperBodyFlinch", "pos") => { let p = &st.upper_body_flinch; Value::Vec3(p.pos) }
        ("upperBodyFlinch", "bodyStiffness") => { let p = &st.upper_body_flinch; Value::Float(p.body_stiffness) }
        _ => return None,
    })
}

