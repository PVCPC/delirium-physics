//! Behaviours from Red Dead Redemption 2's Euphoria, written from descriptions of how they behave (see the RDR2
//! Euphoria notes); none of RDR2's code, so what each does is Delirium's reading of it, not RDR2's own. Most parameter
//! names are RDR2's; some are Delirium's own, where the behaviour needs a setting RDR2's table doesn't show (rdr2Sit's
//! seat and stirrups, wallCollision's wall, keyframe's hold, the injured limbs' looseness and the like). A default is
//! RDR2's own value (as RDR2's descriptor table gives it) wherever the parameter means the same here: times in seconds,
//! distances, shares 0..1, bends and rates. Where RDR2's value is on a scale of its own (its forces, torques and
//! "amount" multipliers: shotFromBehind's and shotInGuts' amounts, shotSpin's turn and torque, shotInjuredArm's hipYaw,
//! reachForWound's armStiffness, wallCollision's snapMag), the default is Delirium's, in the units its doc gives.
//! rdr2Bolas and setFallingReaction's hands-and-knees settings are Delirium's own throughout. Any of them can be sent
//! as parameters. They are extra messages: nothing here runs unless one is sent with start=true, so every existing
//! behaviour is unchanged.
use crate::extras::{Extra, P};
use crate::msg::{Error, Kind, Message, Param};
use crate::nmdbm::LeanMode;
use crate::nmrs::{HIP, KNEE, NECK, SPINE};
use crate::ragdoll::GRAVITY;
use crate::nmshot::Ctx;
use bevy_math::Vec3;

pub static MESSAGES: [Message; 27] = [
    Message { name: "rdr2Sit", params: &[
        Param { name: "seatPos", kind: Kind::Vec3, used: true, doc: "where his pelvis sits, world; send rdr2Sit again without start to move it with the horse (zero: where his pelvis is)" },
        Param { name: "seatUp", kind: Kind::Vec3, used: true, doc: "the seat's up, world (0,0,1). RDR2 gives the seat as a part of its horse with seatuplocal and seatrightlocal in that part's frame" },
        Param { name: "seatForward", kind: Kind::Vec3, used: true, doc: "which way the horse faces, world (zero: his facing)" },
        Param { name: "stirrupL", kind: Kind::Vec3, used: true, doc: "where his left foot goes, world (zero: 0.55 m under the seat, 0.26 m to the side)" },
        Param { name: "stirrupR", kind: Kind::Vec3, used: true, doc: "his right foot (zero: likewise)" },
        Param { name: "mountTime", kind: Kind::Float, used: true, doc: "s he takes to get from where he is into the seat, swung up over it (0.6)" },
        Param { name: "constrain", kind: Kind::Bool, used: true, doc: "hold his pelvis in the seat (true; false: only his pose, he sits on whatever's under him)" },
        Param { name: "constrainstrength", kind: Kind::Float, used: true, doc: "how hard his hips are held square to the seat (40)" },
        Param { name: "constrainstrengthstirrup", kind: Kind::Float, used: true, doc: "how hard his feet are held in the stirrups: a spring of 10 times this, N per m (40; 0: they aren't)" },
        Param { name: "hippitchstart", kind: Kind::Float, used: true, doc: "his hips tip back (less than 0) or forward, -1..1 of half a radian (-0.5)" },
        Param { name: "hippitchramp", kind: Kind::Float, used: true, doc: "s they take to get there (0.25)" },
        Param { name: "straightnessstart", kind: Kind::Float, used: true, doc: "his back slouches (less than 0) or arches, -1..1 (-0.1)" },
        Param { name: "straightnessramp", kind: Kind::Float, used: true, doc: "s it takes (0.35)" },
        Param { name: "failangle", kind: Kind::Float, used: true, doc: "he's thrown once his body leans this far from the seat's up, a cosine (0.71)" },
        Param { name: "seatfailangle", kind: Kind::Float, used: true, doc: "or the seat tips this far from the sky's up (0.71)" },
        Param { name: "softweight", kind: Kind::Float, used: true, doc: "how hard his chest is held upright in the seat, as RDR2 holds riders to their animation, 0..1 (0.8; slumped: none)" },
        Param { name: "Slump", kind: Kind::Bool, used: true, doc: "he sits slumped, his back and arms loose, the reins let go; leaning doesn't throw him (false)" },
        Param { name: "drag", kind: Kind::Bool, used: true, doc: "thrown, a foot stays caught in its stirrup and he's dragged by it (false)" },
        Param { name: "dragTime", kind: Kind::Float, used: true, doc: "s until the foot comes free (10)" },
        Param { name: "dragreach", kind: Kind::Bool, used: true, doc: "dragged, his arms reach back toward the caught foot (true)" },
        Param { name: "dragreachdelay", kind: Kind::Float, used: true, doc: "s after he's thrown that he starts reaching (0.2)" },
        Param { name: "dragstiffness", kind: Kind::Float, used: true, doc: "his stiffness while dragged (9)" },
        Param { name: "knockSpeed", kind: Kind::Float, used: true, doc: "a hit that moves his chest this fast against his hips knocks him off, m/s (4; 0: none)" },
        Param { name: "throwSpeed", kind: Kind::Float, used: true, doc: "thrown, how hard he's flung off to the side he leans to, and up, m/s (3)" },
        Param { name: "throw", kind: Kind::Bool, used: true, doc: "sent without start: throw him now, as if he'd failed (false)" },
        Param { name: "posefailangle", kind: Kind::Float, used: false, doc: "RDR2's (0.71)" },
        Param { name: "standfailangle", kind: Kind::Float, used: false, doc: "RDR2's (0.71)" },
        Param { name: "rolldelay", kind: Kind::Float, used: false, doc: "RDR2's (0.3)" },
        Param { name: "rollspeedthreshold", kind: Kind::Float, used: false, doc: "RDR2's (2)" },
        Param { name: "healthfaildelay", kind: Kind::Float, used: false, doc: "RDR2's (0)" },
    ] },
    Message { name: "keyframe", params: &[
        Param { name: "blendTime", kind: Kind::Float, used: true, doc: "s to blend into the neutral stand (0.3)" },
        Param { name: "holdTime", kind: Kind::Float, used: true, doc: "s it then holds it (0.5)" },
        Param { name: "settle", kind: Kind::Float, used: true, doc: "how hard the upper body's motion is damped meanwhile, 1/s (3)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "spine and neck stiffness; the arms 0.8 of it (11)" },
    ] },
    Message { name: "rdr2Balance", params: &[
        Param { name: "runningThresh", kind: Kind::Float, used: true, doc: "his speed along the ground above which he balances as running, m/s (0.65)" },
        Param { name: "legStiffnessRun", kind: Kind::Float, used: true, doc: "hips and knees at least this stiff when running (13; 0: off)" },
        Param { name: "startFlat", kind: Kind::Float, used: true, doc: "a foot this little above the other (m) is turned fully flat (0.1)" },
        Param { name: "endFlat", kind: Kind::Float, used: true, doc: "from this high above the other (m) it isn't (0.3)" },
        Param { name: "flatStiffness", kind: Kind::Float, used: true, doc: "the turn's strength, N m per rad (12; 0: off)" },
    ] },
    Message { name: "setFallingReaction", params: &[
        Param { name: "handsAndKnees", kind: Kind::Bool, used: true, doc: "end a forward fall on all fours (true; false: nothing happens)" },
        Param { name: "hkHoldTime", kind: Kind::Float, used: true, doc: "s he stays on hands and knees (1.5)" },
        Param { name: "stoppedStrengthDecay", kind: Kind::Float, used: true, doc: "then his strength fades by this per s (1)" },
        Param { name: "minArmStiffness", kind: Kind::Float, used: true, doc: "arms' stiffness on all fours (14)" },
        Param { name: "minLegStiffness", kind: Kind::Float, used: true, doc: "legs' stiffness (12)" },
        Param { name: "minSpineStiffness", kind: Kind::Float, used: true, doc: "spine and neck stiffness (9)" },
        Param { name: "hkHeadAvoid", kind: Kind::Bool, used: true, doc: "keep the head up off the ground (true)" },
        Param { name: "pushForward", kind: Kind::Float, used: true, doc: "a push on the chest tipping him forwards first, N (0)" },
    ] },
    Message { name: "wallCollision", params: &[
        Param { name: "objectBehindVictimPos", kind: Kind::Vec3, used: true, doc: "a point on the wall he ran into, world" },
        Param { name: "objectBehindVictimNormal", kind: Kind::Vec3, used: true, doc: "the wall's normal, out of it towards him (zero: nothing happens)" },
        Param { name: "solidWall", kind: Kind::Bool, used: true, doc: "make the wall solid for his body (true; false when the world already has it)" },
        Param { name: "wallStiffness", kind: Kind::Float, used: true, doc: "that wall's spring, 1/s^2 (400)" },
        Param { name: "wallHalfWidth", kind: Kind::Float, used: true, doc: "the wall reaches this far either side of the point, m (0: on and on)" },
        Param { name: "slumpMode", kind: Kind::Int, used: true, doc: "0: slump when his back meets it, 1: always slump, 2: never (0)" },
        Param { name: "braceWall", kind: Kind::Bool, used: true, doc: "facing it: both hands on the wall (true); false: just a rebound" },
        Param { name: "braceDistance", kind: Kind::Float, used: true, doc: "brace when his chest is within this of it, m (2)" },
        Param { name: "braceArmStiffness", kind: Kind::Float, used: true, doc: "the bracing arms' stiffness (12)" },
        Param { name: "reactTime", kind: Kind::Float, used: true, doc: "s the impact weakens his legs (0.3)" },
        Param { name: "impactLegStiffReduction", kind: Kind::Float, used: true, doc: "leg stiffness lost at the impact (3)" },
        Param { name: "snapMag", kind: Kind::Float, used: true, doc: "the snap away from the wall at the impact, as shotSnap's (0.3)" },
        Param { name: "sitStartTime", kind: Kind::Float, used: true, doc: "slumping: s before he lets go of balance (0.3)" },
        Param { name: "sitRate", kind: Kind::Float, used: true, doc: "slumping: how fast his legs give, per s (1)" },
        Param { name: "glanceSpinMag", kind: Kind::Float, used: true, doc: "side on: the spin away from the wall, N m (50)" },
        Param { name: "glanceSpinTime", kind: Kind::Float, used: true, doc: "side on: s that spin decays over (0.5)" },
        Param { name: "timeToForward", kind: Kind::Float, used: true, doc: "s the hands hold the wall before he pushes off it (0.5)" },
        Param { name: "reboundForce", kind: Kind::Float, used: true, doc: "the push off the wall, N (0: none)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "his stiffness (10)" },
    ] },
    Message { name: "reachForWound", params: &[
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "the wound, world (zero: in front of the part)" },
        Param { name: "hitPart", kind: Kind::Int, used: true, doc: "the part it is on, 0..20 (-1: the belly)" },
        Param { name: "timeBeforeReachForWound", kind: Kind::Float, used: true, doc: "s before reaching (0.2)" },
        Param { name: "grabHoldTime", kind: Kind::Float, used: true, doc: "s the hand holds the wound (2)" },
        Param { name: "reachWithOneHand", kind: Kind::Bool, used: true, doc: "one hand for a trunk wound (true); false: both" },
        Param { name: "allowInjuredThighReach", kind: Kind::Bool, used: true, doc: "reach for a thigh, bending down to it (true)" },
        Param { name: "allowInjuredLowerLegReach", kind: Kind::Bool, used: true, doc: "reach for a shin or foot, bending further (false)" },
        Param { name: "armStiffness", kind: Kind::Float, used: true, doc: "the reaching arm's stiffness (10)" },
        Param { name: "bend2spine", kind: Kind::Float, used: true, doc: "forward bend of the spine for a leg wound, rad (0.7)" },
        Param { name: "bend2hip", kind: Kind::Float, used: true, doc: "hip bend for a leg wound, rad (0.6)" },
        Param { name: "bend2leg", kind: Kind::Float, used: true, doc: "knee bend for a leg wound, rad (0.6)" },
        Param { name: "spineTwistMag", kind: Kind::Float, used: true, doc: "twist of the spine round to a trunk wound, rad (0.2)" },
        Param { name: "bendTimeUp", kind: Kind::Float, used: true, doc: "s the bend comes in over (0.4)" },
        Param { name: "bendTimeDown", kind: Kind::Float, used: true, doc: "s it goes over after (0.5)" },
        Param { name: "spineStiffness", kind: Kind::Float, used: true, doc: "trunk stiffness while bending (12)" },
    ] },
    Message { name: "shotFromBehind", params: &[
        Param { name: "sfbSpineAmount", kind: Kind::Float, used: true, doc: "how far the spine arches back, rad over the whole spine (0.5)" },
        Param { name: "sfbNeckAmount", kind: Kind::Float, used: true, doc: "how far the neck goes back, rad (0.3)" },
        Param { name: "sfbHipAmount", kind: Kind::Float, used: true, doc: "how far the hips push forward, rad (0.25)" },
        Param { name: "sfbKneeAmount", kind: Kind::Float, used: true, doc: "how far the knees give, rad (0.15)" },
        Param { name: "sfbPeriod", kind: Kind::Float, used: true, doc: "s the arch swells and goes over (0.7)" },
        Param { name: "sfbArmsOnset", kind: Kind::Float, used: true, doc: "s before the arms go loose (0)" },
        Param { name: "sfbKneesOnset", kind: Kind::Float, used: true, doc: "s before the knees give (0)" },
        Param { name: "sfbNoiseGain", kind: Kind::Float, used: true, doc: "tremble in the arch, 0..2 (0)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "trunk stiffness while arching (10)" },
    ] },
    Message { name: "shotInGuts", params: &[
        Param { name: "sigSpineAmount", kind: Kind::Float, used: true, doc: "how far the spine hunches forward, rad over the whole spine (0.6)" },
        Param { name: "sigNeckAmount", kind: Kind::Float, used: true, doc: "how far the neck bends forward, rad (0.25)" },
        Param { name: "sigKneeAmount", kind: Kind::Float, used: true, doc: "how far the knees bend, rad; the hips flex by half (0.15)" },
        Param { name: "sigPeriod", kind: Kind::Float, used: true, doc: "s he stays folded (1.5)" },
        Param { name: "sigBlend", kind: Kind::Float, used: true, doc: "s it eases off over after (0.5)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "trunk stiffness (10)" },
    ] },
    Message { name: "shotInjuredLeg", params: &[
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part hit: 1..3 left leg, 4..6 right (-1: leftLeg says)" },
        Param { name: "leftLeg", kind: Kind::Bool, used: true, doc: "the left leg, when bodyPart doesn't say (true)" },
        Param { name: "legLiftTime", kind: Kind::Float, used: true, doc: "s the leg jerks up for (0: none)" },
        Param { name: "legInjuryLift", kind: Kind::Float, used: true, doc: "how far it jerks up: hip flex, rad; the knee bends twice that (0)" },
        Param { name: "legInjuryTime", kind: Kind::Float, used: true, doc: "s the leg stays weak (0.4)" },
        Param { name: "legInjury", kind: Kind::Float, used: true, doc: "share of the leg's strength lost, 0..1 (0.3)" },
        Param { name: "legInjuryHipPitch", kind: Kind::Float, used: true, doc: "hip flex on the weak leg, rad (0)" },
        Param { name: "legInjurySpineBend", kind: Kind::Float, used: true, doc: "forward bend of the spine meanwhile, rad (0.1)" },
        Param { name: "legLimpBend", kind: Kind::Float, used: true, doc: "knee bend on the weak leg, rad (0)" },
        Param { name: "legLean", kind: Kind::Float, used: true, doc: "push sagging him towards the weak leg, N (15)" },
        Param { name: "legSag", kind: Kind::Float, used: true, doc: "how far the balancer sinks him meanwhile, m (0.04)" },
        Param { name: "legBlend", kind: Kind::Float, used: true, doc: "s it fades over after (0.5)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "the leg's stiffness when well (10)" },
    ] },
    Message { name: "shotInjuredArm", params: &[
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part hit: 13..16 left arm, 17..20 right (-1: leftArm says)" },
        Param { name: "leftArm", kind: Kind::Bool, used: true, doc: "the left arm, when bodyPart doesn't say (true)" },
        Param { name: "bulletDir", kind: Kind::Vec3, used: true, doc: "the bullet's direction (zero: from in front)" },
        Param { name: "injuredArmTime", kind: Kind::Float, used: true, doc: "s the shoulder is knocked round for (0.25)" },
        Param { name: "hipYaw", kind: Kind::Float, used: true, doc: "torque turning the trunk with the bullet, N m (80)" },
        Param { name: "hipRoll", kind: Kind::Float, used: true, doc: "torque dropping the hit shoulder, N m (0: none)" },
        Param { name: "armLoose", kind: Kind::Float, used: true, doc: "share of the arm's stiffness lost, 0..1 (0.7)" },
        Param { name: "armLooseTime", kind: Kind::Float, used: true, doc: "s the arm hangs loose (1)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "the arm's stiffness when well (10)" },
    ] },
    Message { name: "meleeReact", params: &[
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "where the blow landed, world (zero)" },
        Param { name: "hitNormal", kind: Kind::Vec3, used: true, doc: "the body's surface normal there, outwards; the blow pushes against it (zero: no push or snap)" },
        Param { name: "hitPart", kind: Kind::Int, used: true, doc: "the part hit, 0..20 (-1: none)" },
        Param { name: "csLooseness", kind: Kind::Float, used: true, doc: "how loose the upper body goes, 0..1 (0.4)" },
        Param { name: "csPeriod", kind: Kind::Float, used: true, doc: "s it stays loose (0.3)" },
        Param { name: "Lean", kind: Kind::Float, used: true, doc: "push leaning him away from the blow, N (0: none)" },
        Param { name: "leanPeriod", kind: Kind::Float, used: true, doc: "s of that push (1)" },
        Param { name: "Flinch", kind: Kind::Bool, used: true, doc: "flinch: a small forward hunch (true)" },
        Param { name: "flinchPeriod", kind: Kind::Float, used: true, doc: "s of the flinch (0.7)" },
        Param { name: "armsType", kind: Kind::Int, used: true, doc: "0: arms as they are, 1: hands up to guard the face while flinching (1)" },
        Param { name: "snap", kind: Kind::Bool, used: true, doc: "snap the hit part's joints along the blow (true)" },
        Param { name: "snapMag", kind: Kind::Float, used: true, doc: "that snap's size, as shotSnap's (1)" },
        Param { name: "rfw", kind: Kind::Bool, used: true, doc: "reach for the spot hit afterwards (true)" },
        Param { name: "rfwDelay", kind: Kind::Float, used: true, doc: "s before reaching (0.2)" },
        Param { name: "rfwPeriod", kind: Kind::Float, used: true, doc: "s of reaching (1)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "upper body stiffness (10)" },
    ] },
    Message { name: "shotFallToKnees", params: &[
        Param { name: "ftkBalanceTime", kind: Kind::Float, used: true, doc: "s he keeps balancing before sinking (0.1)" },
        Param { name: "ftkBendRate", kind: Kind::Float, used: true, doc: "how fast knees and hips bend, rad/s (3)" },
        Param { name: "ftkHelperForce", kind: Kind::Float, used: true, doc: "forward push on the pelvis while sinking, N (0)" },
        Param { name: "ftkSpineBend", kind: Kind::Float, used: true, doc: "forward bend of the whole spine while sinking, rad (0.1)" },
        Param { name: "ftkStiffness", kind: Kind::Float, used: true, doc: "legs and spine stiffness (9)" },
        Param { name: "ftkKneelTime", kind: Kind::Float, used: true, doc: "s he stays on his knees (2)" },
        Param { name: "ftkKneelHip", kind: Kind::Float, used: true, doc: "hip bend while kneeling, rad; more leans him forwards (0.3)" },
        Param { name: "ftkLeanHelp", kind: Kind::Float, used: true, doc: "pull of his centre over his knees while kneeling, N per m, damped (1500)" },
        Param { name: "ftkOnKneesArmType", kind: Kind::Int, used: true, doc: "0: arms as they are, 1: arms hang (1)" },
        Param { name: "ftkPitchForwards", kind: Kind::Bool, used: true, doc: "topple forwards (true) or back (false) at the end" },
        Param { name: "ftkPitchForce", kind: Kind::Float, used: true, doc: "push on the chest that tips him over, N (300)" },
    ] },
    Message { name: "shotSpin", params: &[
        Param { name: "spin", kind: Kind::Float, used: true, doc: "the hit's spin, m (none: from hitPoint and bulletDir: the lever arm about his centre across the bullet)" },
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "where the bullet hit, world (zero)" },
        Param { name: "bulletDir", kind: Kind::Vec3, used: true, doc: "the bullet's direction (zero)" },
        Param { name: "shotSpinMin", kind: Kind::Float, used: true, doc: "spins smaller than this (m) do nothing (0.02)" },
        Param { name: "shotSpinTime", kind: Kind::Float, used: true, doc: "s the heading swings round over, with the yaw torque on (0.3)" },
        Param { name: "shotSpinSettle", kind: Kind::Float, used: true, doc: "s it then holds the new heading; the feet stay planted, so he eases part way back after (0.3)" },
        Param { name: "shotSpinTwistMult", kind: Kind::Float, used: true, doc: "rad of turn per m of spin (10)" },
        Param { name: "shotSpinTwist", kind: Kind::Float, used: true, doc: "most turn, rad; stops once he has turned this far (1.5)" },
        Param { name: "shotSpinTwistVel", kind: Kind::Float, used: true, doc: "stops while he already turns faster than this, rad/s (6)" },
        Param { name: "shotSpinTorque", kind: Kind::Float, used: true, doc: "yaw torque on pelvis and trunk per rad off the heading, N m (150)" },
        Param { name: "shotSpinTorqueMax", kind: Kind::Float, used: true, doc: "most yaw torque, N m (300)" },
        Param { name: "stepTurnType", kind: Kind::Int, used: true, doc: "after the spin: 0 nothing, 1 step round to face attackerPos (0)" },
        Param { name: "stepTurnWait", kind: Kind::Float, used: true, doc: "s after the spin before the step turn (0.3)" },
        Param { name: "stepTurnTime", kind: Kind::Float, used: true, doc: "s the step turn may take (2)" },
        Param { name: "attackerPos", kind: Kind::Vec3, used: true, doc: "where the shooter is, world (zero)" },
    ] },
    Message { name: "duck", params: &[
        Param { name: "magMin", kind: Kind::Float, used: true, doc: "least share of each joint's remaining range the duck bends into, 0..1 (0.2)" },
        Param { name: "magMax", kind: Kind::Float, used: true, doc: "most share (1); the share is random between the two" },
        Param { name: "lsRate", kind: Kind::Float, used: true, doc: "lower spine's bend rate, rad/s (0.4)" },
        Param { name: "usRate", kind: Kind::Float, used: true, doc: "upper spine's bend rate, rad/s (0.4)" },
        Param { name: "headRate", kind: Kind::Float, used: true, doc: "neck's bend rate, rad/s (0.4)" },
        Param { name: "spineTwistRate", kind: Kind::Float, used: true, doc: "spine's side-lean and twist rate, rad/s (0.5)" },
        Param { name: "neckTwistRate", kind: Kind::Float, used: true, doc: "neck's side-lean and twist rate, rad/s (0.5)" },
        Param { name: "timeBeforeDuck", kind: Kind::Float, used: true, doc: "s before the duck starts (0)" },
        Param { name: "duckTime", kind: Kind::Float, used: true, doc: "s spent bending in; then it unbends (0.5)" },
        Param { name: "blendTime", kind: Kind::Float, used: true, doc: "s over which the duck fades back out after duckTime (0.5)" },
        Param { name: "leanTwistStart", kind: Kind::Bool, used: true, doc: "also lean sideways and twist away (true)" },
        Param { name: "bendForward", kind: Kind::Bool, used: true, doc: "bend forwards (true) or back (false)" },
        Param { name: "duckLeft", kind: Kind::Bool, used: true, doc: "lean and twist to the left (false: right)" },
        Param { name: "crouch", kind: Kind::Float, used: true, doc: "how much the balancer bends the legs while ducking (0.05; 0: no crouch)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "spine and neck stiffness while ducking (10)" },
    ] },
    Message { name: "shotSnap", params: &[
        Param { name: "snapMag", kind: Kind::Float, used: true, doc: "the kick's size; negative kicks the other way (0.4)" },
        Param { name: "snapDelay", kind: Kind::Float, used: true, doc: "s before the kick (0)" },
        Param { name: "snapDuration", kind: Kind::Float, used: true, doc: "s the kick is spread over; 0: all at once (0)" },
        Param { name: "unSnapInterval", kind: Kind::Float, used: true, doc: "s after the kick before the counter-kick (0.01)" },
        Param { name: "unSnapRatio", kind: Kind::Float, used: true, doc: "the counter-kick's size against the kick's, back the other way; 0: none (0.7)" },
        Param { name: "snapDirectionRandomness", kind: Kind::Float, used: true, doc: "0: about the axis across the bullet, 1: a random axis (0.3)" },
        Param { name: "snapMovingMult", kind: Kind::Float, used: true, doc: "kick times this when moving on his feet (1)" },
        Param { name: "snapBalancingMult", kind: Kind::Float, used: true, doc: "when standing still on his feet (1)" },
        Param { name: "snapAirborneMult", kind: Kind::Float, used: true, doc: "when nothing touches the ground (1)" },
        Param { name: "snapMovingThresh", kind: Kind::Float, used: true, doc: "m/s above which he is moving (1)" },
        Param { name: "snapUseBulletDir", kind: Kind::Bool, used: true, doc: "kick about the axis across the bullet (true); false: about his own side axis" },
        Param { name: "bulletDir", kind: Kind::Vec3, used: true, doc: "the bullet's direction (zero: coming from in front)" },
        Param { name: "snapHitPart", kind: Kind::Bool, used: true, doc: "also kick the joints either side of the part hit (true)" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part hit, 0..20 (-1: none)" },
        Param { name: "snapHipType", kind: Kind::Int, used: true, doc: "legs: 0 no kick, 1 about the kick's axis, 2 about the bullet's direction (0)" },
        Param { name: "snapLeftLeg", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapRightLeg", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapLeftArm", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapRightArm", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapSpine", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "snapNeck", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "snapPhasedLegs", kind: Kind::Bool, used: true, doc: "the right leg kicks the other way (true)" },
    ] },
    Message { name: "rdr2Lasso", params: &[
        Param { name: "lassoedtype", kind: Kind::Int, used: true, doc: "where the rope is: 0 round his neck (his hands claw at it), 1 round his chest and arms (arms pinned, he wriggles), 2 round his feet (legs held together, his arms reach out past his head clawing at the ground) (0)" },
        Param { name: "grabPart", kind: Kind::Int, used: true, doc: "round the neck: the part his hands claw at (11, the neck; -1: they don't)" },
        Param { name: "grabposl", kind: Kind::Vec3, used: true, doc: "where the left hand claws, from that part: forward, left, up (0.07,0.05,-0.051)" },
        Param { name: "grabposr", kind: Kind::Vec3, used: true, doc: "the right hand (0.09,-0.05,-0.051)" },
        Param { name: "grabpointamp", kind: Kind::Vec3, used: true, doc: "how far the hands claw about it, m (zero: they hold)" },
        Param { name: "grabpointfreq", kind: Kind::Vec3, used: true, doc: "how fast, Hz per axis (4,5,3)" },
        Param { name: "legAmplitude", kind: Kind::Float, used: true, doc: "down, how hard the legs kick (0.4)" },
        Param { name: "spinefreq", kind: Kind::Float, used: true, doc: "down, the spine writhes forwards and back this often, Hz (2.5)" },
        Param { name: "spineAmplitude", kind: Kind::Float, used: true, doc: "that much (1)" },
        Param { name: "spsidefreq", kind: Kind::Float, used: true, doc: "and side to side this often, Hz (2)" },
        Param { name: "spsideamp", kind: Kind::Float, used: true, doc: "that much (1)" },
        Param { name: "groundLegStiffness", kind: Kind::Float, used: true, doc: "down (11)" },
        Param { name: "groundArmStiffness", kind: Kind::Float, used: true, doc: "down (11)" },
        Param { name: "groundSpineStiffness", kind: Kind::Float, used: true, doc: "down (11)" },
        Param { name: "groundneckstiffness", kind: Kind::Float, used: true, doc: "down (11)" },
        Param { name: "groundLegDamping", kind: Kind::Float, used: true, doc: "down (0.5)" },
        Param { name: "groundArmDamping", kind: Kind::Float, used: true, doc: "down (0.5)" },
        Param { name: "groundSpineDamping", kind: Kind::Float, used: true, doc: "down (0.5)" },
        Param { name: "bumpsim", kind: Kind::Float, used: true, doc: "down, a bump off the ground now and then, N for a step (400; 0: none)" },
        Param { name: "bumpsimprob", kind: Kind::Float, used: true, doc: "chance of a bump each tenth of a second (0.08)" },
        Param { name: "stunnedtime", kind: Kind::Float, used: true, doc: "s he lies stunned, limp, once down (0)" },
        Param { name: "stunnedblendout", kind: Kind::Float, used: true, doc: "s coming out of it (0)" },
        Param { name: "groundFriction", kind: Kind::Float, used: false, doc: "the ground's grip on him (1)" },
        Param { name: "ropeendfriction", kind: Kind::Float, used: false, doc: "the roped part's grip (0.5)" },
        Param { name: "otherendfriction", kind: Kind::Float, used: false, doc: "the other end's grip (2)" },
    ] },
    Message { name: "rdr2Bolas", params: &[
        Param { name: "bindStrength", kind: Kind::Float, used: true, doc: "how hard the bolas holds his ankles together, N per m (900)" },
        Param { name: "fallTime", kind: Kind::Float, used: true, doc: "s before his balance gives (0.3)" },
        Param { name: "struggleTime", kind: Kind::Float, used: true, doc: "s he struggles on the ground before he's free (5; 0: until stopped)" },
        Param { name: "struggle", kind: Kind::Float, used: true, doc: "how hard he struggles, 0..1 (0.7)" },
    ] },
    Message { name: "leanRandom", params: LEAN_RANDOM },
    Message { name: "hipsLeanInDirection", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "how far the hips go; less than 0 away (0.5)" },
        Param { name: "dir", kind: Kind::Vec3, used: true, doc: "which way, world (zero: none)" },
    ] },
    Message { name: "hipsLeanToPosition", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "how far the hips go; less than 0 away (0.2)" },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "towards this point, world" },
    ] },
    Message { name: "hipsLeanTowardsObject", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "how far the hips go; less than 0 away (0.5)" },
        Param { name: "offset", kind: Kind::Vec3, used: true, doc: "towards this point, world (as leanTowardsObject)" },
        Param { name: "instanceIndex", kind: Kind::Int, used: false, doc: "the object (-1)" },
        Param { name: "boundIndex", kind: Kind::Int, used: false, doc: "its part (0)" },
    ] },
    Message { name: "hipsLeanRandom", params: HIPS_LEAN_RANDOM },
    Message { name: "forceLeanInDirection", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "the push, times his weight; less than 0 away (0.2)" },
        Param { name: "dir", kind: Kind::Vec3, used: true, doc: "which way, world (zero: none)" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part pushed, 0..20 (0, the pelvis)" },
    ] },
    Message { name: "forceLeanToPosition", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "the push, times his weight; less than 0 away (0.2)" },
        Param { name: "pos", kind: Kind::Vec3, used: true, doc: "towards this point, world" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part pushed, 0..20 (0, the pelvis)" },
    ] },
    Message { name: "forceLeanTowardsObject", params: &[
        Param { name: "leanAmount", kind: Kind::Float, used: true, doc: "the push, times his weight; less than 0 away (0.2)" },
        Param { name: "offset", kind: Kind::Vec3, used: true, doc: "towards this point, world (as leanTowardsObject)" },
        Param { name: "instanceIndex", kind: Kind::Int, used: false, doc: "the object (-1)" },
        Param { name: "boundIndex", kind: Kind::Int, used: false, doc: "its part (0)" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part pushed, 0..20 (0, the pelvis)" },
    ] },
    Message { name: "forceLeanRandom", params: FORCE_LEAN_RANDOM },
    Message { name: "onFire", params: &[
        Param { name: "staggerTime", kind: Kind::Float, used: true, doc: "s he staggers on his feet before he goes down (2.5)" },
        Param { name: "staggerLeanRate", kind: Kind::Float, used: true, doc: "how fast his stagger's lean swings, 0..1 (0.9)" },
        Param { name: "stumbleMaxLeanBack", kind: Kind::Float, used: true, doc: "most he leans back while staggering (0.4)" },
        Param { name: "stumbleMaxLeanForward", kind: Kind::Float, used: true, doc: "most he leans forward while staggering (0.5)" },
        Param { name: "armsWindmillWritheBlend", kind: Kind::Float, used: true, doc: "on his feet, arms: 0 windmill, 1 writhe (0.4)" },
        Param { name: "spineStumbleWritheBlend", kind: Kind::Float, used: true, doc: "on his feet, spine: 0 held, 1 writhes (0.7)" },
        Param { name: "legsStumbleWritheBlend", kind: Kind::Float, used: true, doc: "on his feet, legs: 0 the balancer's, 1 writhe (0.2)" },
        Param { name: "armsPoseWritheBlend", kind: Kind::Float, used: true, doc: "down, arms: 0 curled to his face, 1 writhe (0.7)" },
        Param { name: "spinePoseWritheBlend", kind: Kind::Float, used: true, doc: "down, spine: 0 curled, 1 writhes (0.55)" },
        Param { name: "legsPoseWritheBlend", kind: Kind::Float, used: true, doc: "down, legs: 0 drawn up, 1 writhe (0.5)" },
        Param { name: "rollTorqueScale", kind: Kind::Float, used: true, doc: "down, how hard he rolls to put the flames out (25)" },
        Param { name: "predictTime", kind: Kind::Float, used: true, doc: "s ahead his stagger leans against where he's heading (0.1)" },
        Param { name: "rollOverFlag", kind: Kind::Bool, used: true, doc: "down, he rolls back and forth (true)" },
        Param { name: "maxRollOverTime", kind: Kind::Float, used: true, doc: "s he keeps rolling (8)" },
        Param { name: "rollOverRadius", kind: Kind::Float, used: true, doc: "m from where he fell he may roll (2)" },
    ] },
];

