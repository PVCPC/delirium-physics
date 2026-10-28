use crate::extras::{Extra, P};
use crate::msg::{Error, Kind, Message, Param};
use crate::nmrs::{ANKLE, CLAVICLE, ELBOW, HIP, KNEE, NECK, SHOULDER, SPINE};
use crate::nmshot::Ctx;
use bevy_math::Vec3;

pub static MESSAGES: [Message; 7] = [
    Message { name: "teeter", params: &[
        Param { name: "edgeLeft", kind: Kind::Vec3, used: true, doc: "one end of the drop's edge (both zero: 0.5 m in front of his feet, 1 m wide)" },
        Param { name: "edgeRight", kind: Kind::Vec3, used: true, doc: "its other end" },
        Param { name: "reactDistance", kind: Kind::Float, used: true, doc: "COM to edge where he starts teetering, m (0.5)" },
        Param { name: "restartDistance", kind: Kind::Float, used: true, doc: "COM back from the edge that counts as recovered, m (0.8)" },
        Param { name: "useExclusionZone", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "look at the edge, then where he falls (true)" },
        Param { name: "callHighFall", kind: Kind::Bool, used: true, doc: "highFall when he goes over (true)" },
        Param { name: "leanAway", kind: Kind::Bool, used: true, doc: "lean away from the edge (true)" },
        Param { name: "preTeeterTime", kind: Kind::Float, used: true, doc: "settle time before teetering, s (0.2)" },
        Param { name: "leanAwayTime", kind: Kind::Float, used: true, doc: "how long the lean away lasts, s (0.8)" },
        Param { name: "teeterTime", kind: Kind::Float, used: true, doc: "how long he teeters before it resolves, s (2)" },
        Param { name: "leanAwayScale", kind: Kind::Float, used: true, doc: "how hard he leans away (0.1)" },
    ] },
    Message { name: "stumble", params: &[
        Param { name: "legsStiffness", kind: Kind::Float, used: true, doc: "legs, damping 0.5 (12)" },
        Param { name: "torsoStiffness", kind: Kind::Float, used: true, doc: "the upper body under the arms (10)" },
        Param { name: "armsStiffness", kind: Kind::Float, used: true, doc: "arms are held at this minus 1 (10)" },
        Param { name: "armReduceSpeed", kind: Kind::Float, used: false, doc: "(not done: catchFall has no such param, 0.2)" },
        Param { name: "wristMS", kind: Kind::Float, used: false, doc: "(not done: no hand mask, 15)" },
        Param { name: "feetMS", kind: Kind::Float, used: false, doc: "(not done: no foot mask, 15)" },
        Param { name: "armTwist", kind: Kind::Float, used: true, doc: "how far the braced arms turn out, of the joint range (0.3)" },
        Param { name: "fallMask", kind: Kind::Str, used: false, doc: "(not done: the arms always brace)" },
        Param { name: "forwardMaxArmOffset", kind: Kind::Float, used: true, doc: "arms' reach forward at full lean, of the joint range (0.9)" },
        Param { name: "backwardsMinArmOffset", kind: Kind::Float, used: true, doc: "arms' reach when upright, of the joint range (0.2)" },
        Param { name: "useArmsBrace", kind: Kind::Bool, used: true, doc: "arms brace forward (true)" },
        Param { name: "grabRadius2", kind: Kind::Float, used: false, doc: "(not done: no grabbing, 0.5)" },
        Param { name: "leanRate", kind: Kind::Float, used: true, doc: "how fast the lean moves to leanTowards, per second (0.5)" },
        Param { name: "maxLeanBack", kind: Kind::Float, used: true, doc: "most he leans back (0.1)" },
        Param { name: "maxLeanForward", kind: Kind::Float, used: true, doc: "most he leans forward (0.3)" },
        Param { name: "leanTowards", kind: Kind::Float, used: true, doc: "the lean he is heading for, + forward (0.2)" },
        Param { name: "zAxisSpinReduction", kind: Kind::Float, used: true, doc: "damping of his spin about up, of weight x metre per rad/s (0.5)" },
        Param { name: "dampPelvis", kind: Kind::Float, used: true, doc: "damping of the pelvis' spin, same units (0)" },
        Param { name: "pitchInContact", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "twistSpine", kind: Kind::Bool, used: true, doc: "the spine twists as he steps (true)" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "look where he is heading (true)" },
        Param { name: "staggerTime", kind: Kind::Float, used: true, doc: "seconds of stumbling before it counts as lost, s (1.5)" },
        Param { name: "dropVal", kind: Kind::Float, used: true, doc: "COM drop from where he began that counts as down, m (0.4)" },
        Param { name: "different", kind: Kind::Bool, used: true, doc: "the two arms brace differently (false)" },
    ] },
    Message { name: "staggerFall", params: &[
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "arm muscle stiffness once ramped (6)" },
        Param { name: "armDamping", kind: Kind::Float, used: true, doc: "arm damping once ramped (0.5)" },
        Param { name: "spineStiffness", kind: Kind::Float, used: true, doc: "spine stiffness once ramped (8)" },
        Param { name: "spineDamping", kind: Kind::Float, used: true, doc: "spine damping once ramped (0.5)" },
        Param { name: "armStiffnessStart", kind: Kind::Float, used: true, doc: "arm stiffness at the start (12)" },
        Param { name: "armDampingStart", kind: Kind::Float, used: true, doc: "arm damping at the start (1)" },
        Param { name: "spineStiffnessStart", kind: Kind::Float, used: true, doc: "spine stiffness at the start (12)" },
        Param { name: "spineDampingStart", kind: Kind::Float, used: true, doc: "spine damping at the start (1)" },
        Param { name: "timeAtStartValues", kind: Kind::Float, used: true, doc: "s the start gains are held (0.5)" },
        Param { name: "rampTimeFromStartValues", kind: Kind::Float, used: true, doc: "s to ramp from them to steady (1)" },
        Param { name: "lowerBodyStiffness", kind: Kind::Float, used: true, doc: "leg stiffness (12)" },
        Param { name: "lowerBodyStiffnessEnd", kind: Kind::Float, used: true, doc: "leg stiffness at the end of the wind-down (3)" },
        Param { name: "staggerStepProb", kind: Kind::Float, used: true, doc: "chance a step is a stagger, pushed off (0.5)" },
        Param { name: "stepsTillStartEnd", kind: Kind::Int, used: true, doc: "steps before the wind-down (6)" },
        Param { name: "timeStartEnd", kind: Kind::Float, used: true, doc: "s before the wind-down, whichever comes first (6)" },
        Param { name: "rampTimeToEndValues", kind: Kind::Float, used: true, doc: "s to ramp to the end gains (2)" },
        Param { name: "predictionTime", kind: Kind::Float, used: false, doc: "not done: the balancer keeps its own (0.3)" },
        Param { name: "perStepReduction1", kind: Kind::Float, used: true, doc: "gains times this each step, floor 0.2 (0.95)" },
        Param { name: "pushOffDist", kind: Kind::Float, used: true, doc: "m a stagger step pushes off (0.1)" },
        Param { name: "maxPushoffVel", kind: Kind::Float, used: true, doc: "its top speed, m/s (1)" },
        Param { name: "leanInDirRate", kind: Kind::Float, used: true, doc: "lean amount rise, per s (1)" },
        Param { name: "leanInDirMaxF", kind: Kind::Float, used: true, doc: "lean cap going forward (0.3)" },
        Param { name: "leanInDirMaxB", kind: Kind::Float, used: true, doc: "lean cap going back (0.15)" },
        Param { name: "leanHipsMaxF", kind: Kind::Float, used: true, doc: "hip lean going forward, of the joint range (0.3)" },
        Param { name: "leanHipsMaxB", kind: Kind::Float, used: true, doc: "hip lean going back (0.1)" },
        Param { name: "lean2multF", kind: Kind::Float, used: true, doc: "hip lean multiplier forward (1)" },
        Param { name: "lean2multB", kind: Kind::Float, used: true, doc: "hip lean multiplier back (1)" },
        Param { name: "hipBendMult", kind: Kind::Float, used: true, doc: "hip bend, 0.3 of range, times this (1)" },
        Param { name: "spineBendMult", kind: Kind::Float, used: true, doc: "spine bend times this (1)" },
        Param { name: "alwaysBendForwards", kind: Kind::Bool, used: true, doc: "bend forward even going back (false)" },
        Param { name: "headLookPos", kind: Kind::Vec3, used: true, doc: "where he looks (2 m ahead)" },
        Param { name: "headLookInstanceIndex", kind: Kind::Int, used: false, doc: "a game object index, not done (-1)" },
        Param { name: "headLookAtVelProb", kind: Kind::Float, used: true, doc: "chance a step looks along his velocity; above 1 always, -1 coin flip (0.5)" },
        Param { name: "turnOffProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.3)" },
        Param { name: "turn2TargetProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.1)" },
        Param { name: "turn2VelProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.3)" },
        Param { name: "turnAwayProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.1)" },
        Param { name: "turnLeftProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.1)" },
        Param { name: "turnRightProb", kind: Kind::Float, used: true, doc: "weight of this body turn (0.1)" },
        Param { name: "useBodyTurn", kind: Kind::Bool, used: true, doc: "body turns on (true)" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "head look on (true)" },
    ] },
    Message { name: "smartFall", params: &[
        Param { name: "bodyStiffness", kind: Kind::Float, used: true, doc: "body stiffness while airborne (10)" },
        Param { name: "bodydamping", kind: Kind::Float, used: true, doc: "its damping ratio (0.6)" },
        Param { name: "pdStrength", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "pdDamping", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "catchfalltime", kind: Kind::Float, used: true, doc: "catchFall runs this long after a land, s (1.5)" },
        Param { name: "catchFallCutOff", kind: Kind::Float, used: true, doc: "speed under which a landing counts settled, m/s (0.3)" },
        Param { name: "crashOrLandCutOff", kind: Kind::Float, used: true, doc: "ends this long after the landing at the latest, s (4)" },
        Param { name: "armsUp", kind: Kind::Float, used: true, doc: "arms raised by this, of the joint range (0.6)" },
        Param { name: "armAngSpeed", kind: Kind::Float, used: true, doc: "arm circling speed, rad/s (8)" },
        Param { name: "armAmplitude", kind: Kind::Float, used: true, doc: "arm circle size, of the joint range (0.7)" },
        Param { name: "armPhase", kind: Kind::Float, used: true, doc: "right arm behind the left, radians (3.14)" },
        Param { name: "armBendElbows", kind: Kind::Float, used: true, doc: "elbow bend, of the joint range (0.3)" },
        Param { name: "legRadius", kind: Kind::Float, used: true, doc: "pedalLegs radius, m (0.15)" },
        Param { name: "legAngSpeed", kind: Kind::Float, used: true, doc: "pedalLegs angular speed, rad/s (8)" },
        Param { name: "legAsymmetry", kind: Kind::Float, used: true, doc: "pedalLegs speed asymmetry (0)" },
        Param { name: "legL", kind: Kind::Bool, used: true, doc: "the left leg pedals too (true)" },
        Param { name: "sideD", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "forwardOffsetOfLegIK", kind: Kind::Float, used: true, doc: "pedalLegs centreForwards, m (0)" },
        Param { name: "legStrength", kind: Kind::Float, used: true, doc: "pedalLegs leg stiffness (8)" },
        Param { name: "arms2LegsPhase", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "arms2LegsSync", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "adaptiveCircling", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "hula", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "aimAngleBase", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "forwardVelRotation", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "footVelCompScale", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "orientateBodyToFallDirection", kind: Kind::Bool, used: true, doc: "turn to face the way he is falling (true)" },
        Param { name: "orientateTwist", kind: Kind::Float, used: true, doc: "spine twist into that turn, of the joint range (0.3)" },
        Param { name: "orientateMax", kind: Kind::Float, used: true, doc: "the turn is clamped to this angle, radians (0.8)" },
        Param { name: "minSpeedForBrace", kind: Kind::Float, used: true, doc: "landing faster than this he rolls rather than lands, m/s (4)" },
        Param { name: "maxSpeedForRecoverableFall", kind: Kind::Float, used: true, doc: "landing faster than this is a crash, m/s (12)" },
        Param { name: "landingNormal", kind: Kind::Vec3, used: true, doc: "the surface he lands on (no ground sense here: given; 0 0 1)" },
        Param { name: "balance", kind: Kind::Bool, used: true, doc: "bodyBalance on a land (true)" },
        Param { name: "forwardRoll", kind: Kind::Bool, used: true, doc: "a roll is bodyRollUp even on a slope (false)" },
        Param { name: "useZeroPose_withForwardRoll", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "ignorWorldCollisions", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "alanRickman", kind: Kind::Bool, used: true, doc: "limp, no flail: straight down (false)" },
        Param { name: "rdsForceMag", kind: Kind::Float, used: true, doc: "rollDownStairs forcemag on a crash down a slope (2)" },
        Param { name: "tumble", kind: Kind::Float, used: true, doc: "forward somersault in the air, rad/s, about the way he is going (0: none)" },
        Param { name: "rdsTargetLinearVelocity", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "rdsTargetLinearVelocityDecayTime", kind: Kind::Float, used: false, doc: "(not done: 0)" },
    ] },
    Message { name: "balancerCollisionsReaction", params: &[
        Param { name: "objectBehindVictimPos", kind: Kind::Vec3, used: true, doc: "the obstacle behind him (zero: a hip-high wall 0.4 m behind)" },
        Param { name: "objectBehindVictimNormal", kind: Kind::Vec3, used: true, doc: "its normal, toward him (zero: from the contact)" },
        Param { name: "numStepsTillSlump", kind: Kind::Int, used: true, doc: "steps allowed before forcing a slump, 0.3 s each (3)" },
        Param { name: "stable2SlumpTime", kind: Kind::Float, used: true, doc: "still this long after impact: slump anyway, s (1.0)" },
        Param { name: "exclusionZone", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "footFrictionMultStart", kind: Kind::Float, used: false, doc: "(not done: 1)" },
        Param { name: "footFrictionMultRate", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "backFrictionMultStart", kind: Kind::Float, used: false, doc: "(not done: 1)" },
        Param { name: "backFrictionMultRate", kind: Kind::Float, used: false, doc: "(not done: 0)" },
        Param { name: "impactLegStiffReduction", kind: Kind::Float, used: true, doc: "leg stiffness dropped on impact, of its value (0.3)" },
        Param { name: "slumpLegStiffReduction", kind: Kind::Float, used: true, doc: "leg stiffness dropped when slumping, of its value (0.8)" },
        Param { name: "slumpLegStiffRate", kind: Kind::Float, used: true, doc: "slump drop reached at this per second (1.0)" },
        Param { name: "reactTime", kind: Kind::Float, used: true, doc: "contact gathered this long before it reacts, s (0.25)" },
        Param { name: "impactExagTime", kind: Kind::Float, used: true, doc: "impact exaggerated for this long, s (0.2)" },
        Param { name: "glanceSpinTime", kind: Kind::Float, used: true, doc: "glance-spin lasts, s (0.5)" },
        Param { name: "glanceSpinMag", kind: Kind::Float, used: true, doc: "glance-spin torque, of weight x metre / 2 (0.3)" },
        Param { name: "glanceSpinDecayMult", kind: Kind::Float, used: true, doc: "its decay per 0.1 s (0.8)" },
    ] },
    Message { name: "injuredOnGround", params: &[
        Param { name: "numInjuries", kind: Kind::Int, used: true, doc: "wounds to reach for, 0 to 2 (0: one on the left of the belly)" },
        Param { name: "injury1LocalPosition", kind: Kind::Vec3, used: true, doc: "wound 1, in its body part's frame, m (zero)" },
        Param { name: "injury1LocalNormal", kind: Kind::Vec3, used: true, doc: "its outward normal, the hand stops this far off (zero)" },
        Param { name: "injury1Component", kind: Kind::Int, used: true, doc: "its body part index (7, the lower spine)" },
        Param { name: "injury2LocalPosition", kind: Kind::Vec3, used: true, doc: "wound 2, as wound 1 (zero)" },
        Param { name: "injury2LocalNormal", kind: Kind::Vec3, used: true, doc: "its outward normal (zero)" },
        Param { name: "injury2Component", kind: Kind::Int, used: true, doc: "its body part index (7, the lower spine)" },
        Param { name: "attackerPos", kind: Kind::Vec3, used: true, doc: "where he looks (zero: 2 m beyond his head, along the ground)" },
        Param { name: "dontReachWithLeft", kind: Kind::Bool, used: true, doc: "the left hand stays off the wounds (false)" },
        Param { name: "dontReachWithRight", kind: Kind::Bool, used: true, doc: "the right hand stays off the wounds (false)" },
        Param { name: "strongRollForce", kind: Kind::Bool, used: true, doc: "roll about twice as hard (false)" },
    ] },
    Message { name: "learnedCrawl", params: &[
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "(10)" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "(0.6)" },
        Param { name: "bLearn", kind: Kind::Bool, used: false, doc: "(not done: the crawl cycle is ours, nothing learned)" },
        Param { name: "numFrames2Learn", kind: Kind::Int, used: false, doc: "(not done: the crawl cycle is ours, nothing learned)" },
        Param { name: "animIndex", kind: Kind::Int, used: false, doc: "(not done: the crawl cycle is ours, nothing learned)" },
        Param { name: "learnFromAnimPlayback", kind: Kind::Bool, used: false, doc: "(not done: the crawl cycle is ours, nothing learned)" },
        Param { name: "inputSequenceSize", kind: Kind::Int, used: false, doc: "(not done: the crawl cycle is ours, nothing learned)" },
        Param { name: "speed", kind: Kind::Float, used: true, doc: "crawl cycles a second (0.8)" },
        Param { name: "targetPosition", kind: Kind::Vec3, used: true, doc: "where he crawls to (zero: 3 m ahead of him)" },
        Param { name: "yawOffset", kind: Kind::Float, used: true, doc: "rad, heading this much off the target (0)" },
        Param { name: "useSpine3Thing", kind: Kind::Bool, used: false, doc: "(not done)" },
        Param { name: "useRollBoneCompensation", kind: Kind::Bool, used: false, doc: "(not done)" },
        Param { name: "useTwister", kind: Kind::Bool, used: true, doc: "the spine twists toward the heading (true)" },
    ] },
];

