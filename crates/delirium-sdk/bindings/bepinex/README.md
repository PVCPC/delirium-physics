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

Also: `nm.Explode(owner, at)`, `nm.MeleeHit(...)`, `nm.OnFire(owner)`, `nm.HighFall(owner)`, `nm.Kill(owner)`, and
`nm.Sever(owner, point, impulse)` to cut off the limb nearest `point` (the piece goes limp, a leg gone it stays down;
hide the limb's mesh and spawn your own gib yourself).

Ragdolls within 5 m of each other share one world: they push and pile on each other.

Or set `[Get up] After` to a number of seconds and give the Animator `GetUpFront` / `GetUpBack` triggers: the plugin sets the right one and blends by itself.

## Body

`[Body] Adaptive` (on) fits each kind of character to its own shape: its weight from its skin (where the game left the
mesh readable) and its balance settings, searched once per kind in the background. Off, every body takes the game man's
weight share and balance on its own bones. `[Body] Strength` (1) scales every body's muscles: 1.5-2 for tough enemies
or a movie giant (at 2 a person stands a push of 80 N s instead of 60). From code: `nm_agent_set_strength`.

## How hard hits land

The `[Force]` settings scale every hit: `Shot`, `Melee` and `Explosion` (1 = as given). `Weapons` is a table for
`nm.ShootWeapon(enemy.transform, camera.position, camera.forward, "shotgun", hp)`, which shoots and returns the health
left. Each entry is `name=set:push:damage`. Starting points (our own, tune to taste):

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