const LEAN_RANDOM: &[Param] = &[
    Param { name: "leanAmountMin", kind: Kind::Float, used: true, doc: "least lean (0.2)" },
    Param { name: "leanAmountMax", kind: Kind::Float, used: true, doc: "most lean (0.2)" },
    Param { name: "changeTimeMin", kind: Kind::Float, used: true, doc: "s before a new way, at least (0.5)" },
    Param { name: "changeTimeMax", kind: Kind::Float, used: true, doc: "at most (1)" },
];

const HIPS_LEAN_RANDOM: &[Param] = &[
    Param { name: "leanAmountMin", kind: Kind::Float, used: true, doc: "least lean of the hips (0.3)" },
    Param { name: "leanAmountMax", kind: Kind::Float, used: true, doc: "most lean of the hips (0.4)" },
    Param { name: "changeTimeMin", kind: Kind::Float, used: true, doc: "s before a new way, at least (2)" },
    Param { name: "changeTimeMax", kind: Kind::Float, used: true, doc: "at most (4)" },
];

const FORCE_LEAN_RANDOM: &[Param] = &[
    Param { name: "leanAmountMin", kind: Kind::Float, used: true, doc: "least push, times his weight (0.3)" },
    Param { name: "leanAmountMax", kind: Kind::Float, used: true, doc: "most push, times his weight (0.4)" },
    Param { name: "changeTimeMin", kind: Kind::Float, used: true, doc: "s before a new way, at least (2)" },
    Param { name: "changeTimeMax", kind: Kind::Float, used: true, doc: "at most (4)" },
    Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part pushed, 0..20 (0, the pelvis)" },
];

pub fn build(name: &str, p: &mut P) -> Option<Result<Box<dyn Extra>, Error>> {
    match name {
        "duck" => Some(Duck::new(p).map(|e| Box::new(e) as _)),
        "shotSnap" => Some(Snap::new(p).map(|e| Box::new(e) as _)),
        "shotSpin" => Some(Spin::new(p).map(|e| Box::new(e) as _)),
        "shotFallToKnees" => Some(Knees::new(p).map(|e| Box::new(e) as _)),
        "meleeReact" => Some(Melee::new(p).map(|e| Box::new(e) as _)),
        "shotFromBehind" => Some(FromBehind::new(p).map(|e| Box::new(e) as _)),
        "shotInGuts" => Some(InGuts::new(p).map(|e| Box::new(e) as _)),
        "shotInjuredLeg" => Some(InjuredLeg::new(p).map(|e| Box::new(e) as _)),
        "shotInjuredArm" => Some(InjuredArm::new(p).map(|e| Box::new(e) as _)),
        "reachForWound" => Some(ReachForWound::new(p).map(|e| Box::new(e) as _)),
        "wallCollision" => Some(WallCollision::new(p).map(|e| Box::new(e) as _)),
        "setFallingReaction" => Some(FallingReaction::new(p).map(|e| Box::new(e) as _)),
        "rdr2Balance" => Some(BalanceRdr2::new(p).map(|e| Box::new(e) as _)),
        "keyframe" => Some(Keyframe::new(p).map(|e| Box::new(e) as _)),
        "onFire" => Some(OnFire::new(p).map(|e| Box::new(e) as _)),
        "rdr2Lasso" => Some(Lasso::new(p).map(|e| Box::new(e) as _)),
        "rdr2Bolas" => Some(Bolas::new(p).map(|e| Box::new(e) as _)),
        "rdr2Sit" => Some(Sit::new(p).map(|e| Box::new(e) as _)),
        "leanRandom" | "hipsLeanInDirection" | "hipsLeanToPosition" | "hipsLeanTowardsObject" | "hipsLeanRandom" | "forceLeanInDirection"
        | "forceLeanToPosition" | "forceLeanTowardsObject" | "forceLeanRandom" => Some(Lean::new(name, p).map(|e| Box::new(e) as _)),
        _ => None,
    }
}

/// One rate-limited channel: a joint axis easing from where it was towards its duck target and back.
struct Channel { joint: usize, axis: usize, rate: f32, start: f32, goal: f32, x: f32 }

/// RDR2's DUCK: a short, fast flinch away from danger. After timeBeforeDuck the spine and neck bend into a random
/// share of what's left of each joint's range (forwards or back, and optionally sideways and twisting away), each at
/// its own rate, for duckTime; the legs crouch a little through the balancer; then it all fades out over blendTime.
pub struct Duck {
    mag: [f32; 2], rates: [f32; 5], wait: f32, time: f32, blend: f32, twist: bool, forward: bool, left: bool,
    crouch: f32, stiffness: f32, t: f32, channels: Vec<Channel>,
}

impl Duck {
    fn new(p: &mut P) -> Result<Duck, Error> {
        Ok(Duck {
            mag: [p.f("magMin", 0.2)?.clamp(0.0, 1.0), p.f("magMax", 1.0)?.clamp(0.0, 1.0)],
            rates: [p.f("lsRate", 0.4)?, p.f("usRate", 0.4)?, p.f("headRate", 0.4)?, p.f("spineTwistRate", 0.5)?, p.f("neckTwistRate", 0.5)?],
            wait: p.f("timeBeforeDuck", 0.0)?.max(0.0), time: p.f("duckTime", 0.5)?.max(0.0), blend: p.f("blendTime", 0.5)?.max(0.01),
            twist: p.b("leanTwistStart", true)?, forward: p.b("bendForward", true)?, left: p.b("duckLeft", false)?,
            crouch: p.f("crouch", 0.05)?.max(0.0), stiffness: p.f("stiffness", 10.0)?,
            t: 0.0, channels: vec![],
        })
    }