pub fn build(name: &str, p: &mut P) -> Option<Result<Box<dyn Extra>, Error>> {
    fn b<T: Extra + 'static>(e: Result<T, Error>) -> Option<Result<Box<dyn Extra>, Error>> {
        Some(e.map(|e| Box::new(e) as _))
    }
    match name {
        "teeter" => b(Teeter::new(p)),
        "stumble" => b(Stumble::new(p)),
        "staggerFall" => b(StaggerFall::new(p)),
        "smartFall" => b(SmartFall::new(p)),
        "balancerCollisionsReaction" => b(Collide::new(p)),
        "injuredOnGround" => b(Injured::new(p)),
        "learnedCrawl" => b(Crawl::new(p)),
        _ => None,
    }
}

#[allow(non_snake_case)]
#[inline]
fn CHEST() -> usize { (*SPINE())[delirium_data::kv!(0 int)] + 1 }
const TAU: f32 = std::f32::consts::TAU;

fn weight(c: &Ctx) -> f32 {
    c.b.doll.mass() * delirium_data::kv!(105)
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0).normalize_or_zero()
}

fn travel(c: &Ctx, d: Vec3) -> Vec3 {
    if c.vel.truncate().length() > delirium_data::kv!(106) { flat(c.vel) } else { flat(d) }
}

