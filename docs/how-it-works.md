# How Delirium Physics works

Delirium Physics is a ragdoll physics and reaction engine. A character is a
body of 21 capsules joined by 20 joints. Every joint has a motor ("muscle").
Reactions such as balancing, catching a fall, flinching from a bullet or
grabbing a ledge are *behaviours*: small controllers that decide, many times
per second, how stiff each muscle should be and where it should aim. Physics
moves the body; behaviours move the muscles.

This page explains the pieces and how a frame flows through them. To drive the
engine from your own code, see [modding.md](modding.md).

## The crates

The project is a Cargo workspace with four crates.

| Crate | What it is |
|---|---|
| `delirium` | The engine: rigid bodies, joints, collision, the character, and every behaviour. Pure Rust, no window, no GPU. |
| `delirium-data` | Readers for the file formats the engine and playroom consume: archives, animation clips, skeletons, character files, task tuning, materials, models, plus the small random-number generator and the get-up clip definitions. |
| `delirium-sdk` | A C interface (`nm_*` functions) over the engine, built as a shared library (`delirium_physics.dll`, `.so` or `.dylib`). Ships bindings for C, C++, Python, C# and Unity. |
| `delirium-room` | The playroom: a desktop app (Bevy + egui) to poke, shoot, drag and script a character. It is also the best reference for how to use the engine. |

```
 your code (Rust / C / C++ / Python / C# / Unity)
        |                              |
        | C ABI: nm_*                  | direct Rust calls
        v                              v
 +---------------+              +-------------+
 | delirium-sdk  |------------->|  delirium   |  <---- delirium-data
 +---------------+              |  engine     |        (file readers)
                                +-------------+
                                      ^
                                      |
                              +---------------+
                              | delirium-room |  (playroom app)
                              +---------------+
```

Everything in the SDK is a thin layer over `delirium`. Nothing in the engine
needs a graphics API. The playroom adds rendering, input, sound and the
scripting commands.

## Inside the engine

| Module group | Job |
|---|---|
| `ragdoll`, `rigid`, `artic` | Parts (capsules), joints with limits, integration, ground contact, the surface table (friction and elasticity per material). |
| `collision`, `bvh`, `gjk` | Level geometry as a triangle mesh with a bounding-volume tree for fast queries, plus convex tests for the moving boxes. |
| `nmrs` | The character: the 21 parts, 20 joints, each joint's *effector* (stiffness, damping, muscle stiffness, limits) and the body-part groups (hips, knees, spine, neck, arms...). |
| `msg` and the `extras*` modules | The message table: every behaviour's name, parameters, types and defaults, and the code that applies a message to the agent. |
| `nm` | The `Agent`: owns one ragdoll plus all behaviour state, steps it, and reports feedback. |
| per-behaviour modules | One module per behaviour family: balance, catch fall, high fall, roll down stairs, brace, grab, shot, fall over wall, foetal curl and so on. |
| `playerctl`, `shottask`, `meleetask`, `explosion`, `onfire`... | Higher-level "tasks" that combine several messages the way a game would (a bullet hit, a punch, a blast). |

### The body

The character has 21 parts, indexed 0 to 20:

| Index | Parts |
|---|---|
| 0 | Buttocks (the root, also called the pelvis) |
| 1-3 | Left thigh, shin, foot |
| 4-6 | Right thigh, shin, foot |
| 7-10 | Spine 0 to Spine 3 (10 is the chest) |
| 11-12 | Neck, head |
| 13-16 | Left clavicle, upper arm, lower arm, hand |
| 17-20 | Right clavicle, upper arm, lower arm, hand |

Each part is a capsule: a radius and a half-length along the part's local Y
axis, with a mass and an inertia. The world is right-handed with **Z up**,
metres, kilograms, seconds.

The 20 joints are ball joints (three rotational degrees of freedom, a cone
limit plus a twist limit) or hinges (one axis, a min/max angle). Each joint has
an *effector*: a spring-damper motor that tries to rotate the joint towards a
desired angle. A behaviour never moves a part directly. It writes desired
angles and stiffnesses into the effectors, and the physics step turns those
into torques.

Three numbers per joint matter most:

- **strength / stiffness**: how hard the motor pulls towards its target.
- **damping**: how much it resists motion.
- **muscle stiffness**: an extra passive stiffness that makes limp limbs
  heavier to swing.

`setStiffness`, `bodyRelax` and `injury` change these directly. Every other
behaviour changes them as a side effect of what it is trying to do.

### Body sizes and the character file