    /// Picks each channel's goal from the joints' current angles and limits.
    fn aim(&mut self, c: &mut Ctx) {
        let mag = self.mag[0] + (self.mag[1] - self.mag[0]) * c.ch.random();
        let (s, n) = (*SPINE(), *NECK());
        // (joint, axis, rate, direction): axis 0 lean forwards/back, 1 lean sideways, 2 twist
        // a spine or neck joint's lean1 bends forwards as it rises, so forwards is towards its upper limit
        let bend = if self.forward { 1.0 } else { -1.0 };
        let side = if self.left { -1.0 } else { 1.0 };
        let mut want: Vec<(usize, usize, f32, f32)> = vec![
            (s[0], 0, self.rates[0], bend), (s[1], 0, self.rates[0], bend),
            (s[2], 0, self.rates[1], bend), (s[3], 0, self.rates[1], bend),
            (n[0], 0, self.rates[2], bend), (n[1], 0, self.rates[2], bend),
        ];
        if self.twist {
            for &j in &s {
                want.extend([(j, 1, self.rates[3], side), (j, 2, self.rates[3], side)]);
            }
            for &j in &n {
                want.extend([(j, 1, self.rates[4], side), (j, 2, self.rates[4], side)]);
            }
        }
        self.channels = want.into_iter().map(|(joint, axis, rate, dir)| {
            let now = c.ch.actual(joint, c.b.frames)[axis];
            let l = c.ch.limits(joint);
            let room = if dir > 0.0 { (l[2 * axis + 1] - now).max(0.0) } else { (l[2 * axis] - now).min(0.0) };
            Channel { joint, axis, rate, start: now, goal: room * mag, x: 0.0 }
        }).collect();
    }
}

impl Extra for Duck {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let t = self.t - self.wait;
        if t < 0.0 {
            return;
        }
        if self.channels.is_empty() {
            self.aim(c);
        }
        let bending = t <= self.time;
        // weight: full while bending, fading to nothing over blendTime after
        let w = if bending { 1.0 } else { (1.0 - (t - self.time) / self.blend).clamp(0.0, 1.0) };
        for ch in &mut self.channels {
            let target = if bending { ch.goal } else { 0.0 };
            let step = ch.rate * dt;
            ch.x += (target - ch.x).clamp(-step, step);
            let e = &mut c.ch.eff[ch.joint];
            if ch.axis == 0 && w > 0.0 {
                e.set_stiffness(self.stiffness, 1.0, None);
            }
            let d = &mut e.desired[ch.axis];
            *d += (ch.start + ch.x - *d) * w;
        }
        if self.crouch > 0.0 {
            c.out.dbm_leg_offset = Some(-self.crouch * w);
        }
    }

    fn done(&self) -> bool {
        self.t > self.wait + self.time + self.blend
    }

    fn stop(&mut self, c: &mut Ctx) {
        if self.crouch > 0.0 {
            c.out.dbm_leg_offset = Some(0.0);
        }
    }
}

/// Each Delirium part's parent part (joint j joins part j + 1 to this).
const PARENT: [usize; 21] = [0, 0, 1, 2, 0, 4, 5, 0, 7, 8, 9, 10, 11, 10, 13, 14, 15, 10, 17, 18, 19];
/// Delirium's own scale: a kick of size 1 turns a part by about this much angular velocity per kg of the part.
const KICK: f32 = 0.3;

/// RDR2's shot snap: a sharp kick across chosen joints at the hit, then a smaller counter-kick back. The kick turns
/// each joint about one axis (across the bullet's direction, or his side axis), sized by his state (moving,
/// standing, airborne) and by joint (spine hardest, neck lightest), spread over snapDuration (0: a single step).
pub struct Snap {
    mag: f32, delay: f32, duration: f32, interval: f32, ratio: f32, random: f32, mult: [f32; 3], moving: f32,
    use_dir: bool, dir: Vec3, part: i32, hip: i32, limbs: [bool; 6], phased: bool,
    t: f32, axis: Option<Vec3>, kicked: f32, countered: f32,
}

impl Snap {
    fn new(p: &mut P) -> Result<Snap, Error> {
        let mag = p.f("snapMag", 0.4)?;
        let (delay, duration) = (p.f("snapDelay", 0.0)?.max(0.0), p.f("snapDuration", 0.0)?.max(0.0));
        let (interval, ratio) = (p.f("unSnapInterval", 0.01)?.max(0.0), p.f("unSnapRatio", 0.7)?);
        let random = p.f("snapDirectionRandomness", 0.3)?.clamp(0.0, 1.0);
        let mult = [p.f("snapMovingMult", 1.0)?, p.f("snapBalancingMult", 1.0)?, p.f("snapAirborneMult", 1.0)?];
        let moving = p.f("snapMovingThresh", 1.0)?;
        let (use_dir, dir) = (p.b("snapUseBulletDir", true)?, p.v("bulletDir", Vec3::ZERO)?);
        let hit = p.b("snapHitPart", true)?;
        let part = p.i("bodyPart", -1)?;
        let hip = p.i("snapHipType", 0)?;
        let limbs = [p.b("snapLeftLeg", false)?, p.b("snapRightLeg", false)?, p.b("snapLeftArm", false)?,
                     p.b("snapRightArm", false)?, p.b("snapSpine", true)?, p.b("snapNeck", true)?];
        Ok(Snap { mag, delay, duration, interval, ratio, random, mult, moving, use_dir, dir, part: if hit { part } else { -1 },
                  hip, limbs, phased: p.b("snapPhasedLegs", true)?, t: 0.0, axis: None, kicked: 0.0, countered: 0.0 })
    }

    /// The share of a kick (spread over the duration) that falls in this step; all of it when the duration is 0.
    fn share(&self, since: f32, dt: f32, done: f32) -> f32 {
        if self.duration <= 0.0 {
            return 1.0 - done;
        }
        (((since + dt).min(self.duration) - since.max(0.0)) / self.duration).clamp(0.0, 1.0 - done)
    }

    /// Kicks joint `j` (the one driving part j + 1) about `axis` by `amount`: equal and opposite on its two parts.
    fn kick(c: &mut Ctx, j: usize, axis: Vec3, amount: f32, dt: f32) {
        let (child, parent) = (j + 1, PARENT[j + 1]);
        let t = axis * (amount * KICK * c.b.doll.part_mass(child) / dt);
        c.out.torque(child, t);
        c.out.torque(parent, -t);
    }

    fn apply(&mut self, c: &mut Ctx, m: f32, dt: f32) {
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        // left shoulder minus right, crossed this way round, points the way he faces
        let forward = side.cross(up).normalize_or(Vec3::Y);
        let dir = if self.dir == Vec3::ZERO { -forward } else { self.dir.normalize_or(-forward) };
        let (use_dir, random) = (self.use_dir, self.random);
        let r = Vec3::new(c.ch.random(), c.ch.random(), c.ch.random()) * 2.0 - 1.0;
        let axis = *self.axis.get_or_insert_with(|| {
            let a = if use_dir { up.cross(dir).normalize_or(side) } else { side };
            a.lerp(r.normalize_or(a), random).normalize_or(a)
        });
        // his state: nothing touching (airborne), only the feet (moving or standing), more than the feet (as it is)
        let feet = c.b.doll.collided(&[3, 6]);
        let other = c.b.doll.collided(&[0, 1, 2, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
        let horizontal = (c.vel - up * c.vel.dot(up)).length();
        let m = if !feet && !other {
            m * self.mult[2]
        } else if !other {
            m * self.mult[if horizontal > self.moving { 0 } else { 1 }]
        } else {
            m
        };
        let (s, n) = (*SPINE(), *NECK());
        if (0..21).contains(&self.part) {
            let k = self.part as usize;
            for j in [k.checked_sub(1), (k < 20).then_some(k)].into_iter().flatten() {
                Self::kick(c, j, axis, m * 15.0, dt);
            }
        }
        if self.limbs[4] {
            for (i, &j) in s.iter().enumerate() {
                Self::kick(c, j, axis, m * if i == 3 { 8.0 } else { 15.0 }, dt);
            }
        }
        if self.limbs[5] {
            for &j in &n {
                Self::kick(c, j, axis, m, dt);
            }
        }
        let mix = (up + dir).normalize_or(up);
        for (on, sign, clavicle, shoulder) in [(self.limbs[2], 1.0, 12, 13), (self.limbs[3], -1.0, 16, 17)] {
            if on {
                Self::kick(c, shoulder, mix, m * 4.0 * sign, dt);
                Self::kick(c, clavicle, up, m * 4.0 * sign, dt);
            }
        }
        if self.hip > 0 {
            let leg_axis = if self.hip >= 2 { dir } else { axis };
            for (on, hip, sign) in [(self.limbs[0], 0, 1.0), (self.limbs[1], 3, if self.phased { -1.0 } else { 1.0 })] {
                if on {
                    Self::kick(c, hip, leg_axis, m * 15.0 * sign, dt);
                }
            }
        }
    }
}

impl Extra for Snap {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let since = self.t - self.delay;
        self.t += dt;
        if since + dt <= 0.0 {
            return;
        }
        let k = self.share(since, dt, self.kicked);
        if k > 0.0 {
            self.kicked += k;
            self.apply(c, self.mag * k, dt);
        }
        let back = since - self.duration - self.interval;
        if self.ratio != 0.0 && back + dt > 0.0 {
            let k = self.share(back, dt, self.countered);
            if k > 0.0 {
                self.countered += k;
                self.apply(c, -self.ratio * self.mag * k, dt);
            }
        }
    }

    fn done(&self) -> bool {
        self.kicked >= 0.999 && (self.ratio == 0.0 || self.countered >= 0.999)
    }
}

/// RDR2's shot spin and step turn. A hit off the body's centre spins him round: the balancer is turned towards a
/// heading that swings round by the hit's spin (its lever arm about his centre, across the bullet, times
/// shotSpinTwistMult, at most shotSpinTwist), and a damped yaw torque on the pelvis and trunk pulls him onto it and
/// holds him there while it settles; it stops early when he's already turning faster than shotSpinTwistVel. Then, with stepTurnType 1, after stepTurnWait he steps
/// round to face attackerPos.
pub struct Spin {
    spin: Option<f32>, hit: Vec3, dir: Vec3, min: f32, time: f32, settle: f32, mult: f32, max_twist: f32,
    max_vel: f32, torque: f32, torque_max: f32, step_type: i32, step_wait: f32, step_time: f32, attacker: Vec3,
    t: f32, heading: Option<(Vec3, f32)>, turned: f32,
}

impl Spin {
    fn new(p: &mut P) -> Result<Spin, Error> {
        let spin = p.f("spin", f32::NAN)?;
        Ok(Spin {
            spin: (!spin.is_nan()).then_some(spin), hit: p.v("hitPoint", Vec3::ZERO)?, dir: p.v("bulletDir", Vec3::ZERO)?,
            min: p.f("shotSpinMin", 0.02)?.max(0.0), time: p.f("shotSpinTime", 0.3)?.max(0.0), settle: p.f("shotSpinSettle", 0.3)?.max(0.0),
            mult: p.f("shotSpinTwistMult", 10.0)?, max_twist: p.f("shotSpinTwist", 1.5)?.abs(), max_vel: p.f("shotSpinTwistVel", 6.0)?.abs(),
            torque: p.f("shotSpinTorque", 150.0)?, torque_max: p.f("shotSpinTorqueMax", 300.0)?.abs(),
            step_type: p.i("stepTurnType", 0)?, step_wait: p.f("stepTurnWait", 0.3)?.max(0.0), step_time: p.f("stepTurnTime", 2.0)?.max(0.0),
            attacker: p.v("attackerPos", Vec3::ZERO)?, t: 0.0, heading: None, turned: 0.0,
        })
    }

    fn facing(c: &Ctx, up: Vec3) -> Vec3 {
        let side = c.b.part(13).m.d - c.b.part(17).m.d;
        let f = side.cross(up);
        (f - up * f.dot(up)).normalize_or(Vec3::Y)
    }
}

impl Extra for Spin {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let yaw_rate = c.angvel.dot(up);
        self.turned += yaw_rate * dt;
        let (spin, hit, dir, mult, max_twist) = (self.spin, self.hit, self.dir, self.mult, self.max_twist);
        let (start, total) = *self.heading.get_or_insert_with(|| {
            // the spin: the hit's lever arm about his centre, across the bullet, about up
            let s = spin.unwrap_or_else(|| (hit - c.com).cross(dir.normalize_or_zero()).dot(up));
            (Self::facing(c, up), (s * mult).clamp(-max_twist, max_twist))
        });
        let spinning = self.t <= self.time + self.settle;
        if spinning && total.abs() > self.min * mult && self.turned.abs() <= max_twist + 0.5 && yaw_rate.abs() < self.max_vel {
            // the heading swings round to start + total over shotSpinTime, then holds while it settles: a yaw torque
            // on pelvis and trunk towards it (shotSpinTorque per rad off), damped by his turn rate so the planted
            // feet don't spring him back, and the balancer told to face it
            let a = total * (self.t / self.time.max(dt)).min(1.0);
            let heading = bevy_math::Quat::from_axis_angle(up, a) * start;
            let now = Self::facing(c, up);
            let off = now.cross(heading).dot(up).atan2(now.dot(heading));
            let tq = (self.torque * off - self.torque * 0.3 * yaw_rate).clamp(-self.torque_max, self.torque_max);
            for k in [0, 7, 8] {
                c.out.torque(k, up * tq / 3.0);
            }
            c.out.dbm_turn = Some(heading * 0.5);
            return;
        }
        let step_from = self.time + self.settle + self.step_wait;
        if self.step_type > 0 && self.t > step_from && self.t <= step_from + self.step_time {
            let to = self.attacker - c.com;
            let to = (to - up * to.dot(up)).normalize_or_zero();
            if to != Vec3::ZERO && Self::facing(c, up).dot(to) < 0.98 {
                c.out.dbm_turn = Some(to * 0.5);
                return;
            }
        }
        c.out.dbm_turn = Some(Vec3::ZERO);
    }

    fn done(&self) -> bool {
        let end = self.time + self.settle + if self.step_type > 0 { self.step_wait + self.step_time } else { 0.0 };
        self.t > end
    }

    fn stop(&mut self, c: &mut Ctx) {
        c.out.dbm_turn = Some(Vec3::ZERO);
    }
}

/// RDR2's fall to knees (a shot module there): after balancing for ftkBalanceTime the balancer lets go and he sinks:
/// knees bend at ftkBendRate, hips flex, the spine bends forward by ftkSpineBend and a helper push on the pelvis
/// tips him forwards over his feet. When both shins touch the ground he is kneeling: thighs come upright under him and
/// a lean help keeps his centre over his knees for ftkKneelTime; then he topples forwards (or back) and goes limp.
pub struct Knees {
    balance: f32, bend_rate: f32, helper: f32, spine_bend: f32, stiffness: f32, kneel_time: f32, lean_help: f32,
    forwards: bool, pitch: f32, arms_down: bool, kneel_hip: f32,
    t: f32, phase: u8, since: f32, knee: [f32; 2], hip: [f32; 2],
}

impl Knees {
    fn new(p: &mut P) -> Result<Knees, Error> {
        Ok(Knees {
            balance: p.f("ftkBalanceTime", 0.1)?.max(0.0), bend_rate: p.f("ftkBendRate", 3.0)?.abs(),
            helper: p.f("ftkHelperForce", 0.0)?, spine_bend: p.f("ftkSpineBend", 0.1)?, stiffness: p.f("ftkStiffness", 9.0)?,
            kneel_time: p.f("ftkKneelTime", 2.0)?.max(0.0), lean_help: p.f("ftkLeanHelp", 1500.0)?,
            forwards: p.b("ftkPitchForwards", true)?, pitch: p.f("ftkPitchForce", 300.0)?, arms_down: p.i("ftkOnKneesArmType", 1)? == 1, kneel_hip: p.f("ftkKneelHip", 0.3)?,
            t: 0.0, phase: 0, since: 0.0, knee: [0.0; 2], hip: [0.0; 2],
        })
    }

    fn forward(c: &Ctx, up: Vec3) -> Vec3 {
        let f = (c.b.part(13).m.d - c.b.part(17).m.d).cross(up);
        (f - up * f.dot(up)).normalize_or(Vec3::Y)
    }

    /// Moves joint j's axis-k target towards `to` by at most `step`, from where it is now the first time.
    fn ease(c: &mut Ctx, j: usize, k: usize, to: f32, step: f32) {
        let now = c.ch.eff[j].desired[k];
        c.ch.eff[j].desired[k] = now + (to - now).clamp(-step, step);
    }
}

impl Extra for Knees {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        self.since += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let fwd = Self::forward(c, up);
        let (knees, hips, spine) = (*KNEE(), *HIP(), *SPINE());
        match self.phase {
            0 => {
                if self.t >= self.balance {
                    // let go of the balancer; start from the legs' current angles
                    c.out.dbm = Some(false);
                    for s in 0..2 {
                        self.knee[s] = c.ch.actual(knees[s], c.b.frames)[0];
                        self.hip[s] = c.ch.actual(hips[s], c.b.frames)[0];
                        c.ch.eff[knees[s]].desired[0] = self.knee[s];
                        c.ch.eff[hips[s]].desired[0] = self.hip[s];
                    }
                    (self.phase, self.since) = (1, 0.0);
                }
            }
            1 => {
                // sinking: knees towards their deepest bend, hips flexing, spine forwards, a push over the feet
                let step = self.bend_rate * dt;
                c.ch.set_stiffness(self.stiffness, 1.0, "lb", None);
                c.ch.set_stiffness(self.stiffness, 1.0, "us", None);
                for s in 0..2 {
                    let kl = c.ch.limits(knees[s]);
                    let deep = if kl[0].abs() > kl[1].abs() { kl[0] } else { kl[1] } * 0.9;
                    Self::ease(c, knees[s], 0, deep, step);
                    let hl = c.ch.limits(hips[s]);
                    Self::ease(c, hips[s], 0, (hl[1] * 0.2).min(0.4), step);
                }
                for &j in &spine {
                    Self::ease(c, j, 0, self.spine_bend / 4.0, step);
                }
                c.out.force(0, fwd * self.helper);
                // on his knees once they are near the ground (measured from his feet; shin contacts are too patchy)
                let ground = c.b.part(3).m.d.dot(up).min(c.b.part(6).m.d.dot(up)) - 0.05;
                let knee = |t: usize, s: usize| (c.b.part(t).m.d + c.b.part(s).m.d).dot(up) * 0.5 - ground;
                let down = knee(1, 2) < 0.2 && knee(4, 5) < 0.2;
                // as the knees come down, start holding his centre over them so he doesn't pitch on
                if knee(1, 2).max(knee(4, 5)) < 0.35 {
                    let knees_at = (c.b.part(1).m.d + c.b.part(2).m.d + c.b.part(4).m.d + c.b.part(5).m.d) * 0.25;
                    let off = knees_at - c.com;
                    let vel = c.vel - up * c.vel.dot(up);
                    let pull = (off - up * off.dot(up)) * self.lean_help - vel * self.lean_help * 0.2;
                    c.out.force(0, pull * 0.5);
                    c.out.force(10, pull * 0.5);
                }
                if down || self.since > 1.5 {
                    (self.phase, self.since) = (2, 0.0);
                }
            }
            2 => {
                // kneeling: thighs upright under him, spine straight, his centre held over the knees
                let step = self.bend_rate * dt;
                c.ch.set_stiffness(self.stiffness, 1.0, "lb", None);
                c.ch.set_stiffness(self.stiffness, 1.0, "us", None);
                // hips held a little flexed and stiffer, so the trunk stays up over the knees
                for s in 0..2 {
                    Self::ease(c, hips[s], 0, self.kneel_hip, step);
                    c.ch.eff[hips[s]].set_stiffness(self.stiffness * 2.5, 1.0, None);
                }
                for &j in &spine {
                    Self::ease(c, j, 0, 0.0, step);
                    c.ch.eff[j].set_stiffness(self.stiffness * 1.5, 1.0, None);
                }
                if self.arms_down {
                    c.ch.set_stiffness(self.stiffness * 0.5, 1.0, "ua", None);
                }
                // the knees: between each thigh and shin; hold his centre over them
                let knees_at = (c.b.part(1).m.d + c.b.part(2).m.d + c.b.part(4).m.d + c.b.part(5).m.d) * 0.25;
                let off = knees_at - c.com;
                let vel = c.vel - up * c.vel.dot(up);
                let pull = (off - up * off.dot(up)) * self.lean_help - vel * self.lean_help * 0.2;
                c.out.force(0, pull * 0.5);
                c.out.force(10, pull * 0.5);
                if self.since > self.kneel_time {
                    (self.phase, self.since) = (3, 0.0);
                }
            }
            _ => {
                // topple: go loose and tip forwards (or back)
                c.ch.set_stiffness(self.stiffness * 0.3, 1.0, "fb", None);
                if self.since < 0.3 {
                    c.out.force(10, if self.forwards { fwd } else { -fwd } * self.pitch);
                }
            }
        }
    }

    fn done(&self) -> bool {
        self.phase == 3 && self.since > 1.0
    }
}