fn windmill(c: &mut Ctx, t: f32, amp: f32) {
    for i in 0..2 {
        let ph = t + i as f32 * TAU * 0.5;
        c.ch.set_relative((*SHOULDER())[i], 0, amp * ph.sin());
        c.ch.set_relative((*SHOULDER())[i], 1, amp * ph.cos());
        c.ch.set_relative((*ELBOW())[i], 0, -delirium_data::kv!(24));
    }
}

pub struct Teeter {
    ends: (Vec3, Vec3),
    react: f32,
    restart: f32,
    head: bool,
    high_fall: bool,
    lean: bool,
    pre: f32,
    lean_time: f32,
    teeter_time: f32,
    lean_scale: f32,
    normal: Vec3,
    state: u8,
    t: f32,
    out: Vec<String>,
}
const PRE: u8 = 0;
const TEETERING: u8 = 1;
const FALLING: u8 = 2;
#[allow(non_snake_case)]
#[inline]
fn RECOVERED() -> u8 { delirium_data::kv!(0 int) }
impl Teeter {
    fn new(p: &mut P) -> Result<Teeter, Error> {
        p.b("useExclusionZone", false)?;
        Ok(Teeter { ends: (p.v("edgeLeft", Vec3::ZERO)?, p.v("edgeRight", Vec3::ZERO)?), react: p.f("reactDistance", 0.5)?,
                    restart: p.f("restartDistance", delirium_data::kv!(20))?, head: p.b("useHeadLook", true)?, high_fall: p.b("callHighFall", true)?,
                    lean: p.b("leanAway", true)?, pre: p.f("preTeeterTime", delirium_data::kv!(57))?, lean_time: p.f("leanAwayTime", delirium_data::kv!(20))?,
                    teeter_time: p.f("teeterTime", 2.0)?, lean_scale: p.f("leanAwayScale", delirium_data::kv!(73))?, normal: Vec3::ZERO, state: PRE,
                    t: 0.0, out: vec![] })
    }

    fn to(&mut self, state: u8, name: &str) {
        (self.state, self.t) = (state, 0.0);
        self.out.push(format!("#say teeter {name}"));
    }

    fn look(&mut self, p: Vec3) {
        if self.head {
            self.out.push(format!("headLook start=true pos={},{},{} stiffness={} damping=1", p.x, p.y, p.z, delirium_data::kv!(107)));
        }
    }
}
impl Extra for Teeter {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let (com, vel) = (c.com, c.vel);
        if self.normal == Vec3::ZERO {
            if self.ends.0 == self.ends.1 {
                let cl = (*crate::nmrs::CLAVICLE());
                let fwd = flat((c.b.part(cl[0] + 1).m.d - c.b.part(cl[1] + 1).m.d).cross(Vec3::Z));
                let side = Vec3::Z.cross(fwd);
                let at = Vec3::new(com.x, com.y, c.b.part((*ANKLE())[0] + 1).m.d.z) + fwd * 0.5;
                self.ends = (at + side * 0.5, at - side * 0.5);
            }
            let n = flat(Vec3::Z.cross(self.ends.1 - self.ends.0));
            self.normal = if (self.ends.0 - com).dot(n) > 0.0 { n } else { -n };
        }
        let (a, b) = self.ends;
        let com2edge = (a - com).dot(self.normal);
        let vel2edge = vel.dot(self.normal);
        self.t += dt;
        if self.state < FALLING && (com2edge < 0.0 || com.z < a.z - delirium_data::kv!(24)) {
            self.to(FALLING, "falling");
            self.look(com + vel);
            self.out.push("leanInDirection dir=0,0,1 leanAmount=0".into());
            if self.high_fall {
                self.out.push("highFall start=true".into());
            }
            return;
        }
        match self.state {
            PRE if self.t >= self.pre && com2edge < self.react => {
                self.to(TEETERING, "teetering");
                let u = ((com - a).dot(b - a) / (b - a).length_squared().max(delirium_data::kv!(47))).clamp(0.0, 1.0);
                self.look(a + (b - a) * u);
                if self.lean {
                    let d = -self.normal;
                    self.out.push(format!("leanInDirection dir={},{},0 leanAmount={}", d.x, d.y, self.lean_scale));
                }
            }
            TEETERING => {
                if self.lean && self.t >= self.lean_time && self.t - dt < self.lean_time {
                    self.out.push("leanInDirection dir=0,0,1 leanAmount=0".into());
                }
                let urgency = (1.0 - com2edge / self.react.max(delirium_data::kv!(48)) + vel2edge.max(0.0)).clamp(delirium_data::kv!(24), 1.0);
                c.ch.set_stiffness(delirium_data::kv!(21), 0.5, "ua", None);
                windmill(c, self.t * delirium_data::kv!(61), delirium_data::kv!(108) * urgency);
                for &j in &(*SPINE()) {
                    c.ch.set_relative(j, 2, delirium_data::kv!(24) * urgency * (self.t * delirium_data::kv!(60)).sin());
                }
                if self.t >= self.teeter_time && com2edge > self.restart {
                    self.to(RECOVERED(), "recovered");
                    if self.head {
                        self.out.push("headLook start=false".into());
                    }
                }
            }
            _ => {}
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.state >= FALLING
    }
}

pub struct Stumble {
    legs: f32, torso: f32, arms_k: f32, twist: f32, fwd_off: f32, back_off: f32, brace: bool,
    rate: f32, max_back: f32, max_fwd: f32, towards: f32, z_spin: f32, damp_pelvis: f32, twist_spine: bool,
    head: bool, stagger: f32, drop: f32, different: bool,
    lean: f32, sent: f32, z0: f32, state: u8, t: f32, out: Vec<String>,
}
impl Stumble {
    fn new(p: &mut P) -> Result<Stumble, Error> {
        p.f("armReduceSpeed", delirium_data::kv!(57))?; p.f("wristMS", delirium_data::kv!(26))?; p.f("feetMS", delirium_data::kv!(26))?; p.s("fallMask", "")?;
        p.f("grabRadius2", 0.5)?; p.f("pitchInContact", 0.0)?;
        Ok(Stumble { legs: p.f("legsStiffness", delirium_data::kv!(21))?, torso: p.f("torsoStiffness", delirium_data::kv!(27))?, arms_k: p.f("armsStiffness", delirium_data::kv!(27))?,
                     twist: p.f("armTwist", delirium_data::kv!(24))?, fwd_off: p.f("forwardMaxArmOffset", delirium_data::kv!(108))?,
                     back_off: p.f("backwardsMinArmOffset", delirium_data::kv!(57))?, brace: p.b("useArmsBrace", true)?,
                     rate: p.f("leanRate", 0.5)?, max_back: p.f("maxLeanBack", delirium_data::kv!(73))?, max_fwd: p.f("maxLeanForward", delirium_data::kv!(24))?,
                     towards: p.f("leanTowards", delirium_data::kv!(57))?, z_spin: p.f("zAxisSpinReduction", 0.5)?,
                     damp_pelvis: p.f("dampPelvis", 0.0)?, twist_spine: p.b("twistSpine", true)?,
                     head: p.b("useHeadLook", true)?, stagger: p.f("staggerTime", delirium_data::kv!(12))?, drop: p.f("dropVal", delirium_data::kv!(106))?,
                     different: p.b("different", false)?, lean: 0.0, sent: 0.0, z0: f32::NAN, state: 0, t: 0.0, out: vec![] })
    }