The built-in tuning is for a body about 1.8 m tall. A character is described
either by a character file (`.xrt`) plus a *build dump* (a text file with each
part's size, mass and joint frames), or by a single JSON file
(`format: "nm-character-1"`, exactly 21 parts and 20 joints). The SDK can
convert the first form into the second (`nm_character_from_xrt`). To fit the
body to a different skeleton, `nm_build_fit` rescales it from a bind pose; the
tuning keeps working best near 1.8 m, so expect worse results far from that.

## One simulation step

An *agent* is one character plus its behaviour state. You create it at a
position and yaw, send it messages, and call `step` with a time step `dt`.

`dt` must be above 0 and at most 0.25 s. The native rate is **1/60 s**. Inside
a call the engine runs `round(dt * 60)` substeps (at least 1, at most 8), so
1/60 s is exactly one substep.

There are two ways to run a step.

### LEAD mode (default)

Delirium owns the body. It integrates the physics itself.

```
 per substep:
   1. physics:   apply the motor drive chosen last substep,
                 integrate parts, solve joints, collide with the world
   2. behaviours: read the new body state, update every running behaviour,
                 write the next drive (desired angles, stiffnesses, forces)
```

Physics runs on what the behaviours decided on the *previous* substep, the
same order a real-time game loop would use. A brand-new agent's first 1/60 s is
therefore driven by its behaviours alone.

### FOLLOW mode

Your engine owns the body. Delirium only plays the "brain".

```
 you, before each step:   set every part's position, rotation, velocity,
                          angular velocity  (13 floats per part)
 step:                    behaviours run on that state
 you, after each step:    read the per-part drive (force at the part centre
                          in newtons, torque in newton-metres; gravity is
                          not included) and apply it in your own engine
```

FOLLOW mode is how the Python Bullet/MuJoCo examples and the Unity package work.
If you forget to set all parts before a step, the step returns the error
`NM_E_FOLLOW` (-8) and nothing moves. Joint information
(`nm_agent_get_joint`: type, pivot, frame, limits, armature) lets you build a
matching ragdoll in your own engine.

### Feedback

Behaviours report events through a *feedback list*. It is cleared at the start
of each step. After the step you can read the entries (a name, a slot and a
value). Names you will see include `balance` (the balancer gave up and the
character is falling), `catchFall`, `shot`, `brace`, `HighFall`,
`FallOverWall` and `GrabNotGrabbing`.

## Behaviours and messages

### Messages

Everything you can ask a character to do is a **message**: a name plus named
parameters. The table is built in and can be listed at run time
(`nm_message_count`, `nm_message_name`, `nm_message_param_*`), so tools can
discover it. There are 55 messages:

- 30 core messages (balance, catch fall, shot, grab, brace, stairs, look,
  point, lean, stiffness, relax...).
- 25 extra messages in three groups: general (`electrocute`, `hardConstraint`,
  `injury`), falling and staggering (`teeter`, `stumble`, `staggerFall`,
  `smartFall`, `balancerCollisionsReaction`, `injuredOnGround`, `learnedCrawl`)
  and being held or moved (`yanked`, `dragged`, `dangle`, `carried`,
  `buoyancy`, `animPose`, `armsWindmill`, `spineTwist`, `wave`, `breathe`,
  `softKeyframe`, `splitBody`, `shootDodge`, `armHang`, `crushReaction`).

The full list, with the parameters that matter, is in
[modding.md](modding.md#message-reference).

Parameters have one of five kinds: float, int, bool, vec3 and string. The
engine converts sensibly (a float with no fraction is accepted for an int, a
bool for an int, an int for a float). A parameter that is *accepted but
unused* is stored but changes nothing; the send call reports how many of those
there were, so scripts can warn about them.

### Lifecycle of a behaviour

```
 start=true  --->  RUNNING  <--- params without start just update it
                      |
   start=false, stopAllBehaviours, or the behaviour finishes itself
                      |
                      v
                   STOPPED
```

- **Start.** Send the behaviour's message with `start=true`. A few
  (`bodyBalance`, `shot`, `pointArm`) reset their parameters to defaults when
  they start.
- **Run.** While running, the behaviour reads the body state every substep and
  rewrites the motors. Sending the same message again *without* `start` only
  changes held parameters, for example steering `bodyBalance` with a new
  `headLookPos`.
- **Stop.** `start=false` stops one behaviour. `stopAllBehaviours` clears
  everything: balance, shot, catch fall, flinch, writhe, windmill, pedal,
  roll-up, brace, grab, head look, relax and the extras.
- **Finish on their own.** Some end by themselves: `wave` stops after its time
  runs out, and several falling behaviours hand over to the next one (a
  falling character can switch to `catchFall`, `bodyRollUp` or `bodyBalance`
  when it lands).
- **One-shots.** `activePose`, `applyImpulse`, `bodyRelax`,
  `defineAttachedObject`, `leanInDirection`, `leanToPosition`,
  `leanTowardsObject`, `setStiffness` and `shotRelax` act immediately and do not
  need `start`.

Several behaviours can run together: `bodyBalance` for the legs, `pointArm` for
an arm, `headLook` for the neck. They each write the effectors they own, and a
*mask* string picks which part of the body a message applies to (`fb` whole
body, `ub` upper body, `ul` / `ur` left / right arm, `ll` / `lr` left / right
leg, `us` spine, `un` neck, `uk` spine and neck, `uw` wrists, `uc` clavicles).

### Balancing and falling

`bodyBalance` is the central behaviour. While it is on, the character plants
its feet, shifts its centre of mass over them and takes steps to recover from
pushes. When a push is too big, the balancer gives up: the agent's `fallen`
flag is set and the `balance` feedback fires. From there, the usual chain is:

```
 bodyBalance --push too big--> fallen (feedback "balance")
 you (or a task helper) then start a fall reaction:
   highFall / smartFall / catchFall / bodyRollUp
 and, once down, bodyRelax / bodyFoetal, or the get-up modes below
```

You can query the state at any time: balancing, fallen, settled (balanced and
still), relaxing, catching a fall, rolled up, awake, stance point, centre of
mass, total mass, kinetic energy and per-part contact flags. Sleeping agents
skip their behaviours until something wakes them.

### Hits

A hit is two things at once: a physical impulse and a reaction. The
`applyImpulse` message or the push functions give the body momentum. The `shot`
behaviour (or the `shot` task helper, which picks parameters from a tuning set
such as `POWERFUL_WEAPON` or `SMG_WEAPON`) tells the character where it was hit
so it can reach for the wound, look at it, buckle or fall. The tuning file
(`taskparams`) is optional; without it the built-in defaults are used.

## The get-up network

Getting up from the floor is hard to hand-code, so the playroom includes a
small neural network (`getup.net`) that was trained for it. It lives in
`delirium-room/src/learn.rs`.

**Shape.** A fully connected network with two hidden layers of 64 tanh units.
The file stores four layer sizes (u32) followed by the weights (f32), little
endian.

**Input.** For each of the 21 parts: height, position, velocity and
orientation relative to the body, plus which behaviour is currently chosen.

**Output.**
- A rotation offset for every non-root joint (three numbers each), applied on
  top of a rest pose.
- A stiffness and a muscle-stiffness multiplier.
- A *menu choice* among 40 slots (the muscles themselves plus about 35
  behaviours such as balance, windmill, pedal, roll-up, catch fall, brace,
  stumble, wave) and a direction. Every 15 steps the network may switch: the
  playroom sends `stopAllBehaviours` and then the chosen behaviour's messages.

So the network is not a separate controller that bypasses the engine. It
*drives* the same muscles and sends the same messages a script would.

**Training.** Evolution strategies on the CPU: each generation tries 64
antithetic noise pairs (two rollouts each), with noise scale 0.04, learning rate
0.02 and Adam-style moments. Episodes start lying or fallen and run 480 steps
(8 s). The reward is height-based: it grows through stages such as being
upright and having the head above 1.4 m. Run it with:

```
delirium-room --learn [generations] [path]
```

It also has `--learn-walk` and `--learn-run`, which produce `walk.net` and
`run.net` for the "walk learned" and "run learned" buttons. The files are found
in the nearest folder above the executable, which is why `getup.net` sits next
to the launcher. While training, a `.live` copy is written so the "Watch"
buttons can show progress.

**Get-up modes.** The `getup` setting (the "get-up" button) cycles through:

| Value | Mode | What it does |
|---|---|---|
| 0 | animation | Plays a get-up clip. |
| 1 | muscles | Muscles follow the clip's poses (can fail). |
| 2 | off | Stays down. |
| 3 | learned | Uses `getup.net`. |

The trigger is the same for all modes: when health is at least 50, the head is
within 0.3 m of pelvis height and the body has been still for a short time. If
`getup.net` is missing in learned mode, the room says
`no getup.net yet (train: --learn)`.

## The playroom

`delirium-room` is a Bevy app on top of the engine. It does four jobs: render
the character and world, turn mouse/keyboard input into hits and messages,
play sounds, and run script files.

Notable parts:

- **Scenes** (`room`, `stairs`, `wall`, `ledge`, `bigstairs`, `rooftop`,
  `lowwall`, `rail`, `street`, `bridge`, `trampoline`, `quake`, `bull`, `cab`)
  set up level geometry for different tests.
- **Bodies**: six presets with different height and mass (see
  [modding.md](modding.md#playroom-commands)).
- **Hero mode**: a second, player-controlled character that can punch, kick,
  block, lock on and use weapons against the ragdoll.
- **Portal mode** (optional, off by default): on request it reads the portal
  ring, particles, gun model, companion cube and sounds from your own Portal 2
  install. Nothing from that install is shipped with Delirium Physics.
- **Skate 3 mode** (optional, off by default): J puts Homer on a skateboard run
  by the Skate 3 Rust engine (its board physics, tricks, grinds and animation
  graphs), on files converted once from your own Skate 3 disc image. The
  skater's bones pose Homer; a bail, a shot or a hit hands him to his ragdoll
  with the board's speed and spin, and the board flies on. With portals open,
  ollie into one and he comes out of the other going as he went in; out of a
  ceiling's or a floor's he bails. Nothing from Skate 3 is shipped.
- **Scripting**: the same message syntax as the engine, plus `!` commands that
  press buttons, move the camera and take screenshots.

## Design in one paragraph

Keep the body simple (capsules and joints), keep the brain modular (named
messages with typed parameters), step at a fixed 60 Hz and keep the
whole thing scriptable. Every reaction the playroom shows is a message you
can also send from your own program through the C interface.