impl Snap {
    /// A snap at a hit part along `dir`, with Delirium's other defaults (used by meleeReact).
    fn at(mag: f32, part: i32, dir: Vec3) -> Snap {
        Snap { mag, delay: 0.0, duration: 0.0, interval: 0.05, ratio: 0.5, random: 0.25, mult: [1.0; 3], moving: 1.0,
               use_dir: true, dir, part, hip: 0, limbs: [false, false, false, false, true, true], phased: true,
               t: 0.0, axis: None, kicked: 0.0, countered: 0.0 }
    }
}

/// RDR2's melee reaction, built like its shot from smaller pieces: the upper body goes loose for csPeriod (by
/// csLooseness), a push leans him away from the blow for leanPeriod, he flinches (a small forward hunch, and with
/// armsType 1 both hands come up in front of his face) for flinchPeriod, the hit part's joints snap along the blow,
/// and after rfwDelay one hand reaches for the spot hit for rfwPeriod.
pub struct Melee {
    point: Vec3, push: Vec3, part: i32, loose: f32, loose_time: f32, lean: f32, lean_time: f32, flinch: bool,
    flinch_time: f32, guard: bool, rfw: bool, rfw_delay: f32, rfw_time: f32, stiffness: f32,
    snap: Option<Snap>, t: f32, offset: Option<Vec3>,
}

impl Melee {
    fn new(p: &mut P) -> Result<Melee, Error> {
        let point = p.v("hitPoint", Vec3::ZERO)?;
        let normal = p.v("hitNormal", Vec3::ZERO)?;
        let part = p.i("hitPart", -1)?;
        let (snap, snap_mag) = (p.b("snap", true)?, p.f("snapMag", 1.0)?);
        let push = -normal.normalize_or_zero();
        Ok(Melee {
            point, push, part, loose: p.f("csLooseness", 0.4)?.clamp(0.0, 1.0), loose_time: p.f("csPeriod", 0.3)?.max(0.0),
            lean: p.f("Lean", 0.0)?, lean_time: p.f("leanPeriod", 1.0)?.max(0.0),
            flinch: p.b("Flinch", true)?, flinch_time: p.f("flinchPeriod", 0.7)?.max(0.0), guard: p.i("armsType", 1)? == 1,
            rfw: p.b("rfw", true)?, rfw_delay: p.f("rfwDelay", 0.2)?.max(0.0), rfw_time: p.f("rfwPeriod", 1.0)?.max(0.0),
            stiffness: p.f("stiffness", 10.0)?,
            snap: (snap && push != Vec3::ZERO).then(|| Snap::at(snap_mag, part, push)), t: 0.0, offset: None,
        })
    }
}

impl Extra for Melee {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        let fwd = side.cross(up).normalize_or(Vec3::Y);
        // the spot hit, carried along with the part it is on
        let part = (0..21).contains(&self.part).then_some(self.part as usize);
        let (point, part_pos) = (self.point, part.map(|k| c.b.part(k).m.d));
        let offset = *self.offset.get_or_insert_with(|| part_pos.map_or(Vec3::ZERO, |q| point - q));
        let wound = part_pos.map(|q| q + offset);
        if let Some(s) = self.snap.as_mut() {
            if !s.done() {
                s.tick(c, dt);
            }
        }
        if self.t < self.loose_time {
            c.ch.set_stiffness(self.stiffness * (1.0 - self.loose), 1.0, "ub", None);
        }
        if self.t < self.lean_time && self.push != Vec3::ZERO {
            let push = (self.push - up * self.push.dot(up)).normalize_or_zero() * self.lean;
            c.out.force(10, push * 0.7);
            c.out.force(0, push * 0.3);
        }
        let flinching = self.flinch && self.t < self.flinch_time;
        if flinching {
            for &j in SPINE() {
                let d = &mut c.ch.eff[j].desired[0];
                *d += (0.08 - *d) * 0.2;
            }
        }
        for i in 0..2 {
            let l = crate::nmshot::limb(i);
            let to = if flinching && self.guard {
                // guard: hands up in front of the face, a little either side
                let head = c.b.part(12).m.d;
                Some(head + fwd * 0.28 - up * 0.08 + side * if i == 0 { 0.1 } else { -0.1 })
            } else if self.rfw && self.t > self.rfw_delay && self.t < self.rfw_delay + self.rfw_time {
                // reach for the wound: the hand on the far side for an arm hit, else the near one
                let k = part.unwrap_or(10);
                let hand = if (13..17).contains(&k) { 1 } else if (17..21).contains(&k) { 0 } else if (point - c.com).dot(side) > 0.0 { 0 } else { 1 };
                wound.filter(|_| hand == i && part.is_some())
            } else {
                None
            };
            if let Some(to) = to {
                let s = if flinching && self.guard { self.stiffness * 1.5 } else { self.stiffness };
                c.ch.set_stiffness(s, 1.0, ["ul", "ur"][i], None);
                crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], to, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
            }
        }
    }

    fn done(&self) -> bool {
        let end = self.loose_time.max(self.lean_time).max(self.flinch_time).max(if self.rfw { self.rfw_delay + self.rfw_time } else { 0.0 });
        self.t > end && self.snap.as_ref().is_none_or(Snap::done)
    }
}

/// A set of joint-axis bends laid over whatever the balancer, the shot or another behaviour wants, scaled by a
/// weight each tick: they go into the joints' offsets (added where the joints are driven, cleared every tick), so
/// they stack with anything else and never build up.
struct Bends { ch: Vec<(usize, usize, f32)> }

impl Bends {
    fn new(ch: Vec<(usize, usize, f32)>) -> Bends {
        Bends { ch }
    }

    fn apply(&mut self, c: &mut Ctx, w: f32, stiffness: f32) {
        for &(j, k, amt) in &self.ch {
            c.ch.eff[j].offset[k] += amt * w;
            if w > 0.0 && k == 0 && stiffness > 0.0 {
                c.ch.eff[j].set_stiffness(stiffness, 1.0, None);
            }
        }
    }
}

/// Which way a knee bends: towards whichever of its limits is further from straight.
fn knee_dir(c: &Ctx, j: usize) -> f32 {
    let l = c.ch.limits(j);
    if l[0].abs() > l[1].abs() { -1.0 } else { 1.0 }
}

/// Drives a joint axis to `x` of its range (-1 one limit, 0 the middle, 1 the other), through its offset.
fn drive(c: &mut Ctx, j: usize, k: usize, x: f32) {
    let l = c.ch.limits(j);
    let (lo, hi) = (l[2 * k], l[2 * k + 1]);
    let to = (hi - lo) * 0.5 * x.clamp(-1.0, 1.0) + (hi + lo) * 0.5;
    let e = &mut c.ch.eff[j];
    e.offset[k] += to - e.desired[k];
}

/// The side a part is on: 0 left (leg 1..=3, arm 13..=16), 1 right (leg 4..=6, arm 17..=20), None otherwise.
fn part_side(part: i32) -> Option<usize> {
    match part {
        1..=3 | 13..=16 => Some(0),
        4..=6 | 17..=20 => Some(1),
        _ => None,
    }
}

/// A quick rise, a short hold and a slower return: 0 before `onset`, 1 a quarter of the way through `period`,
/// held to halfway, back to 0 at its end.
fn swell(t: f32, onset: f32, period: f32) -> f32 {
    let x = (t - onset) / period.max(0.01);
    if !(0.0..1.0).contains(&x) { 0.0 } else if x < 0.25 { x / 0.25 } else if x < 0.5 { 1.0 } else { (1.0 - x) / 0.5 }
}

/// RDR2's shot from behind: a bullet in the back arches him. The spine bends back by sfbSpineAmount (spread over its
/// joints), the neck by sfbNeckAmount, the hips push forward (sfbHipAmount) and the knees give a little
/// (sfbKneeAmount, after sfbKneesOnset); after sfbArmsOnset the arms go loose so they fling with it. It swells over
/// sfbPeriod; sfbNoiseGain adds a tremble.
pub struct FromBehind {
    period: f32, arms_onset: f32, knees_onset: f32, noise: f32, stiffness: f32, amounts: [f32; 4],
    t: f32, trunk: Option<Bends>, knees: Option<Bends>,
}

impl FromBehind {
    fn new(p: &mut P) -> Result<FromBehind, Error> {
        Ok(FromBehind {
            amounts: [p.f("sfbSpineAmount", 0.5)?, p.f("sfbNeckAmount", 0.3)?, p.f("sfbHipAmount", 0.25)?, p.f("sfbKneeAmount", 0.15)?],
            period: p.f("sfbPeriod", 0.7)?.max(0.05), arms_onset: p.f("sfbArmsOnset", 0.0)?.max(0.0),
            knees_onset: p.f("sfbKneesOnset", 0.0)?.max(0.0), noise: p.f("sfbNoiseGain", 0.0)?.max(0.0),
            stiffness: p.f("stiffness", 10.0)?, t: 0.0, trunk: None, knees: None,
        })
    }
}

impl Extra for FromBehind {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let [spine, neck, hip, knee] = self.amounts;
        let trunk = self.trunk.get_or_insert_with(|| {
            let mut v: Vec<_> = SPINE().iter().map(|&j| (j, 0, -spine / 4.0)).collect();
            v.extend(NECK().iter().map(|&j| (j, 0, -neck / 2.0)));
            v.extend(HIP().iter().map(|&j| (j, 0, -hip)));
            Bends::new(v)
        });
        let w = swell(self.t, 0.0, self.period);
        let w = if self.noise > 0.0 && w > 0.0 { w * (1.0 + self.noise * 0.2 * (c.ch.random() - 0.5)) } else { w };
        trunk.apply(c, w, self.stiffness);
        let knees = self.knees.get_or_insert_with(|| Bends::new(KNEE().iter().map(|&j| (j, 0, knee * knee_dir(c, j))).collect()));
        knees.apply(c, swell(self.t, self.knees_onset, self.period), 0.0);
        if self.t > self.arms_onset && self.t < self.arms_onset + self.period * 0.5 {
            c.ch.set_stiffness(self.stiffness * 0.4, 1.0, "ua", None);
        }
    }

    fn done(&self) -> bool {
        self.t > self.period + self.knees_onset.max(self.arms_onset)
    }
}

/// RDR2's shot in the guts: he folds over the wound. The spine hunches forward by sigSpineAmount and the neck by
/// sigNeckAmount and the knees bend a little (sigKneeAmount). It comes in over a fifth of a second, holds for
/// sigPeriod and eases off over sigBlend. (The hands are reachForWound's, with reachWithOneHand false.)
pub struct InGuts {
    spine: f32, neck: f32, knee: f32, period: f32, blend: f32, stiffness: f32, t: f32, bends: Option<Bends>,
}

impl InGuts {
    fn new(p: &mut P) -> Result<InGuts, Error> {
        Ok(InGuts {
            spine: p.f("sigSpineAmount", 0.6)?, neck: p.f("sigNeckAmount", 0.25)?, knee: p.f("sigKneeAmount", 0.15)?,
            period: p.f("sigPeriod", 1.5)?.max(0.0), blend: p.f("sigBlend", 0.5)?.max(0.05), stiffness: p.f("stiffness", 10.0)?,
            t: 0.0, bends: None,
        })
    }

    fn weight(&self) -> f32 {
        if self.t < 0.2 { self.t / 0.2 } else if self.t < 0.2 + self.period { 1.0 } else { (1.0 - (self.t - 0.2 - self.period) / self.blend).max(0.0) }
    }
}

impl Extra for InGuts {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let w = self.weight();
        let (spine, neck, knee) = (self.spine, self.neck, self.knee);
        let bends = self.bends.get_or_insert_with(|| {
            let mut v: Vec<_> = SPINE().iter().map(|&j| (j, 0, spine / 4.0)).collect();
            v.extend(NECK().iter().map(|&j| (j, 0, neck / 2.0)));
            v.extend(KNEE().iter().map(|&j| (j, 0, knee * knee_dir(c, j))));
            v.extend(HIP().iter().map(|&j| (j, 0, knee * 0.5)));
            Bends::new(v)
        });
        bends.apply(c, w, self.stiffness);
    }

    fn done(&self) -> bool {
        self.t > 0.2 + self.period + self.blend
    }
}

/// RDR2's injured leg: a hit leg first jerks up (hip and knee flex by legInjuryLift over legLiftTime), then for
/// legInjuryTime it is weak (its strength down by legInjury, the knee bent by legLimpBend, the hip by
/// legInjuryHipPitch) while the spine bends forward (legInjurySpineBend) and he sinks (legSag) and sags towards it
/// (legLean); then it
/// fades over legBlend. The leg is bodyPart's (1..=6), or leftLeg's.
pub struct InjuredLeg {
    side: usize, lift_time: f32, lift: f32, time: f32, injury: f32, hip_pitch: f32, spine_bend: f32, limp: f32,
    lean: f32, sag: f32, blend: f32, stiffness: f32, t: f32, lift_b: Option<Bends>, limp_b: Option<Bends>,
}

impl InjuredLeg {
    fn new(p: &mut P) -> Result<InjuredLeg, Error> {
        let part = p.i("bodyPart", -1)?;
        let left = p.b("leftLeg", true)?;
        Ok(InjuredLeg {
            side: part_side(part).unwrap_or(if left { 0 } else { 1 }),
            lift_time: p.f("legLiftTime", 0.0)?.max(0.0), lift: p.f("legInjuryLift", 0.0)?,
            time: p.f("legInjuryTime", 0.4)?.max(0.0), injury: p.f("legInjury", 0.3)?.clamp(0.0, 1.0),
            hip_pitch: p.f("legInjuryHipPitch", 0.0)?, spine_bend: p.f("legInjurySpineBend", 0.1)?, limp: p.f("legLimpBend", 0.0)?,
            lean: p.f("legLean", 15.0)?, sag: p.f("legSag", 0.04)?.max(0.0), blend: p.f("legBlend", 0.5)?.max(0.05), stiffness: p.f("stiffness", 10.0)?,
            t: 0.0, lift_b: None, limp_b: None,
        })
    }
}

impl Extra for InjuredLeg {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let s = self.side;
        let (hip, knee) = (HIP()[s], KNEE()[s]);
        let (lift, limp, pitch, bend) = (self.lift, self.limp, self.hip_pitch, self.spine_bend);
        let kd = knee_dir(c, knee);
        let lift_b = self.lift_b.get_or_insert_with(|| Bends::new(vec![(hip, 0, lift), (knee, 0, 2.0 * lift * kd)]));
        lift_b.apply(c, swell(self.t, 0.0, self.lift_time * 2.0), 0.0);
        let limp_b = self.limp_b.get_or_insert_with(|| {
            let mut v = vec![(knee, 0, limp * kd), (hip, 0, pitch)];
            v.extend(SPINE().iter().map(|&j| (j, 0, bend / 4.0)));
            Bends::new(v)
        });
        let t = self.t - self.lift_time;
        let w = if t < 0.0 { 0.0 } else if t < 0.15 { t / 0.15 } else if t < self.time { 1.0 } else { (1.0 - (t - self.time) / self.blend).max(0.0) };
        limp_b.apply(c, w, 0.0);
        if w > 0.0 {
            // the weak leg: its strength down by legInjury (stiffness by its square root)
            let k = (1.0 - self.injury * w).max(0.05).sqrt();
            c.ch.set_stiffness(self.stiffness * k, 1.0, ["ll", "lr"][s], None);
            // the balancer sinks him a little onto the weak leg
            if self.sag > 0.0 {
                c.out.dbm_leg_offset = Some(-self.sag * w);
            }
            if self.lean != 0.0 {
                let up = c.ch.up.normalize_or(Vec3::Z);
                let foot = c.b.part([3, 6][s]).m.d - c.com;
                let towards = (foot - up * foot.dot(up)).normalize_or_zero();
                c.out.force(10, towards * self.lean * w);
            }
        }
    }

    fn done(&self) -> bool {
        self.t > self.lift_time + self.time + self.blend
    }

    fn stop(&mut self, c: &mut Ctx) {
        if self.sag > 0.0 {
            c.out.dbm_leg_offset = Some(0.0);
        }
    }
}

/// RDR2's injured arm: a hit arm knocks that shoulder back and down. For injuredArmTime a yaw torque (hipYaw, N m)
/// turns the trunk with the bullet and a roll torque (hipRoll, N m) drops the hit shoulder, fading out; the arm hangs
/// loose (its stiffness down by armLoose) for armLooseTime. The arm is bodyPart's (13..=20), or leftArm's.
pub struct InjuredArm {
    side: usize, dir: Vec3, time: f32, yaw: f32, roll: f32, loose: f32, loose_time: f32, stiffness: f32, t: f32,
}

impl InjuredArm {
    fn new(p: &mut P) -> Result<InjuredArm, Error> {
        let part = p.i("bodyPart", -1)?;
        let left = p.b("leftArm", true)?;
        Ok(InjuredArm {
            side: part_side(part).unwrap_or(if left { 0 } else { 1 }), dir: p.v("bulletDir", Vec3::ZERO)?,
            time: p.f("injuredArmTime", 0.25)?.max(0.0), yaw: p.f("hipYaw", 80.0)?, roll: p.f("hipRoll", 0.0)?,
            loose: p.f("armLoose", 0.7)?.clamp(0.0, 1.0), loose_time: p.f("armLooseTime", 1.0)?.max(0.0),
            stiffness: p.f("stiffness", 10.0)?, t: 0.0,
        })
    }
}

impl Extra for InjuredArm {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let r = c.b.part([13, 17][self.side]).m.d - c.com;
        if self.t < self.time {
            let w = 1.0 - self.t / self.time.max(0.01);
            // yaw: the way that carries the hit shoulder along the bullet (from in front if none given)
            let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
            let push = if self.dir == Vec3::ZERO { up.cross(side) } else { self.dir };
            let push = (push - up * push.dot(up)).normalize_or_zero();
            let yaw = up * r.cross(push).dot(up).signum() * self.yaw * w;
            // roll: the way that drops the hit shoulder
            let roll = r.cross(-up).normalize_or_zero() * self.roll * w;
            c.out.torque(10, (yaw + roll) * 0.7);
            c.out.torque(0, (yaw + roll) * 0.3);
        }
        if self.t < self.loose_time {
            c.ch.set_stiffness(self.stiffness * (1.0 - self.loose), 1.0, ["ul", "ur"][self.side], None);
        }
    }

    fn done(&self) -> bool {
        self.t > self.time.max(self.loose_time)
    }
}