    fn end(&mut self, state: u8, name: &str) {
        self.state = state;
        self.out.push(format!("#say stumble {name}"));
        if self.head {
            self.out.push("headLook start=false".into());
        }
        if state == 2 {
            self.out.push(format!("catchFall start=true useHeadLook={} armsStiffness={} torsoStiffness={} legsStiffness={} \
                                   forwardMaxArmOffset={} backwardsMinArmOffset={} zAxisSpinReduction={}",
                                  self.head, self.arms_k, self.torso, self.legs, self.fwd_off, self.back_off, self.z_spin));
        }
        self.out.push("leanInDirection dir=0,0,1 leanAmount=0".into());
    }
}
impl Extra for Stumble {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let w = weight(c);
        let d = travel(c, -Vec3::Y);
        let first = self.t == 0.0;
        if first {
            self.z0 = c.com.z;
            self.out.push("#say stumble start".into());
            self.out.push(format!("bodyBalance start=true bodyStiffness={} useHeadLook=false", self.torso));
            if self.head {
                let p = c.com + d * 2.0;
                self.out.push(format!("headLook start=true pos={},{},{} stiffness={} damping=1", p.x, p.y, p.z, delirium_data::kv!(107)));
            }
        }
        self.t += dt;
        if self.state != 0 {
            return;
        }
        if c.com.z < self.z0 - self.drop || self.t >= self.stagger {
            return self.end(2, "down");
        }
        if self.t > 0.5 && c.vel.truncate().length() < delirium_data::kv!(24) {
            return self.end(1, "recovered");
        }
        c.ch.set_stiffness(self.torso, 0.5, "ub", None);
        c.ch.set_stiffness((self.arms_k - 1.0).max(0.0), 0.5, "ua", None);
        c.ch.set_stiffness(self.legs, 0.5, "lb", None);
        let goal = self.towards.clamp(-self.max_back, self.max_fwd);
        self.lean += (goal - self.lean).clamp(-self.rate * dt, self.rate * dt);
        if (self.lean - self.sent).abs() > delirium_data::kv!(109) {
            self.sent = self.lean;
            self.out.push(format!("leanInDirection dir={},{},0 leanAmount={}", d.x, d.y, self.lean));
        }
        c.out.torque(0, -Vec3::Z * (c.angvel.z * self.z_spin * w * delirium_data::kv!(73)));
        if self.damp_pelvis > 0.0 {
            c.out.torque(0, -c.angvel * (self.damp_pelvis * w * delirium_data::kv!(73)));
        }
        if self.brace {
            let u = (self.lean / self.max_fwd.max(delirium_data::kv!(48))).clamp(0.0, 1.0);
            let reach = self.back_off + (self.fwd_off - self.back_off) * u;
            for i in 0..2 {
                let s = if i == 0 { 1.0 } else { -1.0 };
                let r = if self.different && i == 1 { reach * delirium_data::kv!(110) } else { reach };
                c.ch.set_relative((*SHOULDER())[i], 0, r);
                c.ch.set_relative((*SHOULDER())[i], 1, delirium_data::kv!(24));
                c.ch.set_relative((*SHOULDER())[i], 2, s * self.twist);
                c.ch.set_relative((*ELBOW())[i], 0, -delirium_data::kv!(57));
            }
        }
        if self.twist_spine {
            for &j in &(*SPINE()) {
                c.ch.set_relative(j, 2, delirium_data::kv!(57) * (self.t * delirium_data::kv!(104)).sin());
            }
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.state != 0
    }
}

