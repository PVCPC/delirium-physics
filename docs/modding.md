# Using and scripting Delirium Physics

A practical guide: build the library, drive a character from your language of
choice, look up every message, and script the playroom. For the concepts behind
it, read [how-it-works.md](how-it-works.md) first.

Contents: [Build](#build) - [Quick start per language](#quick-start-per-language) -
[Message reference](#message-reference) - [Playroom scripting](#playroom-scripting) -
[Playroom commands](#playroom-commands) - [settings.txt](#settingstxt) -
[Tips and troubleshooting](#tips-and-troubleshooting)

## Build

You need a recent stable Rust toolchain.

```
cargo build --release -p delirium-sdk     # the shared library
cargo build --release -p delirium-room    # the playroom
```

The SDK produces `delirium_physics.dll` (Windows), `libdelirium_physics.so`
(Linux) or `libdelirium_physics.dylib` (macOS) in `target/release/`. The crates
are not published to crates.io; use path dependencies.

Files you will use:

| File | Purpose |
|---|---|
| `crates/delirium-sdk/include/delirium_physics.h` | C header (generated from the Rust source). |
| `crates/delirium-sdk/include/delirium_physics.hpp` | Header-only C++ wrapper, namespace `nm`. |
| `crates/delirium-sdk/bindings/python/delirium_physics.py` | Python (ctypes). |
| `crates/delirium-sdk/bindings/csharp/Delirium.cs` | C# (P/Invoke). |
| `crates/delirium-sdk/bindings/unity/` | Unity package `com.delirium_physics.bridge` (Unity 2022.3). |
| `crates/delirium-sdk/examples/` | Rust, C++ and Python examples. |

On Windows the header declares functions `dllimport` unless you define
`DELIRIUM_PHYSICS_STATIC`.

### What you feed it

- A **character**: either a character file (`.xrt`) plus a **build dump**
  (text), or one JSON file in the `nm-character-1` format. Load them with
  `nm_init` + `nm_build_load`, or `nm_init_character_file`.
- Optionally a **tuning file** (`nm_load_task_params`) for the shot, balance and
  flinch task helpers. Without it, defaults are used.
- **World geometry**: a flat floor, triangle meshes, and moving boxes.

The playroom writes a build dump when started with the environment variable
`NM_DUMP=path`.

### Conventions

- Z up, right-handed. Metres, kilograms, seconds. Rotations are quaternions in
  (x, y, z, w) order.
- 21 parts (0 pelvis, 10 chest, 12 head, 16 and 20 hands, 3 and 6 feet).
- Step with `dt` in (0, 0.25]; 1/60 is native.
- Functions return `0` or a count on success and a negative error code on
  failure: `NM_E_ARG` -1, `NM_E_IO` -2, `NM_E_PARSE` -3, `NM_E_MESSAGE` -4,
  `NM_E_PARAM` -5, `NM_E_TYPE` -6, `NM_E_STATE` -7, `NM_E_FOLLOW` -8.
  `nm_last_error(buf, len)` returns the text for the calling thread. Call it
  with `NULL, 0` to learn the needed size.

## Quick start per language

Every example does the same: load, make a floor, make a character, tell it to
balance, step one second, hit it in the chest, and watch.

### Rust (native engine)

Add path dependencies on `delirium` (and `delirium-data` if you read files).

```rust
use delirium::collision::Collision;
use delirium::msg::{self, Value};
use delirium::nm::Agent;
use delirium::ragdoll::{Build, Ragdoll};
use delirium::prelude::*;

fn main() {
    // build dump text -> Build + its rest frames
    let text = std::fs::read_to_string("build.txt").unwrap();
    let (build, frames) = Build::undump(&text);

    let mut world = Collision::default();
    world.add_floor(0.0, 0.0, 0.0, 50.0, 0);

    let doll = Ragdoll::new(&build, &frames, Vec3::ZERO);
    let mut agent = Agent::new(&build, doll, Quat::IDENTITY, 0.3);

    // send "bodyBalance start=true bodyStiffness=10"
    let mut st = std::mem::take(&mut agent.msgs);
    msg::send(&mut agent, &build, &world, &mut st, "bodyBalance",
              &[("start", Value::Bool(true)), ("bodyStiffness", Value::Float(10.0))]);
    agent.msgs = st;

    for _ in 0..60 {
        agent.step(&build, &world, 1.0 / 60.0, &[], &[]);
    }
    println!("balancing {} fallen {}", agent.balancing(), agent.fallen);
}
```

The last two arguments of `step` are moving obstacles and extra capsules (pass
empty slices if you have none). `agent.doll.frames()` gives part transforms and
`agent.doll.com()` the centre of mass. The character asset is loaded once
with `delirium::nmrs::load` or `nmrs::set`.

### Rust (through the C interface)

`crates/delirium-sdk/examples/bench.rs` links the library as an ordinary
dependency:

```rust
use delirium_physics::*;
use std::ffi::CString;

unsafe {
    let c = CString::new("character.xrt").unwrap();
    nm_init(c.as_ptr());
    let b = CString::new("build.txt").unwrap();
    let build = nm_build_load(b.as_ptr());
    let w = nm_world_create();
    nm_world_add_floor(w, 0.0, 0.0, 0.0, 50.0);
    let g = nm_agent_create(build, [0.0f32; 3].as_ptr(), 0.0, 0.3);

    nm_msg_begin(g, c"bodyBalance".as_ptr());
    nm_msg_set_bool(g, c"start".as_ptr(), 1);
    nm_msg_send(g, w);
    for _ in 0..60 { nm_agent_step(g, w, 1.0 / 60.0); }
}
```

### C++

Header-only wrapper over the C API. Errors throw `nm::Error` (it carries the
code and `what()`).

```cpp
#include "delirium_physics.hpp"

int main() {
    nm::init("character.xrt");
    nm::Build build("build.txt");
    nm::World world;
    world.add_floor(0, 0, 0, 50);
    nm::Agent agent(build, {0, 0, 0}, 0.f);

    agent.message("bodyBalance").set("start", true).set("bodyStiffness", 10.f).send(world);
    for (int i = 0; i < 60; i++) agent.step(world);

    auto [chest, q] = agent.transform(10);      // position of part 10
    agent.push(chest, {0, 40, 0});              // momentum into the body
    agent.shot(world, "POWERFUL_WEAPON", 9, chest, {0, 1, 0});  // the reaction
    for (int i = 0; i < 90; i++) agent.step(world);
    bool up = agent.balancing(), down = agent.fallen();
}
```

Also in the wrapper: `Build::fit`, `World::add_mesh`, `World::set_boxes`,
`nm::set_surface`, `Agent::set_mode`, `Agent::set_state`, `Agent::drive`,
`nm::load_task_params`.

### Python

Set `DELIRIUM_PHYSICS_LIB` to the folder (or file) holding the library. The
module otherwise looks beside itself, then in the system loader path.

```python
import ctypes
from delirium_physics import lib, vec, text, check

check(lib.nm_init(b"character.xrt"))
build = lib.nm_build_load(b"build.txt")
world = lib.nm_world_create()
lib.nm_world_add_floor(world, 0, 0, 0, 50)
agent = lib.nm_agent_create(build, vec(0, 0, 0), 0.0, 0.3)

check(lib.nm_msg_begin(agent, b"bodyBalance"))
check(lib.nm_msg_set_bool(agent, b"start", 1))
check(lib.nm_msg_set_float(agent, b"bodyStiffness", 10.0))
check(lib.nm_msg_send(agent, world))

for _ in range(60):
    check(lib.nm_agent_step(agent, world, 1 / 60))

p = (ctypes.c_float * 3)()
lib.nm_agent_get_part_transform(agent, 10, p, None)   # chest
lib.nm_agent_apply_push(agent, p, vec(0, 40, 0))
lib.nm_agent_task_shot(agent, world, b"POWERFUL_WEAPON", 10, p, vec(0, 1, 0), 200.0)
for _ in range(90):
    lib.nm_agent_step(agent, world, 1 / 60)

for i in range(lib.nm_agent_feedback_count(agent)):
    buf = ctypes.create_string_buffer(64)
    lib.nm_agent_feedback_get(agent, i, buf, 64, None, None)
    print(buf.value.decode())
print("balancing", lib.nm_agent_is_balancing(agent), "fallen", lib.nm_agent_has_fallen(agent))
```

`examples/bullet_follow.py` and `examples/mujoco_follow.py` show FOLLOW mode
with PyBullet and MuJoCo: those engines own the body and Delirium supplies
forces.

### C#

Put `delirium_physics.dll` next to your executable (or on `PATH`). Add
`Delirium.cs` to the project; the sample project targets .NET 8, x64.

```csharp
using Delirium;
using static Delirium.Native;

nm_init("character.xrt");
var build = nm_build_load("build.txt");
var world = nm_world_create();
nm_world_add_floor(world, 0, 0, 0, 50);
var agent = nm_agent_create(build, new float[] { 0, 0, 0 }, 0f, 0.3f);

nm_msg_begin(agent, "bodyBalance");
nm_msg_set_bool(agent, "start", 1);
nm_msg_set_float(agent, "bodyStiffness", 10f);
nm_msg_send(agent, world);

for (int i = 0; i < 60; i++) nm_agent_step(agent, world, 1f / 60);

var p = new float[3];
nm_agent_get_part_transform(agent, 0, p, null);   // pelvis
Console.WriteLine($"pelvis {p[0]:F2} {p[1]:F2} {p[2]:F2}");
// on failure: Delirium.Native.LastError()
```

### Unity

1. Build the SDK library and copy it where Unity loads native plugins (for
   example `Assets/Plugins/x86_64/`).
2. In the Package Manager choose "Add package from disk" and pick
   `crates/delirium-sdk/bindings/unity/package.json`.
3. Add the `NmCharacter` component to a GameObject. Fields:
   - `xrtPath` and `buildPath`: the character file and build dump.
   - `taskParamsPath`: optional tuning file.
   - `mode`: `Lead` (Delirium simulates, you copy poses to bones) or `Follow`
     (Unity rigidbodies are the body, Delirium adds forces).
   - `balanceOnStart`: start `bodyBalance` automatically.
   - `bones`: the 21 transforms to pose, in part order.
   - `fitToSkeleton`: fit the body to your humanoid `Animator` in its bind pose
     (uses `nm_build_fit`).
4. Run Unity physics at 60 Hz (`Time.fixedDeltaTime = 1f / 60`).

Send messages and hits from your own script:

```csharp
var nm = GetComponent<NmCharacter>();
nm.Send("bodyBalance", ("start", true), ("bodyStiffness", 10f));
nm.Shoot(hitPoint, direction);             // push + shot reaction
if (nm.Fallen) { /* play your own stand-up */ }
```

Delirium works in Z-up right-handed space and Unity in Y-up left-handed. The
component converts for you: points swap y and z; rotations (x, y, z, w) become
(-x, -z, -y, w); angular velocities and torques swap and negate. If you call
`nm_*` directly, convert the same way. In Follow mode the component builds 21
capsule rigidbodies with `ConfigurableJoint`s from `nm_agent_get_joint`; parts
collide with the world but not with each other. Level geometry is added with
`nm_world_add_mesh`.

### Driving another physics engine (FOLLOW mode)

```
 each 1/60 s:
   nm_agent_set_part_states(agent, states, 21)   // 13 floats per part:
                                                 // pos3 rot4(xyzw) vel3 angvel3
   nm_agent_step(agent, world, 1/60)
   nm_agent_get_part_drives(agent, drives, 21)   // 6 floats per part:
                                                 // force3 (N, at part centre) torque3 (Nm)
   apply drives as external force/torque in your engine, then step it
```

Switch with `nm_agent_set_mode(agent, NM_MODE_FOLLOW)` before the first step.
Do not add gravity compensation: gravity is not included in the drive.

### Reading and hitting a character (any language)

| Need | Calls |
|---|---|
| Part pose | `nm_agent_get_part_transform`, `nm_agent_part_count` |
| State flags | `nm_agent_is_balancing`, `has_fallen`, `is_settled`, `is_relaxing`, `is_catching_fall`, `is_rolled_up`, `is_awake` |
| Body stats | `nm_agent_get_com`, `get_mass`, `get_kinetic_energy`, `get_stance` |
| Push | `nm_agent_apply_impulse(a, part, at, impulse)`, `nm_agent_apply_push(a, at, impulse)` (nearest part), `nm_agent_add_velocity` |
| Find a part | `nm_agent_nearest_part`, `nm_agent_raycast` |
| Reactions | `nm_agent_task_shot`, `task_balance`, `task_catch_fall`, `task_flinch` |
| RDR2 reactions (optional) | `nm_agent_rdr2_shot(a, w, part, at, dir)` after `nm_agent_task_shot`; `nm_agent_rdr2_melee(a, w, part, at, dir)` instead of a flinch; `nm_rdr2_set(name, value)` to tune them |
| Animation pose | `nm_agent_set_incoming_transforms(a, transforms, parts)` (7 floats per part; `parts = 0` clears), used by `activePose` and `incomingTransforms` |
| Events | `nm_agent_feedback_count`, `nm_agent_feedback_get` |
| World | `nm_world_add_floor`, `nm_world_add_mesh`, `nm_world_set_boxes`, `nm_world_raycast`, `nm_set_surface(material, friction, elasticity)` |

`nm_world_set_boxes` takes 13 floats per box (centre, quaternion, half extents,
velocity) for cars and platforms; set them again before every step. The
box's index in the array is its `instanceIndex`.

## Message reference

Send a message with `begin`, any number of `set_*`, then `send`. Use
`start=true` to turn a behaviour on, `start=false` to turn it off, and no
`start` to just change parameters of a running one. Parameters not listed here
exist; use `nm_message_param_*` (or `msg ?` in the playroom) to list them all
with defaults and docs.

Vectors are `x,y,z`. "mask" is a body-part string (see
[how-it-works.md](how-it-works.md#behaviours-and-messages)).

### Core messages

| Message | What it does | Key parameters |
|---|---|---|
| `bodyBalance` | Stand, step, recover from pushes. | `bodyStiffness`, `damping`, `elbow`, `armsOutOnPush`, `headLookPos`, `useHeadLook`, `useBodyTurn`, `maxSteps`, `somersaultAngle` |
| `configureBalance` | Tune the balancer. | `legStiffness`, `stepHeight`, `stepClampScale`, `balanceAbortThreshold`, `taperKneeStrength` |
| `configureBalanceReset` | Reset balancer tuning. | none |
| `stayUpright` | Weak uprightness hold. | `stiffness` (3), `dampingScale` (0.5) |
| `setStiffness` | One-shot body stiffness. | `bodyStiffness` (12), `damping` (1), `mask` ("fb") |
| `bodyRelax` | One-shot go limp. | `mask`, `holdPose`, `relaxation` (50), `damping` |
| `shotRelax` | One-shot relax after a hit. | `relaxPeriod` (2) |
| `shot` | Bullet or melee reaction. | `bodyPart`, `hitPoint`, `normal`, `bulletVel`, `bodyStiffness`, `reachForWound`, `useHeadLook`, `fling`, `chickenArms`, `crouching`, `melee`, `loosenessAmount`, `useCatchFallOnFall` |
| `upperBodyFlinch` | Cover up, turn away. | `pos`, `newHit`, `bodyStiffness`, `bodyDamping`, `backBendAmount`, `useLeftArm`, `useRightArm`, `turnTowards` |
| `catchFall` | Arms out to break a fall. | `armsStiffness`, `torsoStiffness`, `legsStiffness`, `useHeadLook`, `zAxisSpinReduction` |
| `highFall` | Long fall, spread out, land. | `bodyStiffness`, `bodydamping`, `armAmplitude`, `armPeriod`, `armsUp`, `catchfalltime`, `crashOrLandCutOff`, `fowardRoll` |
| `rollDownStairs` | Tumble down a slope. | `stiffness`, `damping`, `forcemag`, `asymmetry`, `useCustomRollDir`, `customRollDir`, `spinWhenInAir` |
| `braceForImpact` | Brace against an approaching object. | `bodyStiffness`, `pos`, `look`, `braceDistance`, `instanceIndex`, `grabStrength`, `legStiffness` |
| `fallOverWall` | Tip over a low wall. | `fallOverPoint`, `fallOverEdge`, `bodyStiffness`, `damping`, `magOfForce`, `bodyTwist` |
| `grab` | Grab a ledge, rail or box. | `instanceIndex` (-1 = world), `pos1`..`pos4`, `normalL`, `normalR`, `useLeft`, `useRight`, `grabStrength`, `pullUpTime`, `grabHoldMaxTimer` |
| `headLook` | Look at a point. | `pos`, `vel`, `stiffness` (10), `damping` (1), `alwaysLook` |
| `pointArm` | Point an arm. | `target`, `useLeftArm`, `twist`, `armStraightness`, `armStiffness` |
| `leanInDirection` | One-shot lean. | `leanAmount` (0.2), `applyAsForce`, `dir` |
| `leanToPosition` | One-shot lean to a point. | `leanAmount`, `applyAsForce`, `pos` |
| `leanTowardsObject` | One-shot lean to an offset. | `leanAmount`, `applyAsForce`, `offset` |
| `applyImpulse` | One-shot push on a part. | `partIndex`, `hitPoint`, `impulse`, `equalizeAmount` |
| `bodyFoetal` | Curl up. | `mask`, `stiffness`, `asymmetry`, `randomSeed`, `dampingFactor`, `backTwist` |
| `bodyRollUp` | Roll into a ball. | `mask`, `stiffness`, `legPush`, `armReachAmount`, `useArmToSlowDown` |
| `bodyWrithe` | Thrash on the ground. | `armStiffness`, `legStiffness`, `backStiffness`, `*Period`, `*Amplitude`, `rollOverFlag` |
| `armsWindmillAdaptive` | Flail arms to regain balance. | `bodyStiffness`, `amplitude`, `phase`, `period`, `armStiffness`, `disableOnImpact` |
| `pedalLegs` | Pedal the legs (cycling or in the air). | `legStiffness`, `pedalRightLeg`, `pedalLeftLeg`, `radius`, `angularSpeed`, `backPedal` |
| `activePose` | One-shot: follow the supplied pose. | `mask` |
| `incomingTransforms` | Toggle using the supplied animation pose. | `start` only |
| `defineAttachedObject` | Tell the body it holds a mass. | `partIndex`, `worldPos`, `objectMass` |
| `stopAllBehaviours` | Clear everything. | none |

### Extra messages: general

| Message | What it does | Key parameters |
|---|---|---|
| `electrocute` | Twitching jolts. | `stunMag` (0.25), `stunInterval` (0.07), `largeMinTime`, `largeMaxTime`, `directionRandomness`, per-limb bools, `applyStiffness`, `useTorques` |
| `hardConstraint` | Hold chosen parts on the supplied pose. | `partIndex` ("16,20"), `constrainOrientation`, `constrainTranslation`, `blendWithPreviousFrame`, `stabiliseHead` |
| `injury` | One-shot: weaken a limb. | `part`, `mask` (e.g. "ur"), `amount` 0 to 1 (0 heals) |

### Extra messages: falling and staggering

| Message | What it does | Key parameters |
|---|---|---|
| `teeter` | Wobble at an edge. | `edgeLeft`, `edgeRight`, `reactDistance` (0.5), `restartDistance` (0.8), `teeterTime` (2), `callHighFall` |
| `stumble` | Stagger forward with arms braced. | `legsStiffness`, `torsoStiffness`, `armsStiffness`, `leanTowards`, `staggerTime` (1.5), `useArmsBrace` |
| `staggerFall` | Stagger then fall. | arm and spine stiffness and ramp values, `staggerStepProb`, `pushOffDist`, `useBodyTurn` |
| `smartFall` | Choose a fall reaction in the air and on landing. | `bodyStiffness` (10), `bodydamping`, `catchfalltime`, `crashOrLandCutOff`, `minSpeedForBrace`, `maxSpeedForRecoverableFall`, `landingNormal`, `forwardRoll` |
| `balancerCollisionsReaction` | React to an obstacle behind. | `objectBehindVictimPos`, `objectBehindVictimNormal`, `numStepsTillSlump`, `reactTime` |
| `injuredOnGround` | Hurt on the floor. | `numInjuries` (0 to 2), `injury1LocalPosition`, `injury2LocalPosition`, `attackerPos`, `strongRollForce` |
| `learnedCrawl` | Crawl. | `stiffness`, `damping`, `speed` (0.8), `targetPosition`, `yawOffset` |

### Extra messages: held, carried, posed

| Message | What it does | Key parameters |
|---|---|---|
| `yanked` | Pulled by a point. | `point`, `part` (10), `force`, `pull`, `lowerBodyStiffness` |
| `dragged` | Dragged on a rope. | `ropePos`, `ropedBodyPart`, `grabLeft`, `grabRight`, `reach`, `force` |
| `dangle` | Hang from a point. | `doGrab`, `grabFrequency`, `point`, `hold` |
| `carried` | Being carried. | `point`, `hold`, trunk, leg, arm, clavicle and neck stiffness |
| `buoyancy` | Float in water. | `buoyancy` (1.1), `chestBuoyancy`, `damping`, `surfacePoint`, `surfaceNormal`, `righting` |
| `animPose` | Hold a pose given in text. | `pose` ("joint:a,b,c;..." radians), `mask`, `stiffness`, `damping` |
| `armsWindmill` | Windmill the arms. | per-arm `Centre`, `Radius`, `Speed`, `phaseOffset`, stiffness, `mirrorMode`, `useLeft`, `useRight` |
| `spineTwist` | Twist the spine toward a point. | `pos`, `offset`, `vel`, `spStrength`, `spDamping`, `twistClavicles` |
| `wave` | Wave a hand; ends by itself. | `right` (true), `time` (2.25 s), `rate` (2), `size` (0.18 m) |
| `breathe` | Breathing motion. | `stiffness`, `effectorMask`, `breathingScale` |
| `softKeyframe` | Soft follow of a target. | `followMultiplier` (10), `maxAcceleration` (20), `mask`, `yawOffset`, `targetPosition` |
| `splitBody` | Aim and fire a held weapon. | `aimTarget`, masks, `stiffness`, `controllerStiffness`, `fireWeapon` (alone = one shot) |
| `shootDodge` | Dodge a shot. | `rightArmTarget`, `leftArmTarget`, `collisionReaction` (0 catch fall, 1 roll up, 2 balance, 3 nothing) |
| `armHang` | Hang by the arms. | `constraintPosition`, `targetPosition`, `instanceIndex`, `boundIndex` |
| `crushReaction` | React to a crushing object. | `effectorMask` ("ub"), `stiffness`, `obstacleID`, `flinchMode` (0 cover head, 1 brace), `obstaclePosition` |

### Extra messages: from Red Dead Redemption 2

Written from how RDR2's Euphoria behaves (none of its code). The parameter names are RDR2's, and some defaults are
RDR2's own default values; the rest are Delirium's own, and any of them can be sent as parameters. Like every extra
they do nothing until sent with `start=true`, so the existing behaviours are unchanged.

| Message | What it does | Key parameters |
|---|---|---|
| `duck` | Short flinch away: after `timeBeforeDuck` the spine and neck bend into a random share of their remaining range, each at its own rate, and the legs crouch through the balancer, for `duckTime`; it fades out over `blendTime`. | `magMin`, `magMax`, `lsRate`, `usRate`, `headRate`, `spineTwistRate`, `neckTwistRate`, `duckTime`, `blendTime`, `bendForward`, `duckLeft`, `leanTwistStart`, `crouch` |
| `shotSpin` | A hit off his centre spins him round: a damped yaw torque pulls him onto a heading that swings round by the hit's spin and holds it, the balancer told to face it; then, with `stepTurnType=1`, he turns to face `attackerPos`. The feet stay planted, so after the hold he eases part way back. | `spin` (or `hitPoint` + `bulletDir`), `shotSpinTwistMult`, `shotSpinTwist`, `shotSpinTime`, `shotSpinSettle`, `shotSpinTorque`, `shotSpinTorqueMax`, `shotSpinTwistVel`, `stepTurnType`, `stepTurnWait`, `attackerPos` |
| `shotFallToKnees` | Sinks to his knees: after `ftkBalanceTime` the balancer lets go, knees and hips bend at `ftkBendRate`; once the knees are down he kneels upright for `ftkKneelTime`, his centre held over his knees; then he topples forwards (or back) and goes loose. | `ftkBalanceTime`, `ftkBendRate`, `ftkKneelTime`, `ftkKneelHip`, `ftkLeanHelp`, `ftkHelperForce`, `ftkSpineBend`, `ftkStiffness`, `ftkOnKneesArmType`, `ftkPitchForwards`, `ftkPitchForce` |
| `meleeReact` | A punch or melee blow: the upper body goes loose for `csPeriod`, a push leans him away, he flinches (hands up to guard his face with `armsType=1`), the hit part snaps along the blow, then a hand reaches for the spot hit. | `hitPoint`, `hitNormal`, `hitPart`, `csLooseness`, `csPeriod`, `Lean`, `leanPeriod`, `Flinch`, `flinchPeriod`, `armsType`, `snap`, `snapMag`, `rfw`, `rfwDelay`, `rfwPeriod` |
| `shotSnap` | A sharp kick across the chosen joints along the bullet's path, then a smaller counter-kick back (send it with `shot`). Sized by his state (moving, standing, airborne) and per joint; the hit part's joints get their own kick. | `snapMag`, `snapDuration`, `unSnapInterval`, `unSnapRatio`, `bulletDir`, `bodyPart`, `snapHitPart`, `snapSpine`, `snapNeck`, `snapLeftArm`, `snapRightArm`, `snapLeftLeg`, `snapRightLeg`, `snapHipType`, `snapPhasedLegs`, `snapMovingMult`, `snapBalancingMult`, `snapAirborneMult` |
| `shotFromBehind` | Hit in the back: the spine arches back, the neck goes back, the hips push forward and the knees give a little; it swells and goes over `sfbPeriod`. | `sfbSpineAmount` (0.5), `sfbNeckAmount` (0.3), `sfbHipAmount` (0.25), `sfbKneeAmount` (0.15), `sfbPeriod` (0.7), `sfbArmsOnset`, `sfbKneesOnset`, `sfbNoiseGain`, `stiffness` |
| `shotInGuts` | Hit in the belly: he hunches forward, knees and hips bent, for `sigPeriod`, then eases off over `sigBlend`. | `sigSpineAmount` (0.6), `sigNeckAmount` (0.25), `sigKneeAmount` (0.15), `sigPeriod` (1.5), `sigBlend` (0.5), `stiffness` |
| `shotInjuredLeg` | Hit in the leg: it jerks up, then stays weak for `legInjuryTime` while he sags and leans onto it. | `bodyPart` (or `leftLeg`), `legLiftTime`, `legInjuryLift`, `legInjuryTime` (1.5), `legInjury` (0.4), `legInjuryHipPitch`, `legInjurySpineBend`, `legLimpBend`, `legLean`, `legSag`, `legBlend`, `stiffness` |
| `shotInjuredArm` | Hit in the arm: the trunk turns with the bullet and the hit shoulder drops, then the arm hangs loose for `armLooseTime`. | `bodyPart` (or `leftArm`), `bulletDir`, `injuredArmTime`, `hipYaw` (80), `hipRoll` (30), `armLoose` (0.7), `armLooseTime` (1), `stiffness` |
| `reachForWound` | A hand goes to the wound and holds it for `grabHoldTime` (both hands for a trunk wound with `reachWithOneHand=false`); for a leg wound he bends down to it. | `hitPoint`, `hitPart` (-1: the belly), `timeBeforeReachForWound` (0.2), `grabHoldTime` (2), `reachWithOneHand`, `allowInjuredThighReach`, `allowInjuredLowerLegReach`, `armStiffness`, `bend2spine`, `bend2hip`, `bend2leg`, `spineTwistMag`, `bendTimeUp`, `bendTimeDown`, `spineStiffness` |
| `wallCollision` | Run into a wall: a snap away from it and weak legs at the impact; his back to it: he slumps down it; facing it: both hands brace on it, then he pushes off; side on: a spin away. Can make the wall solid for his body (a plane, or `wallHalfWidth` wide). | `objectBehindVictimPos`, `objectBehindVictimNormal` (zero: nothing happens), `solidWall` (true), `wallStiffness`, `wallHalfWidth`, `slumpMode` (0 back meets it, 1 always, 2 never), `braceWall`, `braceDistance`, `braceArmStiffness`, `reactTime`, `impactLegStiffReduction`, `snapMag`, `sitStartTime`, `sitRate`, `glanceSpinMag`, `glanceSpinTime`, `timeToForward`, `reboundForce`, `stiffness` |
| `setFallingReaction` | Down onto hands and knees: the balancer lets go, he folds and puts his hands to the ground; once hands and a knee are down (or after 1.5 s) he holds there on all fours, head up, for `hkHoldTime`, then his strength fades and he sinks onto his front. | `handsAndKnees` (true), `hkHoldTime` (1.5), `stoppedStrengthDecay` (1), `minArmStiffness`, `minLegStiffness`, `minSpineStiffness`, `hkHeadAvoid`, `pushForward` (0) |
| `rdr2Balance` | Balancer extras while it runs: stiffer legs when he moves faster than `runningThresh`, and a foot stepping up onto something only a little higher is turned flat on it. Runs until stopped (`start=false`). | `runningThresh` (0.65), `legStiffnessRun` (13; 0 off), `startFlat` (0.1), `endFlat` (0.3), `flatStiffness` (12; 0 off) |
| `leanRandom` | Leans his balance a random way (as `leanInDirection`), gliding to a new one every `changeTimeMin`..`changeTimeMax` s. Runs until stopped. | `leanAmountMin` (0.2), `leanAmountMax` (0.2), `changeTimeMin` (0.5), `changeTimeMax` (1) |
| `hipsLeanInDirection`, `hipsLeanToPosition`, `hipsLeanTowardsObject`, `hipsLeanRandom` | His hips swing out that way (the pelvis pushed one way, the chest the other) while his feet stay under him. Runs until stopped. | `leanAmount` (0.5; 0.2 to a position), `dir` / `pos` / `offset`, or for random `leanAmountMin` (0.3), `leanAmountMax` (0.4), `changeTimeMin` (2), `changeTimeMax` (4) |
| `forceLeanInDirection`, `forceLeanToPosition`, `forceLeanTowardsObject`, `forceLeanRandom` | A push on `bodyPart` that way, `leanAmount` times his weight, while he's on his feet. Runs until stopped. | `leanAmount` (0.2), `bodyPart` (0), `dir` / `pos` / `offset`, or for random `leanAmountMin` (0.3), `leanAmountMax` (0.4), `changeTimeMin` (2), `changeTimeMax` (4) |
| `onFire` | On fire: for `staggerTime` he staggers, his lean swinging forwards and back (up to `stumbleMaxLeanForward` / `stumbleMaxLeanBack`) against where he's heading, his arms windmilling in big circles (shaken by a writhe, `armsWindmillWritheBlend`), still windmilling as he goes down; then the balancer lets go and, down, he writhes between curled up and thrashing and, with `rollOverFlag`, rolls back and forth for `maxRollOverTime` within `rollOverRadius` of where he fell. Runs until stopped. Its defaults are RDR2's. | `staggerTime` (2.5), `staggerLeanRate` (0.9), `stumbleMaxLeanBack` (0.4), `stumbleMaxLeanForward` (0.5), `armsWindmillWritheBlend` (0.4), `spineStumbleWritheBlend` (0.7), `legsStumbleWritheBlend` (0.2), `armsPoseWritheBlend` (0.7), `spinePoseWritheBlend` (0.55), `legsPoseWritheBlend` (0.5), `rollTorqueScale` (25), `predictTime` (0.1), `rollOverFlag` (true), `maxRollOverTime` (8), `rollOverRadius` (2) |
| `rdr2Lasso` | Lassoed and dragged (send it with `dragged`, which does the pulling, its `ropedBodyPart` where the rope is: 11 the neck, 10 the chest, 3 or 6 a foot). By `lassoedtype`: 0 round the neck, his hands claw at the rope where it's round `grabPart`; 1 round the chest and arms, his arms pinned to his sides, wriggling more; 2 round the feet, his legs held straight and his arms reaching past his head, clawing at the ground. Down, his legs kick (not tied at the feet), his spine writhes forwards and back and side to side, and the ground bumps him now and then; with `stunnedtime` he first lies limp. Runs until stopped. Its defaults are RDR2's. | `lassoedtype` (0), `grabPart` (11), `grabposl`, `grabposr`, `grabpointamp` (zero), `grabpointfreq` (4,5,3), `legAmplitude` (0.4), `spinefreq` (2.5), `spineAmplitude` (1), `spsidefreq` (2), `spsideamp` (1), `ground*Stiffness` (11), `ground*Damping` (0.5), `bumpsim` (400), `bumpsimprob` (0.08), `stunnedtime` (0), `stunnedblendout` (0) |
| `rdr2Sit` | Riding (RDR2's sit). He's swung up into the seat over `mountTime`, then his pelvis is held there and his feet sprung into the stirrups, his hips square to the seat (tipped by `hippitchstart`), his back by `straightnessstart`, his chest held upright (`softweight`, as RDR2 holds riders to their animation), his hands forward on the reins; with `Slump` he sits loose. The game moves the seat with the horse every frame by sending rdr2Sit again **without** `start` (`seatPos`, `seatUp`, `seatForward`, `stirrupL`, `stirrupR`; RDR2 instead gives a part of its horse with `seatuplocal` / `seatrightlocal`). Thrown when his body leans past `failangle` from the seat's up, the seat tips past `seatfailangle`, a hit moves his chest faster than `knockSpeed` against his hips, or it's sent `throw=true`: he's flung off (`throwSpeed`) and catchFall starts; with `drag` a foot stays caught and he's dragged by it for `dragTime`, reaching back for it. RDR2's names and defaults where it has them (`mountTime`, `knockSpeed`, `throwSpeed`, `throw` and the seat ones are Delirium's); what each does is read from its name. | `seatPos`, `seatUp` (0,0,1), `seatForward`, `stirrupL`, `stirrupR`, `mountTime` (0.6), `constrain` (true), `constrainstrength` (40), `constrainstrengthstirrup` (40), `hippitchstart` (-0.5), `hippitchramp` (0.25), `straightnessstart` (-0.1), `straightnessramp` (0.35), `failangle` (0.71), `seatfailangle` (0.71), `softweight` (0.8), `Slump` (false), `drag` (false), `dragTime` (10), `dragreach` (true), `dragreachdelay` (0.2), `dragstiffness` (9), `knockSpeed` (4), `throwSpeed` (3), `throw` |
| `rdr2Bolas` | A bolas round his legs: his ankles (and knees) held together, his balance gives after `fallTime` and he topples, then he struggles on the ground, both legs kicking together, for `struggleTime`, and is free. RDR2's own bolas parameters aren't known, so these are Delirium's. | `bindStrength` (900), `fallTime` (0.3), `struggleTime` (5; 0 until stopped), `struggle` (0.7) |
| `keyframe` | Before handing the body back to animation: blends spine and neck straight and the hands to his sides over `blendTime`, damping the upper body, and holds it for `holdTime`, so the animation picks up from a pose close to its own. The legs stay the balancer's. | `blendTime` (0.3), `holdTime` (0.5), `settle` (3), `stiffness` (11) |

In the lean family, `instanceIndex` and `boundIndex` are accepted but unused: the point (`offset`) is a world point,
as with `leanTowardsObject`. A lean held in one direction makes the balancer step after it, as Delirium's own
`leanInDirection` does.

Several of them bend the same joints at once (say `shotInGuts` with `reachForWound`): each lays its bend over the
joint's target as an offset, cleared every step, so they add up without building up or leaving a joint bent after
they end. Numbers in brackets are the defaults.

RDR2 bodies: `nm_character_from_rdr2` turns one of RDR2's naturalmotion XML files (fred.xml, wilma.xml and the
size variants, from your own copy) into an `nm-character-1` body on top of a base body (Homer's, or Marge's). Both of
RDR2's part namings are read. On Homer's GTA IV base, RDR2's regular, fat, large, short, skinny and tall men and its
two women stand and take a shot; the very small (0.9 m) and very large (scaled 2.2 times) ones are outside what the
balancer is tuned for.

#### RDR2 weapons (playroom)

The playroom's weapon picker has a Red Dead Redemption 2 group, thrown (or loosed) from the floating gun or by the
Hero:

- Lasso: it locks onto the part nearest where you aim, as RDR2's does, and catches it even if it lands on the floor
  beside it: head or neck, round the neck; legs, round the feet; otherwise round the chest and arms (`rdr2Lasso`'s
  type). Then hold the left button away from him to pull the rope towards the pointer, as a grab pulls but only
  while the rope is taut, building up over half a second; let go and it goes slack, and 4 s later the loop slips off.
  Clicking on him takes the loop off and throws a new one at the part clicked.
- Bolas: `rdr2Bolas`, his legs wrapped.
- Dynamite: rolls, and goes off after 3 s.
- Fire bottle: a fire where it breaks. In Red Dead Redemption 2 reaction style, catching fire starts RDR2's `onFire`
  in place of GTA IV's.
- Tomahawk and bow: hit as shots, so RDR2 reaction style adds RDR2's reactions by where they hit.

#### RDR2 horse (playroom, experimental)

Experimental: still being worked on, and its settings may change.

Character, at the bottom, "Red Dead Redemption 2 horse (experimental)": RDR2's horse, built from your own copies of its two files.
- **horse.xml**, its Euphoria body: common_0.rpf, data/naturalmotion (`rdr2_horse` in settings.txt).
- **Its ragdoll**, the fragment meta_base_horse_01.yft: packs_0.rpf, packs/base/models/metaped_fragments.rpf. In
  OpenIV right-click it, Save content, and give the `.yft.sys` it writes (`rdr2_horse_frag`; `!horse frag <path>`).

Bring the horse puts it 2.5 m beside Homer (`!horse`; `!horse off` takes it away). From the fragment come its 26
capsules (size, place, turn, mass and inertia: 617 kg in all), its 22 joints' frames and which links collide with each
other; from horse.xml each joint's limits and muscle. The two files are checked against each other (the fragment keeps
the same joint ranges, less RAGE's 0.03 margin). While it's sound it's held in its pose, as RDR2 holds its horses on
their animation; shoot, blast, tomahawk or arrow it: a leg hit twice gives, or killed (400 health), and it falls as
RDR2's ragdoll with the hits' pushes. Like Homer, it's see-through by See-through, with its skeleton drawn by Skeleton
(its left legs red, its right blue), and the mouse grabs it: drag a part and it's let go of its pose and pulled
(Homer's pull, times how much heavier it is, capped by the part's own mass); a click pokes it.

Where it isn't RDR2's, because of what Delirium's solver does:
- **Centre of mass:** a part's sits at its capsule's centre, so the 0.2 m RDR2 moves the pelvis's and the spine links'
  off theirs isn't there.
- **Two-capsule links:** the top of the neck, its middle and the head each have two capsules in RDR2. The second is held
  to the first by a stiff joint with a little play (Delirium's locked joint is only for a body's root).
- **Hinge-like joints:** knees, hocks, elbows and fetlocks, which RDR2 makes three-way joints bending on their twist
  with ±0.07 rad of lean, are hinges on the same axis and range (Delirium's three-way limit with that little lean threw
  the legs about).
- **Root:** the pelvis is the root, with RDR2's 0.475 kg root link hanging from it on the same joint.
- **Starting pose:** RDR2's standing pose has its pelvis, hips' twist and hind knees past RDR2's own limits; it starts
  turned just inside them.
- **Muscle stiffness:** the muscles' Euphoria stiffness (0.5) isn't used; Delirium's muscle takes RDR2's strength and
  damping.

Down and alive it does what RDR2's `quadrupedbase` behaviour does on the ground, with its names and defaults (from
RDR2's own table; what each does here is read from its name, not RDR2's code). Under "On the ground" in the horse's
section, or `!horse set <name> <value>`:

| Setting | What it does (default) |
|---|---|
| `alwayslifthead` | holds its head up off the ground, its neck bent toward the sky (on) |
| `scrabbleonside` | on its side its legs paddle, diagonal pairs together (on) |
| `minscrabblespeed`, `maxscrabblespeed` | how fast they paddle, rad/s, fresh to faded (11, 3) |
| `writheonground`, `writhespeed` | its neck writhes, this many times a second (on, 0.5) |
| `legStiffness`, `legStiffnessMin`, `spineStiffness`, `spinestiffnessmin` | its legs' and neck's strength, fresh and faded; 15 is full (15, 7, 15, 7) |
| `strengthdecay` | how fast, per s, it fades from fresh to faded (0.5) |
| `initialWeaknessZeroDuration`, `initialWeaknessRampDuration` | after a hit, s it's limp, then s to recover (0, 0.4) |
| `snap`, `snapMag` | a hit jolts its whole body too (on, 1) |
| `failangle` | tipped till its back's up is this cosine from the sky's, it goes down (0.6; held in its pose while sound, it isn't tipped) |
| `rollonground`, `rollvelocity`, `rollscale` | it heaves to roll onto its chest every other second (off, 0.7, 0.7) |
| `rightingTorque` | a steady turn toward lying on its chest (0) |

Rolling onto its chest doesn't get there yet: lying on its side, its legs stick out and hold it, so getting up is for
later (with balance).

Ride it (or `!horse ride`) puts Homer on it with `rdr2Sit`: swung up into the saddle, his seat following the saddle
every step; ridden, his body doesn't push on the horse (held in the seat, it would act as if he weighed
everything). Throw him off (`!horse getoff`), shoot or blast him hard
enough (`knockSpeed`), or bring the horse down, and he's thrown; set `drag` in Parameters for a foot caught in the
stirrup.

#### RDR2 reactions on hits (optional)

A game can have its hits pick these the way RDR2 does, with one call per hit. In the playroom, switch Reactions'
"Reaction style" from GTA IV to Red Dead Redemption 2 (or `!rdr2`, or `rdr2_reactions 1` in settings.txt): shots
then also start RDR2's reactions, and punches get `meleeReact` instead of the flinch. GTA IV style, the default, is
Delirium's own, unchanged.

- `nm_agent_rdr2_shot(agent, world, part, hitPoint, bulletDir)`, right after `nm_agent_task_shot`. By where the bullet
  hit: `shotFromBehind` (the trunk, from behind), `shotInGuts` (parts 7-8), `shotInjuredLeg` (thighs and shins),
  `shotInjuredArm` (upper arms to hands); a `shotSnap` every time; by chance `shotSpin` (trunk) or `shotFallToKnees`
  (belly, pelvis, legs), and `reachForWound` (not the head, neck, hands or feet; both hands for the belly). Returns
  how many started.
- `nm_agent_rdr2_melee(agent, world, part, hitPoint, blowDir)`, instead of a flinch: `meleeReact`, softened so a punch
  staggers him and rarely drops him.
- Both keep `rdr2Balance` on.
- `nm_rdr2_set(name, value)`, for every agent: `snap`, `hitLocation`, `melee`, `balance` (0 off, 1 on); `spin`
  (0.25), `knees` (0.15), `reach` (0.7), chances 0 to 1.

Rust callers use `delirium::extras_rdr2::Reactions` (`on_shot`, `on_melee`), which returns the messages to send.

Parameters marked unused in the listing are accepted (and counted in the return
value of `nm_msg_send`) but do not change anything.

### Parameters tab (playroom)

The playroom's Parameters tab lists every behaviour's parameters (GTA IV's, Delirium's extras and RDR2's), grouped,
with a search box. Hover a name for what it does, its type and default, and the behaviour it belongs to (and, for
RDR2's, which hits start it). A changed value is sent with that behaviour every time the room sends it, from a
button, a hit, a shot or the console, to every character, in place of the value the room would have used; Reset puts
it back. Positions, directions, text and the part hit are set by the room each time, so they are shown but not
edited. The tab also holds the RDR2 reaction picker (spin, knees and reach chances, and which reactions are on). Changes
are saved in settings.txt (`param`, `rdr2_pick`); with nothing changed every message goes out exactly as before.
Scripted and headless runs don't read settings.txt, so they never use changed values unless a script loads it
(`!settings`).

## Playroom scripting

Start the playroom with a script:

```
delirium-room.bat --script my.txt
```

(`delirium-room.command` on macOS and Linux.) Other useful flags:

| Flag | Meaning |
|---|---|
| `--script FILE` | Run the commands in FILE. |
| `--headless [N]` | No window; run N steps and print part transforms. Use with `--press a,b,walk`. |
| `--rec FILE` | Record. |
| `--safe` | Safe Mode: every quality setting at its lowest; your saved settings are neither used nor changed until you change one in the menu. |
| `--stage NAME` | Start in a scene (see `!scene`). |
| `--marge` | Use the second character. |
| `--body S\|A\|L\|K\|F\|T` | Body preset. |
| `--learn [generations] [path]` | Train the get-up network. |
| `--learn-walk`, `--learn-run [gens] [path]` | Train `walk.net`, `run.net`. |

### File format

One command per line: `seconds command args`.

- `seconds` is wall-clock time since the app started. Lines run in time order.
- `#` starts a comment; blank lines are ignored.
- A line that does **not** start with `!` is sent to the character as an engine
  message: `MessageName param=value param=value ...`. Vectors are `x,y,z`;
  booleans are `true`/`false`/`1`/`0`.
- A line that starts with `!` is a playroom command (below).
- Typing `?` lists all message names. `MessageName ?` lists a message's
  parameters with their current values (an asterisk marks a parameter that is
  accepted but unused).
- The Enter key in the playroom opens a console that takes the same lines.
- A paused script waits while the room is in photo mode.

Example:

```
# stand, take a shot to the chest, record it
0.0   !scene room
0.2   bodyBalance start=true bodyStiffness=10
1.0   !look daylight
1.5   !shoot 9
2.0   headLook start=true pos=0,2,1.6
3.0   !screenshot after_shot.png
5.0   !quit
```

Scripted runs neither read nor write `settings.txt`, so they always start from
defaults. Use `!settings` to load the file on purpose.

## Playroom commands

Names in brackets are optional. Part indices are the 0 to 20 above.

### Characters and reactions

| Command | Effect |
|---|---|
| `!button NAME` | Press a UI button by its label: `balance`, `drunk`, `windmill`, `pedal`, `writhe`, `rollup`, `catchfall`, `highfall`, `stairs`, `shot`, `flinch`, `brace`, `look`, `point`, `lean`, `relax`, `stayup`, `foetal`, `grab`, `overwall`, `STOP all`, `RESET pose`, `shot back`, `throw ball`, `throw box`, `drop crate`, `clear`, `push front`, `push back`, `car`, `fall`, `explosion pile`, `wrecking ball`, `ice`, `crowd`, `grid`, `add taxi`, `clear taxis`, `tackle high`, `tackle low`, `jump`, `jump dive`, `dismember`, `hat`, `avoid`, `get-up`, `infinite health`, `walk`, `passive`, `super punch`, `hero physics`, `replay`, `walk learned`, `run learned`, `rdr2 reactions` (switches the reaction style), or any extra message's name |
| `!body S\|A\|L\|K\|F\|T` | Body preset: S small 1.60 m / 55 kg, A medium 1.75 / 77, L large 1.90 / 110, K lanky 1.90 / 64, F fat 1.72 / 125, T stocky 1.63 / 88. |
| `!strength N` | Set the strength value. |
| `!walk x,y` | Walk towards a point (`!walk 0,0` stops). |
| `!soft` | Toggle soft mode. |
| `!infinite` | Toggle infinite health. |
| `!nododge` | Stop the character dodging. |
| `!oncue LINE` | Run `LINE` (a script line) when the character next goes over. |
| `!rdr2` | Switch the reaction style between GTA IV (the default) and Red Dead Redemption 2. |

### Hits and forces

| Command | Effect |
|---|---|
| `!poke PART` | Poke a part. |
| `!shoot PART [fatal]` | Shoot a part (`fatal` for a killing hit). |
| `!miss M` | A bullet passing M metres to the side of his head (a near miss, he flinches). |
| `!hatshot` | A bullet that only clips the hat; he flinches. |
| `!blast x,y,z` | Explosion at a point. |
| `!grab PART dx,dy,dz` | Pull a part by an offset; `!release` lets go. |
| `!throw KIND SPEED [weight X]` | Throw `ball`, `box`, `crate`, `barrel` or `wrecking ball` at SPEED m/s. |
| `!taxi_run M GAS [back M]` | Run the taxi at him. |
| `!drive PEDAL,WHEEL[,TAXI]` | Drive a taxi. |
| `!superpunch` | Super punch. |
| `!pause` | Toggle pause. |

### Hero mode (the second character)

| Command | Effect |
|---|---|
| `!hero` | Toggle hero mode. |
| `!hero_go x,y [n]` | Hero walks to a point. |
| `!punch` | Throw a punch. |
| `!pad HEX` | Hold fight bits: 10 attack 2, 80 kick, 40 block, 1 lock-on; `0` releases. |
| `!hero_weapon I` | Select a weapon row (`-1` or invalid = fists). |
| `!hero_fire dx,dy,dz` | Fire in a direction. |
| `!weapon I` | 0 pistol, 1 heavy pistol, 2 SMG, 3 rifle, 4 sniper, 5 shotgun, 6 rocket launcher, 7 grenade, 8 fire bottle, 9 bat, 10 knife, 11 taser, 12 stun baton; Red Dead Redemption 2's: 13 lasso, 14 bolas, 15 dynamite, 16 fire bottle, 17 tomahawk, 18 bow. |
| `!aim gun\|camera` | Aim mode. |
| `!gunat dx,dy,dz`, `!gunreach K [up]`, `!gun dx,dy,dz`, `!gunfire` | Floating-gun controls: aim, reach, aim off the chest and fire, fire. |
| `!horse [path]`, `!horse off` | Bring Red Dead Redemption 2's horse (from your horse.xml) beside him; take it away. |
| `!horse set name value` | Set one of the horse's quadrupedbase settings by RDR2's name (yes/no as 1/0). |
| `!horse frag path` | Where your export of the horse's ragdoll (meta_base_horse_01.yft.sys) is. |
| `!horse ride`, `!horse getoff` | Homer gets on the horse (rdr2Sit); he's thrown off. |
| `!rope dx,dy,dz` | With a lasso round him: pull the rope to that far from the roped part (`!rope off` lets go). |
| `!portal gun` | Pick the portal gun (needs portal mode). |
| `!skate` | J: Homer on the skateboard or off it (needs Skate 3 linked). |
| `!skate x,y,yaw` | On the board at that spot, facing yaw (radians from +X). |
| `!skate_key K 1\|0` | Hold or let go a board key: `w s a d r space q e shift ctrl`. |
| `!skate_stick x,y` | Hold the right stick (`0,0` lets go). |

### Scene, camera, look

| Command | Effect |
|---|---|
| `!scene NAME` | `room`, `stairs`, `wall`, `ledge`, `bigstairs`, `rooftop`, `lowwall`, `rail`, `street`, `bridge`, `trampoline`, `quake`, `bull`, `cab`. |
| `!look NAME` | `studio`, `void`, `daylight`, `blueprint`. |
| `!sun az,el` | Sun direction. |
| `!shadows`, `!shadow SIZE` | Toggle shadows; shadow size. |
| `!pedcol NAME` | `mixed`, `blue`, `red`, `solid`, `opaque` (numbers also work). |
| `!eyes`, `!skel`, `!clean` | Toggle eyes, skeleton overlay, clean view. |
| `!cam yaw pitch dist height` | Place the camera. |
| `!camat x y z` | Look at a point. |
| `!camhead` | Filming follows the head. |
| `!pov` | Toggle a camera at the hero's eyes. |
| `!crane yaw pitch dist z secs [x y z]` | Camera move over `secs` seconds. |
| `!shot NAME`, `!photo`, `!film RATE` | Camera presets, photo mode, filming rate. |
| `!screenshot FILE.png` | Save a screenshot. |
| `!hide` | Hide the panels. |
| `!tab N` | Show a panel tab: 1 Reactions, 2 Interact, 4 Scene, 8 Character, 16 View, 32 Help, 64 Inspect. |
| `!settings` | Load `settings.txt` now. |
| `!strict` | Strict mode. |
| `!quit` | Exit. |

## settings.txt

The playroom saves your choices automatically whenever one changes.

- Windows: `%APPDATA%\delirium\settings.txt`
- Elsewhere: `~/.config/delirium/settings.txt`

One `name value` per line. Booleans are `0` or `1`. Unknown keys are ignored.

| Key | Values | Meaning |
|---|---|---|
| `pedcol` | name or number | Character colouring. |
| `opacity` | 0.1 to 1 | Character opacity. |
| `eyes`, `brows`, `skel`, `clean` | 0/1 | Face and overlay options. |
| `look` | name | Lighting look. |
| `avoid`, `tasks` | 0/1 | Dodging; use the task helpers. |
| `getup` | 0 to 3 | Get-up mode (animation, muscles, off, learned). |
| `god`, `weak`, `passive`, `walk` | 0/1 | Invulnerable, weak muscles, limp, walking. |
| `hero_physics`, `super_punch` | 0/1 | Hero options. |
| `shadows`, `soft` | 0/1 | Rendering. |
| `shadow_soft` | 0 to 10 | Shadow softness. |
| `fill` | 0 to 3 | Fill light level. |
| `sun_r`, `sun_g`, `sun_b` | 0 to 1 | Sun colour. |
| `sound` | 0/1 | Sound on. |
| `vol`, `vol_fx`, `vol_portal` | 0 to 1 | Volumes. |
| `rdr2_reactions` | 0/1 | Reaction style: 0 GTA IV (the default), 1 Red Dead Redemption 2. |
| `param` | `behaviour.parameter value` | A value changed in the Parameters tab, one line each (e.g. `param shotFallToKnees.ftkPitchForce 150`). |
| `rdr2_horse` | path | Your RDR2 horse.xml, for the horse. |
| `rdr2_horse_frag` | path | Your OpenIV export of RDR2's meta_base_horse_01.yft (the `.yft.sys`), the horse's ragdoll. |
| `rdr2_pick` | 7 numbers | The RDR2 reaction picker, when changed: snap, spin chance, knees chance, where-hit, reach chance, melee, balance. |

## Tips and troubleshooting

**Errors**

| Symptom | Cause and fix |
|---|---|
| `NM_E_ARG` from `step` | `dt` was not in (0, 0.25]. Use 1/60. |
| `NM_E_FOLLOW` (-8) | FOLLOW mode: set every part's state before each step. |
| `NM_E_STATE` on a `set_*` or `send` | No `nm_msg_begin` first. |
| `NM_E_MESSAGE` / `NM_E_PARAM` / `NM_E_TYPE` | Unknown message, unknown parameter, or wrong value type. Check the spelling (names are case sensitive) and the kind with `nm_message_param_kind`. |
| `nm_init` returns 1 | Already initialised; later calls are ignored. |

**Behaviour**

- Call `nm_init` before creating agents. Agents made before it run simpler
  stand-ins.
- A character that does nothing may be asleep (`nm_agent_is_awake`); sleeping
  bodies skip behaviours until woken.
- A character that "will not balance" usually needs `bodyBalance start=true`.
  Sending only parameters to a behaviour that is not running does nothing
  visible.
- Pushes alone do not make a reaction; send `shot` (or call the shot task)
  as well as applying an impulse. The task helper bullet speed is 400 m/s.
- Moving boxes must be set again before every step, or they stand still.
- Mesh triangles carry a material number 0 to 255. Set its friction and
  elasticity with `nm_set_surface` (ice is about 0.05 friction).
- The tuning is made for about 1.8 m. Very tall, short or heavy bodies behave
  worse; use `nm_build_fit` or the body presets.
- Last-error text is per thread. Read it on the thread that failed.
- The `send` return value is the count of parameters kept but unused. A
  nonzero count is not an error.
- Keep the message-send order simple: stop (`stopAllBehaviours`), then start
  the next behaviours. Several can run together, but ones that write the same
  muscles (two stiffness-setting behaviours) will fight.

**Playroom**

- `delirium-room` needs your own copy of the game files described in the
  README. The game path is remembered in `game_path.txt`.
- On macOS, clear the quarantine flag once: `xattr -dr com.apple.quarantine .`
- If learned get-up says `no getup.net yet (train: --learn)`, the network file
  is missing from the folder above the executable; copy `getup.net` there or
  train one.
- Portal mode: Interact, then "Portal 2 - bonus", then link your install
  folder or use "Find Steam install". Left click shoots blue, right click
  orange.
- Skate 3 mode: Settings, then "Skate 3 - bonus", then give your Skate 3 (Xbox
  360) .iso or its default.xex and Link (converted once, about a second). J
  puts Homer on the board: W push, S brake, A/D steer, Space ollie, Q
  kickflip, E heelflip, wheel button + mouse the right stick, Shift/Ctrl
  crouch and grab, R off the board. The clicks stay the portal gun's.
- Keyboard: click to poke, drag to pull a part, Shift+click to shoot,
  1/2/3 throw ball/box/crate, `[` `]` slower/faster (1/16x to 4x), Space pause,
  `.` one step, `R` reset pose, `X` stop all, `H` hide panels, `P` photo mode,
  `C` camera, `F` taxi in/out.