/// A point and a direction carried along with a part: kept in the part's own frame.
#[derive(Clone, Copy)]
struct OnPart { part: usize, pos: Vec3, dir: Vec3 }

impl OnPart {
    fn new(c: &Ctx, part: usize, at: Vec3, dir: Vec3) -> OnPart {
        let m = c.b.part(part).m;
        let local = |v: Vec3| Vec3::new(v.dot(m.a), v.dot(m.b), v.dot(m.c));
        OnPart { part, pos: local(at - m.d), dir: local(dir) }
    }

    fn now(&self, c: &Ctx) -> (Vec3, Vec3) {
        let m = c.b.part(self.part).m;
        let world = |v: Vec3| m.a * v.x + m.b * v.y + m.c * v.z;
        (world(self.pos) + m.d, world(self.dir).normalize_or_zero())
    }
}

/// Clavicle (lean1, lean2), shoulder (lean1, lean2, twist) and elbow angles, inside their limits, that put the hand
/// nearest `target`: a coordinate search over the body's own forward kinematics from its current pose, starting at
/// `start` and kept close to it, so the answer moves smoothly from tick to tick. (A two-bone solve can't find poses
/// that need the clavicle and the shoulder's twist, such as a hand on the chest.) Returns the angles and how far
/// the hand would be from the target.
fn arm_solve(c: &Ctx, l: crate::nmshot::Limb, target: Vec3, start: [f32; 6]) -> ([f32; 6], f32) {
    let (cl, sh, el, hand) = (l.e[0], l.e[1], l.e[2], l.p[delirium_data::kv!(0 int)]);
    let mut ang: Vec<[f32; 3]> = (0..c.ch.joints.len()).map(|j| c.ch.actual(j, c.b.frames)).collect();
    let root = c.b.frames[0];
    let base = c.ch.pose(c.b.build, root, &ang)[hand].translation;
    let now = c.b.frames[hand].translation;
    let (lc, ls, le) = (c.ch.limits(cl), c.ch.limits(sh), c.ch.limits(el));
    let lo = [lc[0], lc[2], ls[0], ls[2], ls[4], le[0]];
    let hi = [lc[1], lc[3], ls[1], ls[3], ls[5], le[1]];
    let twist = ang[cl][2];
    let mut hand_at = |x: [f32; 6]| {
        ang[cl] = [x[0], x[1], twist];
        ang[sh] = [x[2], x[3], x[4]];
        ang[el] = [x[5], 0.0, 0.0];
        c.ch.pose(c.b.build, root, &ang)[hand].translation - base + now
    };
    let mut cost = |x: [f32; 6]| {
        let keep: f32 = (0..6).map(|k| (x[k] - start[k]).powi(2)).sum();
        (hand_at(x) - target).length_squared() + 0.002 * keep
    };
    let mut x: [f32; 6] = std::array::from_fn(|k| start[k].clamp(lo[k], hi[k]));
    let mut best = cost(x);
    for step in [0.25, 0.12, 0.06, 0.03] {
        for _ in 0..3 {
            let mut moved = false;
            for k in 0..6 {
                for d in [step, -step] {
                    let mut y = x;
                    y[k] = (y[k] + d).clamp(lo[k], hi[k]);
                    let f = cost(y);
                    if f < best {
                        (best, x, moved) = (f, y, true);
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }
    let miss = (hand_at(x) - target).length();
    (x, miss)
}

/// RDR2's reach for wound, a behaviour of its own there: after timeBeforeReachForWound one hand (or, without
/// reachWithOneHand, both) goes to the wound and holds it for grabHoldTime, the palm on the body and pulled onto
/// it, while the body bends towards the wound: a little hunch and a twist round to it for the trunk, and for a
/// thigh (allowInjuredThighReach) the hips, knees and spine bend down so the hand gets there. The bend comes in over
/// bendTimeUp and goes over bendTimeDown. The hand is the far one for an arm wound, the same side's for a leg, the
/// nearer one for the trunk.
pub struct ReachForWound {
    point: Vec3, part: i32, wait: f32, hold: f32, one_hand: bool, thigh: bool, lower_leg: bool, arm_stiffness: f32,
    spine_bend: f32, hip_bend: f32, knee_bend: f32, twist: f32, up: f32, down: f32, stiffness: f32,
    t: f32, wound: Option<OnPart>, hands: [bool; 2], sign: [f32; 2], bends: Option<Bends>, arm: [Option<[f32; 6]>; 2],
}

impl ReachForWound {
    fn new(p: &mut P) -> Result<ReachForWound, Error> {
        Ok(ReachForWound {
            point: p.v("hitPoint", Vec3::ZERO)?, part: p.i("hitPart", -1)?,
            wait: p.f("timeBeforeReachForWound", 0.2)?.max(0.0), hold: p.f("grabHoldTime", 2.0)?.max(0.0),
            one_hand: p.b("reachWithOneHand", true)?, thigh: p.b("allowInjuredThighReach", true)?,
            lower_leg: p.b("allowInjuredLowerLegReach", false)?, arm_stiffness: p.f("armStiffness", 10.0)?,
            spine_bend: p.f("bend2spine", 0.7)?, hip_bend: p.f("bend2hip", 0.6)?, knee_bend: p.f("bend2leg", 0.6)?,
            twist: p.f("spineTwistMag", 0.2)?, up: p.f("bendTimeUp", 0.4)?.max(0.01), down: p.f("bendTimeDown", 0.5)?.max(0.01),
            stiffness: p.f("spineStiffness", 12.0)?,
            t: 0.0, wound: None, hands: [false; 2], sign: [1.0; 2], bends: None, arm: [None; 2],
        })
    }

    /// The hands to use and the body's bend, once, from where the wound is.
    fn aim(&mut self, c: &mut Ctx) {
        let part = self.part as usize;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        let fwd = side.cross(up).normalize_or(Vec3::Y);
        let q = c.b.part(part).m.d;
        let point = if self.point == Vec3::ZERO { q + fwd * if (7..=10).contains(&part) { 0.18 } else { 0.12 } } else { self.point };
        // the body's surface there: out from the part's axis (for the trunk, out from the spine)
        let out = point - q;
        let out = (out - up * out.dot(up)).normalize_or(fwd);
        self.wound = Some(OnPart::new(c, part, point, out));
        let leg = (1..=6).contains(&part);
        let lower = matches!(part, 2 | 3 | 5 | 6);
        let arm = (13..=20).contains(&part);
        let left = (point - c.com).dot(side) > 0.0;
        self.hands = if arm {
            if part_side(self.part) == Some(0) { [false, true] } else { [true, false] }
        } else if leg {
            let ok = if lower { self.lower_leg } else { self.thigh };
            let s = part_side(self.part).unwrap_or(0);
            [ok && s == 0, ok && s == 1]
        } else if self.one_hand {
            [left, !left]
        } else {
            [true, true]
        };
        let (sb, hb, kb, tw) = (self.spine_bend, self.hip_bend, self.knee_bend, self.twist);
        let mut v: Vec<(usize, usize, f32)> = vec![];
        if leg && self.hands.contains(&true) {
            // bend down to the leg: hips and knees (both legs) and the spine forwards
            let more = if lower { 1.5 } else { 1.0 };
            v.extend(SPINE().iter().map(|&j| (j, 0, sb * more / 4.0)));
            v.extend(HIP().iter().map(|&j| (j, 0, hb * more)));
            v.extend(KNEE().iter().map(|&j| (j, 0, kb * more * knee_dir(c, j))));
        } else if !arm {
            // a hunch over the wound and a twist round to it
            v.extend(SPINE().iter().map(|&j| (j, 0, 0.15 / 4.0)));
        }
        if tw != 0.0 && !leg {
            let to = if left { 1.0 } else { -1.0 };
            v.extend(SPINE().iter().map(|&j| (j, 2, tw * to / 4.0)));
        }
        self.bends = Some(Bends::new(v));
    }

    /// One hand to the wound: the clavicle forward, the arm by IK to just off the wound, the palm turned onto it,
    /// and a pull once close (as Delirium's shot reaches).
    fn reach(&mut self, c: &mut Ctx, i: usize, t: Vec3, n: Vec3, dt: f32) {
        let l = crate::nmshot::limb(i);
        let part = self.wound.map_or(0, |w| w.part);
        for j in [l.e[0], l.e[1], l.e[2]] {
            c.ch.eff[j].oppose_gravity = 1.0;
        }
        c.ch.set_stiffness(self.arm_stiffness, 1.0, ["ul", "ur"][i], None);
        // aim just off the wound, closing in as the hand gets near
        let hand = c.b.part(l.p[delirium_data::kv!(0 int)]).m.d;
        let mut dh = hand - t;
        let s = dh.dot(n);
        dh -= n * if s < 0.0 { s } else { 0.0 };
        // and from a little above while far, so the hand comes round the hip rather than into it
        let up = c.ch.up.normalize_or(Vec3::Z);
        let t2 = t + n * (dh.length() * 0.5 - 0.05).min(0.25) + up * (dh.length() * 0.5).min(0.15);
        let start = self.arm[i].unwrap_or_else(|| {
            let [k, a, e] = [0, 1, 2].map(|n| c.ch.actual(l.e[n], c.b.frames));
            [k[0], k[1], a[0], a[1], a[2], e[0]]
        });
        let (mut x, mut miss) = arm_solve(c, l, t2, start);
        if self.arm[i].is_none() && miss > 0.08 {
            // the search only finds the nearest pose, so the first time also try from the arm raised forwards and
            // across (once: after that it follows on from where it got to)
            for (sl1, stw) in [(0.6, 0.0), (1.2, -0.6), (0.9, 0.4)] {
                let (y, m) = arm_solve(c, l, t2, [start[0], start[1], sl1, start[3], stw, 1.5]);
                if m + 0.02 < miss {
                    (x, miss) = (y, m);
                }
            }
        }
        self.arm[i] = Some(x);
        (c.ch.eff[l.e[0]].desired[0], c.ch.eff[l.e[0]].desired[1]) = (x[0], x[1]);
        c.ch.eff[l.e[1]].desired = [x[2], x[3], x[4]];
        c.ch.eff[l.e[2]].desired[0] = x[5];
        let side = if i == 0 { 1.0 } else { -1.0 };
        if crate::nmshot::Shot::wrist(c, l, t, n, side) < 0.0 && part != 12 {
            self.sign[i] = -self.sign[i];
        }
        // the pull: once within 0.3 m, a growing impulse draws the palm onto the wound (and the body back onto it)
        let h = c.b.part(l.p[delirium_data::kv!(0 int)]);
        let at = h.m.d - h.m.a * (self.sign[i] * 0.02);
        let d = (t - at).length();
        if d < 0.3 {
            let k = ((0.3 - d) / 0.2).clamp(0.0, 1.0) * 4.0;
            let mut imp = (t - at) * k * k + (c.b.part(part).v - h.v) * (k * 0.2);
            if dt < 1.0 / 60.0 {
                imp *= dt * 60.0;
            }
            let wrist = c.b.joint_pos(l.e[3]);
            c.out.impulse.push((l.p[delirium_data::kv!(0 int)], wrist, imp));
            c.out.impulse.push((part, t, -imp));
        }
    }
}

impl Extra for ReachForWound {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        if !(0..21).contains(&self.part) {
            self.part = 8;
        }
        if self.wound.is_none() {
            self.aim(c);
        }
        let t = self.t - self.wait;
        let w = if t < 0.0 { 0.0 } else if t < self.hold { (t / self.up).min(1.0) } else { (1.0 - (t - self.hold) / self.down).max(0.0) };
        if let Some(b) = self.bends.as_mut() {
            b.apply(c, w, self.stiffness);
        }
        if t >= 0.0 && t < self.hold {
            let (at, n) = self.wound.expect("aimed").now(c);
            // two hands: side by side over the wound, one a little higher, so they don't fight for the spot
            let both = self.hands == [true, true];
            let up = c.ch.up.normalize_or(Vec3::Z);
            let across = n.cross(up).normalize_or_zero();
            for i in 0..2 {
                if self.hands[i] && !c.ch.busy[i] {
                    let s = if i == 0 { 1.0 } else { -1.0 };
                    let at = if both { at + across * (0.07 * s * across.dot(c.b.part(13).m.d - c.b.part(17).m.d).signum()) + up * (0.03 * s) } else { at };
                    self.reach(c, i, at, n, dt);
                }
            }
        }
    }

    fn done(&self) -> bool {
        self.t > self.wait + self.hold + self.down
    }
}

/// What a balancerCollisionsReaction does with the wall, from how he meets it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum WallMode { Slump, Brace, Glance, Rebound }

/// RDR2's balancer collisions reaction (wallCollision here: Delirium's GTA V balancerCollisionsReaction, the slump
/// alone, keeps that name): he has run or stumbled into a wall (objectBehindVictimPos, with
/// objectBehindVictimNormal pointing out of it towards him). The wall is made solid for his body here (Delirium's world
/// has only the ground): a spring pushes any part back out of its plane. At the impact (reactTime) his legs weaken
/// by impactLegStiffReduction and he snaps away from it. Then, by how he meets it:
/// - back to it (or slumpMode 1): he slumps, letting go of balance after sitStartTime and sliding down it to sit, legs
///   weakening at sitRate, his back pressed to it and his feet sliding out;
/// - facing it (with braceWall, within braceDistance): both hands go to the wall at chest height and hold it with
///   braceArmStiffness, then after timeToForward he pushes himself off it (reboundForce);
/// - side on: a glancing spin away from it (glanceSpinMag, decaying over glanceSpinTime), the near hand on the wall.
/// slumpMode 2 never slumps.
pub struct WallCollision {
    at: Vec3, normal: Vec3, solid: bool, slump_mode: i32, brace: bool, brace_dist: f32, brace_stiffness: f32,
    react: f32, impact_legs: f32, sit_start: f32, sit_rate: f32, glance: f32, glance_time: f32, rebound: f32,
    forward_time: f32, stiffness: f32, snap_mag: f32, wall_k: f32, half_width: f32,
    t: f32, mode: Option<WallMode>, snap: Option<Snap>, arm: [Option<[f32; 6]>; 2], legs: f32,
}

impl WallCollision {
    fn new(p: &mut P) -> Result<WallCollision, Error> {
        Ok(WallCollision {
            at: p.v("objectBehindVictimPos", Vec3::ZERO)?, normal: p.v("objectBehindVictimNormal", Vec3::ZERO)?,
            solid: p.b("solidWall", true)?, slump_mode: p.i("slumpMode", 0)?, brace: p.b("braceWall", true)?,
            brace_dist: p.f("braceDistance", 2.0)?, brace_stiffness: p.f("braceArmStiffness", 12.0)?,
            react: p.f("reactTime", 0.3)?.max(0.0), impact_legs: p.f("impactLegStiffReduction", 3.0)?.max(0.0),
            sit_start: p.f("sitStartTime", 0.3)?.max(0.0), sit_rate: p.f("sitRate", 1.0)?.max(0.01),
            glance: p.f("glanceSpinMag", 50.0)?, glance_time: p.f("glanceSpinTime", 0.5)?.max(0.01),
            rebound: p.f("reboundForce", 0.0)?, forward_time: p.f("timeToForward", 0.5)?.max(0.0),
            stiffness: p.f("stiffness", 10.0)?, snap_mag: p.f("snapMag", 0.3)?, wall_k: p.f("wallStiffness", 400.0)?,
            half_width: p.f("wallHalfWidth", 0.0)?.max(0.0),
            t: 0.0, mode: None, snap: None, arm: [None; 2], legs: 0.0,
        })
    }

    /// The wall's plane, horizontal normal out towards him.
    fn plane(&self, up: Vec3) -> (Vec3, Vec3) {
        let n = self.normal - up * self.normal.dot(up);
        (self.at, n.normalize_or_zero())
    }

    /// Keeps his parts on his side of the wall: a damped spring per part on how far it is inside (and, with a
    /// wallHalfWidth, only along that much of it either side of the point given).
    fn wall(&self, c: &mut Ctx, at: Vec3, n: Vec3) {
        let along = c.ch.up.normalize_or(Vec3::Z).cross(n).normalize_or_zero();
        for k in 0..c.b.build.parts.len().min(21) {
            let pd = &c.b.build.parts[k];
            let q = c.b.part(k);
            if self.half_width > 0.0 && (q.m.d - at).dot(along).abs() > self.half_width {
                continue;
            }
            let reach = pd.radius + pd.half * 0.5;
            let inside = reach - (q.m.d - at).dot(n);
            if inside > 0.0 {
                let vn = q.v.dot(n);
                let a = self.wall_k * inside - 2.0 * self.wall_k.sqrt() * vn.min(0.0);
                c.out.force(k, n * pd.mass * a.max(0.0));
            }
        }
    }

    /// One hand flat on the wall in front of its shoulder (a little lower), as reachForWound places hands.
    fn hand_on_wall(&mut self, c: &mut Ctx, i: usize, at: Vec3, n: Vec3, up: Vec3) {
        let l = crate::nmshot::limb(i);
        let sh = c.b.part(l.p[0]).m.d;
        let on = sh - n * (sh - at).dot(n) - up * 0.15;
        let target = on + n * 0.04;
        let start = self.arm[i].unwrap_or_else(|| {
            let [k, a] = [0, 1].map(|j| c.ch.actual(l.e[j], c.b.frames));
            [k[0], k[1], 0.9, a[1], a[2], 0.4]
        });
        let (x, _) = arm_solve(c, l, target, start);
        self.arm[i] = Some(x);
        (c.ch.eff[l.e[0]].desired[0], c.ch.eff[l.e[0]].desired[1]) = (x[0], x[1]);
        c.ch.eff[l.e[1]].desired = [x[2], x[3], x[4]];
        c.ch.eff[l.e[2]].desired[0] = x[5];
        c.ch.set_stiffness(self.brace_stiffness, 1.0, ["ul", "ur"][i], None);
        let side = if i == 0 { 1.0 } else { -1.0 };
        crate::nmshot::Shot::wrist(c, l, on, n, side);
    }
}

impl Extra for WallCollision {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let (at, n) = self.plane(up);
        if n == Vec3::ZERO {
            self.t = f32::INFINITY;
            return;
        }
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        let fwd = side.cross(up).normalize_or(Vec3::Y);
        let mode = *self.mode.get_or_insert_with(|| {
            let facing = -fwd.dot(n);
            let dist = (c.b.part(10).m.d - at).dot(n);
            if self.slump_mode == 1 || (self.slump_mode == 0 && facing < -0.5) {
                WallMode::Slump
            } else if facing > 0.5 {
                if self.brace && dist < self.brace_dist { WallMode::Brace } else { WallMode::Rebound }
            } else {
                WallMode::Glance
            }
        });
        if self.solid {
            self.wall(c, at, n);
        }
        // the impact: legs weaken, a snap away from the wall
        if self.t < self.react {
            c.ch.set_stiffness((self.stiffness - self.impact_legs).max(2.0), 1.0, "lb", None);
        }
        let snap = self.snap.get_or_insert_with(|| Snap::at(self.snap_mag, 10, n));
        if !snap.done() {
            snap.tick(c, dt);
        }
        match mode {
            WallMode::Slump => {
                // he sags back onto the wall: pelvis and chest drawn to it (a damped spring down to touching), the
                // spine held straight so the back lies on it
                for k in [0, 10] {
                    let pd = &c.b.build.parts[k];
                    let q = c.b.part(k);
                    let gap = (q.m.d - at).dot(n) - (pd.radius + 0.02);
                    if gap > 0.0 {
                        let a = 60.0 * gap.min(0.4) + 12.0 * q.v.dot(n).max(0.0);
                        c.out.force(k, -n * pd.mass * a);
                    }
                }
                for &j in SPINE() {
                    Knees::ease(c, j, 0, 0.0, 2.0 * dt);
                }
                let t = self.t - self.sit_start;
                if t < 0.0 {
                    return;
                }
                c.out.dbm = Some(false);
                // legs give at sitRate; the hips and knees fold into a sit, the back stays against the wall
                self.legs = (self.legs + dt * self.sit_rate).min(1.0);
                let w = self.legs;
                let k = (self.stiffness * (1.0 - 0.6 * w)).max(3.0);
                c.ch.set_stiffness(k, 1.0, "lb", None);
                c.ch.set_stiffness(self.stiffness * 0.8, 1.0, "us", None);
                for s in 0..2 {
                    Knees::ease(c, HIP()[s], 0, 1.3 * w, 3.0 * dt);
                    Knees::ease(c, KNEE()[s], 0, 1.4 * w * knee_dir(c, KNEE()[s]), 3.0 * dt);
                }
                c.ch.set_stiffness(self.stiffness * 0.5, 1.0, "ua", None);
                // sitting up against it: the chest kept over the pelvis along the wall and held up above it (his
                // back's friction on the wall, which Delirium's virtual wall doesn't have)
                let along = up.cross(n).normalize_or_zero();
                let (pel, chest) = (c.b.part(0), c.b.part(10));
                let off = (chest.m.d - pel.m.d).dot(along);
                let rise = (chest.m.d - pel.m.d).dot(up);
                let cm = c.b.build.parts[10].mass;
                let side = -along * (150.0 * off + 20.0 * (chest.v - pel.v).dot(along));
                let lift = up * (150.0 * (0.42 - rise).max(0.0) - 15.0 * chest.v.dot(up).min(0.0));
                c.out.force(10, (side + lift) * cm * w);
                // sliding down it; the feet slide out from it
                for f in [3, 6] {
                    c.out.force(f, n * 4.0 * c.b.build.parts[f].mass * w);
                }
            }
            WallMode::Brace => {
                if self.t < self.forward_time {
                    for i in 0..2 {
                        if !c.ch.busy[i] {
                            self.hand_on_wall(c, i, at, n, up);
                        }
                    }
                } else if self.t < self.forward_time + 0.25 {
                    // push off: arms straighten against it, a shove on the chest away from it
                    c.out.force(10, n * self.rebound);
                    c.out.force(0, n * self.rebound * 0.4);
                }
            }
            WallMode::Rebound => {
                if self.t < 0.25 {
                    c.out.force(10, n * self.rebound);
                }
            }
            WallMode::Glance => {
                // turn away from the wall: the way that swings his facing towards its normal
                let sign = up.cross(fwd).dot(n).signum();
                let tq = self.glance * (-self.t / self.glance_time).exp();
                c.out.torque(10, up * sign * tq * 0.7);
                c.out.torque(0, up * sign * tq * 0.3);
                if self.t < self.forward_time {
                    let near = if (at - c.com).dot(side) > 0.0 { 0 } else { 1 };
                    if !c.ch.busy[near] {
                        self.hand_on_wall(c, near, at, n, up);
                    }
                }
            }
        }
    }

    fn done(&self) -> bool {
        match self.mode {
            Some(WallMode::Slump) => self.t > self.sit_start + 6.0,
            Some(WallMode::Brace) => self.t > self.forward_time + 0.6,
            Some(_) => self.t > self.forward_time.max(self.glance_time * 3.0).max(self.react),
            None => self.t.is_infinite(),
        }
    }
}

/// RDR2's falling reaction with handsAndKnees (its setfallingreaction, folded into catchFall there): a forward fall
/// ends on all fours. The balancer lets go and, as he goes down, the hips and knees fold (about a right angle each)
/// and the hands reach for the ground under the shoulders. Once hands and knees are down he holds there for
/// hkHoldTime, the trunk level and the head up, the arms at least minArmStiffness; then his strength fades
/// (stoppedStrengthDecay per second) and he sinks onto his front. pushForward tips him over first (testing, or a
/// stumble forwards).
pub struct FallingReaction {
    on: bool, hold: f32, decay: f32, arm_k: f32, leg_k: f32, spine_k: f32, push: f32, head_up: bool,
    t: f32, phase: u8, since: f32, ground: Option<f32>, arm: [Option<[f32; 6]>; 2], strength: f32,
}

impl FallingReaction {
    fn new(p: &mut P) -> Result<FallingReaction, Error> {
        Ok(FallingReaction {
            on: p.b("handsAndKnees", true)?, hold: p.f("hkHoldTime", 1.5)?.max(0.0), decay: p.f("stoppedStrengthDecay", 1.0)?.max(0.01),
            arm_k: p.f("minArmStiffness", 14.0)?, leg_k: p.f("minLegStiffness", 12.0)?, spine_k: p.f("minSpineStiffness", 9.0)?,
            push: p.f("pushForward", 0.0)?, head_up: p.b("hkHeadAvoid", true)?,
            t: 0.0, phase: 0, since: 0.0, ground: None, arm: [None; 2], strength: 1.0,
        })
    }

    /// The pose: hips and knees folded by `w` of a right angle, spine level, neck up, the hands to the ground under
    /// the shoulders, stiffness by `k` of the set floors.
    fn pose(&mut self, c: &mut Ctx, w: f32, k: f32, ground: f32, up: Vec3, fwd: Vec3, dt: f32) {
        c.ch.set_stiffness(self.leg_k * k, 1.0, "lb", None);
        c.ch.set_stiffness(self.spine_k * k, 1.0, "us", None);
        c.ch.set_stiffness(self.spine_k * k, 1.0, "un", None);
        for s in 0..2 {
            Knees::ease(c, HIP()[s], 0, 1.5 * w, 6.0 * dt);
            Knees::ease(c, KNEE()[s], 0, 1.6 * w * knee_dir(c, KNEE()[s]), 6.0 * dt);
        }
        for &j in SPINE() {
            Knees::ease(c, j, 0, 0.0, 3.0 * dt);
        }
        if self.head_up {
            for &j in NECK() {
                Knees::ease(c, j, 0, -0.35 * w, 3.0 * dt);
            }
        }
        for i in 0..2 {
            if c.ch.busy[i] {
                continue;
            }
            let l = crate::nmshot::limb(i);
            // under the shoulder (the clavicle's outer end, roughly), a little ahead of it, on the ground
            let sh = c.b.part(l.p[0]).m.d;
            let mid = (c.b.part(13).m.d + c.b.part(17).m.d) * 0.5;
            let out = sh + (sh - mid) * 0.6;
            let below = out - up * ((out.dot(up) - ground) - 0.04) + fwd * 0.08;
            let start = self.arm[i].unwrap_or_else(|| {
                let [kk, a] = [0, 1].map(|j| c.ch.actual(l.e[j], c.b.frames));
                [kk[0], kk[1], 0.6, a[1], a[2], 0.2]
            });
            let (x, _) = arm_solve(c, l, below, start);
            self.arm[i] = Some(x);
            (c.ch.eff[l.e[0]].desired[0], c.ch.eff[l.e[0]].desired[1]) = (x[0], x[1]);
            c.ch.eff[l.e[1]].desired = [x[2], x[3], x[4]];
            c.ch.eff[l.e[2]].desired[0] = x[5];
            c.ch.set_stiffness(self.arm_k * k, 1.0, ["ul", "ur"][i], None);
        }
    }
}

impl FallingReaction {
    /// What his arms and knees carry on all fours (NaturalMotion uses such helpers too: joint strength alone
    /// can't hold a body up): the upper body's weight held at the chest over the hands, the lower body's at the
    /// pelvis over the knees, each a damped spring to its height, by `k` - each only while hands (or knees) touch.
    fn support(&self, c: &mut Ctx, ground: f32, up: Vec3, k: f32) {
        let mass = |r: std::ops::RangeInclusive<usize>| r.map(|i| c.b.build.parts[i].mass).sum::<f32>();
        let (upper, lower) = (mass(7..=20), mass(0..=6));
        let hands_at = (c.b.part(16).m.d + c.b.part(20).m.d) * 0.5;
        let knees_at = (c.b.part(2).m.d + c.b.part(5).m.d) * 0.5;
        // only what the hands and knees really rest on: a helper with nothing under it would hold him up in the
        // air (a slope down, a ledge, stairs)
        let hands = c.b.doll.collided(&[15, 16, 19, 20]);
        let knees = c.b.doll.collided(&[1, 2, 4, 5]);
        for (part, over, height, m, on) in [(10, hands_at, 0.6, upper, hands), (0, knees_at, 0.55, lower * 0.6, knees)] {
            if !on {
                continue;
            }
            let q = c.b.part(part);
            let off = over - q.m.d;
            let flat = off - up * off.dot(up);
            let vflat = q.v - up * q.v.dot(up);
            let h = q.m.d.dot(up) - ground;
            let lift = 4.9 + 90.0 * (height - h).clamp(-0.2, 0.3) - 10.0 * q.v.dot(up);
            c.out.force(part, (flat * 20.0 - vflat * 5.0 + up * lift.max(0.0)) * m * k);
        }
        // the knees drawn in under the hips (the legs trail out behind him otherwise)
        let under = c.b.part(0).m.d;
        for shin in [2, 5] {
            let q = c.b.part(shin);
            let off = under - q.m.d;
            let flat = off - up * off.dot(up);
            let vflat = q.v - up * q.v.dot(up);
            c.out.force(shin, (flat * 25.0 - vflat * 5.0) * c.b.build.parts[shin].mass * 2.0 * k);
        }
    }
}

impl Extra for FallingReaction {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if !self.on {
            return;
        }
        self.t += dt;
        self.since += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        let fwd = side.cross(up).normalize_or(Vec3::Y);
        let ground = *self.ground.get_or_insert_with(|| c.b.part(3).m.d.dot(up).min(c.b.part(6).m.d.dot(up)) - 0.06);
        c.out.dbm = Some(false);
        if self.t < 0.3 && self.push != 0.0 {
            c.out.force(10, fwd * self.push);
        }
        let h = |c: &Ctx, k: usize| c.b.part(k).m.d.dot(up) - ground;
        match self.phase {
            0 => {
                // going down: fold in and reach for the ground
                let w = (self.t / 0.25).min(1.0);
                self.pose(c, w, 1.0, ground, up, fwd, dt);
                // arms and knees take the landing once he is low
                if h(c, 10) < 0.75 {
                    self.support(c, ground, up, 0.7);
                }
                let hands = h(c, 16).max(h(c, 20)) < 0.12;
                let knees = h(c, 2).min(h(c, 5)) < 0.25;
                if (hands && knees) || self.since > 1.5 {
                    (self.phase, self.since) = (1, 0.0);
                }
            }
            1 => {
                // on all fours: the pose held, his weight on hands and knees
                self.pose(c, 1.0, 1.0, ground, up, fwd, dt);
                self.support(c, ground, up, 1.0);
                if self.since > self.hold {
                    (self.phase, self.since) = (2, 0.0);
                }
            }
            _ => {
                // strength fades: he sinks down onto his front
                self.strength = (self.strength - self.decay * dt).max(0.15);
                let k = self.strength;
                self.pose(c, 1.0, k, ground, up, fwd, dt);
            }
        }
    }

    fn done(&self) -> bool {
        !self.on || (self.phase == 2 && self.strength <= 0.15 && self.since > 0.5)
    }
}

/// RDR2's balancer additions, laid over Delirium's balancer while he balances (it stays on till stopped):
/// - running balance: above runningThresh (m/s, his centre's speed along the ground) the hips and knees are at least
///   legStiffnessRun, so a running stumble stays a stumble rather than folding;
/// - feet flattening: a foot coming down is turned flat to the ground (its sole square to up), fully from startFlat
///   above the other foot down, not at all from endFlat up (as RDR2's startFlat / endFlat), by a muscle-like torque
///   pair between foot and shin (flatStiffness, N m per rad).
pub struct BalanceRdr2 {
    run_thresh: f32, run_k: f32, start_flat: f32, end_flat: f32, flat_k: f32,
}

impl BalanceRdr2 {
    fn new(p: &mut P) -> Result<BalanceRdr2, Error> {
        Ok(BalanceRdr2 {
            run_thresh: p.f("runningThresh", 0.65)?, run_k: p.f("legStiffnessRun", 13.0)?,
            start_flat: p.f("startFlat", 0.1)?.max(0.0), end_flat: p.f("endFlat", 0.3)?, flat_k: p.f("flatStiffness", 12.0)?.max(0.0),
        })
    }
}

impl Extra for BalanceRdr2 {
    fn tick(&mut self, c: &mut Ctx, _dt: f32) {
        if c.dbm_failed {
            return;
        }
        let up = c.ch.up.normalize_or(Vec3::Z);
        let speed = (c.vel - up * c.vel.dot(up)).length();
        if speed > self.run_thresh && self.run_k > 0.0 {
            let (s, d) = (self.run_k * self.run_k, 2.0 * self.run_k);
            for j in [HIP()[0], KNEE()[0], HIP()[1], KNEE()[1]] {
                let e = &mut c.ch.eff[j];
                (e.strength, e.damping) = (e.strength.max(s), e.damping.max(d));
            }
        }
        if self.flat_k > 0.0 {
            for (foot, shin, other) in [(3, 2, 6), (6, 5, 3)] {
                let f = c.b.part(foot);
                let h = (f.m.d - c.b.part(other).m.d).dot(up);
                let span = (self.end_flat - self.start_flat).max(0.01);
                let w = ((self.end_flat - h) / span).clamp(0.0, 1.0);
                if w <= 0.0 {
                    continue;
                }
                // the sole's normal is the foot's b axis (the balancer squares it to the ground the same way)
                let err = f.m.b.normalize_or(up).cross(up);
                let spin = f.w - c.b.part(shin).w;
                let t = (err * self.flat_k - spin * self.flat_k * 0.1) * w;
                c.out.torque(foot, t);
                c.out.torque(shin, -t);
            }
        }
    }
}

/// RDR2's keyframe behaviour, used here to blend a balancing body into a neutral stand before it is handed back to
/// animation: over blendTime the spine and neck ease straight, the hands go to his sides, and the parts' motion is
/// damped (settle, 1/s), so the animation picks up from a pose close to its own; then it holds that for holdTime.
/// The legs stay the balancer's.
pub struct Keyframe {
    blend: f32, hold: f32, settle: f32, stiffness: f32,
    t: f32, start: Option<Vec<(usize, [f32; 3])>>, arm: [Option<[f32; 6]>; 2],
}

impl Keyframe {
    fn new(p: &mut P) -> Result<Keyframe, Error> {
        Ok(Keyframe {
            blend: p.f("blendTime", 0.3)?.max(0.02), hold: p.f("holdTime", 0.5)?.max(0.0), settle: p.f("settle", 3.0)?.max(0.0),
            stiffness: p.f("stiffness", 11.0)?, t: 0.0, start: None, arm: [None; 2],
        })
    }
}

impl Extra for Keyframe {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let x = (self.t / self.blend).min(1.0);
        let w = x * x * (3.0 - 2.0 * x);
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        let fwd = side.cross(up).normalize_or(Vec3::Y);
        // spine and neck: from where they were towards straight
        let start = self.start.get_or_insert_with(|| SPINE().iter().chain(NECK().iter()).map(|&j| (j, c.ch.eff[j].desired)).collect());
        for &(j, d0) in start.iter() {
            c.ch.eff[j].desired = d0.map(|a| a * (1.0 - w));
            c.ch.eff[j].set_stiffness(self.stiffness, 1.0, None);
        }
        // hands down at his sides
        let pelvis = c.b.part(0).m.d;
        for i in 0..2 {
            if c.ch.busy[i] {
                continue;
            }
            let l = crate::nmshot::limb(i);
            let s = if i == 0 { 1.0 } else { -1.0 };
            let target = pelvis + side * (0.24 * s) - up * 0.06 + fwd * 0.03;
            let st = self.arm[i].unwrap_or_else(|| {
                let [k, a, e] = [0, 1, 2].map(|n| c.ch.actual(l.e[n], c.b.frames));
                [k[0], k[1], a[0], a[1], a[2], e[0]]
            });
            let (xs, _) = arm_solve(c, l, target, st);
            self.arm[i] = Some(xs);
            let blend = |d: &mut f32, to: f32| *d += (to - *d) * w;
            {
                let e0 = &mut c.ch.eff[l.e[0]].desired;
                blend(&mut e0[0], xs[0]);
                blend(&mut e0[1], xs[1]);
            }
            for k in 0..3 {
                blend(&mut c.ch.eff[l.e[1]].desired[k], xs[2 + k]);
            }
            blend(&mut c.ch.eff[l.e[2]].desired[0], xs[5]);
            c.ch.set_stiffness(self.stiffness * 0.8, 1.0, ["ul", "ur"][i], None);
        }
        // settle: the upper body's motion relative to the pelvis damped out
        if self.settle > 0.0 {
            let pv = c.b.part(0).v;
            for k in 7..21 {
                let q = c.b.part(k);
                c.out.force(k, -(q.v - pv) * c.b.build.parts[k].mass * self.settle * w);
            }
        }
    }

    fn done(&self) -> bool {
        self.t > self.blend + self.hold
    }
}