pub struct StaggerFall {
    arm: [f32; 2],
    arm_start: [f32; 2],
    spine: [f32; 2],
    spine_start: [f32; 2],
    hold: f32,
    ramp: f32,
    legs: f32,
    legs_end: f32,
    prob: f32,
    steps_end: f32,
    time_end: f32,
    ramp_end: f32,
    reduce: f32,
    push_dist: f32,
    push_vel: f32,
    lean_rate: f32,
    lean_max: [f32; 2],
    hips_max: [f32; 2],
    lean2: [f32; 2],
    hip_bend: f32,
    spine_bend: f32,
    always_fwd: bool,
    head_pos: Vec3,
    head_vel_prob: f32,
    head: bool,
    turn: String,
    t: f32,
    t_end: f32,
    steps: u32,
    side: f32,
    push: f32,
    state: u8,
    seed: u32,
    out: Vec<String>,
}
impl StaggerFall {
    fn new(p: &mut P) -> Result<StaggerFall, Error> {
        let arm = [p.f("armStiffness", delirium_data::kv!(32))?, p.f("armDamping", 0.5)?];
        let spine = [p.f("spineStiffness", delirium_data::kv!(61))?, p.f("spineDamping", 0.5)?];
        let arm_start = [p.f("armStiffnessStart", delirium_data::kv!(21))?, p.f("armDampingStart", 1.0)?];
        let spine_start = [p.f("spineStiffnessStart", delirium_data::kv!(21))?, p.f("spineDampingStart", 1.0)?];
        let (hold, ramp) = (p.f("timeAtStartValues", 0.5)?, p.f("rampTimeFromStartValues", 1.0)?);
        let (legs, legs_end) = (p.f("lowerBodyStiffness", delirium_data::kv!(21))?, p.f("lowerBodyStiffnessEnd", delirium_data::kv!(5))?);
        let prob = p.f("staggerStepProb", 0.5)?;
        let steps_end = p.i("stepsTillStartEnd", delirium_data::kv!(6 int))? as f32;
        let (time_end, ramp_end) = (p.f("timeStartEnd", delirium_data::kv!(32))?, p.f("rampTimeToEndValues", 2.0)?);
        p.f("predictionTime", delirium_data::kv!(24))?;
        let reduce = p.f("perStepReduction1", delirium_data::kv!(19))?;
        let (push_dist, push_vel) = (p.f("pushOffDist", delirium_data::kv!(73))?, p.f("maxPushoffVel", 1.0)?);
        let lean_rate = p.f("leanInDirRate", 1.0)?;
        let lean_max = [p.f("leanInDirMaxF", delirium_data::kv!(24))?, p.f("leanInDirMaxB", delirium_data::kv!(9))?];
        let hips_max = [p.f("leanHipsMaxF", delirium_data::kv!(24))?, p.f("leanHipsMaxB", delirium_data::kv!(73))?];
        let lean2 = [p.f("lean2multF", 1.0)?, p.f("lean2multB", 1.0)?];
        let (hip_bend, spine_bend) = (p.f("hipBendMult", 1.0)?, p.f("spineBendMult", 1.0)?);
        let always_fwd = p.b("alwaysBendForwards", false)?;
        let head_pos = p.v("headLookPos", Vec3::ZERO)?;
        p.i("headLookInstanceIndex", -1)?;
        let head_vel_prob = p.f("headLookAtVelProb", 0.5)?;
        let turn_off = p.f("turnOffProb", delirium_data::kv!(24))?;
        let (t2t, t2v) = (p.f("turn2TargetProb", delirium_data::kv!(73))?, p.f("turn2VelProb", delirium_data::kv!(24))?);
        let (away, left, right) = (p.f("turnAwayProb", delirium_data::kv!(73))?, p.f("turnLeftProb", delirium_data::kv!(73))?, p.f("turnRightProb", delirium_data::kv!(73))?);
        let body_turn = p.b("useBodyTurn", true)?;
        let head = p.b("useHeadLook", true)?;
        let turn = format!("turnOffProb={turn_off} turn2TargetProb={t2t} turn2VelProb={t2v} turnAwayProb={away} turnLeftProb={left} \
                            turnRightProb={right} useBodyTurn={body_turn}");
        Ok(StaggerFall { arm, arm_start, spine, spine_start, hold, ramp, legs, legs_end, prob, steps_end, time_end, ramp_end, reduce,
                         push_dist, push_vel, lean_rate, lean_max, hips_max, lean2, hip_bend, spine_bend, always_fwd, head_pos,
                         head_vel_prob, head, turn, t: 0.0, t_end: 0.0, steps: 0, side: 0.0, push: 0.0, state: 0, seed: delirium_data::kv!(111 int),
                         out: vec![] })
    }

    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(delirium_data::kv!(112 int)).wrapping_add(delirium_data::kv!(113 int));
        (self.seed >> delirium_data::kv!(23 int)) as f32 / (1u32 << delirium_data::kv!(96 int)) as f32
    }
}
impl Extra for StaggerFall {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let cl = (*crate::nmrs::CLAVICLE());
        let fwd = flat((c.b.part(cl[0] + 1).m.d - c.b.part(cl[1] + 1).m.d).cross(Vec3::Z));
        let dir = travel(c, fwd);
        if self.t == 0.0 {
            self.out.push(format!("bodyBalance start=true bodyStiffness={} useHeadLook=false {}", self.legs, self.turn));
            self.out.push("#say staggerFall staggering".into());
        }
        self.t += dt;
        let s = (c.b.part((*ANKLE())[0] + 1).m.d - c.b.part((*ANKLE())[1] + 1).m.d).dot(dir);
        let stepped = s.abs() > delirium_data::kv!(64) && self.side != 0.0 && s.signum() != self.side;
        if s.abs() > delirium_data::kv!(64) {
            self.side = s.signum();
        }
        let fwd_dot = dir.dot(fwd);
        let k = (fwd_dot < 0.0) as usize;
        if stepped {
            self.steps += 1;
            if self.rand() < self.prob {
                self.push = delirium_data::kv!(9);
            }
            let amount = (self.lean_rate * self.t).min(self.lean_max[k]);
            self.out.push(format!("leanInDirection dir={},{},{} leanAmount={amount}", dir.x, dir.y, dir.z));
            if self.head {
                let v = self.head_vel_prob;
                let at_vel = v > 1.0 || (v < 0.0 && self.rand() < 0.5) || (v > 0.0 && self.rand() < v);
                let at = if self.head_pos == Vec3::ZERO { c.com + dir * 2.0 } else { self.head_pos };
                self.out.push(if at_vel {
                    format!("headLook start=true vel={},{},{} stiffness={} damping=1", c.vel.x, c.vel.y, c.vel.z, delirium_data::kv!(107))
                } else {
                    format!("headLook start=true pos={},{},{} stiffness={} damping=1", at.x, at.y, at.z, delirium_data::kv!(107))
                });
            }
        }
        if self.state == 0 && (self.steps as f32 >= self.steps_end || self.t >= self.time_end) {
            (self.state, self.t_end) = (1, 0.0);
            self.out.push("#say staggerFall winding down".into());
        }
        if self.state == 1 {
            self.t_end += dt;
        }
        let u = ((self.t - self.hold) / self.ramp.max(dt)).clamp(0.0, 1.0);
        let e = if self.state == 1 { (self.t_end / self.ramp_end.max(dt)).min(1.0) } else { 0.0 };
        let fat = self.reduce.powi(self.steps as i32).max(delirium_data::kv!(57));
        let mix = |a: [f32; 2], b: [f32; 2], i: usize| (a[i] + (b[i] - a[i]) * u) * fat * (1.0 - e) + e;
        c.ch.set_stiffness(mix(self.arm_start, self.arm, 0), mix(self.arm_start, self.arm, 1), "ua", None);
        c.ch.set_stiffness(mix(self.spine_start, self.spine, 0), mix(self.spine_start, self.spine, 1), "us", None);
        c.ch.set_stiffness(self.legs + (self.legs_end - self.legs) * e, 0.5, "lb", None);
        for j in (*NECK()).into_iter().chain((*SPINE())[..delirium_data::kv!(0 int)].iter().copied()).chain(cl) {
            c.ch.eff[j].oppose_gravity = 0.0;
        }
        let sign = if self.always_fwd || fwd_dot >= 0.0 { 1.0 } else { -1.0 };
        let lean = (self.lean_rate * self.t).min(1.0);
        let hips = (delirium_data::kv!(24) * self.hip_bend + self.hips_max[k] * self.lean2[k] * lean).min(1.0);
        for i in 0..2 {
            c.ch.set_relative((*HIP())[i], 0, -sign * hips);
        }
        for &j in &(*SPINE())[..delirium_data::kv!(0 int)] {
            c.ch.set_relative(j, 0, sign * (delirium_data::kv!(24) * self.spine_bend).min(1.0));
        }
        if self.push > 0.0 {
            self.push -= dt;
            let v = (self.push_dist / delirium_data::kv!(9)).min(self.push_vel);
            c.out.force(0, dir * (c.b.doll.mass() * v / delirium_data::kv!(9)));
        }
        if (c.dbm_failed && self.t > 0.5) || self.t_end > self.ramp_end + delirium_data::kv!(104) {
            self.state = 2;
            self.out.push("#say staggerFall fallen".into());
            for j in (*NECK()).into_iter().chain((*SPINE())[..delirium_data::kv!(0 int)].iter().copied()).chain(cl).chain((*ANKLE())).chain((*KNEE())).chain((*HIP())) {
                c.ch.eff[j].oppose_gravity = 1.0;
            }
            if self.head {
                self.out.push("headLook start=false".into());
            }
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.state == 2
    }
}

pub struct SmartFall {
    stiff: (f32, f32),
    catch_time: f32,
    cutoff: f32,
    end_time: f32,
    arms: (f32, f32, f32, f32, f32),
    legs: String,
    orient: (bool, f32, f32),
    brace: f32,
    max: f32,
    normal: Vec3,
    balance: bool,
    fwd_roll: bool,
    limp: bool,
    rds: f32,
    tumble: f32,
    axis: Vec3,
    z0: f32,
    state: u8,
    t: f32,
    peak: f32,
    air: bool,
    started: Vec<&'static str>,
    out: Vec<String>,
}
const SF_AIR: u8 = 0;
const SF_LAND: u8 = 1;
const SF_ROLL: u8 = 2;
#[allow(non_snake_case)]
#[inline]
fn SF_CRASH() -> u8 { delirium_data::kv!(0 int) }
#[allow(non_snake_case)]
#[inline]
fn SF_DONE() -> u8 { delirium_data::kv!(1 int) }
impl SmartFall {
    fn new(p: &mut P) -> Result<SmartFall, Error> {
        let stiff = (p.f("bodyStiffness", delirium_data::kv!(27))?, p.f("bodydamping", delirium_data::kv!(110))?);
        let (catch_time, cutoff, end_time) = (p.f("catchfalltime", delirium_data::kv!(12))?, p.f("catchFallCutOff", delirium_data::kv!(24))?, p.f("crashOrLandCutOff", delirium_data::kv!(104))?);
        let arms = (p.f("armsUp", delirium_data::kv!(110))?, p.f("armAngSpeed", delirium_data::kv!(61))?, p.f("armAmplitude", delirium_data::kv!(70))?, p.f("armPhase", delirium_data::kv!(114))?,
                    p.f("armBendElbows", delirium_data::kv!(24))?);
        let legs = format!("pedalLegs start=true radius={} angularSpeed={} speedAsymmetry={} pedalLeftLeg={} pedalRightLeg=true \
                            centreForwards={} legStiffness={}", p.f("legRadius", delirium_data::kv!(9))?, p.f("legAngSpeed", delirium_data::kv!(61))?,
                           p.f("legAsymmetry", 0.0)?, p.b("legL", true)?, p.f("forwardOffsetOfLegIK", 0.0)?, p.f("legStrength", delirium_data::kv!(61))?);
        let orient = (p.b("orientateBodyToFallDirection", true)?, p.f("orientateTwist", delirium_data::kv!(24))?, p.f("orientateMax", delirium_data::kv!(20))?);
        let (brace, max) = (p.f("minSpeedForBrace", delirium_data::kv!(104))?, p.f("maxSpeedForRecoverableFall", delirium_data::kv!(21))?);
        let normal = p.v("landingNormal", Vec3::Z)?.normalize_or(Vec3::Z);
        let (balance, fwd_roll, limp) = (p.b("balance", true)?, p.b("forwardRoll", false)?, p.b("alanRickman", false)?);
        let (rds, tumble) = (p.f("rdsForceMag", 2.0)?, p.f("tumble", 0.0)?);
        for n in ["pdStrength", "pdDamping", "sideD", "arms2LegsPhase", "hula", "aimAngleBase", "forwardVelRotation", "footVelCompScale",
                  "rdsTargetLinearVelocity", "rdsTargetLinearVelocityDecayTime"] {
            p.f(n, 0.0)?;
        }
        for n in ["arms2LegsSync", "adaptiveCircling", "useZeroPose_withForwardRoll", "ignorWorldCollisions"] {
            p.b(n, false)?;
        }
        Ok(SmartFall { stiff, catch_time, cutoff, end_time, arms, legs, orient, brace, max, normal, balance, fwd_roll, limp, rds,
                       tumble, axis: Vec3::ZERO, z0: 0.0, state: SF_AIR, t: 0.0, peak: 0.0, air: false, started: vec![], out: vec![] })
    }

