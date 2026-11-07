# Ragdoll plugin for Unity games (BepInEx)

Aim at any character in a Mono Unity game, press **F9**: it's pushed and the physics takes over (balancing, staggering, falling). F9 again pushes again; **F10** hands every character back to the game.

## Start here

1. **Install BepInEx 5** in the game ([BepInEx releases](https://github.com/BepInEx/BepInEx/releases), the `x64` zip unzipped into the game's folder), and run the game once.
2. **Export your data files** with the Playroom, from the same game folder or disc image you give it:
   `delirium-room --game "<your game folder or .iso>" --export mod-files`
   This writes `character.xrt`, `build.txt` and `taskparams.txt` into `mod-files`.
3. **Get the plugin**: the release's `unity-mod-kit` zip has it ready, in a `DeliriumRagdoll` folder with the SDK library. (Or build it: `dotnet build -c Release` here, and `cargo build --release -p delirium-sdk` for `delirium_physics.dll`.)
4. **Copy the `DeliriumRagdoll` folder into `<Unity game>/BepInEx/plugins/`**, and the three files from step 2 into it.
5. **Run the game once**, then set `GameExe` in `BepInEx/config/delirium_physics.ragdoll.cfg` to your own game's exe (or the disc's `default.xex`). Start the game again and press F9 aiming at a character.

**Never ship the exported files or the game exe with a mod.** They come from the player's own game: every player runs step 2 and sets `GameExe` themselves. Without their own game the plugin doesn't start.

`BepInEx/LogOutput.log` says `ready` when it loaded, or why not.

## When it doesn't pick a character

The plugin finds a skeleton by its bone names (Unity humanoid, Mixamo, 3ds Max Biped, Unreal style names all work). If yours uses other names, the log says which body part has no bone: add it to `BoneMap` in the config, e.g.

```
BoneMap = Pelvis_jnt=Buttocks, Torso=Spine2, Tail=None
```

Parts: Buttocks, Thigh_Left, Shin_Left, Foot_Left (and _Right), Spine0 to Spine3, Neck, Head, Clavicle_Left, Upper_Arm_Left, Lower_Arm_Left, Hand_Left (and _Right). `None` keeps a bone out.

## How it fits any skeleton

- The body is resized to the character's own bone lengths (`nm_build_fit`).
- Bones are only turned, never moved (except the hips), so nothing stretches: no long necks or arms (`nm_rig_pose`).
- Each bone's own roll is measured once in the standing pose, so elbows and knees bend the right way whatever the skeleton's conventions.
- Extra bones (twist bones, fingers, toes, tails) ride along with their parent; missing ones (a skeleton with 3 spine bones, no neck or clavicles) are filled in.

## Big and small characters

`Size` in the config's `[Feel]`:

- `Real`: the body is the character's own size. True physics, but the balancer only keeps its feet within about 0.9 to 1.1 of a person's size.
- `Homer`: the body runs at a person's size and the world is scaled to it, its clock slowed for big characters and sped up for small ones, so a giant keeps its balance and falls at a giant's pace.
- `Auto` (default): `Real` when its legs are within 0.9 to 1.1 of a person's, `Homer` otherwise. Leg length is what counts: a long-legged character falls over at its own size.

## Shooting and getting up (for mods)

```csharp
var nm = Delirium.Ragdoll.Plugin.Instance;
nm.ShootAlong(enemy.transform, camera.position, camera.forward, health: hp);  // hits the bone the aim passes
// (or nm.Shoot(enemy.transform, hitPoint, bulletDirection, health: hp) with an exact hit point)
// health is yours to keep, 0-200: 100+ staggers and fights to stay up, under 100 goes down, under 50 is a kill (limp,
// never gets up by itself). HeadshotKills (on by default) makes any head hit a kill. nm.Head(enemy.transform) says
// where its head is.
nm.LyingStill += (owner, faceUp, facing) => {               // lying still for half a second
    // start your own get-up animation here, then:
    nm.GetUp(owner);                                        // blends from the ragdoll into the animation
};
```

Also: `nm.Explode(owner, at)`, `nm.MeleeHit(...)`, `nm.OnFire(owner)`, `nm.Tase(owner, seconds)`, `nm.HighFall(owner)`, `nm.Kill(owner)`, and
`nm.Sever(owner, point, impulse)` to cut off the limb nearest `point` (the piece goes limp, a leg gone it stays down;
hide the limb's mesh and spawn your own gib yourself). Also `nm.CatchFall(owner)`, `nm.FallDown(owner, heading)`, `nm.Grab(owner, left, leftNormal, right,
rightNormal, hold)`, `nm.JumpRoll(owner)`, `nm.Flinch(owner, from)`, `nm.Tackle(owner, target)`, `nm.SetWater(owner, level)`,
and leashes: `nm.Leash(owner, point, anchor, length)` (returns the part), `nm.LeashLength(owner, part, length)`,
`nm.LeashAnchor(owner, part, anchor, length)`, `nm.Unleash(owner, part)`.
Cars: `nm.RegisterCar(transform, size, playerDriven)` makes anything a car (zero size: its BoxCollider or renderers):
ragdolls meet its box, characters in its way are ragdolled, hits react as the game's and raise `nm.CarHit(who, damage,
car)`; `nm.UnregisterCar`, `nm.CarComing(owner, car)` (brace; config `ArmsOutEarly`), `nm.CarDoor(owner, car)`.
`nm.React(owner, line)` starts any reaction as the Playroom
console's line: `"rollUp start=true"`, `"bodyWrithe start=true"`, `"stopAllBehaviours"` (vectors x,y,z in NM's space, Z up).

Ragdolls within 5 m of each other share one world: they push and pile on each other.

Or set `[Get up] After` to a number of seconds and give the Animator `GetUpFront` / `GetUpBack` triggers: the plugin sets the right one and blends by itself.

## Body

`[Body] Adaptive` (on) fits each kind of character to its own shape: its weight from its skin (where the game left the
mesh readable) and its balance settings, searched once per kind in the background. Off, every body takes the game man's
weight share and balance on its own bones. `[Body] Strength` (1) scales every body's muscles: 1.5-2 for tough enemies
or a movie giant (at 2 a person stands a push of 80 N s instead of 60). One character's: `nm.SetStrength(enemy.transform, 1.5f)`. A custom body
that won't stand: "Tuning a custom body" in `docs/modding.md`.

`[Feel] EnginePhysics` (off; the component's `enginePhysics`): his parts as Unity bodies. NM's own physics still moves him (the game's
exactly); his parts follow it as kinematic rigidbodies that push your rigidbodies and meet triggers, and loose rigidbodies within 5 m hit him
as Object movers (sized as `RegisterMover` does). Checked in Unity 2022.3 PlayMode: every body stands, a thrown ball knocks him, he shoves a crate.

`[Body] RecentreJoints` (off) re-centres each joint's range on the pose the character is handed over in, for rigs whose
animation bends joints past NM's ranges (they would snap on the first step); not the game's. `[Body] Hold` (empty) keeps
joints at least this stiff whatever the reactions set: `mask=stiffness:damping:gravity`, comma separated (`fb` the whole
body, `ua` upper arms, `ub` upper body, `lb` lower body); `fb=6:1:0.5` stops a limp body folding. From code:
`nm_agent_recentre_joint`, `nm_agent_hold`. `[Body] Follow` (empty) keeps joints playing the animation while the rest
ragdolls (a boxer's guard, a held gun): `mask=weight`, `ua=1` the upper arms all the way; the Animator keeps running
under the ragdoll. Give them strength with Hold (`ua=10:1:1`). From code: `nm_agent_follow` + `nm_rig_incoming`.

`[Body] Body` gives every character one of the Playroom's bodies: `Fitted` (its own, the default), `Small`, `Average`,
`Large`, `Lanky`, `Fat`, `Stocky`, `Waddler`, `Guard`, `Brute`, `Brawler`, `Bruiser`, `Random` (`Seed`, 0: each its own;
`Variance`; `Symmetric`) or `Custom` (`CustomBody`: legs, arms, trunk, width, head, neck against the fitted body,
weight in kg, height in m, 0 keeps it). `[Body] Overrides` changes behaviour parameters as the Playroom's Parameters
tab does: `shot.bodyStiffness=7, bodyBalance.useHeadLook=0`. The mesh keeps its look, so pick a body near it.

## How hard hits land

The `[Force]` settings scale every hit: `Shot`, `Melee` and `Explosion` (1 = as given). `Weapons` is a table for
`nm.ShootWeapon(enemy.transform, camera.position, camera.forward, "shotgun", hp)`, which shoots and returns the health
left; `nm.Shotgun(enemy.transform, camera.position, camera.forward, "shotgun", hp)` fans its pellets as the game does.
Each entry is `name=set:push:damage[:pellets:between:clip:accuracy:range]`, or give the game's `weaponinfo.xml`. Empty
(the default) is this table (our own starting points, tune to taste):

| Hit | Call | Strength | Health taken |
|---|---|---|---|
| Pistol | ShootWeapon "pistol" | push 30 N s, PISTOL_WEAPON | 25 |
| SMG | ShootWeapon "smg" | push 25 N s, SMG_WEAPON | 15 |
| Rifle | ShootWeapon "rifle" | push 45 N s, POWERFUL_WEAPON | 35 |
| Shotgun | ShootWeapon "shotgun" | push 90 N s, POWERFUL_WEAPON | 80 |
| Punch | MeleeHit | impulse 40-60 N s | yours |
| Kick / bat | MeleeHit | impulse 80-120 N s | yours |
| Grenade | Explode | radius 5 m, speed 15 m/s | yours |
| Shove | Ragdoll | impulse 60 N s (`[Feel] Push`) | none |

## Limits for now

- Mono games only (BepInEx 5). IL2CPP games need BepInEx 6.
- The ragdoll hits the level's static colliders within 15 m (readable meshes, boxes, terrain), read once when you grab it; spheres, capsules and moving objects are left out.
- The hips bone must be the parent of the spine and legs (true of nearly every Unity character).

## State and events

`nm.State(owner)` says what a ragdoll is doing now: Ragdolled, Balancing, Fallen, CatchingFall, RolledUp, Relaxing, Dead,
InWater, Burning, Tackling (2 diving, 1 holding), LyingFor (s), Centre, Velocity, Mass. Events: `nm.Fell(owner)` once per
fall, `nm.Feedback(owner, name, slot, value)` for NM's own behaviour messages (the Playroom's Feedback list), plus
`LyingStill` and `CarHit`. It also has Settled, Awake, Jumping, TaskRunning, LearnedGetUp, Resting, Stance and Contacts.

The rest of the Playroom's tools are calls on `nm` too: `SetReactions`, `NearMiss`, `SetDive`, `ThrownAt`, `Jump`,
`Balance`, `Stop`, `Sit`, `Pose`, `RollUpRelax`, `Script`, `MuscleRise`, `LearnedGetUp` (`[Data] GetUpNet`),
`LearnedWalk` (`[Data] WalkNet`) with `SetStick`, `SetAttacker` / `GunReady`, `GetParam`, `Raycast`, `CheckSetup`,
`Messages()` (the Unity package's README has each). `[Feel] StepLimit` and `FireReaction` match the Playroom's switches.
A collider's physics material reaches NM (ice slides). A panic in the SDK writes `delirium_physics-crash.txt` next to the config.


## Tuning reactions

Walls, floors and falls need nothing: ragdolls get the level's collision and catch, stagger and brace on it by
themselves. Moving things that hit characters are registered once (`nm.RegisterMover(transform, kind, size, playerDriven, brace, knockDown, centre)`, kinds Car, Object, Train) and each picks
its own: box size and centre, whether characters in its way brace (`brace`) or are knocked down at all (`knockDown`).
Each character's health and agility (`nm.SetToughness(owner, health, agility)`) decide which reaction the game picks when hit (agility under 3 is weak).

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