/// One message to start: its name and parameters (start=true among them).
pub type Start = (&'static str, Vec<(&'static str, crate::msg::Value)>);

/// The RDR2 reactions option, for a game or the playroom that wants them on its hits: which of the messages above a
/// hit starts, and how often. Chances are 0..1. Nothing calls this by itself; a caller that leaves the option off
/// gets Delirium's reactions unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reactions {
    /// every shot: shotSnap, the hit part jerked along the bullet and snapped back
    pub snap: bool,
    /// chance a shot in the trunk spins him round (shotSpin)
    pub spin: f32,
    /// chance a shot in the belly, pelvis or legs drops him to his knees (shotFallToKnees; not with a spin)
    pub knees: f32,
    /// where he is hit matters: shotFromBehind (the back), shotInGuts (the belly), shotInjuredLeg, shotInjuredArm
    pub hit_location: bool,
    /// chance a hand goes to the wound (reachForWound; not the head, neck, hands or feet; both hands for the belly)
    pub reach: f32,
    /// a melee blow: meleeReact instead of the usual flinch
    pub melee: bool,
    /// rdr2Balance on from the first hit: stiffer legs when running, feet set down flat
    pub balance: bool,
}

impl Default for Reactions {
    fn default() -> Reactions {
        Reactions { snap: true, spin: 0.25, knees: 0.15, hit_location: true, reach: 0.7, melee: true, balance: true }
    }
}