    fn to(&mut self, state: u8, name: &str) {
        (self.state, self.t) = (state, 0.0);
        self.out.push(format!("#say smartFall {name}"));
    }

    fn start(&mut self, name: &'static str, line: String) {
        self.out.push(line);
        self.started.push(name);
    }

    fn stop(&mut self, name: &'static str) {
        if self.started.contains(&name) {
            self.started.retain(|&s| s != name);
            self.out.push(format!("{name} start=false"));
        }
    }
}
impl Extra for SmartFall {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let w = weight(c);
        if self.state == SF_AIR {
            self.peak = self.peak.max(-c.vel.z);
            if self.peak < delirium_data::kv!(12) {
                return;
            }
            if !self.air {
                self.air = true;
                self.t = 0.0;
                self.out.push("#say smartFall airborne".into());
                let cl = (*crate::nmrs::CLAVICLE());
                let fwd = flat((c.b.part(cl[0] + 1).m.d - c.b.part(cl[1] + 1).m.d).cross(Vec3::Z));
                let d = if c.vel.truncate().length() > delirium_data::kv!(24) { flat(c.vel) } else { fwd };
                self.axis = Vec3::Z.cross(d).normalize_or_zero();
                self.z0 = c.com.z;
                if !self.limp {
                    self.start("pedalLegs", self.legs.clone());
                }
            }
            if self.limp {
                c.ch.set_stiffness(2.0, delirium_data::kv!(20), "fb", None);
            } else {
                c.ch.set_stiffness(self.stiff.0, self.stiff.1, "fb", None);
                let (up, rate, amp, phase, bend) = self.arms;
                for i in 0..2 {
                    let ph = self.t * rate + i as f32 * phase;
                    c.ch.set_relative((*SHOULDER())[i], 0, (up + amp * ph.sin()).clamp(-1.0, 1.0));
                    c.ch.set_relative((*SHOULDER())[i], 1, (amp * ph.cos()).clamp(-1.0, 1.0));
                    c.ch.set_relative((*ELBOW())[i], 0, -bend);
                }
                let (on, twist, max) = self.orient;
                if on && c.vel.truncate().length() > 0.5 {
                    let cl = (*crate::nmrs::CLAVICLE());
                    let fwd = flat((c.b.part(cl[0] + 1).m.d - c.b.part(cl[1] + 1).m.d).cross(Vec3::Z));
                    let d = flat(c.vel);
                    let a = (fwd.cross(d).z.atan2(fwd.dot(d)) / max.max(delirium_data::kv!(48))).clamp(-1.0, 1.0);
                    c.out.torque(0, Vec3::Z * (w * (delirium_data::kv!(9) * a - delirium_data::kv!(109) * c.angvel.z)));
                    for &j in &(*SPINE()) {
                        c.ch.set_relative(j, 2, twist * a);
                    }
                }
            }
            if self.tumble != 0.0 && self.axis != Vec3::ZERO {
                c.out.torque(0, self.axis * (w * delirium_data::kv!(73) * (self.tumble - c.angvel.dot(self.axis))));
            }
            let all: Vec<usize> = (0..c.b.build.parts.len()).collect();
            if self.t > delirium_data::kv!(9) && c.b.doll.collided(&all) && (self.tumble == 0.0 || c.com.z < self.z0 - 1.0 && c.vel.z > -delirium_data::kv!(110) * self.peak) {
                self.out.push(format!("#say smartFall landing speed={:.1}", self.peak));
                let d = flat(-self.normal);
                let slope = self.normal.z < delirium_data::kv!(115);
                if self.limp || self.peak > self.max {
                    self.to(SF_CRASH(), "crash");
                    if slope {
                        self.start("rollDownStairs", format!("rollDownStairs start=true forcemag={} useCustomRollDir=true customRollDir={},{},{}",
                                                             self.rds, d.x, d.y, d.z));
                    }
                    self.out.push(format!("catchFall start=true useHeadLook=true armsStiffness={} torsoStiffness={} legsStiffness={}                                    forwardMaxArmOffset={} backwardsMinArmOffset={} zAxisSpinReduction=0.5", delirium_data::kv!(27), delirium_data::kv!(27), delirium_data::kv!(21), delirium_data::kv!(108), delirium_data::kv!(57)));
                } else if self.peak > self.brace {
                    self.to(SF_ROLL, "roll");
                    if slope && !self.fwd_roll {
                        self.start("rollDownStairs", format!("rollDownStairs start=true useCustomRollDir=true customRollDir={},{},{}", d.x, d.y, d.z));
                    } else {
                        self.start("bodyRollUp", "bodyRollUp start=true".into());
                    }
                } else {
                    self.to(SF_LAND, "land");
                    self.start("catchFall", "catchFall start=true".into());
                    if self.balance {
                        self.start("bodyBalance", "bodyBalance start=true".into());
                    }
                }
                self.stop("pedalLegs");
            }
        } else {
            if self.state == SF_LAND && self.t >= self.catch_time {
                self.stop("catchFall");
            }
            if self.t > 0.5 && (c.vel.length() < self.cutoff || self.t >= self.end_time) {
                self.to(SF_DONE(), "settled");
                for s in std::mem::take(&mut self.started) {
                    self.out.push(format!("{s} start=false"));
                }
            }
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.state == SF_DONE()
    }
}

pub struct Collide {
    pos: Vec3, normal: Vec3,
    steps: i32, stable: f32, leg_red: f32, slump_red: f32, slump_rate: f32,
    react: f32, exag: f32, spin_t: f32, spin_mag: f32, spin_decay: f32,
    given: bool, virt: bool, first: bool, state: u8, t: f32, calm: f32, hit_t: f32,
    ub_n: Vec3, lb_n: Vec3, ub: bool, lb: bool, glance: f32, fall: bool, out: Vec<String>,
}
const BAL_NORMAL: u8 = 0;
const BAL_IMPACT: u8 = 1;
const BAL_REBOUND: u8 = 2;
const BAL_DRAPE: u8 = 3;
const BAL_DRAPE_FWD: u8 = 4;
const BAL_SLUMP: u8 = 5;
const BAL_END: u8 = 7;
#[allow(non_snake_case)]
#[inline]
fn BCR_STEP() -> f32 { delirium_data::kv!(24) }
const BCR_NAMES: [&str; 8] = ["normal", "impact", "rebound", "drape", "drapeForward", "slump", "trip", "end"];
impl Collide {
    fn new(p: &mut P) -> Result<Collide, Error> {
        p.f("exclusionZone", 0.0)?; p.f("footFrictionMultStart", 1.0)?; p.f("footFrictionMultRate", 0.0)?;
        p.f("backFrictionMultStart", 1.0)?; p.f("backFrictionMultRate", 0.0)?;
        Ok(Collide { pos: p.v("objectBehindVictimPos", Vec3::ZERO)?, normal: p.v("objectBehindVictimNormal", Vec3::ZERO)?,
                     steps: p.i("numStepsTillSlump", delirium_data::kv!(0 int))?, stable: p.f("stable2SlumpTime", 1.0)?,
                     leg_red: p.f("impactLegStiffReduction", delirium_data::kv!(24))?, slump_red: p.f("slumpLegStiffReduction", delirium_data::kv!(20))?,
                     slump_rate: p.f("slumpLegStiffRate", 1.0)?, react: p.f("reactTime", delirium_data::kv!(17))?,
                     exag: p.f("impactExagTime", delirium_data::kv!(57))?, spin_t: p.f("glanceSpinTime", 0.5)?,
                     spin_mag: p.f("glanceSpinMag", delirium_data::kv!(24))?, spin_decay: p.f("glanceSpinDecayMult", delirium_data::kv!(20))?,
                     given: false, virt: false, first: true, state: BAL_NORMAL, t: 0.0, calm: 0.0, hit_t: 0.0,
                     ub_n: Vec3::ZERO, lb_n: Vec3::ZERO, ub: false, lb: false, glance: 0.0, fall: false, out: vec![] })
    }

    fn to(&mut self, state: u8) {
        (self.state, self.t) = (state, 0.0);
        self.out.push(format!("#say bcr {}", BCR_NAMES[state as usize]));
    }

