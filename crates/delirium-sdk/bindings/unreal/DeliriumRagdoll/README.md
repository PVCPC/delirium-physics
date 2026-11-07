# Delirium Ragdoll for Unreal Engine 5

Active ragdolls for any Unreal character: they keep their balance when pushed, stagger when shot, catch their fall,
lie still and hand the character back to get up. Built and tested on UE 5.8 with the Third Person template's Manny.

## Install

1. Copy this `DeliriumRagdoll` folder into your project's `Plugins` folder.
2. Put `delirium_physics.h`, `delirium_physics.dll` and `delirium_physics.dll.lib` in `Source/ThirdParty/delirium_physics`. The release's `unreal-plugin` download has them in place.
3. Build the project (it's a C++ plugin: a Blueprint-only project needs one C++ class added first).
4. Project Settings > Plugins > Delirium Ragdoll: point `Game Exe` at your own copy of the game and `Character` /
   `Build` at the data files (absolute, or under the project folder).
   `Engine Physics` (experimental): NM drives a ragdoll of Chaos bodies and constraints instead of its own physics, so
   it meets your physics objects; it stands, takes pushes, falls to hard ones, shots and a lost leg (not the Brawler body yet).

## Use

Add a **Delirium Ragdoll** component to the character. From Blueprint or C++:

| Call | What happens |
|---|---|
| `Ragdoll()` | Takes it (no hit): it stands and keeps its balance |
| `Shoot(Point, Direction, Force, Set, Health)` | A bullet; `Health` is what you leave it (100+ staggers, under 100 goes down, under 50 is a kill, a head hit kills) |
| `ShootAlong(From, Aim, ...)` | The same, striking the body part the aim passes |
| `Push(Impulse)` | A shove at the chest, N s (60 stays up, 200 knocks down) |
| `Explode(At, Radius, Speed)` | Thrown by a blast |
| `MeleeHit(Point, Impulse, AttackerHead, bTimid)` | A punch or club that doesn't kill |
| `OnFire(bBurning, Health)` | Flails and drops; call with false once out |
| `Tase(Seconds)` | A taser: jolts and locks for Seconds (2.5), then ends by itself; 0 ends it |
| `HighFall()` | Arms out, legs ready |
| `CatchFall()` / `FallDown(Heading)` / `JumpRoll()` | Hands out as it falls / pushed off a ledge / out of a moving car |
| `Grab(Left, LeftNormal, Right, RightNormal, Hold)` | Hands reach for and hold points (a zero normal leaves that hand free) |
| `Flinch(From)` / `Tackle(Target, bLow, bKeep)` | Flinch from a threat / dive at another ragdoll |
| `SetWater(Level)` | Water under it (cm; NaN none): floats face up |
| `RegisterCar(Actor, Size, bPlayerDriven)` / `UnregisterCar(Actor)` | Anything you call a car: ragdolls meet its box, standing characters in its way are ragdolled, hits react as the game's and raise `OnCarHit(Damage, Car)` |
| `CarComing(Car)` / `CarDoor(Car)` | Brace against a registered car (setting `bArmsOutEarly`) / dragged from it |
| `React(Line)` | Any reaction as the Playroom console's line: `rollUp start=true`, `bodyWrithe start=true`, `stopAllBehaviours` |
| `Kill()` | Limp, stays down |
| `Sever(Point, Impulse)` | Cuts off the limb nearest `Point` (hide its mesh and spawn your gib yourself) |
| `Leash(Point, Anchor, Length)` | Ties the part nearest `Point` to `Anchor`, at most `Length` cm away: slack inside, he hangs and swings past it; returns the part |
| `LeashLength(Part, Length)` / `LeashAnchor(Part, Anchor, Length)` / `Unleash(Part)` | A new length (shorter reels him in), a moved anchor (a moving holder, a corner it wraps round), let go (-1: all) |
| `GetUp()` | Hands it back: the actor stands where the body lies, mesh, animation and movement back on |
| `ShootWeapon(Weapon, From, Aim, Health)` / `Shotgun(From, Aim, Weapon, Health)` | A shot from the weapons table (setting `Weapons`: `name=set:push:damage[:pellets:between:clip:accuracy:range]` or the game's weaponinfo.xml; empty: pistol, smg, rifle, shotgun), returning the health left / a blast's pellets fanned as the game's, returning how many struck. `WeaponNames()`, `LoadWeapons(Text)` |
| `SetReactions(Bits)` / `NearMiss(From, Direction)` / `SetDive(Gap)` / `ThrownAt(At, Velocity, bLook, bAvoid)` | The Playroom's extra reactions (EDeliriumReaction) / a bullet that missed / a dive when shot at again soon after / something thrown at it |
| `Jump(bDive, Direction)` / `Balance(LookAt)` / `Stop()` | Jumps or dives / the game's balance / stops the running task |
| `Sit(LookAt, Seat)` / `TakePose(Low, High)` / `RollUpRelax(...)` / `Script(Ms)` | Sits / the animation's pose as muscle targets / rolls up then goes limp / only your `React` lines move it |
| `MuscleRise(bOn, Help)` / `LearnedGetUp(Net)` / `LearnedWalk(bOn, Net, Cycle)` + `SetStick(bOn, X, Y, CameraYaw)` | Gets up with its own muscles / as the learned network did (settings `GetUpNet`, `WalkNet`: the release's getup.net, walk.net) / walks under the stick |
| `SetAttacker(Actor, bArmed)` / `GunReady()` | Who it's up against (it looks at them; armed, says when to fire) |
| `GetParam(...)` / `Raycast(From, To, Point, Normal)` / `CheckSetup()` / `Messages()` | A behaviour parameter as it runs / the part a segment hits / what's off in its setup / every message and parameter, for pickers |

`OnLyingStill(bFaceUp, Facing)` fires once it has lain still half a second: start your get-up animation there and
call `GetUp`. Or set `Get Up After` (seconds) and it hands itself back.

**Bodies and parameters (the Playroom's)**, in the component's Delirium|Body settings: `Body` is `Fitted` (its own
skeleton's, the default), `Small`, `Average`, `Large`, `Lanky`, `Fat`, `Stocky`, `Waddler`, `Guard`, `Brute` or
`Brawler` (that shape of this character), `Random` (`Body Seed`, 0: its own; `Body Variance`; `Body Symmetric`) or
`Custom` (`Custom Body`: legs, arms, trunk, width, head, neck against the fitted body, weight in kg, height in m, 0
keeps it). `Overrides` changes behaviour parameters as the Playroom's Parameters tab does
(`shot.bodyStiffness=7, bodyBalance.useHeadLook=0`); `SetOverride(Behaviour, Param, Value)` and `ClearOverrides()`
from Blueprint. The mesh keeps its look (its bones follow the body's turns), so pick a body near the mesh: a Brawler under a tall mesh
shows its feet off the ground.

`Strength` (0: the project settings') or `SetStrength(K)` makes one character's muscles stronger, 1.3-2 for a heavy
custom body. A custom body that won't stand: "Tuning a custom body" in `docs/modding.md`.

Settings `bStepLimit` (the game's balance runs out of steps; off: the Playroom's) and `bFireReaction`. A physical
material's friction and restitution reach NM (ice slides); a turning mover carries what it touches round. A panic in
the SDK writes `Saved/delirium_physics-crash.txt`.

The skeleton is found by bone names (Unreal, Mixamo, Unity humanoid, Biped style); `BoneMap` fixes the rest
(`Pelvis_jnt=Buttocks, Tail=None`).

Places are Unreal's (cm); impulses are N s. Ragdolls within 5 m share one world, so they push and pile on each other.

## Try it: click to fight

Add a **Delirium Click To Fight** component to your player pawn and press Play: aim at a character and left click
to shoot (the fourth shot kills), F to punch, G for a blast. Downed characters get up after 3 s. `Fire`, `Punch`
and `Blast` take any view (From, Aim), from Blueprint too; its source (`DeliriumClickToFight.cpp`) is the starting
point for your own weapons.

## How it works

While ragdolled, the character's skeletal mesh is hidden, its animation paused and its movement stopped; a posed copy
of the mesh shows the body each frame. The level's static collision within 15 m (boxes, convex hulls, triangle
meshes) is read once when it's taken, plus a floor under its feet.

## Limits for now

- Spheres, capsules and landscapes aren't read as level (the floor under the feet covers flat ground).
- No snaps: the ragdoll starts from the animated pose and the actor's speed, and getting up blends into the animation over `GetUpBlend` seconds (0.4).
- Windows only (the SDK builds for Linux and macOS too; add their libraries to the Build.cs to use them there).

## Test

`UnrealEditor-Cmd.exe YourProject.uproject /Engine/Maps/Entry -ExecCmds="Automation RunTests Delirium.Ragdoll;Quit"
-unattended -RenderOffscreen`: Manny stands 5 s, stays up through a 60 N s push, goes down to a 200 N s one, to a
killing shot and without a leg; then every call above once, and ice under a shove (`DELIRIUM_NETS=<folder with
getup-reward.net and walk.net>` adds the learned get-up and walk; `DELIRIUM_TEST_ENGINE=1` runs it all with Chaos bodies).

## State and events

`GetState()` returns FDeliriumState: bRagdolled, bBalancing, bFallen, bCatchingFall, bRolledUp, bRelaxing, bDead,
bInWater, bBurning, Tackling (2 diving, 1 holding), LyingFor (s), Centre, Velocity, Mass, bSettled, bAwake, bJumping,
bTaskRunning, bLearnedGetUp, bResting, bHasStance / Stance, Contacts (21 flags). Events: `OnFell` once per
fall, `OnFeedback(Name, Slot, Value)` for NM's own behaviour messages (the Playroom's Feedback list), plus `OnLyingStill`
and `OnCarHit`.


## Tuning reactions

Walls, floors and falls need nothing: ragdolls get the level's collision and catch, stagger and brace on it by
themselves. Moving things that hit characters are registered once (`RegisterMover(Actor, Kind, Size, bPlayerDriven, bBrace, bKnockDown, Centre)`, kinds Car, Object, Train) and each picks
its own: box size and centre, whether characters in its way brace (`brace`) or are knocked down at all (`knockDown`).
Each character's health and agility (`HitHealth, HitAgility`) decide which reaction the game picks when hit (agility under 3 is weak).

Every reaction's feel is an override (`behaviour.param=value`, comma-separated; see Overrides):

| Reaction | Useful parameters |
|---|---|
| Bracing for a car or object | `braceForImpact.braceDistance` (how early the arms go out, m), `.targetPredictionTime` (s ahead), `.bodyStiffness` |
| Catching a fall | `catchFall.armsStiffness`, `.legsStiffness`, `.torsoStiffness` |
| Staggering | `staggerFall.lowerBodyStiffness` |
| Rolling up | `bodyRollUp.stiffness` |
| Keeping balance | `bodyBalance.armsOutOnPushTimeout` (how long the arms stay out after a push, s) |
| Shot / high fall | `shot.bodyStiffness`, `highFall.bodyStiffness` |

The Playroom's Parameters tab lists every parameter with its default and lets you try values live.
