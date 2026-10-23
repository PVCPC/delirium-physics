use bevy_math::Vec3;

use crate::extras::{Extra, P};
use crate::msg::{Error, Kind, Message, Param};
use crate::nmrs::{Character, ANKLE, CLAVICLE, ELBOW, HIP, KNEE, NECK, SHOULDER, SPINE, WRIST};
use crate::nmshot::Ctx;
use bevy_math::Quat;

const CHEST: usize = 10;
const HANDS: [usize; 2] = [16, 20];
const FEET: [usize; 2] = [3, 6];

pub static MESSAGES: [Message; 15] = [
    Message { name: "yanked", params: &[
        Param { name: "point", kind: Kind::Vec3, used: true, doc: "world point he is pulled toward (zero: 2 m in front of the chest)" },
        Param { name: "part", kind: Kind::Int, used: true, doc: "the part pulled (10, the chest)" },
        Param { name: "force", kind: Kind::Float, used: true, doc: "pull, in body weights (1; 0: no pull, just the reaction)" },
        Param { name: "pull", kind: Kind::Float, used: true, doc: "seconds the pull lasts (0.2)" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "armDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "spineStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "spineDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "armStiffnessStart", kind: Kind::Float, used: true, doc: "arms while the yank lands (3)" },
        Param { name: "armDampingStart", kind: Kind::Float, used: true, doc: "(0.1)" },
        Param { name: "spineStiffnessStart", kind: Kind::Float, used: true, doc: "(3)" },
        Param { name: "spineDampingStart", kind: Kind::Float, used: true, doc: "(0.1)" },
        Param { name: "timeAtStartValues", kind: Kind::Float, used: true, doc: "s at the start values (0.4)" },
        Param { name: "rampTimeFromStartValues", kind: Kind::Float, used: true, doc: "s ramping from them (0.1)" },
        Param { name: "stepsTillStartEnd", kind: Kind::Int, used: false, doc: "(not done: time only, see timeStartEnd)" },
        Param { name: "timeStartEnd", kind: Kind::Float, used: true, doc: "s until the ramp to the end values (3)" },
        Param { name: "rampTimeToEndValues", kind: Kind::Float, used: true, doc: "s of that ramp (1)" },
        Param { name: "lowerBodyStiffness", kind: Kind::Float, used: true, doc: "legs (12)" },
        Param { name: "lowerBodyStiffnessEnd", kind: Kind::Float, used: true, doc: "legs at the end (8)" },
        Param { name: "perStepReduction", kind: Kind::Float, used: false, doc: "(not done: no step count)" },
        Param { name: "hipPitchForward", kind: Kind::Float, used: true, doc: "hip pitch when pulled forward, of the range (0.6)" },
        Param { name: "hipPitchBack", kind: Kind::Float, used: true, doc: "when pulled back (1)" },
        Param { name: "spineBend", kind: Kind::Float, used: true, doc: "spine bend with the pull, of the range (0.7)" },
        Param { name: "footFriction", kind: Kind::Float, used: false, doc: "(not done)" },
        Param { name: "comVelRDSThresh", kind: Kind::Float, used: true, doc: "COM speed, m/s, past which he goes down (4)" },
        Param { name: "turnThresholdMin", kind: Kind::Float, used: false, doc: "(not done)" },
        Param { name: "turnThresholdMax", kind: Kind::Float, used: false, doc: "(not done)" },
        Param { name: "hulaPeriod", kind: Kind::Float, used: true, doc: "s per hip/spine sway (0.25)" },
        Param { name: "hipAmplitude", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "spineAmplitude", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "minRelaxPeriod", kind: Kind::Float, used: true, doc: "shortest s between relax moments (0.3)" },
        Param { name: "maxRelaxPeriod", kind: Kind::Float, used: true, doc: "longest (1.5)" },
        Param { name: "rollHelp", kind: Kind::Float, used: true, doc: "roll push on the ground, of his weight (0.5)" },
        Param { name: "groundLegStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "groundArmStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "groundSpineStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "groundLegDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "groundArmDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "groundSpineDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "groundFriction", kind: Kind::Float, used: false, doc: "(not done)" },
        Param { name: "useHeadLook", kind: Kind::Bool, used: true, doc: "look at headLookPos (true)" },
        Param { name: "headLookPos", kind: Kind::Vec3, used: true, doc: "(zero: the pull point)" },
        Param { name: "headLookInstanceIndex", kind: Kind::Int, used: true, doc: "(-1)" },
        Param { name: "headLookAtVelProb", kind: Kind::Float, used: false, doc: "(not done)" },
    ] },
    Message { name: "dragged", params: &[
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "(11)" },
        Param { name: "armDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "armMuscleStiffness", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "radiusTolerance", kind: Kind::Float, used: true, doc: "how near the rope a hand grabs it, m (0.25)" },
        Param { name: "lengthTolerance", kind: Kind::Float, used: true, doc: "slack before the rope is taut, m (0.1)" },
        Param { name: "ropePos", kind: Kind::Vec3, used: true, doc: "the rope's anchor (zero: 2 m in front of his chest)" },
        Param { name: "ropeAttachedToInstance", kind: Kind::Int, used: false, doc: "(not done)" },
        Param { name: "ropedBodyPart", kind: Kind::Int, used: true, doc: "the part it's tied to (-1: none, the hands grab it)" },
        Param { name: "ropeTaut", kind: Kind::Bool, used: false, doc: "(not done)" },
        Param { name: "playerControl", kind: Kind::Bool, used: true, doc: "grabLeft / grabRight pick the hands (false)" },
        Param { name: "grabLeft", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "grabRight", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "reach", kind: Kind::Float, used: true, doc: "how far from the chest a hand reaches, m (0.7)" },
        Param { name: "armTwist", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "direction", kind: Kind::Vec3, used: true, doc: "room: which way the anchor moves off (zero: the way he faces)" },
        Param { name: "speed", kind: Kind::Float, used: true, doc: "room: its speed, m/s (1.2)" },
        Param { name: "delay", kind: Kind::Float, used: true, doc: "room: s before it moves (0.7)" },
        Param { name: "force", kind: Kind::Float, used: true, doc: "the rope's most pull, body weights (0.6)" },
    ] },
    Message { name: "dangle", params: &[
        Param { name: "doGrab", kind: Kind::Bool, used: true, doc: "reach up for what he hangs from (false)" },
        Param { name: "grabFrequency", kind: Kind::Float, used: true, doc: "how often, 0 to 1 (0.5)" },
        Param { name: "point", kind: Kind::Vec3, used: true, doc: "room: where his hands are held (zero: 1 m over his head, his feet off the floor)" },
        Param { name: "hold", kind: Kind::Bool, used: true, doc: "room: hold them there (true; false: something else holds him)" },
    ] },
    Message { name: "carried", params: &[
        Param { name: "point", kind: Kind::Vec3, used: true, doc: "the room's stand-in for the carrier's shoulder, his waist held there (zero: 0.6 m over his pelvis)" },
        Param { name: "hold", kind: Kind::Bool, used: true, doc: "hold him at point (true; false: just the stiffnesses, for when something else carries him)" },
        Param { name: "trunkStiffness", kind: Kind::Float, used: true, doc: "spine (5)" },
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "left leg while moving (2)" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "(1.5)" },
        Param { name: "clavicleStiffness", kind: Kind::Float, used: true, doc: "(4)" },
        Param { name: "neckStiffness", kind: Kind::Float, used: true, doc: "(7)" },
        Param { name: "stillLegStiffness", kind: Kind::Float, used: true, doc: "right leg, and the left once he's still (4)" },
    ] },
    Message { name: "buoyancy", params: &[
        Param { name: "buoyancy", kind: Kind::Float, used: true, doc: "up-force on a part under, of its weight (1.1)" },
        Param { name: "chestBuoyancy", kind: Kind::Float, used: true, doc: "extra on the chest, tenths of his weight (0.5)" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "water drag per second (3)" },
        Param { name: "surfacePoint", kind: Kind::Vec3, used: true, doc: "a point on the water (zero: his chest when it starts)" },
        Param { name: "surfaceNormal", kind: Kind::Vec3, used: true, doc: "the water's up (0 0 1)" },
        Param { name: "righting", kind: Kind::Bool, used: true, doc: "turn him face up (true)" },
        Param { name: "rightingStrength", kind: Kind::Float, used: true, doc: "of his weight, as torque (0.3)" },
        Param { name: "rightingTime", kind: Kind::Float, used: true, doc: "s it lasts, easing off (2)" },
    ] },
    Message { name: "animPose", params: &[
        Param { name: "pose", kind: Kind::Str, used: true, doc: "joint:a,b,c;joint:a,b,c; angles in radians (empty: hold the pose he is in)" },
        Param { name: "mask", kind: Kind::Str, used: true, doc: "the joints it may drive (fb)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "(12)" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "muscleStiffness", kind: Kind::Float, used: true, doc: "(-1: the joints' own)" },
        Param { name: "gravityCompensation", kind: Kind::Float, used: true, doc: "gravity opposition (0)" },
        Param { name: "stiffnessLeftArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "dampingLeftArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "muscleStiffnessLeftArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "gravCompLeftArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "stiffnessRightArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "dampingRightArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "muscleStiffnessRightArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "gravCompRightArm", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "stiffnessSpine", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "dampingSpine", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "muscleStiffnessSpine", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "gravCompSpine", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "stiffnessLeftLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "dampingLeftLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "muscleStiffnessLeftLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "gravCompLeftLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "stiffnessRightLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "dampingRightLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "muscleStiffnessRightLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "gravCompRightLeg", kind: Kind::Float, used: true, doc: "(-1: the global one)" },
        Param { name: "useZMPGravityCompensation", kind: Kind::Bool, used: false, doc: "(not done: false)" },
        Param { name: "overideHeadlook", kind: Kind::Bool, used: true, doc: "leave the neck alone (false)" },
        Param { name: "overidePointArm", kind: Kind::Bool, used: true, doc: "leave the left arm alone (false)" },
        Param { name: "overidePointGun", kind: Kind::Bool, used: true, doc: "leave the right arm alone (false)" },
        Param { name: "connectedLeftHand", kind: Kind::Int, used: true, doc: "nonzero: anchored, its limb at half stiffness (0)" },
        Param { name: "connectedRightHand", kind: Kind::Int, used: true, doc: "nonzero: anchored, its limb at half stiffness (0)" },
        Param { name: "connectedLeftFoot", kind: Kind::Int, used: true, doc: "nonzero: anchored, its limb at half stiffness (0)" },
        Param { name: "connectedRightFoot", kind: Kind::Int, used: true, doc: "nonzero: anchored, its limb at half stiffness (0)" },
        Param { name: "animSource", kind: Kind::Int, used: false, doc: "(not done: our pose only)" },
        Param { name: "dampenSideMotionInstanceIndex", kind: Kind::Int, used: false, doc: "(not done)" },
        Param { name: "vDampenPlanePoint", kind: Kind::Vec3, used: false, doc: "(not done)" },
        Param { name: "vDampenPlaneNormal", kind: Kind::Vec3, used: true, doc: "brakes motion along it (zero: off)" },
    ] },
    Message { name: "armsWindmill", params: &[
        Param { name: "leftNormal", kind: Kind::Vec3, used: true, doc: "the circle's plane normal, his frame: x forward, y left, z up (0 1 0)" },
        Param { name: "leftCentre", kind: Kind::Vec3, used: true, doc: "the circle's centre from the part, his frame (0.15 0.25 0.05)" },
        Param { name: "leftPartID", kind: Kind::Int, used: true, doc: "the part the circle is from (10, the chest)" },
        Param { name: "leftRadius1", kind: Kind::Float, used: true, doc: "m (0.45)" },
        Param { name: "leftRadius2", kind: Kind::Float, used: true, doc: "m (0.45)" },
        Param { name: "leftSpeed", kind: Kind::Float, used: true, doc: "revolutions a second (1)" },
        Param { name: "rightNormal", kind: Kind::Vec3, used: true, doc: "the circle's plane normal, his frame: x forward, y left, z up (0 -1 0)" },
        Param { name: "rightCentre", kind: Kind::Vec3, used: true, doc: "the circle's centre from the part, his frame (0.15 -0.25 0.05)" },
        Param { name: "rightPartID", kind: Kind::Int, used: true, doc: "the part the circle is from (10, the chest)" },
        Param { name: "rightRadius1", kind: Kind::Float, used: true, doc: "m (0.45)" },
        Param { name: "rightRadius2", kind: Kind::Float, used: true, doc: "m (0.45)" },
        Param { name: "rightSpeed", kind: Kind::Float, used: true, doc: "revolutions a second (1)" },
        Param { name: "radius1Delta", kind: Kind::Float, used: true, doc: "adaptive, a second (0)" },
        Param { name: "radius2Delta", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "speedDelta", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "shoulderStiffness", kind: Kind::Float, used: true, doc: "(12)" },
        Param { name: "shoulderDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "elbowStiffness", kind: Kind::Float, used: true, doc: "(12)" },
        Param { name: "elbowDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "shoulderStiffnessDelta", kind: Kind::Float, used: true, doc: "adaptive, a second (0)" },
        Param { name: "elbowStiffnessDelta", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "phaseOffset", kind: Kind::Float, used: true, doc: "the right arm behind the left, radians (3.14)" },
        Param { name: "dragReduction", kind: Kind::Float, used: false, doc: "(not done)" },
        Param { name: "IKtwist", kind: Kind::Float, used: true, doc: "shoulder twist in the IK (0)" },
        Param { name: "angVelThreshold", kind: Kind::Float, used: true, doc: "adaptive: body spin, rad/s, before it speeds up (0.1)" },
        Param { name: "angVelGain", kind: Kind::Float, used: true, doc: "adaptive: faster by this per rad/s over (1)" },
        Param { name: "leftElbowMin", kind: Kind::Float, used: true, doc: "least elbow bend, rad (0.3)" },
        Param { name: "rightElbowMin", kind: Kind::Float, used: true, doc: "(0.3)" },
        Param { name: "mirrorMode", kind: Kind::Int, used: true, doc: "1: the right arm goes round the other way (0)" },
        Param { name: "adaptiveMode", kind: Kind::Bool, used: true, doc: "speed tracks his spin (false)" },
        Param { name: "forceSync", kind: Kind::Bool, used: true, doc: "no phase offset (false)" },
        Param { name: "useLeft", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "useRight", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "disableOnImpact", kind: Kind::Bool, used: true, doc: "a hand stops once it hits something (true)" },
    ] },
    Message { name: "spineTwist", params: &[
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "the point his chest turns to (zero: off to his left, a little behind)" },
        Param { name: "offset", kind: Kind::Float, used: true, doc: "rad, turn this much past it (0)" },
        Param { name: "velX", kind: Kind::Float, used: true, doc: "the point's speed, to lead it (0)" },
        Param { name: "velY", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "velZ", kind: Kind::Float, used: true, doc: "(0)" },
        Param { name: "spineTwistExcess", kind: Kind::Float, used: true, doc: "each spine joint may twist this much past its range (0.35)" },
        Param { name: "spStrength", kind: Kind::Float, used: true, doc: "the spine's stiffness (12)" },
        Param { name: "spDamping", kind: Kind::Float, used: true, doc: "(2)" },
        Param { name: "twistClavicles", kind: Kind::Bool, used: true, doc: "the clavicles turn with it (true)" },
        Param { name: "allwaysTwist", kind: Kind::Bool, used: true, doc: "keep twisting even when it's right behind him (false)" },
    ] },
    Message { name: "wave", params: &[
        Param { name: "right", kind: Kind::Bool, used: true, doc: "wave the right hand (true; false: the left)" },
        Param { name: "time", kind: Kind::Float, used: true, doc: "s of waving (2.25)" },
        Param { name: "rate", kind: Kind::Float, used: true, doc: "waves a second (2)" },
        Param { name: "size", kind: Kind::Float, used: true, doc: "how far the hand sways each side, m (0.18)" },
    ] },
    Message { name: "breathe", params: &[
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "the spine's stiffness while breathing (10)" },
        Param { name: "effectorMask", kind: Kind::Str, used: true, doc: "the joints that breathe (us: the spine)" },
        Param { name: "breathingScale", kind: Kind::Float, used: true, doc: "how deep, 0 none (1)" },
    ] },
    Message { name: "softKeyframe", params: &[
        Param { name: "followMultiplier", kind: Kind::Float, used: true, doc: "how hard each part chases its place, 1/s^2 (10)" },
        Param { name: "maxAcceleration", kind: Kind::Float, used: true, doc: "the chase's most, m/s^2 (20)" },
        Param { name: "goSlowerWhenWeaker", kind: Kind::Bool, used: true, doc: "a weak joint's part chases less (true)" },
        Param { name: "yawOffset", kind: Kind::Float, used: true, doc: "rad, the pose turned about up (0)" },
        Param { name: "targetPosition", kind: Kind::Vec3, used: true, doc: "where the pose's pelvis is (zero: wherever his pelvis is)" },
        Param { name: "mask", kind: Kind::Str, used: true, doc: "the joints whose parts follow (fb)" },
    ] },
    Message { name: "splitBody", params: &[
        Param { name: "aimTarget", kind: Kind::Vec3, used: true, doc: "the point the gun aims at (zero: straight ahead)" },
        Param { name: "activePoseMask", kind: Kind::Str, used: true, doc: "joints held in their start pose (uk: spine and neck)" },
        Param { name: "hardConstraintMask", kind: Kind::Str, used: true, doc: "joints whose parts are pinned where they start (none)" },
        Param { name: "softConstraintMask", kind: Kind::Str, used: true, doc: "joints whose parts are eased back to where they start (none)" },
        Param { name: "collisionMask", kind: Kind::Str, used: true, doc: "kept, not used (fb)" },
        Param { name: "gravityCompensationMask", kind: Kind::Str, used: true, doc: "joints that hold up against gravity (ub)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "the pose's stiffness (12)" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "controllerStiffness", kind: Kind::Float, used: true, doc: "the aiming arm's stiffness (14)" },
        Param { name: "controllerDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "leftHandWeaponIndex", kind: Kind::Int, used: true, doc: "the left hand's gun, -1 none (-1)" },
        Param { name: "rightHandWeaponIndex", kind: Kind::Int, used: true, doc: "the right hand's gun, -1 none (0)" },
        Param { name: "constrainHands", kind: Kind::Bool, used: true, doc: "the support hand pinned on the grip (true)" },
        Param { name: "controlHandOrientation", kind: Kind::Bool, used: true, doc: "the support hand turns with the gun hand (true)" },
        Param { name: "leftHandStatus", kind: Kind::Int, used: true, doc: "0 free, 1 on the gun, 2 supporting (2)" },
        Param { name: "rightHandStatus", kind: Kind::Int, used: true, doc: "(1)" },
        Param { name: "adjustJointLimits", kind: Kind::Bool, used: true, doc: "soft limits on the elbows, a steadier aim (true)" },
        Param { name: "handStable", kind: Kind::Bool, used: true, doc: "the wrists stiffer (false)" },
        Param { name: "handStableStiff", kind: Kind::Float, used: true, doc: "(14)" },
        Param { name: "handStableDamp", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "recoilRelax", kind: Kind::Bool, used: true, doc: "the arms give on each shot (true)" },
        Param { name: "recoilRelaxAmount", kind: Kind::Float, used: true, doc: "how much, 0..1 (0.5)" },
        Param { name: "recoilRelaxTime", kind: Kind::Float, used: true, doc: "s back to stiff (0.3)" },
        Param { name: "breathingScale", kind: Kind::Float, used: true, doc: "the breathing sway, 0 none (1)" },
        Param { name: "fireWeapon", kind: Kind::Bool, used: true, doc: "send alone while it runs: a shot" },
    ] },
    Message { name: "shootDodge", params: &[
        Param { name: "rightArmTarget", kind: Kind::Vec3, used: true, doc: "the right arm aims here (zero: not aiming)" },
        Param { name: "leftArmTarget", kind: Kind::Vec3, used: true, doc: "the left arm aims here (zero: not aiming)" },
        Param { name: "targetValid", kind: Kind::Bool, used: true, doc: "aim at all (true)" },
        Param { name: "collisionPoint", kind: Kind::Vec3, used: true, doc: "kept, not used" },
        Param { name: "collisionNormal", kind: Kind::Vec3, used: true, doc: "kept, not used" },
        Param { name: "collisionObjectVelocity", kind: Kind::Vec3, used: true, doc: "kept, not used" },
        Param { name: "collisionIndex", kind: Kind::Int, used: true, doc: "kept, not used" },
        Param { name: "collisionTime", kind: Kind::Float, used: true, doc: "kept, not used" },
        Param { name: "collisionTimeThreshold", kind: Kind::Float, used: true, doc: "s touching that ends it (0.2)" },
        Param { name: "collisionAccelThreshold", kind: Kind::Float, used: true, doc: "m/s^2 of a hit that ends it (40)" },
        Param { name: "collisionVelThreshold", kind: Kind::Float, used: true, doc: "m/s lost in a step that ends it (2)" },
        Param { name: "collisionReaction", kind: Kind::Int, used: true, doc: "after: 0 catchFall, 1 bodyRollUp, 2 bodyBalance, 3 nothing (0)" },
        Param { name: "trunkStiffness", kind: Kind::Float, used: true, doc: "(12)" },
        Param { name: "trunkDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "legStiffness", kind: Kind::Float, used: true, doc: "(10)" },
        Param { name: "legDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "legTaper", kind: Kind::Float, used: true, doc: "each joint down the leg this times softer (0.8)" },
        Param { name: "aimingArmStiffness", kind: Kind::Float, used: true, doc: "(14)" },
        Param { name: "aimingArmDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "notAimingArmStiffness", kind: Kind::Float, used: true, doc: "(6)" },
        Param { name: "notAimingArmDamping", kind: Kind::Float, used: true, doc: "(0.7)" },
        Param { name: "notAimingArmTaper", kind: Kind::Float, used: true, doc: "each joint down the arm this times softer (0.7)" },
        Param { name: "headStiffness", kind: Kind::Float, used: true, doc: "(10)" },
        Param { name: "headDamping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "wristMuscleStiffness", kind: Kind::Float, used: true, doc: "the aiming wrist's muscle stiffness (10)" },
        Param { name: "oriStiff", kind: Kind::Float, used: true, doc: "holding the dive's attitude, 1/s^2 (30)" },
        Param { name: "oriDamp", kind: Kind::Float, used: true, doc: "1/s (8)" },
        Param { name: "oriSplit", kind: Kind::Float, used: true, doc: "its share on the pelvis, the rest on the chest (0.5)" },
        Param { name: "supportedHeightThreshold", kind: Kind::Float, used: true, doc: "kept, not used" },
        Param { name: "legNoiseScale", kind: Kind::Float, used: true, doc: "the legs' drift, rad (0.05)" },
        Param { name: "breathingScale", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "armAnimTask", kind: Kind::Bool, used: true, doc: "kept, not used (no animation stream)" },
        Param { name: "leftArmAnimTaskParent", kind: Kind::Int, used: true, doc: "kept, not used" },
        Param { name: "rightArmAnimTaskParent", kind: Kind::Int, used: true, doc: "kept, not used" },
        Param { name: "state", kind: Kind::Int, used: true, doc: "where it starts: 0 enter, 1 hold (0)" },
    ] },
    Message { name: "armHang", params: &[
        Param { name: "constraintPosition", kind: Kind::Vec3, used: true, doc: "the world point his hands hang on, carried with the object after (zero: between them where they are)" },
        Param { name: "targetPosition", kind: Kind::Vec3, used: true, doc: "where the arm reaches and he looks (zero: the anchor)" },
        Param { name: "instanceIndex", kind: Kind::Int, used: true, doc: "the object hung from, -1 the world (-1)" },
        Param { name: "boundIndex", kind: Kind::Int, used: true, doc: "its part, from 1 (0: the object as a whole)" },
        Param { name: "letGo", kind: Kind::Int, used: true, doc: "sent without start while hanging: that hand lets go, 0 left, 1 right (both: he drops)" },
    ] },
    Message { name: "crushReaction", params: &[
        Param { name: "effectorMask", kind: Kind::Str, used: true, doc: "the joints that react, an arm only if all of it is in (ub)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "(12)" },
        Param { name: "damping", kind: Kind::Float, used: true, doc: "(1)" },
        Param { name: "obstacleID", kind: Kind::Int, used: true, doc: "the object crushing him, -1 none (-1)" },
        Param { name: "flinchMode", kind: Kind::Int, used: true, doc: "0 cover the head, 1 brace the arms at it (0)" },
        Param { name: "useInjuries", kind: Kind::Bool, used: true, doc: "a hurt arm doesn't cover or brace (false)" },
        Param { name: "obstaclePosition", kind: Kind::Vec3, used: true, doc: "room: where the thing crushing him is without an obstacleID (zero: 1 m over his head)" },
    ] },
];

pub fn build(name: &str, p: &mut P) -> Option<Result<Box<dyn Extra>, Error>> {
    fn b<E: Extra + 'static>(e: Result<E, Error>) -> Option<Result<Box<dyn Extra>, Error>> {
        Some(e.map(|e| Box::new(e) as _))
    }
    match name {
        "yanked" => b(Yanked::new(p)),
        "dragged" => b(Dragged::new(p)),
        "dangle" => b(Dangle::new(p)),
        "carried" => b(Carried::new(p)),
        "buoyancy" => b(Buoyancy::new(p)),
        "animPose" => b(AnimPose::new(p)),
        "armsWindmill" => b(ArmsWindmill::new(p)),
        "spineTwist" => b(Twist::new(p)),
        "wave" => b(Wave::new(p)),
        "breathe" => b(Breathe::new(p)),
        "softKeyframe" => b(SoftKeyframe::new(p)),
        "splitBody" => b(SplitBody::new(p)),
        "shootDodge" => b(ShootDodge::new(p)),
        "armHang" => b(ArmHang::new(p)),
        "crushReaction" => b(CrushReaction::new(p)),
        _ => None,
    }
}

fn pos(c: &Ctx, k: usize) -> Vec3 {
    c.b.part(k).m.d
}

fn weight(c: &Ctx) -> f32 {
    c.b.doll.mass() * 9.81
}

fn facing(c: &Ctx) -> Vec3 {
    let up = c.ch.up.normalize_or_zero();
    let f = (pos(c, CLAVICLE[0] + 1) - pos(c, CLAVICLE[1] + 1)).cross(up);
    (f - up * f.dot(up)).normalize_or_zero()
}

fn spring(c: &mut Ctx, part: usize, to: Vec3, k: f32, d: f32, cap: f32) {
    let f = (to - pos(c, part)) * k - c.b.part(part).v * d;
    c.out.force(part, f.clamp_length_max(cap));
}

fn parse_pose(s: &str) -> Vec<(usize, [f32; 3])> {
    s.split(';').filter_map(|e| {
        let (j, a) = e.split_once(':')?;
        let mut a = a.split(',').map(|x| x.trim().parse::<f32>().unwrap_or(0.0));
        Some((j.trim().parse().ok().filter(|&j| j < 20)?, [a.next()?, a.next().unwrap_or(0.0), a.next().unwrap_or(0.0)]))
    }).collect()
}

pub struct Yanked {
    point: Vec3,
    part: usize,
    force: f32,
    pull: f32,
    arm: [f32; 4],
    spine: [f32; 4],
    start_time: f32,
    ramp_from: f32,
    time_end: f32,
    ramp_to: f32,
    legs: (f32, f32),
    pitch: (f32, f32),
    bend: f32,
    thresh: f32,
    hula: (f32, f32, f32),
    relax: (f32, f32),
    roll: f32,
    ground: [f32; 6],
    head: bool,
    head_pos: Vec3,
    head_index: i32,
    t: f32,
    next_relax: f32,
    down: bool,
    still: f32,
    finished: bool,
    out: Vec<String>,
}

impl Yanked {
    fn new(p: &mut P) -> Result<Yanked, Error> {
        let _ = (p.i("stepsTillStartEnd", 0)?, p.f("perStepReduction", 0.0)?, p.f("footFriction", 1.0)?);
        let _ = (p.f("turnThresholdMin", 0.0)?, p.f("turnThresholdMax", 0.0)?, p.f("groundFriction", 1.0)?);
        let _ = p.f("headLookAtVelProb", 0.0)?;
        Ok(Yanked {
            point: p.v("point", Vec3::ZERO)?,
            part: p.i("part", CHEST as i32)?.clamp(0, 20) as usize,
            force: p.f("force", 1.0)?,
            pull: p.f("pull", 0.2)?,
            arm: [p.f("armStiffnessStart", 3.0)?, p.f("armDampingStart", 0.1)?, p.f("armStiffness", 11.0)?, p.f("armDamping", 1.0)?],
            spine: [p.f("spineStiffnessStart", 3.0)?, p.f("spineDampingStart", 0.1)?, p.f("spineStiffness", 11.0)?,
                    p.f("spineDamping", 1.0)?],
            start_time: p.f("timeAtStartValues", 0.4)?,
            ramp_from: p.f("rampTimeFromStartValues", 0.1)?,
            time_end: p.f("timeStartEnd", 3.0)?,
            ramp_to: p.f("rampTimeToEndValues", 1.0)?,
            legs: (p.f("lowerBodyStiffness", 12.0)?, p.f("lowerBodyStiffnessEnd", 8.0)?),
            pitch: (p.f("hipPitchForward", 0.6)?, p.f("hipPitchBack", 1.0)?),
            bend: p.f("spineBend", 0.7)?,
            thresh: p.f("comVelRDSThresh", 4.0)?,
            hula: (p.f("hulaPeriod", 0.25)?, p.f("hipAmplitude", 1.0)?, p.f("spineAmplitude", 1.0)?),
            relax: (p.f("minRelaxPeriod", 0.3)?, p.f("maxRelaxPeriod", 1.5)?),
            roll: p.f("rollHelp", 0.5)?,
            ground: [p.f("groundLegStiffness", 11.0)?, p.f("groundLegDamping", 1.0)?, p.f("groundArmStiffness", 11.0)?,
                     p.f("groundArmDamping", 1.0)?, p.f("groundSpineStiffness", 11.0)?, p.f("groundSpineDamping", 1.0)?],
            head: p.b("useHeadLook", true)?,
            head_pos: p.v("headLookPos", Vec3::ZERO)?,
            head_index: p.i("headLookInstanceIndex", -1)?,
            t: 0.0,
            next_relax: 0.0,
            down: false,
            still: 0.0,
            finished: false,
            out: vec![],
        })
    }

    fn end(&mut self, why: &str) {
        self.finished = true;
        self.out.push(format!("#say yanked {why}"));
        if self.head {
            self.out.push("headLook start=false".into());
        }
    }
}

fn ramp(a: f32, b: f32, t: f32, t0: f32, time: f32) -> f32 {
    a + (b - a) * ((t - t0) / time.max(1e-3)).clamp(0.0, 1.0)
}

impl Extra for Yanked {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.t == 0.0 {
            if self.point == Vec3::ZERO {
                self.point = pos(c, CHEST) + facing(c) * 2.0;
            }
            if self.head_pos == Vec3::ZERO {
                self.head_pos = self.point;
            }
            self.out.push("bodyBalance start=true".into());
            if self.head {
                let h = self.head_pos;
                self.out.push(format!("headLook start=true instanceIndex={} pos={},{},{} stiffness=11 damping=1", self.head_index,
                                      h.x, h.y, h.z));
            }
            self.out.push("#say yanked pulled".into());
            self.next_relax = self.relax.1;
        }
        self.t += dt;
        let t = self.t;
        let w = weight(c);
        let to = self.point - pos(c, self.part);
        let dir = Vec3::new(to.x, to.y, 0.0).normalize_or_zero();
        if t < self.pull && self.force > 0.0 {
            spring(c, self.part, self.point, w * 3.0, w, w * self.force);
        }
        let feet = pos(c, FEET[0]).z.min(pos(c, FEET[1]).z);
        if !self.down && (c.vel.length() > self.thresh || c.com.z - feet < 0.45) {
            self.down = true;
            self.out.push("#say yanked ground".into());
        }
        let speed = c.vel.length();
        self.still = if speed < 0.3 { self.still + dt } else { 0.0 };
        if self.down {
            let g = self.ground;
            c.ch.set_stiffness(g[0], g[1], "lb", None);
            c.ch.set_stiffness(g[2], g[3], "ua", None);
            c.ch.set_stiffness(g[4], g[5], "uk", None);
            c.out.torque(0, Vec3::Z.cross(dir) * (w * self.roll * 0.1));
            if self.still > 1.0 {
                self.end("settled");
            }
            return;
        }
        let k = |v: &[f32; 4]| {
            let up = ramp(v[0], v[2], t, self.start_time, self.ramp_from);
            (up, ramp(v[1], v[3], t, self.start_time, self.ramp_from))
        };
        let ((ak, ad), (sk, sd)) = (k(&self.arm), k(&self.spine));
        let lk = ramp(self.legs.0, self.legs.1, t, self.time_end, self.ramp_to);
        self.next_relax -= dt;
        if self.next_relax < 0.0 {
            let r = c.ch.random();
            self.next_relax = self.relax.0 + (self.relax.1 - self.relax.0) * r;
        }
        let relax = if self.next_relax < 0.15 { 0.33 } else { 1.0 };
        c.ch.set_stiffness(ak * relax, ad, "ua", None);
        c.ch.set_stiffness(sk * relax, sd, "uk", None);
        c.ch.set_stiffness(lk, 1.0, "lb", None);
        let along = dir.dot(facing(c));
        let fade = if t < self.pull { 1.0 } else { ramp(1.0, 0.0, t, self.pull, 1.0) };
        let pitch = if along > 0.0 { self.pitch.0 } else { self.pitch.1 } * along * fade;
        let sway = (t / self.hula.0.max(0.05) * std::f32::consts::TAU).sin() * fade;
        c.ch.set_relative(SPINE[0], 0, 0.5 * pitch);
        c.ch.set_relative(SPINE[0], 1, 0.3 * self.hula.1 * sway);
        for &j in &SPINE[1..] {
            c.ch.set_relative(j, 0, -0.25 * self.bend * along * fade);
            c.ch.set_relative(j, 1, -0.3 * self.hula.2 * sway);
        }
        if t > self.time_end + self.ramp_to && self.still > 0.5 {
            self.end("recovered");
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.finished
    }
}

pub struct Dragged {
    arm: [f32; 3],
    radius_tol: f32,
    length_tol: f32,
    anchor: Vec3,
    part: i32,
    player: bool,
    want: [bool; 2],
    reach: f32,
    twist: f32,
    dir: Vec3,
    speed: f32,
    delay: f32,
    force: f32,
    length: f32,
    grabbed: [Option<f32>; 2],
    pulled: bool,
    t: f32,
    out: Vec<String>,
}

impl Dragged {
    fn new(p: &mut P) -> Result<Dragged, Error> {
        let _ = (p.i("ropeAttachedToInstance", -1)?, p.b("ropeTaut", false)?);
        Ok(Dragged {
            arm: [p.f("armStiffness", 11.0)?, p.f("armDamping", 1.0)?, p.f("armMuscleStiffness", 1.0)?],
            radius_tol: p.f("radiusTolerance", 0.25)?,
            length_tol: p.f("lengthTolerance", 0.1)?,
            anchor: p.v("ropePos", Vec3::ZERO)?,
            part: p.i("ropedBodyPart", -1)?.clamp(-1, 20),
            player: p.b("playerControl", false)?,
            want: [p.b("grabLeft", true)?, p.b("grabRight", true)?],
            reach: p.f("reach", 0.7)?,
            twist: p.f("armTwist", 0.0)?,
            dir: p.v("direction", Vec3::ZERO)?,
            speed: p.f("speed", 1.2)?,
            delay: p.f("delay", 0.7)?,
            force: p.f("force", 0.6)?,
            length: 0.0,
            grabbed: [None; 2],
            pulled: false,
            t: 0.0,
            out: vec![],
        })
    }
}

impl Extra for Dragged {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let chest = pos(c, CHEST);
        if self.t == 0.0 {
            self.out.push("bodyBalance start=true".into());
            if self.dir == Vec3::ZERO {
                self.dir = facing(c);
            }
            self.dir = Vec3::new(self.dir.x, self.dir.y, 0.0).normalize_or(Vec3::NEG_Y);
            if self.anchor == Vec3::ZERO {
                self.anchor = chest + self.dir * 2.0 - Vec3::Z * 0.2;
            }
            self.length = match self.part {
                -1 => (self.anchor - chest).length() - 0.3,
                k => (self.anchor - pos(c, k as usize)).length(),
            };
        }
        self.t += dt;
        if self.t > self.delay {
            self.anchor += self.dir * (self.speed * dt);
        }
        let w = weight(c);
        let cap = self.force * w;
        let along = |s: f32| self.anchor - self.dir * s;
        let mut taut = false;
        if self.part >= 0 {
            let k = self.part as usize;
            let over = (self.anchor - pos(c, k)).length() - self.length - self.length_tol;
            if over > 0.0 {
                taut = true;
                spring(c, k, self.anchor, w * 3.0, w, cap);
            }
        }
        if self.pulled {
            c.ch.set_stiffness(3.0, 0.6, "fb", None);
        }
        for i in 0..2 {
            if !if self.player { self.want[i] } else { self.part < 0 } {
                continue;
            }
            let (l, hand) = (crate::nmshot::limb(i), HANDS[i]);
            match self.grabbed[i] {
                Some(s) => {
                    let to = along(s);
                    if (to - pos(c, hand)).length() > self.length_tol {
                        taut = true;
                    }
                    spring(c, hand, to, w * 6.0, w, cap * 0.5);
                    c.ch.eff[l.e[1]].set_stiffness(self.arm[0], self.arm[1], Some(self.arm[2]));
                    c.ch.eff[l.e[2]].set_stiffness(self.arm[0], self.arm[1], Some(self.arm[2]));
                }
                None => {
                    let h = pos(c, hand);
                    let s = (self.anchor - h).dot(self.dir).clamp(0.0, self.length);
                    let to = along(s);
                    if (to - h).length() < self.radius_tol {
                        self.grabbed[i] = Some(s);
                        self.out.push(format!("#say dragged grabbed {}", ["left", "right"][i]));
                        continue;
                    }
                    let target = chest + (to - chest).clamp_length_max(self.reach);
                    c.ch.eff[l.e[1]].set_stiffness(self.arm[0], self.arm[1], Some(self.arm[2]));
                    c.ch.eff[l.e[2]].set_stiffness(self.arm[0], self.arm[1], Some(self.arm[2]));
                    crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], target, self.twist, 0.0,
                                          1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
                }
            }
        }
        if taut != self.pulled {
            self.pulled = taut;
            self.out.push(format!("pedalLegs start={taut}"));
            if taut {
                self.out.push("bodyBalance start=false".into());
            }
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }
}

pub struct Dangle {
    grab: bool,
    freq: f32,
    point: Vec3,
    hold: bool,
    t: f32,
    pedal: (bool, f32),
    hang: (bool, f32),
    reach: f32,
    reaching: bool,
    out: Vec<String>,
}

impl Dangle {
    fn new(p: &mut P) -> Result<Dangle, Error> {
        Ok(Dangle { grab: p.b("doGrab", false)?, freq: p.f("grabFrequency", 0.5)?.clamp(0.0, 1.0), point: p.v("point", Vec3::ZERO)?,
                    hold: p.b("hold", true)?, t: 0.0, pedal: (false, 1.0), hang: (false, 2.0), reach: 1.0, reaching: false,
                    out: vec![] })
    }

    fn wait(c: &mut Ctx) -> f32 {
        1.0 + 3.0 * c.ch.random()
    }
}

impl Extra for Dangle {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.point == Vec3::ZERO {
            self.point = pos(c, NECK[1] + 1) + Vec3::Z * 1.0;
            self.out.push("bodyBalance start=false".into());
        }
        self.t += dt;
        let w = weight(c);
        if self.hold {
            for (i, k) in HANDS.into_iter().enumerate() {
                let at = self.point + Vec3::Z.cross(facing(c)) * if i == 0 { 0.15 } else { -0.15 };
                spring(c, k, at, w * 2.0, w * 0.3, w * 0.3);
            }
            spring(c, CHEST, self.point - Vec3::Z * 0.75, w * 6.0, w * 1.5, w * 1.2);
        }
        for i in 0..2 {
            for (j, s) in [(CLAVICLE[i], 9.0), (SHOULDER[i], 8.0), (ELBOW[i], 7.0), (crate::nmshot::limb(i).e[3], 6.0)] {
                c.ch.eff[j].set_stiffness(s, s / 10.0, None);
            }
        }
        c.ch.set_stiffness(3.0, 0.3, "us", None);
        c.ch.set_stiffness(3.0, 0.3, "lb", None);
        self.hang.1 -= dt;
        if self.hang.1 < 0.0 {
            self.hang = (!self.hang.0, Self::wait(c));
        }
        if self.hang.0 {
            let s = (self.t * 2.5).sin() * 0.4;
            for j in HIP {
                c.ch.set_relative(j, 0, s);
            }
        }
        self.pedal.1 -= dt;
        if self.pedal.1 < 0.0 {
            self.pedal = (!self.pedal.0, Self::wait(c));
            self.out.push(format!("pedalLegs start={}", self.pedal.0));
        }
        if !self.grab {
            return;
        }
        self.reach -= dt;
        if self.reach < 0.0 {
            self.reaching = !self.reaching;
            self.reach = if self.reaching { 1.0 } else { Self::wait(c) * (1.0 + 3.0 * (1.0 - self.freq)) };
            let p = self.point;
            self.out.push(match self.reaching {
                true => format!("headLook start=true pos={},{},{} stiffness=11 damping=1", p.x, p.y, p.z),
                false => "headLook start=false".into(),
            });
        }
        if self.reaching {
            for i in 0..2 {
                let l = crate::nmshot::limb(i);
                let at = self.point + Vec3::Z * 0.15;
                crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], at, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0,
                                      Vec3::ZERO, None);
            }
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }
}

pub struct Carried { point: Vec3, hold: bool, s: [f32; 6] }

impl Carried {
    fn new(p: &mut P) -> Result<Carried, Error> {
        Ok(Carried { point: p.v("point", Vec3::ZERO)?, hold: p.b("hold", true)?,
                     s: [p.f("trunkStiffness", 5.0)?, p.f("legStiffness", 2.0)?, p.f("armStiffness", 1.5)?, p.f("clavicleStiffness", 4.0)?,
                         p.f("neckStiffness", 7.0)?, p.f("stillLegStiffness", 4.0)?] })
    }
}

impl Extra for Carried {
    fn tick(&mut self, c: &mut Ctx, _dt: f32) {
        if self.point == Vec3::ZERO {
            self.point = pos(c, 0) + Vec3::Z * 0.6;
        }
        let [trunk, leg, arm, clav, neck, still_leg] = self.s;
        let still = c.b.part(0).v.length() < 0.3;
        c.ch.set_stiffness(trunk, 0.5, "us", None);
        c.ch.set_stiffness(if still { still_leg } else { leg }, 0.5, "ll", None);
        c.ch.set_stiffness(still_leg, 0.5, "lr", None);
        c.ch.set_stiffness(arm, 0.5, "ua", None);
        c.ch.set_stiffness(clav, 0.5, "uc", None);
        c.ch.set_stiffness(neck, 1.0, "un", None);
        if self.hold {
            let w = weight(c);
            spring(c, 0, self.point, w * 25.0, w * 5.0, w * 2.0);
        }
    }
}

pub struct Buoyancy {
    buoyancy: f32,
    chest: f32,
    damping: f32,
    point: Vec3,
    normal: Vec3,
    righting: bool,
    strength: f32,
    time: f32,
    t: f32,
}

impl Buoyancy {
    fn new(p: &mut P) -> Result<Buoyancy, Error> {
        Ok(Buoyancy { buoyancy: p.f("buoyancy", 1.1)?, chest: p.f("chestBuoyancy", 0.5)?, damping: p.f("damping", 3.0)?,
                      point: p.v("surfacePoint", Vec3::ZERO)?, normal: p.v("surfaceNormal", Vec3::Z)?.normalize_or(Vec3::Z),
                      righting: p.b("righting", true)?, strength: p.f("rightingStrength", 0.3)?, time: p.f("rightingTime", 2.0)?,
                      t: 0.0 })
    }
}

impl Extra for Buoyancy {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.point == Vec3::ZERO {
            self.point = pos(c, CHEST);
        }
        let n = self.normal;
        let mut wet = false;
        for k in 0..21 {
            let (p, m) = (c.b.part(k), c.b.doll.part_mass(k));
            let under = ((self.point - p.m.d).dot(n) / 0.2).clamp(0.0, 1.0);
            if under > 0.0 {
                wet = true;
                let chest = if k == CHEST { self.chest * weight(c) * 0.1 } else { 0.0 };
                c.out.force(k, (n * (m * 9.81 * self.buoyancy + chest) - p.v * (m * self.damping)) * under);
            }
        }
        if !wet {
            return;
        }
        self.t += dt;
        c.ch.set_stiffness(6.0, 0.8, "fb", None);
        if self.righting && self.t < self.time {
            let spine = (pos(c, CHEST) - pos(c, 0)).normalize_or_zero();
            let front = (pos(c, CLAVICLE[0] + 1) - pos(c, CLAVICLE[1] + 1)).cross(spine).normalize_or_zero();
            let ease = 1.0 - self.t / self.time.max(0.01);
            let tq = front.cross(n) * (self.strength * weight(c) * ease);
            c.out.torque(CHEST, tq);
            c.out.torque(0, tq);
        }
    }
}

pub struct AnimPose {
    pose: Vec<(usize, [f32; 3])>,
    mask: String,
    global: [f32; 4],
    regions: [[f32; 4]; 5],
    over: [bool; 3],
    connected: [bool; 4],
    plane: Vec3,
    frozen: bool,
}

const REGIONS: [(&str, &str); 5] = [("LeftArm", "ul"), ("RightArm", "ur"), ("Spine", "uk"), ("LeftLeg", "ll"), ("RightLeg", "lr")];

impl AnimPose {
    fn new(p: &mut P) -> Result<AnimPose, Error> {
        let global = [p.f("stiffness", 12.0)?, p.f("damping", 1.0)?, p.f("muscleStiffness", -1.0)?, p.f("gravityCompensation", 0.0)?];
        let mut regions = [[-1.0; 4]; 5];
        for (r, (name, _)) in regions.iter_mut().zip(REGIONS) {
            for (x, what) in r.iter_mut().zip(["stiffness", "damping", "muscleStiffness", "gravComp"]) {
                *x = p.f(&format!("{what}{name}"), -1.0)?;
            }
        }
        let _ = (p.b("useZMPGravityCompensation", false)?, p.i("animSource", 0)?, p.i("dampenSideMotionInstanceIndex", -1)?);
        let _ = p.v("vDampenPlanePoint", Vec3::ZERO)?;
        Ok(AnimPose {
            pose: parse_pose(&p.s("pose", "")?),
            mask: p.s("mask", "fb")?,
            global,
            regions,
            over: [p.b("overideHeadlook", false)?, p.b("overidePointArm", false)?, p.b("overidePointGun", false)?],
            connected: [p.i("connectedLeftHand", 0)? != 0, p.i("connectedRightHand", 0)? != 0, p.i("connectedLeftFoot", 0)? != 0,
                        p.i("connectedRightFoot", 0)? != 0],
            plane: p.v("vDampenPlaneNormal", Vec3::ZERO)?.normalize_or_zero(),
            frozen: false,
        })
    }
}

impl Extra for AnimPose {
    fn tick(&mut self, c: &mut Ctx, _dt: f32) {
        let mine = Character::mask(&self.mask);
        if !self.frozen {
            self.frozen = true;
            if self.pose.is_empty() {
                self.pose = mine.iter().map(|&j| (j, c.ch.actual(j, c.b.frames))).collect();
            }
        }
        let mut skip = vec![];
        for (o, joints) in self.over.iter().zip([&NECK[..], &Character::mask("ul"), &Character::mask("ur")]) {
            if *o {
                skip.extend_from_slice(joints);
            }
        }
        let connected = [self.connected[0], self.connected[1], false, self.connected[2], self.connected[3]];
        for (i, (_, m)) in REGIONS.iter().enumerate() {
            let v: Vec<f32> = (0..4).map(|k| if self.regions[i][k] < 0.0 { self.global[k] } else { self.regions[i][k] }).collect();
            let s = if connected[i] { v[0] * 0.5 } else { v[0] };
            for j in Character::mask(m) {
                if skip.contains(&j) || !mine.contains(&j) {
                    continue;
                }
                let ms = if WRIST.contains(&j) || ANKLE.contains(&j) { Some(16.0) } else { (v[2] >= 0.0).then_some(v[2]) };
                c.ch.eff[j].set_stiffness(s, v[1], ms);
                c.ch.eff[j].oppose_gravity = v[3];
            }
        }
        for &(j, a) in &self.pose {
            if !skip.contains(&j) {
                c.ch.eff[j].desired = a;
            }
        }
        if self.plane != Vec3::ZERO {
            for k in 0..21 {
                let v = c.b.part(k).v.dot(self.plane);
                c.out.force(k, self.plane * (-v * c.b.doll.part_mass(k) * 10.0));
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Circle { normal: Vec3, centre: Vec3, part: usize, r1: f32, r2: f32, speed: f32 }

pub struct ArmsWindmill {
    arm: [Circle; 2],
    deltas: [f32; 5],
    gains: [f32; 4],
    phase: f32,
    twist: f32,
    spin: (f32, f32),
    elbow_min: [f32; 2],
    mirror: i32,
    adaptive: bool,
    use_arm: [bool; 2],
    disable_on_impact: bool,
    angle: [f32; 2],
    hit: [bool; 2],
}

impl ArmsWindmill {
    fn new(p: &mut P) -> Result<ArmsWindmill, Error> {
        let mut circle = |side: &str, y: f32| -> Result<Circle, Error> {
            Ok(Circle { normal: p.v(&format!("{side}Normal"), Vec3::new(0.0, y, 0.0))?.normalize_or(Vec3::Y),
                        centre: p.v(&format!("{side}Centre"), Vec3::new(0.15, 0.25 * y, 0.05))?,
                        part: p.i(&format!("{side}PartID"), CHEST as i32)?.clamp(0, 20) as usize,
                        r1: p.f(&format!("{side}Radius1"), 0.45)?, r2: p.f(&format!("{side}Radius2"), 0.45)?,
                        speed: p.f(&format!("{side}Speed"), 1.0)? })
        };
        let arm = [circle("left", 1.0)?, circle("right", -1.0)?];
        let _ = p.f("dragReduction", 0.0)?;
        let force_sync = p.b("forceSync", false)?;
        let phase = p.f("phaseOffset", std::f32::consts::PI)?;
        Ok(ArmsWindmill {
            arm,
            deltas: [p.f("radius1Delta", 0.0)?, p.f("radius2Delta", 0.0)?, p.f("speedDelta", 0.0)?, p.f("shoulderStiffnessDelta", 0.0)?,
                     p.f("elbowStiffnessDelta", 0.0)?],
            gains: [p.f("shoulderStiffness", 12.0)?, p.f("shoulderDamping", 1.0)?, p.f("elbowStiffness", 12.0)?, p.f("elbowDamping", 1.0)?],
            phase: if force_sync { 0.0 } else { phase },
            twist: p.f("IKtwist", 0.0)?,
            spin: (p.f("angVelThreshold", 0.1)?, p.f("angVelGain", 1.0)?),
            elbow_min: [p.f("leftElbowMin", 0.3)?, p.f("rightElbowMin", 0.3)?],
            mirror: p.i("mirrorMode", 0)?,
            adaptive: p.b("adaptiveMode", false)?,
            use_arm: [p.b("useLeft", true)?, p.b("useRight", true)?],
            disable_on_impact: p.b("disableOnImpact", true)?,
            angle: [0.0; 2],
            hit: [false; 2],
        })
    }
}

impl Extra for ArmsWindmill {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let over = if self.adaptive { (c.angvel.length() - self.spin.0).max(0.0) } else { 0.0 };
        if over > 0.0 {
            for a in &mut self.arm {
                (a.r1, a.r2, a.speed) = (a.r1 + self.deltas[0] * dt, a.r2 + self.deltas[1] * dt, a.speed + self.deltas[2] * dt);
            }
            self.gains[0] += self.deltas[3] * dt;
            self.gains[2] += self.deltas[4] * dt;
        }
        let fwd = facing(c);
        let up = c.ch.up.normalize_or(Vec3::Z);
        let left = up.cross(fwd);
        let world = |v: Vec3| fwd * v.x + left * v.y + up * v.z;
        for i in 0..2 {
            if self.disable_on_impact && c.b.doll.collided(&[HANDS[i]]) {
                self.hit[i] = true;
            }
            if !self.use_arm[i] || self.hit[i] {
                continue;
            }
            let a = self.arm[i];
            let dir = if i == 1 && self.mirror == 1 { -1.0 } else { 1.0 };
            self.angle[i] += dir * a.speed * (1.0 + over * self.spin.1) * std::f32::consts::TAU * dt;
            let th = self.angle[i] - if i == 1 { self.phase } else { 0.0 };
            let n = world(a.normal).normalize_or(left);
            let u = n.cross(if n.dot(up).abs() > 0.9 { fwd } else { up }).normalize();
            let v = n.cross(u);
            let target = pos(c, a.part) + world(a.centre) + u * (a.r1 * th.cos()) + v * (a.r2 * th.sin());
            let l = crate::nmshot::limb(i);
            c.ch.eff[l.e[1]].set_stiffness(self.gains[0], self.gains[1], None);
            c.ch.eff[l.e[2]].set_stiffness(self.gains[2], self.gains[3], None);
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], target, self.twist, 0.0, 1.0 - 2.0 * i as f32, 1.0,
                                  Vec3::ZERO, None);
            let e = &mut c.ch.eff[l.e[2]].desired[0];
            *e = e.max(self.elbow_min[i]);
        }
    }
}

pub struct Twist {
    pos: Vec3,
    offset: f32,
    vel: Vec3,
    excess: f32,
    k: (f32, f32),
    clav: bool,
    always: bool,
    twisting: Option<bool>,
    out: Vec<String>,
}

impl Twist {
    fn new(p: &mut P) -> Result<Twist, Error> {
        Ok(Twist { pos: p.v("pos", Vec3::ZERO)?, offset: p.f("offset", 0.0)?,
                   vel: Vec3::new(p.f("velX", 0.0)?, p.f("velY", 0.0)?, p.f("velZ", 0.0)?), excess: p.f("spineTwistExcess", 0.35)?,
                   k: (p.f("spStrength", 12.0)?, p.f("spDamping", 2.0)?), clav: p.b("twistClavicles", true)?,
                   always: p.b("allwaysTwist", false)?, twisting: None, out: vec![] })
    }
}

impl Extra for Twist {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.pos == Vec3::ZERO {
            self.pos = pos(c, CHEST) + Vec3::Z.cross(facing(c)) * 1.5 - facing(c) * 0.5;
        }
        self.pos += self.vel * dt;
        let p = c.b.part(0);
        let n = (self.pos + (self.vel - p.v) * 0.1 - p.m.d).normalize_or_zero();
        let twist = p.m.b.dot(n).atan2(-p.m.c.dot(n)) + self.offset;
        let on = self.always || twist.abs() < 0.8 * std::f32::consts::PI;
        if self.twisting != Some(on) {
            self.twisting = Some(on);
            self.out.push(format!("#say spineTwist {}", if on { "twisting" } else { "stopped" }));
        }
        if !on {
            return;
        }
        c.ch.set_stiffness(self.k.0, self.k.1, "us", None);
        let range = SPINE.map(|j| {
            let l = c.ch.limits(j);
            (l[4] * (1.0 + self.excess), l[5] * (1.0 + self.excess))
        });
        let sum = range.iter().fold((0.0, 0.0), |s, r| (s.0 + r.0, s.1 + r.1));
        let up = (pos(c, CHEST) - pos(c, 0)).normalize_or_zero().z.abs();
        let x = (up * twist).clamp(sum.0, sum.1);
        for (k, j) in SPINE.into_iter().enumerate() {
            c.ch.eff[j].desired[2] = if x >= 0.0 { range[k].1 / sum.1 } else { range[k].0 / sum.0 } * x;
        }
        if self.clav {
            c.ch.eff[CLAVICLE[1]].desired[0] = x;
            c.ch.eff[CLAVICLE[0]].desired[0] = -x;
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }
}

pub struct Wave { side: usize, time: f32, rate: f32, size: f32, t: f32, out: Vec<String> }

impl Wave {
    fn new(p: &mut P) -> Result<Wave, Error> {
        Ok(Wave { side: p.b("right", true)? as usize, time: p.f("time", 2.25)?, rate: p.f("rate", 2.0)?, size: p.f("size", 0.18)?,
                  t: 0.0, out: vec![] })
    }
}

const WAVE_UP: f32 = 0.3;
const WAVE_DOWN: f32 = 0.3;

impl Extra for Wave {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.t == 0.0 {
            self.out.push("#say wave Hi!".into());
        }
        self.t += dt;
        let i = self.side;
        let l = crate::nmshot::limb(i);
        let (f, left) = (facing(c), Vec3::Z.cross(facing(c)));
        let out = if i == 0 { left } else { -left };
        let shoulder = pos(c, l.p[0]);
        let to = if self.t < WAVE_UP + self.time {
            let sway = (self.t * self.rate * std::f32::consts::TAU).sin() * self.size * ((self.t / WAVE_UP).min(1.0));
            shoulder + out * (0.35 + sway) + Vec3::Z * 0.45 * (self.t / WAVE_UP).min(1.0) + f * 0.05
        } else {
            shoulder + out * 0.65
        };
        c.ch.set_stiffness(12.0, 1.0, ["ul", "ur"][i], None);
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], to, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.t > WAVE_UP + self.time + WAVE_DOWN
    }
}

pub struct Breathe { s: f32, joints: Vec<usize>, scale: f32, t: f32, last: f32, wrote: Vec<f32> }

const BREATH: f32 = 4.0;
const BREATH_LEAN: f32 = 0.03;

impl Breathe {
    fn new(p: &mut P) -> Result<Breathe, Error> {
        Ok(Breathe { s: p.f("stiffness", 10.0)?, joints: Character::mask(&p.s("effectorMask", "us")?), scale: p.f("breathingScale", 1.0)?,
                     t: 0.0, last: 0.0, wrote: vec![] })
    }
}

impl Extra for Breathe {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let lean = -BREATH_LEAN * self.scale * (0.5 - 0.5 * (self.t * std::f32::consts::TAU / BREATH).cos());
        self.wrote.resize(self.joints.len(), f32::NAN);
        for (&j, w) in self.joints.iter().zip(&mut self.wrote) {
            c.ch.eff[j].set_stiffness(self.s, 1.0, None);
            let d = &mut c.ch.eff[j].desired[0];
            *d += if *d == *w { lean - self.last } else { lean };
            *w = *d;
        }
        c.ch.eff[NECK[0]].set_stiffness(self.s * 0.9, 1.0, None);
        self.last = lean;
    }
}

pub struct SoftKeyframe { follow: f32, most: f32, slower: bool, yaw: f32, target: Vec3, parts: Vec<usize>, pose: Vec<Vec3> }

impl SoftKeyframe {
    fn new(p: &mut P) -> Result<SoftKeyframe, Error> {
        let (follow, most, slower, yaw, target) = (p.f("followMultiplier", 10.0)?, p.f("maxAcceleration", 20.0)?,
                                                   p.b("goSlowerWhenWeaker", true)?, p.f("yawOffset", 0.0)?, p.v("targetPosition", Vec3::ZERO)?);
        let parts = Character::mask(&p.s("mask", "fb")?).into_iter().map(|j| j + 1).collect();
        Ok(SoftKeyframe { follow, most, slower, yaw, target, parts, pose: vec![] })
    }
}

impl Extra for SoftKeyframe {
    fn tick(&mut self, c: &mut Ctx, _dt: f32) {
        let pelvis = pos(c, 0);
        if self.pose.is_empty() {
            self.pose = self.parts.iter().map(|&k| pos(c, k) - pelvis).collect();
        }
        let (anchor, v0) = if self.target == Vec3::ZERO { (pelvis, c.b.part(0).v) } else { (self.target, Vec3::ZERO) };
        let turn = bevy_math::Quat::from_rotation_z(self.yaw);
        for (&k, &off) in self.parts.iter().zip(&self.pose) {
            let gap = anchor + turn * off - pos(c, k);
            let mut a = (gap * self.follow - (c.b.part(k).v - v0) * 2.0 * self.follow.max(0.0).sqrt()).clamp_length_max(self.most);
            if self.slower {
                a *= (c.ch.eff[k - 1].strength.max(0.0).sqrt() / 12.0).min(1.0);
            }
            c.out.force(k, a * c.b.doll.part_mass(k));
        }
    }
}

fn aim(c: &mut Ctx, i: usize, at: Vec3, s: f32, d: f32) {
    let shoulder = pos(c, crate::nmshot::limb(i).p[0]);
    let dir = if at == Vec3::ZERO { facing(c) } else { (at - shoulder).normalize_or(facing(c)) };
    reach(c, i, shoulder + dir * 0.55, s, d);
}

fn reach(c: &mut Ctx, i: usize, at: Vec3, s: f32, d: f32) {
    let l = crate::nmshot::limb(i);
    c.ch.set_stiffness(s, d, ["ul", "ur"][i], None);
    crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[3], at, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
}

fn v3(p: Vec3) -> String {
    format!("{},{},{}", p.x, p.y, p.z)
}

pub struct SplitBody {
    aim: Vec3,
    pose_mask: Vec<usize>,
    hard: Vec<usize>,
    soft: SoftKeyframe,
    grav: Vec<usize>,
    s: (f32, f32),
    ctrl: (f32, f32),
    gun: Option<usize>,
    support: Option<usize>,
    constrain: bool,
    orient: bool,
    limits: bool,
    stable: Option<(f32, f32)>,
    recoil: Option<(f32, f32)>,
    breathe: Breathe,
    pose: Vec<(usize, [f32; 3])>,
    hold: Vec<(usize, Vec3, Quat)>,
    turn: Quat,
    relax: f32,
    kick: bool,
    cons: Vec<(usize, Option<Vec3>, Option<Quat>)>,
    start: Vec<Vec3>,
    t: f32,
}

impl SplitBody {
    fn new(p: &mut P) -> Result<SplitBody, Error> {
        let aim = p.v("aimTarget", Vec3::ZERO)?;
        let pose_mask = Character::mask(&p.s("activePoseMask", "uk")?);
        let parts = |m: String| Character::mask(&m).into_iter().map(|j| j + 1).collect::<Vec<_>>();
        let hard = if p.s("hardConstraintMask", "")?.is_empty() { vec![] } else { parts(p.s("hardConstraintMask", "")?) };
        let soft_parts = if p.s("softConstraintMask", "")?.is_empty() { vec![] } else { parts(p.s("softConstraintMask", "")?) };
        let _ = p.s("collisionMask", "fb")?;
        let grav = Character::mask(&p.s("gravityCompensationMask", "ub")?);
        let (s, ctrl) = ((p.f("stiffness", 12.0)?, p.f("damping", 1.0)?), (p.f("controllerStiffness", 14.0)?, p.f("controllerDamping", 1.0)?));
        let weapon = [p.i("leftHandWeaponIndex", -1)?, p.i("rightHandWeaponIndex", 0)?];
        let status = [p.i("leftHandStatus", 2)?, p.i("rightHandStatus", 1)?];
        let gun = (0..2).find(|&i| status[i] == 1 && weapon[i] >= 0);
        let support = gun.and_then(|g| (status[1 - g] == 2).then_some(1 - g));
        let (constrain, orient, limits) = (p.b("constrainHands", true)?, p.b("controlHandOrientation", true)?, p.b("adjustJointLimits", true)?);
        let stable = p.b("handStable", false)?.then_some(()).map(|()| (0.0, 0.0));
        let stable_g = (p.f("handStableStiff", 14.0)?, p.f("handStableDamp", 1.0)?);
        let recoil = p.b("recoilRelax", true)?;
        let recoil_g = (p.f("recoilRelaxAmount", 0.5)?, p.f("recoilRelaxTime", 0.3)?);
        let breathe = Breathe { s: s.0, joints: Character::mask("us"), scale: p.f("breathingScale", 1.0)?, t: 0.0, last: 0.0, wrote: vec![] };
        let _ = p.b("fireWeapon", false)?;
        Ok(SplitBody { aim, pose_mask, hard, soft: SoftKeyframe { follow: 10.0, most: 20.0, slower: true, yaw: 0.0, target: Vec3::ZERO, parts: soft_parts, pose: vec![] },
                       grav, s, ctrl, gun, support, constrain, orient, limits, stable: stable.map(|_| stable_g), recoil: recoil.then_some(recoil_g),
                       breathe, pose: vec![], hold: vec![], turn: Quat::IDENTITY, relax: 0.0, kick: false, cons: vec![], start: vec![], t: 0.0 })
    }

    fn hand(i: usize) -> usize {
        crate::nmshot::limb(i).p[3]
    }
}

const GRIP_ON: f32 = 0.4;
const GRIP_W: f32 = 25.0;
const GRIP_MOST: f32 = 200.0;

impl Extra for SplitBody {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let f = c.b.frames;
        if self.pose.is_empty() {
            self.pose = self.pose_mask.iter().map(|&j| (j, c.ch.actual(j, f))).collect();
            self.hold = self.hard.iter().map(|&k| (k, f[k].translation, f[k].rotation)).collect();
            if let (Some(g), Some(h)) = (self.gun, self.support) {
                self.turn = f[Self::hand(g)].rotation.inverse() * f[Self::hand(h)].rotation;
            }
            self.start = (0..2).map(|i| f[Self::hand(i)].translation).collect();
        }
        self.t += dt;
        let give = match self.recoil {
            Some((amount, time)) if self.relax > 0.0 => 1.0 - amount * (self.relax / time.max(1e-3)),
            _ => 1.0,
        };
        self.relax = (self.relax - dt).max(0.0);
        for &(j, a) in &self.pose {
            c.ch.eff[j].set_stiffness(self.s.0 * give, self.s.1, None);
            c.ch.eff[j].desired = a;
            c.ch.eff[j].oppose_gravity = if self.grav.contains(&j) { 1.0 } else { 0.0 };
        }
        self.breathe.tick(c, dt);
        self.cons = self.hold.iter().map(|&(k, x, q)| (k, Some(x), Some(q))).collect();
        self.soft.tick(c, dt);
        let Some(g) = self.gun else { return };
        aim(c, g, self.aim, self.ctrl.0 * give, self.ctrl.1);
        if self.limits {
            for e in ELBOW {
                let at = c.ch.eff[e].desired[0];
                let mut l = crate::nmrs::SoftLimit { stiffness: 200.0, damping: 1.0, enabled: true, ..Default::default() };
                l.set_limit(at + 0.15 * at.signum(), at.signum(), false);
                c.ch.eff[e].soft[0] = l;
            }
        }
        if let Some((s, d)) = self.stable {
            c.ch.eff[WRIST[g]].set_stiffness(s * give, d, None);
        }
        let gh = Self::hand(g);
        if std::mem::take(&mut self.kick) {
            let back = -facing(c);
            c.out.force(gh, (Vec3::Z + back).normalize() * (2.0 / dt));
        }
        if let (Some(h), true) = (self.support, self.constrain) {
            let across = Vec3::Z.cross(facing(c)) * if h == 0 { 1.0 } else { -1.0 };
            let grip = f[gh].translation + across * 0.07 - Vec3::Z * 0.03;
            let on = (self.t / GRIP_ON).min(1.0);
            let hand = Self::hand(h);
            let to = self.start[h].lerp(grip, on);
            reach(c, h, to, self.ctrl.0 * give, self.ctrl.1);
            let (m, w) = (c.b.doll.part_mass(hand), GRIP_W);
            let (a, b) = (c.b.part(hand), c.b.part(gh));
            let pull = ((to - a.m.d) * (m * w * w) - (a.v - b.v) * (2.0 * m * w)).clamp_length_max(GRIP_MOST);
            c.out.force(hand, pull);
            c.out.force(gh, -pull);
            if self.orient {
                let to = f[hand].rotation.slerp(f[gh].rotation * self.turn, on);
                let i = m * 0.002;
                let twist = (short_turn(to * f[hand].rotation.inverse()) * (i * w * w) - (a.w - b.w) * (2.0 * i * w)).clamp_length_max(GRIP_MOST * 0.05);
                c.out.torque(hand, twist);
                c.out.torque(gh, -twist);
            }
        }
    }

    fn constraints(&self) -> Vec<(usize, Option<Vec3>, Option<Quat>)> {
        self.cons.clone()
    }

    fn fire(&mut self) {
        self.kick = true;
        if let Some((_, time)) = self.recoil {
            self.relax = time;
        }
    }

    fn stop(&mut self, c: &mut Ctx) {
        if self.limits {
            for e in ELBOW {
                c.ch.eff[e].soft[0].set_enabled(false);
            }
        }
    }
}

pub struct ShootDodge {
    targets: [Vec3; 2],
    valid: bool,
    ends: (f32, f32, f32),
    reaction: i32,
    trunk: (f32, f32),
    legs: (f32, f32, f32),
    aiming: (f32, f32),
    loose: (f32, f32, f32),
    head: (f32, f32),
    wrist_ms: f32,
    ori: (f32, f32, f32),
    noise: f32,
    breathe: Breathe,
    state: u8,
    t: f32,
    touching: f32,
    last_vel: Option<Vec3>,
    attitude: Quat,
    leg_pose: Vec<(usize, [f32; 3])>,
    done: bool,
    out: Vec<String>,
}

const SD_ENTER: u8 = 0;
const SD_HOLD: u8 = 1;

impl ShootDodge {
    fn new(p: &mut P) -> Result<ShootDodge, Error> {
        let targets = [p.v("leftArmTarget", Vec3::ZERO)?, p.v("rightArmTarget", Vec3::ZERO)?];
        let valid = p.b("targetValid", true)?;
        for n in ["collisionPoint", "collisionNormal", "collisionObjectVelocity"] {
            p.v(n, Vec3::ZERO)?;
        }
        let _ = (p.i("collisionIndex", -1)?, p.f("collisionTime", 0.0)?, p.f("supportedHeightThreshold", 0.0)?);
        let _ = (p.b("armAnimTask", false)?, p.i("leftArmAnimTaskParent", -1)?, p.i("rightArmAnimTaskParent", -1)?);
        let ends = (p.f("collisionTimeThreshold", 0.2)?, p.f("collisionAccelThreshold", 40.0)?, p.f("collisionVelThreshold", 2.0)?);
        Ok(ShootDodge {
            targets, valid, ends, reaction: p.i("collisionReaction", 0)?,
            trunk: (p.f("trunkStiffness", 12.0)?, p.f("trunkDamping", 1.0)?),
            legs: (p.f("legStiffness", 10.0)?, p.f("legDamping", 1.0)?, p.f("legTaper", 0.8)?),
            aiming: (p.f("aimingArmStiffness", 14.0)?, p.f("aimingArmDamping", 1.0)?),
            loose: (p.f("notAimingArmStiffness", 6.0)?, p.f("notAimingArmDamping", 0.7)?, p.f("notAimingArmTaper", 0.7)?),
            head: (p.f("headStiffness", 10.0)?, p.f("headDamping", 1.0)?),
            wrist_ms: p.f("wristMuscleStiffness", 10.0)?,
            ori: (p.f("oriStiff", 30.0)?, p.f("oriDamp", 8.0)?, p.f("oriSplit", 0.5)?.clamp(0.0, 1.0)),
            noise: p.f("legNoiseScale", 0.05)?,
            breathe: Breathe { s: 12.0, joints: Character::mask("us"), scale: p.f("breathingScale", 1.0)?, t: 0.0, last: 0.0, wrote: vec![] },
            state: if p.i("state", 0)? == 1 { SD_HOLD } else { SD_ENTER },
            t: 0.0, touching: 0.0, last_vel: None, attitude: Quat::IDENTITY, leg_pose: vec![], done: false, out: vec![],
        })
    }

    fn aiming_arm(&self) -> Option<usize> {
        if !self.valid { None } else if self.targets[1] != Vec3::ZERO { Some(1) } else if self.targets[0] != Vec3::ZERO { Some(0) } else { None }
    }
}

impl Extra for ShootDodge {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.done {
            return;
        }
        self.t += dt;
        let f = c.b.frames;
        if self.state == SD_ENTER || self.leg_pose.is_empty() {
            self.attitude = f[0].rotation;
            self.leg_pose = Character::mask("lb").into_iter().map(|j| (j, c.ch.actual(j, f))).collect();
            let look = self.aiming_arm().map_or(pos(c, 12) + facing(c) * 5.0, |i| self.targets[i]);
            self.out.push(format!("headLook start=true pos={},{},{} stiffness={} damping={}", look.x, look.y, look.z, self.head.0, self.head.1));
            self.out.push("#say shootDodge hold".into());
            self.state = SD_HOLD;
        }
        c.ch.set_stiffness(self.trunk.0, self.trunk.1, "us", None);
        c.ch.set_stiffness(self.head.0, self.head.1, "un", None);
        let (ls, ld, lt) = self.legs;
        for (k, (j, a)) in self.leg_pose.iter().enumerate() {
            let down = [HIP, KNEE, ANKLE].iter().position(|set| set.contains(j)).unwrap_or(0) as i32;
            c.ch.eff[*j].set_stiffness(ls * lt.powi(down), ld, None);
            let drift = self.noise * (self.t * 1.7 + k as f32 * 2.1).sin();
            c.ch.eff[*j].desired = [a[0] + drift, a[1], a[2]];
        }
        let arm = self.aiming_arm();
        for i in 0..2 {
            if Some(i) == arm {
                aim(c, i, self.targets[i], self.aiming.0, self.aiming.1);
                c.ch.eff[WRIST[i]].muscle_stiffness = self.wrist_ms;
            } else {
                let (s, d, taper) = self.loose;
                for (n, &j) in [CLAVICLE[i], SHOULDER[i], ELBOW[i], WRIST[i]].iter().enumerate() {
                    c.ch.eff[j].set_stiffness(s * taper.powi(n.saturating_sub(1) as i32), d, None);
                }
            }
        }
        for (k, share) in [(0, self.ori.2), (10, 1.0 - self.ori.2)] {
            let err = short_turn(self.attitude * f[0].rotation.inverse());
            let t = (err * self.ori.0 - c.b.part(0).w * self.ori.1) * (c.b.doll.part_mass(k) * 0.1 * share);
            c.out.torque(k, t);
        }
        self.breathe.tick(c, dt);
        let touching = c.b.doll.collided(&(0..21).collect::<Vec<_>>());
        self.touching = if touching { self.touching + dt } else { 0.0 };
        let (accel, lost) = self.last_vel.map_or((0.0, 0.0), |v: Vec3| ((c.vel - v).length() / dt, v.length() - c.vel.length()));
        self.last_vel = Some(c.vel);
        if touching && (accel > self.ends.1 || lost > self.ends.2 || self.touching > self.ends.0) {
            self.out.push("headLook start=false".into());
            match self.reaction {
                0 => self.out.push("catchFall start=true".into()),
                1 => self.out.push("bodyRollUp start=true".into()),
                2 => self.out.push("bodyBalance start=true".into()),
                _ => {}
            }
            self.out.push("#say shootDodge end".into());
            self.done = true;
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        self.done
    }
}

pub struct ArmHang {
    anchor: Vec3,
    target: Vec3,
    inst: (i32, i32),
    frame: Option<(Vec3, Quat)>,
    local: Option<[Vec3; 2]>,
    start: Vec<Vec3>,
    across: Vec3,
    t: f32,
    cons: Vec<(usize, Option<Vec3>, Option<Quat>)>,
    out: Vec<String>,
    free: [bool; 2],
}

const HANG_ON: f32 = 0.5;

impl ArmHang {
    fn new(p: &mut P) -> Result<ArmHang, Error> {
        let (anchor, target) = (p.v("constraintPosition", Vec3::ZERO)?, p.v("targetPosition", Vec3::ZERO)?);
        let inst = (p.i("instanceIndex", -1)?, p.i("boundIndex", 0)?);
        Ok(ArmHang { anchor, target, inst, frame: None, local: None, start: vec![], across: Vec3::X, t: 0.0, cons: vec![], out: vec![], free: [false; 2] })
    }
}

impl Extra for ArmHang {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.start.is_empty() {
            self.start = HANDS.iter().map(|&k| pos(c, k)).collect();
            self.across = Vec3::Z.cross(facing(c));
            if self.anchor == Vec3::ZERO {
                self.anchor = (self.start[0] + self.start[1]) * 0.5;
            }
            let to = v3(if self.target == Vec3::ZERO { self.anchor } else { self.target });
            self.out.extend(["bodyBalance start=false".into(), format!("pointArm start=true useLeftArm=false target={to}"),
                             format!("headLook start=true pos={to} stiffness=10 damping=1"), format!("spineTwist start=true pos={to}"),
                             "pedalLegs start=true".into()]);
        }
        self.t += dt;
        let on = (self.t / HANG_ON).min(1.0);
        let mut pins = [0, 1].map(|i| self.anchor + self.across * (0.15 - 0.3 * i as f32));
        if let Some((at, q)) = self.frame {
            let local = *self.local.get_or_insert_with(|| pins.map(|p| q.inverse() * (p - at)));
            pins = local.map(|l| at + q * l);
            (self.anchor, self.across) = ((pins[0] + pins[1]) * 0.5, (pins[0] - pins[1]).normalize_or(self.across));
        }
        self.cons = (0..2).filter(|&i| !self.free[i]).map(|i| (HANDS[i], Some(self.start[i].lerp(pins[i], on)), None)).collect();
        for i in (0..2).filter(|&i| !self.free[i]) {
            reach(c, i, self.start[i].lerp(pins[i], on), 8.0, 0.8);
        }
        c.ch.set_stiffness(3.0, 0.3, "us", None);
    }

    fn constraints(&self) -> Vec<(usize, Option<Vec3>, Option<Quat>)> {
        self.cons.clone()
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn let_go(&mut self, hand: usize) {
        self.free[hand] = true;
    }

    fn done(&self) -> bool {
        self.free == [true; 2]
    }

    fn stop(&mut self, _c: &mut Ctx) {
        self.out.extend(["pointArm", "headLook", "spineTwist", "pedalLegs"].map(|n| format!("{n} start=false")));
    }

    fn objects(&mut self, obs: &[crate::ragdoll::Obstacle]) {
        if let Some(o) = crate::ragdoll::Obstacle::find(obs, self.inst.0) {
            self.frame = Some(match self.inst.1 {
                k @ 1.. => { let (at, m) = o.component(k as usize); (at, Quat::from_mat3(&m)) }
                _ => (o.at, o.rot),
            });
        }
    }
}

pub struct CrushReaction {
    mask: Vec<usize>,
    s: (f32, f32),
    mode: i32,
    id: i32,
    ob: Option<crate::ragdoll::Obstacle>,
    use_hurt: bool,
    obstacle: Vec3,
    started: bool,
    fell: bool,
    out: Vec<String>,
}

impl CrushReaction {
    fn new(p: &mut P) -> Result<CrushReaction, Error> {
        let mask = Character::mask(&p.s("effectorMask", "ub")?);
        let s = (p.f("stiffness", 12.0)?, p.f("damping", 1.0)?);
        let (id, use_hurt) = (p.i("obstacleID", -1)?, p.b("useInjuries", false)?);
        Ok(CrushReaction { mask, s, mode: p.i("flinchMode", 0)?, id, ob: None, use_hurt, obstacle: p.v("obstaclePosition", Vec3::ZERO)?,
                           started: false, fell: false, out: vec![] })
    }
}

impl Extra for CrushReaction {
    fn tick(&mut self, c: &mut Ctx, _dt: f32) {
        let head = pos(c, NECK[1] + 1);
        if let Some(o) = self.ob {
            let local = o.rot.inverse() * (pos(c, CHEST) - o.centre);
            self.obstacle = o.centre + o.rot * local.clamp(-o.half, o.half);
        }
        if !self.started {
            self.started = true;
            if self.obstacle == Vec3::ZERO {
                self.obstacle = head + Vec3::Z;
            }
            let look = if self.mode == 1 { self.obstacle } else { pos(c, CHEST) + facing(c) - Vec3::Z * 1.5 };
            self.out.extend(["bodyBalance start=true".into(),
                             format!("upperBodyFlinch start=true useLeftArm=false useRightArm=false pos={}", v3(self.obstacle)),
                             format!("headLook start=true pos={} stiffness={} damping={}", v3(look), self.s.0, self.s.1)]);
        }
        for &j in &self.mask {
            c.ch.eff[j].set_stiffness(self.s.0, self.s.1, None);
        }
        let across = Vec3::Z.cross(facing(c));
        for i in 0..2 {
            let arm = [CLAVICLE[i], SHOULDER[i], ELBOW[i], WRIST[i]];
            if (self.use_hurt && arm.iter().any(|&j| c.ch.eff[j].injury > 0.0)) || !arm.iter().all(|j| self.mask.contains(j)) {
                continue;
            }
            let at = if self.mode == 1 {
                let shoulder = pos(c, crate::nmshot::limb(i).p[0]);
                let gap = self.obstacle - shoulder;
                shoulder + gap.normalize_or(Vec3::Z) * gap.length().min(0.6)
            } else {
                head + across * (0.12 - 0.24 * i as f32) + Vec3::Z * 0.1 + facing(c) * 0.06
            };
            reach(c, i, at, self.s.0, self.s.1);
        }
        if c.dbm_failed && !self.fell {
            self.fell = true;
            self.out.push("catchFall start=true".into());
        }
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn stop(&mut self, _c: &mut Ctx) {
        self.out.extend(["upperBodyFlinch", "headLook", "bodyBalance"].map(|n| format!("{n} start=false")));
        if self.fell {
            self.out.push("catchFall start=false".into());
        }
    }

    fn objects(&mut self, obs: &[crate::ragdoll::Obstacle]) {
        self.ob = crate::ragdoll::Obstacle::find(obs, self.id).copied();
    }
}

fn short_turn(q: Quat) -> Vec3 {
    (if q.w < 0.0 { -q } else { q }).to_scaled_axis()
}