    fn catch_fall(&mut self) {
        if !self.fall {
            self.fall = true;
            self.out.push("catchFall start=true useHeadLook=true armsStiffness=10 torsoStiffness=10 legsStiffness=12 \
                           forwardMaxArmOffset=0.9 backwardsMinArmOffset=0.2 zAxisSpinReduction=0.5".into());
        }
    }
}
impl Extra for Collide {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let w = weight(c);
        let (com, vel) = (c.com, c.vel);
        let cl = (*crate::nmrs::CLAVICLE());
        let facing = flat((c.b.frames[cl[0] + 1].translation - c.b.frames[cl[1] + 1].translation).cross(Vec3::Z));
        if self.first {
            self.first = false;
            self.given = self.pos != Vec3::ZERO;
            self.virt = !self.given;
            if self.virt {
                self.pos = com - facing * delirium_data::kv!(106);
                self.normal = facing;
            } else if self.normal == Vec3::ZERO {
                self.normal = flat(com - self.pos);
            }
            self.normal = flat(self.normal);
            self.out.push("#say bcr start".into());
        }
        self.t += dt;
        self.hit_t += dt;
        let n_obj = self.normal;
        let top = c.b.frames[(*ANKLE())[0] + 1].translation.z + delirium_data::kv!(108);
        let (lower, upper) = ([0, (*SPINE())[0] + 1, (*SPINE())[1] + 1], [(*SPINE())[2] + 1, (*SPINE())[delirium_data::kv!(0 int)] + 1]);
        let (mut lb, mut ub, mut n) = (false, false, Vec3::ZERO);
        let real: Vec<usize> = lower.iter().chain(&upper).copied().filter(|&k| c.b.doll.collided(&[k])).collect();
        if !real.is_empty() {
            self.virt = false;
            lb = real.iter().any(|k| lower.contains(k));
            ub = real.iter().any(|k| upper.contains(k));
            let away = -real.iter().map(|&k| c.b.frames[k].translation - com).fold(Vec3::ZERO, |a, b| a + b);
            n = if self.given { n_obj }
                else if away.truncate().length() > delirium_data::kv!(64) { flat(away) }
                else if vel.truncate().length() > delirium_data::kv!(24) { -flat(vel) } else { facing };
        } else if self.virt {
            for &k in &lower {
                let p = c.b.frames[k].translation;
                let pen = delirium_data::kv!(116) - (p - self.pos).dot(n_obj);
                if pen > 0.0 && p.z < top {
                    lb = true;
                    n = n_obj;
                    c.out.force(k, n_obj * (w * (pen * delirium_data::kv!(80) + delirium_data::kv!(24) * (-vel.dot(n_obj)).max(0.0))));
                }
            }
        }
        if lb || ub {
            if self.state == BAL_NORMAL {
                self.to(BAL_IMPACT);
                self.ub_n = Vec3::ZERO;
                self.lb_n = Vec3::ZERO;
                (self.ub, self.lb, self.hit_t) = (false, false, 0.0);
                self.normal = if n == Vec3::ZERO { n_obj } else { n };
                let vt = vel - self.normal * vel.dot(self.normal);
                let side = self.normal.cross(vt).z;
                self.glance = if vt.truncate().length() > delirium_data::kv!(73) { side.signum() * self.spin_mag } else { 0.0 };
            }
            if self.state == BAL_IMPACT {
                self.ub |= ub;
                self.lb |= lb;
                if ub { self.ub_n += n; }
                if lb { self.lb_n += n; }
            }
        }
        let f_exag = if self.state == BAL_IMPACT { (1.0 - self.t / self.exag.max(dt)).clamp(0.0, 1.0) } else { 0.0 };
        let still = vel.truncate().length() < delirium_data::kv!(24);
        self.calm = if still { self.calm + dt } else { 0.0 };
        let leg = |c: &mut Ctx, k: f32| c.ch.set_stiffness(delirium_data::kv!(117) * k.max(delirium_data::kv!(64)), delirium_data::kv!(110), "lb", None);
        match self.state {
            BAL_NORMAL => {
                if self.virt && self.t < delirium_data::kv!(17) {
                    c.out.force(CHEST(), -self.normal * (w * delirium_data::kv!(110)));
                }
                if self.t > 2.0 && self.t - dt <= 2.0 {
                    self.to(BAL_END);
                }
            }
            BAL_IMPACT => {
                leg(c, 1.0 - self.leg_red);
                c.out.force(CHEST(), self.normal * (w * delirium_data::kv!(24) * f_exag));
                if self.t < self.spin_t {
                    self.glance *= self.spin_decay.powf(dt * delirium_data::kv!(27));
                    c.out.torque(0, Vec3::Z * (self.glance * w * 0.5));
                }
                for i in 0..2 {
                    c.ch.set_relative((*SHOULDER())[i], 1, 0.5);
                    c.ch.set_relative((*ELBOW())[i], 0, -delirium_data::kv!(24));
                }
                if self.t >= self.react {
                    let acc = flat(self.ub_n + self.lb_n);
                    if acc != Vec3::ZERO && !self.given {
                        self.normal = acc;
                    }
                    let s = if self.ub && self.lb { BAL_SLUMP } else if self.lb {
                        if facing.dot(self.normal) > 0.0 { BAL_DRAPE } else { BAL_DRAPE_FWD }
                    } else { BAL_REBOUND };
                    self.to(s);
                    self.calm = 0.0;
                    if s != BAL_DRAPE && s != BAL_DRAPE_FWD {
                        self.catch_fall();
                    }
                }
            }
            BAL_REBOUND => {
                leg(c, 1.0 - self.leg_red * 0.5);
                if self.t > delirium_data::kv!(106) && still {
                    self.to(BAL_END);
                    self.out.push("catchFall start=false".into());
                } else if self.hit_t > self.steps as f32 * BCR_STEP() {
                    self.to(BAL_SLUMP);
                }
            }
            BAL_DRAPE | BAL_DRAPE_FWD => {
                let dir = if self.state == BAL_DRAPE { 1.0 } else { -1.0 };
                leg(c, 1.0 - self.leg_red);
                c.ch.set_stiffness(delirium_data::kv!(5), delirium_data::kv!(110), "ub", None);
                c.out.force(CHEST(), -self.normal * (w * delirium_data::kv!(17) * dir));
                for &j in &(*SPINE()) {
                    c.ch.set_relative(j, 0, -delirium_data::kv!(24) * dir);
                }
                if self.t >= self.steps as f32 * BCR_STEP() || self.calm >= self.stable {
                    self.to(BAL_SLUMP);
                    self.catch_fall();
                }
            }
            BAL_SLUMP => {
                leg(c, 1.0 - self.slump_red * (self.slump_rate * self.t).min(1.0));
                if (self.t > delirium_data::kv!(73) && c.dbm_failed) || self.t > delirium_data::kv!(12) {
                    self.to(BAL_END);
                }
            }
            _ => {}
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.state == BAL_END
    }

    fn stop(&mut self, _c: &mut Ctx) {
        if !matches!(self.state, BAL_NORMAL | BAL_END) {
            self.catch_fall();
        }
    }
}