impl Reactions {
    /// What a bullet starts, sent after the shot itself: `part` hit at `at`, travelling along `dir`; `fwd` is the
    /// way he faces ((left clavicle - right clavicle) x up).
    pub fn on_shot(&self, rand: &mut delirium_data::rng::Rand, part: usize, at: Vec3, dir: Vec3, fwd: Vec3) -> Vec<Start> {
        use crate::msg::Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        let mut out: Vec<Start> = vec![];
        let mut go = |name, mut ps: Vec<_>| {
            ps.insert(0, ("start", B(true)));
            out.push((name, ps));
        };
        if self.balance {
            go("rdr2Balance", vec![]);
        }
        if self.snap {
            go("shotSnap", vec![("bulletDir", V(dir)), ("bodyPart", I(part as i32)), ("snapMag", F(rand.float_in_range(0.3, 0.5)))]);
        }
        // Delirium's parts: 1-3 / 4-6 the legs, 7-10 the spine (7-8 the belly), 13-16 / 17-20 the arms
        let (trunk, low) = ((7..=10).contains(&part), part <= 8);
        let spun = trunk && rand.float_in_range(0.0, 1.0) < self.spin;
        let mut knees = false;
        if spun {
            go("shotSpin", vec![("hitPoint", V(at)), ("bulletDir", V(dir))]);
        } else if low && rand.float_in_range(0.0, 1.0) < self.knees {
            knees = true;
            go("shotFallToKnees", vec![("ftkPitchForwards", B(rand.float_in_range(0.0, 1.0) < 0.6))]);
        }
        let mut gut = false;
        if self.hit_location {
            let (f, d) = (fwd.with_z(0.0).normalize_or_zero(), dir.with_z(0.0).normalize_or_zero());
            if (trunk || part == 0) && f.dot(d) > 0.4 {
                go("shotFromBehind", vec![("sfbSpineAmount", F(rand.float_in_range(0.4, 0.6))), ("sfbNeckAmount", F(rand.float_in_range(0.2, 0.4)))]);
            } else if part == 7 || part == 8 {
                gut = true;
                go("shotInGuts", vec![("sigSpineAmount", F(rand.float_in_range(0.45, 0.7))), ("sigPeriod", F(rand.float_in_range(1.0, 2.0)))]);
            } else if !knees && matches!(part, 1 | 2 | 4 | 5) {
                go("shotInjuredLeg", vec![("bodyPart", I(part as i32)), ("legInjuryTime", F(rand.float_in_range(1.0, 2.0)))]);
            } else if matches!(part, 14..=16 | 18..=20) {
                go("shotInjuredArm", vec![("bodyPart", I(part as i32)), ("bulletDir", V(dir))]);
            }
        }
        if !matches!(part, 3 | 6 | 11 | 12 | 16 | 20) && rand.float_in_range(0.0, 1.0) < self.reach {
            go("reachForWound", vec![("hitPoint", V(at)), ("hitPart", I(part as i32)), ("reachWithOneHand", B(!gut)),
                                     ("timeBeforeReachForWound", F(rand.float_in_range(0.1, 0.3))), ("grabHoldTime", F(rand.float_in_range(1.5, 3.0)))]);
        }
        out
    }

    /// What a melee blow starts (instead of a flinch): `part` hit at `at` by a blow along `dir`. Softer than the
    /// message's defaults, so a punch staggers him and rarely drops him.
    pub fn on_melee(&self, rand: &mut delirium_data::rng::Rand, part: usize, at: Vec3, dir: Vec3) -> Vec<Start> {
        use crate::msg::Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        if !self.melee {
            return vec![];
        }
        let mut out: Vec<Start> = vec![];
        if self.balance {
            out.push(("rdr2Balance", vec![("start", B(true))]));
        }
        out.push(("meleeReact", vec![("start", B(true)), ("hitPoint", V(at)), ("hitNormal", V(-dir.normalize_or_zero())), ("hitPart", I(part as i32)),
                                     ("Lean", F(rand.float_in_range(40.0, 55.0))), ("snapMag", F(rand.float_in_range(0.15, 0.22))), ("csLooseness", F(0.35))]));
        out
    }
}

/// How hard the hips leans push, m/s² per unit of leanAmount on his whole mass (Delirium's own).
const HIPS_LEAN: f32 = 4.0;
/// onFire's roll: torque per unit of rollTorqueScale per kg (Delirium's own).
const ROLL: f32 = 0.075;

/// What a lean moves: his balance, his hips, or a push on one part.
#[derive(Clone, Copy, PartialEq)]
enum LeanBy {
    Balance,
    Hips,
    Push(usize),
}

/// Which way a lean goes.
#[derive(Clone, Copy)]
enum LeanWay {
    Dir(Vec3),
    Pos(Vec3),
    Random { amount: [f32; 2], change: [f32; 2], goal: Vec3, now: Vec3, left: f32 },
}

/// The lean family (RDR2's, as GTA V's): leanRandom, hipsLean* and forceLean*, each in a direction, to a position,
/// towards a point or at random. The plain one leans him through the balancer, as leanInDirection does; the hips ones
/// push his pelvis that way and his chest the other, so his hips swing out over his feet; the force ones push bodyPart
/// that way, leanAmount times his weight, while he's on his feet. A negative leanAmount goes away. The random ones pick a new way (and an amount between leanAmountMin
/// and leanAmountMax) every changeTimeMin..changeTimeMax s. They run until stopped.
pub struct Lean {
    by: LeanBy,
    way: LeanWay,
    amount: f32,
}

impl Lean {
    fn new(name: &str, p: &mut P) -> Result<Lean, Error> {
        let by = if name.starts_with("hips") {
            LeanBy::Hips
        } else if name.starts_with("force") {
            LeanBy::Push(p.i("bodyPart", 0)?.clamp(0, 20) as usize)
        } else {
            LeanBy::Balance
        };
        if name.ends_with("Random") {
            let (lo, hi, t0, t1) = if by == LeanBy::Balance { (0.2, 0.2, 0.5, 1.0) } else { (0.3, 0.4, 2.0, 4.0) };
            let amount = [p.f("leanAmountMin", lo)?, p.f("leanAmountMax", hi)?];
            let change = [p.f("changeTimeMin", t0)?.max(0.05), p.f("changeTimeMax", t1)?.max(0.05)];
            return Ok(Lean { by, way: LeanWay::Random { amount, change, goal: Vec3::ZERO, now: Vec3::ZERO, left: 0.0 }, amount: 0.0 });
        }
        let amount = p.f("leanAmount", if by == LeanBy::Hips && !name.ends_with("ToPosition") { 0.5 } else { 0.2 })?;
        let way = if name.ends_with("InDirection") {
            LeanWay::Dir(p.v("dir", Vec3::ZERO)?)
        } else if name.ends_with("ToPosition") {
            LeanWay::Pos(p.v("pos", Vec3::ZERO)?)
        } else {
            let _ = (p.i("instanceIndex", -1)?, p.i("boundIndex", 0)?);
            LeanWay::Pos(p.v("offset", Vec3::ZERO)?)
        };
        Ok(Lean { by, way, amount })
    }
}

impl Extra for Lean {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let up = c.up;
        let flat = |v: Vec3| (v - up * v.dot(up)).normalize_or_zero();
        let (dir, amount) = match &mut self.way {
            LeanWay::Dir(d) => (flat(*d), self.amount),
            LeanWay::Pos(p) => (flat(*p - c.com), self.amount),
            LeanWay::Random { amount, change, goal, now, left } => {
                *left -= dt;
                if *left <= 0.0 {
                    let a = c.ch.random() * std::f32::consts::TAU;
                    let side = flat(up.any_orthonormal_vector());
                    let size = amount[0] + (amount[1] - amount[0]) * c.ch.random();
                    *goal = (side * a.cos() + up.cross(side) * a.sin()) * size;
                    *left = change[0] + (change[1] - change[0]) * c.ch.random();
                }
                // glide to the new lean over half the shortest change, so a reversal doesn't throw him over
                *now += (*goal - *now) * (dt / (change[0] * 0.5)).min(1.0);
                (now.normalize_or_zero(), now.length())
            }
        };
        // the pushes stop once he's down: they lean him, they don't drag him along the floor
        let up_now = c.b.part(0).m.d.dot(up) > 0.55 && !c.dbm_failed;
        if dir == Vec3::ZERO || (self.by != LeanBy::Balance && !up_now) {
            return;
        }
        match self.by {
            LeanBy::Balance => {
                let mode = match self.way {
                    LeanWay::Pos(p) => LeanMode::Pos(p),
                    _ => LeanMode::Dir(dir),
                };
                c.out.lean = Some((mode, amount, false));
            }
            LeanBy::Hips => {
                // the hips one way and the chest the other: the hips swing out over his feet without pushing him off
                let f = dir * amount * c.b.doll.mass() * HIPS_LEAN;
                c.out.force(0, f);
                c.out.force(10, -f);
            }
            LeanBy::Push(k) => c.out.force(k, dir * amount * c.b.doll.mass() * GRAVITY()),
        }
    }
}

/// RDR2's onFire: on his feet for staggerTime he staggers, his lean swinging forwards and back (up to
/// stumbleMaxLeanForward / stumbleMaxLeanBack, at staggerLeanRate) and against where he's heading (predictTime), his
/// arms flailing between a windmill and a writhe (armsWindmillWritheBlend), his spine and legs joining in by their
/// blends. Then (or once his balance goes) the balancer lets go; down, he writhes between curled up and thrashing (the
/// *PoseWritheBlend values) and, with rollOverFlag, rolls back and forth (rollTorqueScale) for maxRollOverTime while
/// within rollOverRadius of where he fell. It runs until stopped.
pub struct OnFire {
    stagger: f32, lean_rate: f32, back: f32, fwd: f32, up_blend: [f32; 3], down_blend: [f32; 3], roll_k: f32, predict: f32,
    roll: bool, roll_time: f32, radius: f32, t: f32, phase: f32, lean: f32, goal: f32, next: f32, down: Option<(f32, Vec3)>,
}

impl OnFire {
    fn new(p: &mut P) -> Result<OnFire, Error> {
        let u = |x: f32| x.clamp(0.0, 1.0);
        Ok(OnFire {
            stagger: p.f("staggerTime", 2.5)?.max(0.0), lean_rate: u(p.f("staggerLeanRate", 0.9)?),
            back: p.f("stumbleMaxLeanBack", 0.4)?.max(0.0), fwd: p.f("stumbleMaxLeanForward", 0.5)?.max(0.0),
            up_blend: [u(p.f("armsWindmillWritheBlend", 0.4)?), u(p.f("spineStumbleWritheBlend", 0.7)?), u(p.f("legsStumbleWritheBlend", 0.2)?)],
            down_blend: [u(p.f("armsPoseWritheBlend", 0.7)?), u(p.f("spinePoseWritheBlend", 0.55)?), u(p.f("legsPoseWritheBlend", 0.5)?)],
            roll_k: p.f("rollTorqueScale", 25.0)?.max(0.0), predict: p.f("predictTime", 0.1)?.max(0.0),
            roll: p.b("rollOverFlag", true)?, roll_time: p.f("maxRollOverTime", 8.0)?.max(0.0), radius: p.f("rollOverRadius", 2.0)?.max(0.0),
            t: 0.0, phase: 0.0, lean: 0.0, goal: 0.0, next: 0.0, down: None,
        })
    }
}

impl Extra for OnFire {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        use std::f32::consts::PI;
        self.t += dt;
        self.phase += dt * 2.0 * PI / 0.45;
        let (ph, up) = (self.phase, c.up);
        let pelvis = c.b.part(0).m.d;
        if self.down.is_none() && (self.t > self.stagger || c.dbm_failed || pelvis.dot(up) < 0.55) {
            self.down = Some((self.t, c.com));
        }
        let ground = self.down.is_some();
        if ground {
            c.out.dbm = Some(false);
            c.ch.set_stiffness(10.0, 1.0, "fb", None);
        }
        // arms: thrown about across most of their range, on his feet a windmill (or a writhe), down a writhe (or
        // curled in towards his face); the elbows bend and straighten as they go
        for i in 0..2 {
            let l = crate::nmshot::limb(i);
            let (sh, el) = (l.e[1], l.e[2]);
            let s = i as f32 * PI;
            let ed = knee_dir(c, el);
            let writhe = [0.85 * (1.7 * ph + s).sin(), 0.8 * (1.3 * ph + 2.0 * s + 0.5).sin(), 0.5 * (1.1 * ph + s).sin(), 0.3 + 0.6 * (2.3 * ph + s).sin()];
            // the windmill: big circles, arms near straight, the writhe laid over it rather than shrinking it
            let windmill = [(ph + s).sin(), 0.6 * (ph + s).cos(), 0.0, -0.6];
            let wu = self.up_blend[0] * 0.5;
            let stand: [f32; 4] = std::array::from_fn(|k| windmill[k] * (1.0 - wu) + writhe[k] * wu);
            let wd = self.down_blend[0];
            let lying: [f32; 4] = std::array::from_fn(|k| [0.5, 0.0, 0.0, 0.8][k] * (1.0 - wd) + writhe[k] * wd);
            // he keeps windmilling as he goes down, into the ground's writhe over a moment
            let g = self.down.map_or(0.0, |(t0, _)| ((self.t - t0 - 0.8) / 0.6).clamp(0.0, 1.0));
            let mix = |k: usize| stand[k] * (1.0 - g) + lying[k] * g;
            if g < 0.5 && self.up_blend[0] < 1.0 {
                // the windmill: the hand goes round a big circle in front of him, up past his head and down, the
                // arms half a turn apart; the writhe then shakes it about
                let at = c.b.part(l.p[0]).m.d;
                let clav = |i: usize| c.b.part(crate::nmshot::limb(i).p[0]).m.d;
                let face = (clav(0) - clav(1)).cross(up);
                let face = (face - up * face.dot(up)).normalize_or_zero();
                let a = self.t * 2.0 * PI / 0.8 + s;
                let to = at + face * (0.15 + 0.5 * a.cos()) + up * (0.6 * a.sin() + 0.1);
                for j in [l.e[0], sh, el] {
                    c.ch.eff[j].oppose_gravity = 1.0;
                }
                crate::nmdbm::limb_ik(c.b, c.ch, sh, el, l.p[0], l.p[delirium_data::kv!(0 int)], to, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
                for k in 0..3 {
                    c.ch.eff[sh].offset[k] += writhe[k] * wu * 0.6;
                }
            } else {
                for k in 0..3 {
                    drive(c, sh, k, mix(k));
                }
                drive(c, el, 0, mix(3) * ed);
            }
            c.ch.set_stiffness(15.0, 0.8, ["ul", "ur"][i], None);
        }
        // spine: hunched forward (more when down), and a writhe
        let w = if ground { self.down_blend[1] } else { self.up_blend[1] };
        let curl = if ground { 0.6 } else { 0.35 };
        let n = SPINE().len() as f32;
        for (k, &j) in SPINE().iter().enumerate() {
            let o = k as f32;
            let writhe = [0.5 * (1.3 * ph + o).sin(), 0.4 * (0.9 * ph + 1.0 + o).sin(), 0.6 * (1.1 * ph + o * 0.5).sin()];
            for a in 0..3 {
                c.ch.eff[j].offset[a] += ((if a == 0 { curl } else { 0.0 }) * (1.0 - w * 0.5) + writhe[a] * w) / n;
            }
        }
        // legs: the balancer's (on his feet) or drawn up (down), and a kicking writhe
        let w = if ground { self.down_blend[2] } else { self.up_blend[2] };
        for s in 0..2 {
            let (hip, knee) = (HIP()[s], KNEE()[s]);
            let kd = knee_dir(c, knee);
            let o = s as f32 * 2.0;
            let big = if ground { 1.0 } else { 0.35 };
            let writhe = [big * 0.9 * (1.9 * ph + o).sin(), big * 0.4 * (1.4 * ph + o + 1.0).sin(), big * (0.8 + 0.8 * (2.1 * ph + o).sin())];
            let posed = if ground { [1.0, 0.0, 1.6] } else { [0.0, 0.0, 0.0] };
            c.ch.eff[hip].offset[0] += posed[0] * (1.0 - w) + writhe[0] * w;
            c.ch.eff[hip].offset[1] += posed[1] * (1.0 - w) + writhe[1] * w;
            c.ch.eff[knee].offset[0] += (posed[2] * (1.0 - w) + writhe[2] * w) * kd;
        }
        match self.down {
            None => {
                // stagger: the lean swings between back and forward, and against where he's heading
                self.next -= dt;
                if self.next <= 0.0 {
                    self.goal = -self.back + (self.fwd + self.back) * c.ch.random();
                    self.next = 0.5 + 0.7 * c.ch.random();
                }
                self.lean += (self.goal - self.lean) * (self.lean_rate * 4.0 * dt).min(1.0);
                let clav = |i: usize| c.b.part(crate::nmshot::limb(i).p[0]).m.d;
                let face = (clav(0) - clav(1)).cross(up);
                let face = (face - up * face.dot(up)).normalize_or_zero();
                let v = face * self.lean - (c.vel - up * c.vel.dot(up)) * self.predict;
                if v.length() > 1e-3 {
                    c.out.lean = Some((LeanMode::Dir(v.normalize()), v.length(), false));
                }
            }
            Some((t0, at)) => {
                // down: roll back and forth along his length to put the flames out
                let since = self.t - t0;
                let d = c.com - at;
                if self.roll && since > 0.5 && since < self.roll_time && (d - up * d.dot(up)).length() < self.radius {
                    let along = c.b.part(12).m.d - pelvis;
                    let along = (along - up * along.dot(up)).normalize_or_zero();
                    let sign = if (since / 1.2) as i32 % 2 == 0 { 1.0 } else { -1.0 };
                    let tq = along * sign * self.roll_k * c.b.doll.mass() * ROLL;
                    c.out.torque(0, tq * 0.5);
                    c.out.torque(10, tq * 0.5);
                }
            }
        }
    }
}

/// RDR2's lassoed reactions (its yanked's, while dragged on a rope; Delirium's `dragged`, with ropedBodyPart, does the
/// pulling). By lassoedtype: round the neck (0), round the chest and arms (1: arms pinned to his sides, more wriggle),
/// round the feet (2: legs held straight, arms reaching past his head clawing at the ground). On his feet nothing;
/// down, round the neck his hands claw at the rope where it's round grabPart (grabposl / grabposr from
/// it, clawing about by grabpointamp at grabpointfreq), his legs kick (legAmplitude), his spine writhes forwards and
/// back (spinefreq, spineAmplitude) and side to side (spsidefreq, spsideamp), at the ground stiffnesses, and the ground
/// bumps him now and then (bumpsim, bumpsimprob). With stunnedtime he first lies limp that long, coming out of it over
/// stunnedblendout. It runs until stopped. Its defaults are RDR2's.
pub struct Lasso {
    kind: i32, part: i32, grab: [Vec3; 2], claw: Vec3, claw_freq: Vec3, legs: f32, spine: [f32; 4], stiff: [(f32, f32); 4],
    bump: (f32, f32), stunned: (f32, f32), t: f32, down: Option<f32>, bump_t: f32,
}

impl Lasso {
    fn new(p: &mut P) -> Result<Lasso, Error> {
        let _ = (p.f("groundFriction", 1.0)?, p.f("ropeendfriction", 0.5)?, p.f("otherendfriction", 2.0)?);
        let (leg_d, arm_d, spine_d) = (p.f("groundLegDamping", 0.5)?, p.f("groundArmDamping", 0.5)?, p.f("groundSpineDamping", 0.5)?);
        Ok(Lasso {
            kind: p.i("lassoedtype", 0)?.clamp(0, 2),
            part: p.i("grabPart", 11)?.clamp(-1, 20),
            grab: [p.v("grabposl", Vec3::new(0.07, 0.05, -0.051))?, p.v("grabposr", Vec3::new(0.09, -0.05, -0.051))?],
            claw: p.v("grabpointamp", Vec3::ZERO)?, claw_freq: p.v("grabpointfreq", Vec3::new(4.0, 5.0, 3.0))?,
            legs: p.f("legAmplitude", 0.4)?.max(0.0),
            spine: [p.f("spinefreq", 2.5)?.max(0.0), p.f("spineAmplitude", 1.0)?.max(0.0), p.f("spsidefreq", 2.0)?.max(0.0), p.f("spsideamp", 1.0)?.max(0.0)],
            stiff: [(p.f("groundLegStiffness", 11.0)?, leg_d), (p.f("groundArmStiffness", 11.0)?, arm_d),
                    (p.f("groundSpineStiffness", 11.0)?, spine_d), (p.f("groundneckstiffness", 11.0)?, spine_d)],
            bump: (p.f("bumpsim", 400.0)?.max(0.0), p.f("bumpsimprob", 0.08)?.clamp(0.0, 1.0)),
            stunned: (p.f("stunnedtime", 0.0)?.max(0.0), p.f("stunnedblendout", 0.0)?.max(0.0)),
            t: 0.0, down: None, bump_t: 0.0,
        })
    }
}

