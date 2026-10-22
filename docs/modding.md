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

Parameters marked unused in the listing are accepted (and counted in the return
value of `nm_msg_send`) but do not change anything.

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
| `!button NAME` | Press a UI button by its label: `balance`, `drunk`, `windmill`, `pedal`, `writhe`, `rollup`, `catchfall`, `highfall`, `stairs`, `shot`, `flinch`, `brace`, `look`, `point`, `lean`, `relax`, `stayup`, `foetal`, `grab`, `overwall`, `STOP all`, `RESET pose`, `shot back`, `throw ball`, `throw box`, `drop crate`, `clear`, `push front`, `push back`, `car`, `fall`, `explosion pile`, `wrecking ball`, `ice`, `crowd`, `grid`, `add taxi`, `clear taxis`, `tackle high`, `tackle low`, `jump`, `jump dive`, `dismember`, `hat`, `avoid`, `get-up`, `infinite health`, `walk`, `passive`, `super punch`, `hero physics`, `replay`, `walk learned`, `run learned`, or any extra message's name |
| `!body S\|A\|L\|K\|F\|T` | Body preset: S small 1.60 m / 55 kg, A medium 1.75 / 77, L large 1.90 / 110, K lanky 1.90 / 64, F fat 1.72 / 125, T stocky 1.63 / 88. |
| `!strength N` | Set the strength value. |
| `!walk x,y` | Walk towards a point (`!walk 0,0` stops). |
| `!soft` | Toggle soft mode. |
| `!infinite` | Toggle infinite health. |
| `!nododge` | Stop the character dodging. |
| `!oncue LINE` | Run `LINE` (a script line) when the character next goes over. |

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
| `!weapon I` | 0 pistol, 1 heavy pistol, 2 SMG, 3 rifle, 4 sniper, 5 shotgun, 6 rocket launcher, 7 grenade, 8 fire bottle, 9 bat, 10 knife. |
| `!aim gun\|camera` | Aim mode. |
| `!gunat dx,dy,dz`, `!gunreach K [up]`, `!gun dx,dy,dz`, `!gunfire` | Floating-gun controls: aim, reach, aim off the chest and fire, fire. |
| `!portal gun` | Pick the portal gun (needs portal mode). |

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
- Keyboard: click to poke, drag to pull a part, Shift+click to shoot,
  1/2/3 throw ball/box/crate, `[` `]` slower/faster (1/16x to 4x), Space pause,
  `.` one step, `R` reset pose, `X` stop all, `H` hide panels, `P` photo mode,
  `C` camera, `F` taxi in/out.