pub struct Injured {
    n: i32,
    wounds: [(Vec3, Vec3, usize); 2],
    attacker: Vec3,
    no_reach: [bool; 2],
    strong: bool,
    t: f32,
    pedal_cd: f32,
    pedalling: bool,
    state: u8,
    out: Vec<String>,
}
const LYING: [&str; 5] = ["upright", "face down", "face up", "on left side", "on right side"];
impl Injured {
    fn new(p: &mut P) -> Result<Injured, Error> {
        let mut w = [(Vec3::ZERO, Vec3::ZERO, (*SPINE())[0] + 1); 2];
        for (i, w) in w.iter_mut().enumerate() {
            let k = i + 1;
            *w = (p.v(&format!("injury{k}LocalPosition"), Vec3::ZERO)?, p.v(&format!("injury{k}LocalNormal"), Vec3::ZERO)?,
                  p.i(&format!("injury{k}Component"), (*SPINE())[0] as i32 + 1)?.clamp(0, delirium_data::kv!(118 int)) as usize);
        }
        Ok(Injured { n: p.i("numInjuries", 0)?.clamp(0, 2), wounds: w, attacker: p.v("attackerPos", Vec3::ZERO)?,
                     no_reach: [p.b("dontReachWithLeft", false)?, p.b("dontReachWithRight", false)?],
                     strong: p.b("strongRollForce", false)?, t: 0.0, pedal_cd: delirium_data::kv!(12), pedalling: false, state: 0, out: vec![] })
    }
}
impl Extra for Injured {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let first = self.t == 0.0;
        self.t += dt;
        let t = self.t;
        if first {
            self.out.push("bodyBalance start=false".into());
        }
        let up = c.ch.up.normalize_or_zero();
        let pelvis = c.b.part(0).m.d;
        let spine = (c.b.part((*NECK())[0] + 1).m.d - pelvis).normalize_or_zero();
        let side = (c.b.part((*CLAVICLE())[0] + 1).m.d - c.b.part((*CLAVICLE())[1] + 1).m.d).normalize_or_zero();
        let front = side.cross(spine);
        let state = if spine.dot(up) > delirium_data::kv!(70) { 0 }
            else if front.dot(up).abs() > delirium_data::kv!(110) { if front.dot(up) < 0.0 { 1 } else { 2 } }
            else if side.dot(up) > 0.0 { delirium_data::kv!(1 int) } else { delirium_data::kv!(0 int) };
        let w = weight(c);
        let g = 1.0 - 0.5 * (t / delirium_data::kv!(61)).min(1.0);
        if state != self.state {
            self.state = state;
            self.out.push(format!("#say injuredOnGround {}", LYING[state as usize]));
            if state != 0 {
                let a = if self.attacker != Vec3::ZERO { self.attacker } else {
                    pelvis + (spine - up * spine.dot(up)).normalize_or_zero() * 2.0
                };
                self.out.push(format!("headLook start=true pos={},{},{} stiffness={} damping=1 alwaysLook=true", a.x, a.y, a.z, delirium_data::kv!(107)));
            }
        }
        c.ch.set_stiffness(delirium_data::kv!(5), delirium_data::kv!(110), "fb", None);
        for i in 0..2 {
            if !self.pedalling {
                c.ch.set_relative((*HIP())[i], 0, -delirium_data::kv!(110) * g);
                c.ch.set_relative((*KNEE())[i], 0, delirium_data::kv!(70) * g);
            }
        }
        for &j in &(*SPINE()) {
            c.ch.set_relative(j, 0, delirium_data::kv!(106) * g);
        }
        if state == 0 {
            return;
        }
        self.pedal_cd -= dt;
        if self.pedal_cd <= 0.0 {
            self.pedalling = !self.pedalling;
            self.pedal_cd = if self.pedalling { 2.0 } else { delirium_data::kv!(12) };
            self.out.push(if self.pedalling { format!("pedalLegs start=true legStiffness={}", delirium_data::kv!(29)) } else { "pedalLegs start=false".into() });
        }
        let (cyc, u) = (((t - 1.0) / delirium_data::kv!(119)).floor(), ((t - 1.0) % delirium_data::kv!(119)) / delirium_data::kv!(70));
        if t > 1.0 && u < 1.0 {
            let s = if cyc as i32 % 2 == 0 { 1.0 } else { -1.0 };
            let torque = spine * (s * w * (if self.strong { delirium_data::kv!(120) } else { delirium_data::kv!(121) }) * g * (u * std::f32::consts::PI).sin());
            c.out.torque(0, torque);
            c.out.torque(CHEST(), torque);
        }
        let brace = (t > 2.0 && (t - 2.0) % delirium_data::kv!(104) < delirium_data::kv!(110)).then(|| ((t - 2.0) / delirium_data::kv!(104)) as usize % 2);
        let wounds: Vec<Vec3> = if self.n == 0 {
            vec![c.b.part((*SPINE())[0] + 1).m.d + side * delirium_data::kv!(116)]
        } else {
            self.wounds[..self.n as usize].iter().map(|&(p, n, k)| {
                let m = c.b.part(k).m;
                crate::vm4(m.a, m.b, m.c, m.d, p) + crate::vm4(m.a, m.b, m.c, Vec3::ZERO, n).normalize_or_zero() * delirium_data::kv!(122)
            }).collect()
        };
        let mut target: [Option<Vec3>; 2] = [None, None];
        for &wd in &wounds {
            let free = |i: usize| !self.no_reach[i] && target[i].is_none();
            let d = |i: usize| (c.b.part((*SHOULDER())[i] + 1).m.d - wd).length();
            let pick = match (free(0), free(1)) {
                (true, true) => Some(if d(0) <= d(1) { 0 } else { 1 }),
                (true, false) => Some(0),
                (false, true) => Some(1),
                _ => None,
            };
            if let Some(i) = pick {
                target[i] = Some(wd);
            }
        }
        for i in 0..2 {
            let braced = brace == Some(i);
            let Some(at) = (if braced { Some(c.b.part((*SHOULDER())[i] + 1).m.d - up * 0.5) } else { target[i] }) else { continue };
            let l = crate::nmshot::limb(i);
            let k = if braced { delirium_data::kv!(117) } else { delirium_data::kv!(21) * g };
            c.ch.eff[l.e[1]].set_stiffness(k, delirium_data::kv!(110), None);
            c.ch.eff[l.e[2]].set_stiffness(k, delirium_data::kv!(110), None);
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], at, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0,
                                  Vec3::ZERO, None);
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }
}

const DOWN: f32 = 1.0;

pub struct Crawl {
    k: (f32, f32),
    speed: f32,
    target: Vec3,
    yaw: f32,
    twister: bool,
    t: f32,
    best: (f32, f32),
    out: Vec<String>,
    over: bool,
}

impl Crawl {
    fn new(p: &mut P) -> Result<Crawl, Error> {
        let _ = (p.b("bLearn", false)?, p.i("numFrames2Learn", 0)?, p.i("animIndex", 0)?, p.b("learnFromAnimPlayback", false)?,
                 p.i("inputSequenceSize", 0)?, p.b("useSpine3Thing", false)?, p.b("useRollBoneCompensation", false)?);
        Ok(Crawl { k: (p.f("stiffness", delirium_data::kv!(27))?, p.f("damping", delirium_data::kv!(110))?), speed: p.f("speed", delirium_data::kv!(20))?, target: p.v("targetPosition", Vec3::ZERO)?,
                   yaw: p.f("yawOffset", 0.0)?, twister: p.b("useTwister", true)?, t: 0.0, best: (f32::MAX, 0.0), out: vec![],
                   over: false })
    }
}

impl Extra for Crawl {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let pelvis = c.b.part(0).m.d;
        let front = flat(c.b.part((*NECK())[1] + 1).m.d - pelvis);
        if self.t == 0.0 {
            self.out.push("bodyBalance start=false".into());
        }
        self.t += dt;
        let w = weight(c);
        c.ch.set_stiffness(self.k.0, self.k.1, "fb", None);
        for (k, h) in [(0, 0.5), (CHEST(), delirium_data::kv!(10))] {
            let p = c.b.part(k);
            c.out.force(k, Vec3::Z * ((h - p.m.d.z) * w * delirium_data::kv!(104) - p.v.z * w * delirium_data::kv!(106)).clamp(0.0, w * 0.5));
        }
        if self.t < DOWN {
            for i in 0..2 {
                c.ch.set_relative((*HIP())[i], 0, -0.5);
                c.ch.set_relative((*KNEE())[i], 0, delirium_data::kv!(70));
                c.ch.set_relative((*SHOULDER())[i], 0, -delirium_data::kv!(110));
            }
            return;
        }
        if self.target == Vec3::ZERO {
            self.target = pelvis + front * delirium_data::kv!(5);
        }
        if self.best.0 == f32::MAX {
            self.best.1 = self.t;
        }
        let to = self.target - pelvis;
        let dist = Vec3::new(to.x, to.y, 0.0).length();
        if dist < 0.5 {
            self.out.push("#say learnedCrawl success".into());
            self.over = true;
            return;
        }
        if dist < self.best.0 - delirium_data::kv!(57) {
            self.best = (dist, self.t);
        } else if self.t - self.best.1 > delirium_data::kv!(104) {
            self.out.push("#say learnedCrawl failure".into());
            self.over = true;
            return;
        }
        let (s, r) = (self.yaw.sin(), self.yaw.cos());
        let d = flat(to);
        let heading = Vec3::new(d.x * r - d.y * s, d.x * s + d.y * r, 0.0);
        let ph = (self.t - DOWN) * self.speed * TAU;
        for i in 0..2 {
            let a = ph + i as f32 * TAU * 0.5;
            let l = ph + (1 - i) as f32 * TAU * 0.5;
            let arm = crate::nmshot::limb(i);
            let reach = c.b.part(arm.p[0]).m.d + heading * (delirium_data::kv!(24) * a.sin()) - Vec3::Z * (delirium_data::kv!(123) - delirium_data::kv!(116) * a.cos().max(0.0));
            crate::nmdbm::limb_ik(c.b, c.ch, arm.e[1], arm.e[2], arm.p[0], arm.p[delirium_data::kv!(0 int)], reach, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0,
                                  Vec3::ZERO, None);
            c.ch.set_relative((*HIP())[i], 0, -0.5 + delirium_data::kv!(106) * l.sin());
            c.ch.set_relative((*KNEE())[i], 0, delirium_data::kv!(110) + delirium_data::kv!(24) * l.cos());
        }
        let turn = front.cross(heading).z.clamp(-1.0, 1.0);
        if self.twister {
            for j in (*SPINE()) {
                c.ch.eff[j].desired[2] = turn * delirium_data::kv!(9);
            }
        }
        c.out.torque(0, Vec3::Z * (turn * w * delirium_data::kv!(9)));
        c.out.force(0, heading * (w * delirium_data::kv!(121)));
        c.out.force(CHEST(), heading * (w * delirium_data::kv!(124)));
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.over
    }
}