impl Extra for Lasso {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        use std::f32::consts::TAU;
        self.t += dt;
        let up = c.up;
        let pelvis = c.b.part(0).m.d;
        if self.down.is_none() && pelvis.dot(up) < 0.45 {
            self.down = Some(self.t);
        }
        let Some(t0) = self.down else { return };
        let since = self.t - t0;
        // stunned: limp, then coming out of it
        let (st, blend) = self.stunned;
        let w = if since < st { 0.0 } else if blend > 0.0 { ((since - st) / blend).min(1.0) } else { 1.0 };
        let scale = |k: f32| 1.0 + (k - 1.0) * w;
        for (mask, (k, d)) in ["ll", "lr"].iter().map(|m| (m, self.stiff[0])).chain(["ul", "ur"].iter().map(|m| (m, self.stiff[1])))
            .chain(std::iter::once((&"us", self.stiff[2]))).chain(std::iter::once((&"un", self.stiff[3]))) {
            c.ch.set_stiffness(scale(k), d, mask, None);
        }
        if w == 0.0 {
            return;
        }
        // the spine writhes, forwards and back and side to side
        let n = SPINE().len() as f32;
        let [f1, a1, f2, a2] = self.spine;
        // arms pinned, he wriggles side to side more
        let wriggle = if self.kind == 1 { 1.8 } else { 1.0 };
        let bend = 0.3 * a1 * (TAU * f1 * since).sin() * w;
        let side = 0.25 * a2 * wriggle * (TAU * f2 * since + 1.0).sin() * w;
        for &j in SPINE().iter() {
            c.ch.eff[j].offset[0] += bend / n;
            c.ch.eff[j].offset[1] += side / n;
        }
        // the legs kick, out of step; tied at the feet they're held together, straight
        for s in 0..2 {
            if self.kind == 2 {
                let knee = KNEE()[s];
                let kd = knee_dir(c, knee);
                drive(c, knee, 0, -0.9 * kd);
                continue;
            }
            let (hip, knee) = (HIP()[s], KNEE()[s]);
            let kd = knee_dir(c, knee);
            let ph = TAU * 1.8 * since + s as f32 * std::f32::consts::PI;
            c.ch.eff[hip].offset[0] += self.legs * 1.2 * ph.sin() * w;
            c.ch.eff[knee].offset[0] += self.legs * (1.0 + 1.5 * (ph + 0.6).sin()).max(0.0) * kd * w;
        }
        let clav = |i: usize| c.b.part(crate::nmshot::limb(i).p[0]).m.d;
        let along = (c.b.part(12).m.d - pelvis).normalize_or(up);
        let left = (clav(0) - clav(1)).normalize_or_zero();
        let reach = |c: &mut Ctx, i: usize, to: Vec3| {
            let l = crate::nmshot::limb(i);
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], to, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
        };
        if self.kind == 1 {
            // round his chest and arms: they're pinned to his sides
            for i in 0..2 {
                let side = if i == 0 { 1.0 } else { -1.0 };
                reach(c, i, pelvis + left * (0.22 * side) + along * 0.15);
            }
            c.ch.set_stiffness(14.0, 1.0, "ul", None);
            c.ch.set_stiffness(14.0, 1.0, "ur", None);
        } else if self.kind == 2 {
            // round his feet, dragged feet first: his arms reach out past his head, clawing at the ground in turn
            let ground = c.b.part(3).m.d.dot(up).min(c.b.part(6).m.d.dot(up));
            let head = c.b.part(12).m.d;
            for i in 0..2 {
                let side = if i == 0 { 1.0 } else { -1.0 };
                let to = head + along * (0.4 + 0.2 * (TAU * 1.2 * since + i as f32 * std::f32::consts::PI).sin()) + left * (0.22 * side);
                reach(c, i, to - up * (to.dot(up) - ground - 0.05));
            }
        }
        // round the neck: the hands claw at the rope round grabPart
        if self.kind == 0 && self.part >= 0 {
            let at = c.b.part(self.part as usize).m.d;
            // his own frame at the neck (lying, his up runs along his spine)
            let fwd = left.cross(along).normalize_or_zero();
            let claw = Vec3::new((TAU * self.claw_freq.x * since).sin(), (TAU * self.claw_freq.y * since).sin(), (TAU * self.claw_freq.z * since).sin()) * self.claw;
            for i in 0..2 {
                let g = self.grab[i] + claw;
                reach(c, i, at + fwd * g.x + left * g.y + along * g.z);
            }
        }
        // now and then the ground bumps him
        self.bump_t += dt;
        if self.bump.0 > 0.0 && self.bump_t >= 0.1 {
            self.bump_t -= 0.1;
            if c.ch.random() < self.bump.1 {
                let k = [0, 8, 10][(c.ch.random() * 2.99) as usize];
                c.out.force(k, up * self.bump.0 * w);
            }
        }
    }
}

/// RDR2's sit: riding, written from its parameters' names and how riders look (none of RDR2's code). He's swung up
/// into the seat over mountTime, then his pelvis is held there and his feet in the stirrups (hard constraints, which
/// the game moves with the horse by sending rdr2Sit again without start), his hips square to the seat, tipped by
/// hippitchstart, his back by straightnessstart, his hands forward on the reins. Thrown when his body leans past
/// failangle from the seat's up, when the seat itself tips past seatfailangle, when a hit moves his chest faster than
/// knockSpeed against his hips, or when sent throw=true: he's flung off (throwSpeed) and everything
/// lets go and he falls (catchFall), or with drag a foot stays caught and he's dragged by it for dragTime, reaching
/// back for it.
pub struct Sit {
    seat: Vec3, up: Vec3, fwd: Vec3, stirrups: [Vec3; 2], given: [bool; 4], mount: f32, constrain: bool, square: f32,
    feet: bool, stirrup_k: f32, soft: f32, hip: (f32, f32), straight: (f32, f32), fail: f32, seat_fail: f32, slump: bool,
    drag: (bool, f32, bool, f32, f32), t: f32, from: Option<[Vec3; 3]>, thrown: Option<(f32, Option<usize>)>,
    throw: bool, knock: f32, fling: (f32, Vec3), cons: Vec<(usize, Option<Vec3>, Option<bevy_math::Quat>)>, out: Vec<String>,
}

/// s after he's in the seat before leaning or the seat tipping can throw him.
const SETTLE: f32 = 0.6;
/// s he's flung over when thrown, and roughly what he weighs, kg.
const FLING: f32 = 0.15;
const FLUNG_MASS: f32 = 80.0;
/// His chest held upright at softweight 1, N m per radian.
const CHEST_HOLD: f32 = 900.0;
/// The pelvis and the feet.
const SIT_PARTS: [usize; 3] = [0, 3, 6];

impl Sit {
    fn new(p: &mut P) -> Result<Sit, Error> {
        let mut s = Sit {
            seat: Vec3::ZERO, up: Vec3::Z, fwd: Vec3::ZERO, stirrups: [Vec3::ZERO; 2], given: [false; 4],
            mount: p.f("mountTime", 0.6)?.max(0.0), constrain: p.b("constrain", true)?, square: p.f("constrainstrength", 40.0)?.max(0.0),
            feet: false, stirrup_k: 0.0, soft: p.f("softweight", 0.8)?.clamp(0.0, 1.0),
            hip: (p.f("hippitchstart", -0.5)?.clamp(-1.0, 1.0), p.f("hippitchramp", 0.25)?.max(0.0)),
            straight: (p.f("straightnessstart", -0.1)?.clamp(-1.0, 1.0), p.f("straightnessramp", 0.35)?.max(0.0)),
            fail: p.f("failangle", 0.71)?, seat_fail: p.f("seatfailangle", 0.71)?, slump: p.b("Slump", false)?,
            drag: (p.b("drag", false)?, p.f("dragTime", 10.0)?.max(0.0), p.b("dragreach", true)?, p.f("dragreachdelay", 0.2)?.max(0.0),
                   p.f("dragstiffness", 9.0)?),
            t: 0.0, from: None, thrown: None, throw: false, knock: p.f("knockSpeed", 4.0)?.max(0.0), fling: (p.f("throwSpeed", 3.0)?.max(0.0), Vec3::ZERO), cons: vec![], out: vec![],
        };
        let stirrup = p.f("constrainstrengthstirrup", 40.0)?;
        (s.feet, s.stirrup_k) = (stirrup > 0.0, stirrup * 10.0);
        for n in ["posefailangle", "standfailangle", "rolldelay", "rollspeedthreshold", "healthfaildelay"] {
            p.f(n, 0.0)?;
        }
        s.update(p)?;
        Ok(s)
    }

    fn seat_frame(&self) -> (Vec3, Vec3, Vec3) {
        let up = self.up.normalize_or(Vec3::Z);
        let fwd = (self.fwd - up * self.fwd.dot(up)).normalize_or(Vec3::X);
        (up, fwd, up.cross(fwd))
    }

    /// Where each foot goes: given, or under the seat to each side.
    fn stirrup(&self, i: usize) -> Vec3 {
        let (up, fwd, left) = self.seat_frame();
        if self.given[2 + i] { self.stirrups[i] } else { self.seat - up * 0.6 + left * (0.26 - 0.52 * i as f32) + fwd * 0.18 }
    }
}

impl Extra for Sit {
    fn update(&mut self, p: &mut P) -> Result<(), Error> {
        for (i, n) in ["seatPos", "seatForward", "stirrupL", "stirrupR"].iter().enumerate() {
            let v = p.v(n, Vec3::ZERO)?;
            if v != Vec3::ZERO {
                match i {
                    0 => self.seat = v,
                    1 => self.fwd = v,
                    _ => self.stirrups[i - 2] = v,
                }
                self.given[i] = true;
            }
        }
        self.up = p.v("seatUp", self.up)?;
        self.throw |= p.b("throw", false)?;
        Ok(())
    }

    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let at = |k: usize| c.b.part(k).m.d;
        let (pelvis, l_foot, r_foot) = (at(0), at(3), at(6));
        let from = *self.from.get_or_insert([pelvis, l_foot, r_foot]);
        if !self.given[0] {
            self.seat = from[0];
            self.given[0] = true;
        }
        if !self.given[1] {
            let left = (at(13) - at(17)).normalize_or(Vec3::Y);
            self.fwd = left.cross(c.up);
            self.given[1] = true;
        }
        let (up, fwd, left) = self.seat_frame();
        c.out.dbm = Some(false);
        if let Some((t0, caught)) = self.thrown {
            // thrown: let go, or dragged by a caught foot
            let since = self.t - t0;
            self.cons.clear();
            // flung off: pelvis and chest pushed for FLING s, enough to move his whole body at throwSpeed
            if since < FLING && self.fling.1 != Vec3::ZERO {
                let f = self.fling.1 * (self.fling.0 * FLUNG_MASS / FLING * 0.5);
                c.out.force(0, f);
                c.out.force(10, f);
            }
            if caught.is_some() && since >= self.drag.1 && since - dt < self.drag.1 {
                self.out.push("#say rdr2Sit: the foot came free".into());
                self.out.push("catchFall start=true".into());
            }
            if let Some(i) = caught.filter(|_| since < self.drag.1) {
                self.cons.push((SIT_PARTS[1 + i], Some(self.stirrup(i)), None));
                c.ch.set_stiffness(self.drag.4, 1.0, "fb", None);
                if self.drag.2 && since > self.drag.3 {
                    let foot = at(SIT_PARTS[1 + i]);
                    for a in 0..2 {
                        let l = crate::nmshot::limb(a);
                        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], foot, 0.0, 0.0, 1.0 - 2.0 * a as f32, 1.0, Vec3::ZERO, None);
                    }
                }
            }
            return;
        }
        // into the seat: swung up and over it
        let s = if self.mount > 0.0 { (self.t / self.mount).min(1.0) } else { 1.0 };
        let ease = s * s * (3.0 - 2.0 * s);
        let arc = up * 0.7 * (std::f32::consts::PI * s).sin();
        // fairly in the seat: swung in and settled (his upper body still swings as he lands)
        let seated = s >= 1.0 && self.t > self.mount + SETTLE;
        // thrown?
        let body = (at(12) - pelvis).normalize_or(up);
        let why = if self.throw {
            Some("thrown")
        } else if seated && up.dot(Vec3::Z) < self.seat_fail {
            Some("the seat tipped")
        } else if seated && !self.slump && body.dot(up) < self.fail {
            Some("leant too far")
        } else if seated && self.knock > 0.0 && (c.b.part(10).v - c.b.part(0).v).length() > self.knock {
            Some("knocked off")
        } else {
            None
        };
        if let Some(why) = why {
            // the foot that's dragged: the lower one
            let caught = self.drag.0.then(|| if l_foot.dot(Vec3::Z) < r_foot.dot(Vec3::Z) { 0 } else { 1 });
            self.thrown = Some((self.t, caught));
            let side = if (at(12) - pelvis).dot(left) >= 0.0 { left } else { -left };
            self.fling.1 = (side + up * 0.6 - fwd * 0.2).normalize();
            self.cons.clear();
            self.out.push(format!("#say rdr2Sit: {why}{}", if caught.is_some() { ", a foot caught" } else { "" }));
            if caught.is_none() {
                self.out.push("catchFall start=true".into());
            }
            return;
        }
        self.cons.clear();
        if self.constrain {
            self.cons.push((0, Some(from[0].lerp(self.seat, ease) + arc), None));
        }
        // the feet: springs into the stirrups (held hard, his legs levered him over)
        if self.feet {
            for i in 0..2 {
                let k = SIT_PARTS[1 + i];
                let to = from[1 + i].lerp(self.stirrup(i), ease) + arc;
                let f = (to - at(k)) * self.stirrup_k - c.b.part(k).v * 2.0 * self.stirrup_k.sqrt();
                c.out.force(k, f.clamp_length_max(self.stirrup_k * 0.5));
            }
        }
        // hips square to the seat, tipped by hippitchstart
        let w = |ramp: f32| if ramp > 0.0 { (self.t / ramp).min(1.0) } else { 1.0 };
        let pitch = self.hip.0 * 0.5 * w(self.hip.1);
        // (a turn about his left tips his top forward)
        let want_up = bevy_math::Quat::from_axis_angle(left, pitch) * up;
        let hips_up = (at(7) - pelvis).normalize_or(up);
        let hips_left = (at(1) - at(4)).normalize_or(left);
        let k = self.square * 4.0 * 3.0;
        let tq = hips_up.cross(want_up) * k + hips_left.cross(left) * k - c.b.part(0).w * 2.0 * (k * 3.0).sqrt();
        c.out.torque(0, tq);
        // his chest held upright over the hips, softly
        if !self.slump && self.soft > 0.0 {
            let lean = bevy_math::Quat::from_axis_angle(left, -self.straight.0 * 0.3 * w(self.straight.1));
            let chest = (at(10) - pelvis).normalize_or(up);
            let k = self.soft * CHEST_HOLD;
            c.out.torque(10, chest.cross(lean * want_up) * k - c.b.part(10).w * 2.0 * (k * 4.0).sqrt());
        }
        // his back
        let n = SPINE().len() as f32;
        for &j in SPINE().iter() {
            c.ch.eff[j].offset[0] += self.straight.0 * 0.4 * w(self.straight.1) / n;
        }
        if self.slump {
            c.ch.set_stiffness(6.0, 0.8, "us", None);
            c.ch.set_stiffness(3.0, 0.8, "ul", None);
            c.ch.set_stiffness(3.0, 0.8, "ur", None);
            return;
        }
        c.ch.set_stiffness(11.0, 1.0, "us", None);
        c.ch.set_stiffness(9.0, 1.0, "ll", None);
        c.ch.set_stiffness(9.0, 1.0, "lr", None);
        // hands forward on the reins
        for i in 0..2 {
            let l = crate::nmshot::limb(i);
            let to = self.seat + fwd * 0.38 + up * 0.32 + left * (0.1 - 0.2 * i as f32);
            crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], to, 0.0, 0.0, 1.0 - 2.0 * i as f32, 1.0, Vec3::ZERO, None);
        }
    }

    fn constraints(&self) -> Vec<(usize, Option<Vec3>, Option<bevy_math::Quat>)> {
        self.cons.clone()
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.out)
    }

    fn done(&self) -> bool {
        // (flung first)
        self.thrown.is_some_and(|(t0, caught)| self.t - t0 >= if caught.is_none() { FLING } else { self.drag.1 })
    }
}

/// RDR2's bolas, written from how it looks (its own parameters aren't known, so these are Delirium's): the bolas
/// wraps his legs, holding his ankles (and less, his knees) together; after fallTime his balance gives and he
/// topples, arms thrown out; down, he struggles, both legs kicking together, his arms pushing and his trunk
/// twisting, for struggleTime, then he's free (the bolas lets go).
pub struct Bolas {
    bind: f32, fall: f32, time: f32, struggle: f32, t: f32,
}

impl Bolas {
    fn new(p: &mut P) -> Result<Bolas, Error> {
        Ok(Bolas {
            bind: p.f("bindStrength", 900.0)?.max(0.0), fall: p.f("fallTime", 0.3)?.max(0.0), time: p.f("struggleTime", 5.0)?.max(0.0),
            struggle: p.f("struggle", 0.7)?.clamp(0.0, 1.0), t: 0.0,
        })
    }
}

impl Extra for Bolas {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        use std::f32::consts::TAU;
        self.t += dt;
        // the cords: ankles held together, knees less so
        for (a, b, k) in [(3, 6, 1.0), (2, 5, 0.5)] {
            let d = c.b.part(b).m.d - c.b.part(a).m.d;
            let f = (d * self.bind * k).clamp_length_max(self.bind);
            c.out.force(a, f);
            c.out.force(b, -f);
        }
        if self.t < self.fall {
            return;
        }
        c.out.dbm = Some(false);
        let up = c.up;
        let down = c.b.part(0).m.d.dot(up) < 0.45;
        let since = self.t - self.fall;
        if !down && since < 1.5 {
            // toppling: arms thrown out
            for i in 0..2 {
                let l = crate::nmshot::limb(i);
                drive(c, l.e[1], 0, 0.7);
                drive(c, l.e[1], 1, 0.6);
                c.ch.set_stiffness(11.0, 1.0, ["ul", "ur"][i], None);
            }
            return;
        }
        // down: struggling, the bound legs kicking together, arms pushing, trunk twisting
        let s = self.struggle;
        let ph = TAU * 1.1 * since;
        for side in 0..2 {
            let (hip, knee) = (HIP()[side], KNEE()[side]);
            let kd = knee_dir(c, knee);
            c.ch.eff[hip].offset[0] += 1.3 * s * ph.sin();
            c.ch.eff[knee].offset[0] += s * (0.9 + 0.9 * (ph + 0.8).sin()) * kd;
        }
        for i in 0..2 {
            let l = crate::nmshot::limb(i);
            let ed = knee_dir(c, l.e[2]);
            drive(c, l.e[1], 0, 0.9 * s * (ph * 0.9 + i as f32).sin());
            drive(c, l.e[1], 1, 0.6 * s * (ph * 0.7 + i as f32 * 1.5).sin());
            drive(c, l.e[2], 0, (0.2 + 0.7 * s * (ph * 1.3 + i as f32 * 2.0).sin()) * ed);
        }
        let n = SPINE().len() as f32;
        for &j in SPINE().iter() {
            c.ch.eff[j].offset[2] += 0.5 * s * (ph * 0.6).sin() / n;
            c.ch.eff[j].offset[1] += 0.3 * s * (ph * 0.8 + 1.0).sin() / n;
        }
        c.ch.set_stiffness(13.0, 0.8, "fb", None);
    }

    fn done(&self) -> bool {
        self.time > 0.0 && self.t > self.fall + self.time
    }
}
