# Unity package

The same ragdolls as the BepInEx mod, for your own Unity game.

## Set up

1. **Add the package**: Window > Package Manager > + > Add package from disk, pick this folder's `package.json`.
2. **The SDK library**: put `delirium_physics.dll` in `Runtime/Plugins/x86_64/` here (the release's `unity-package` download has it in place).
3. **The player's data**: each player exports `character.xrt`, `build.txt` and `taskparams.txt` from their own game
   (`delirium-room --game "<their game>" --export mod-files`). Your game reads them from `StreamingAssets`, or
   any absolute path you set. **Never ship them, or the game exe, with your game.**
4. **Add `DeliriumRagdolls`** to one object in your first scene and set `Game Exe` to the player's game exe.
   The Console says `[Delirium] ready`, or why not.

## Try it: click to fight

Package Manager > Delirium ragdolls > Samples > **Click to fight** > Import. Put `ClickToFight` on your camera and
press Play: aim at a character and left click to shoot (the fourth shot kills), F to punch, G for a blast. Downed
characters get up after 3 s. Its ~80 lines are the starting point for your own weapons.

## Use

```csharp
var nm = Delirium.Ragdoll.DeliriumRagdolls.Instance;
nm.ShootAlong(enemy.transform, cam.position, cam.forward, health: hp);   // hits the bone the aim passes
nm.MeleeHit(enemy.transform, hitPoint, fist * 30f, player.head.position);
nm.Explode(enemy.transform, blast, radius: 5f);
nm.Sever(enemy.transform, hitPoint, impulse);   // hide the limb's mesh yourself
int p = nm.Leash(enemy.transform, hand, hook, 2f); // a leash: hangs and swings, never further than 2 m
nm.LeashLength(enemy.transform, p, 1f);          // shorter: reels him in; nm.Unleash(enemy.transform) lets go
nm.LyingStill += (who, faceUp, facing) => nm.GetUp(who);
```

Every call takes the character's root (the object with its Animator). The skeleton is found by bone names (Unity
humanoid, Mixamo, Biped, Unreal style); add others to `Bone Map`.

- **No snaps**: the ragdoll starts from the animated pose and the character's speed (NavMeshAgent, Rigidbody or
  CharacterController), and getting up blends into the Animator over `Get Up Blend` seconds. Name your get-up
  animations' triggers in `Get Up Front` / `Get Up Back`.
- Ragdolls near each other share one physics world, so they push and trip over each other.
- `Ragdoll Key` / `Release Key` are test keys: aim and press to ragdoll whoever you look at (None: off).

**Bodies and parameters (the Playroom's):** `Body` picks every character's body: `Fitted` (its own skeleton's, the
default), `Small`, `Average`, `Large`, `Lanky`, `Fat`, `Stocky`, `Waddler`, `Guard`, `Brute`, `Brawler` or `Bruiser` (that shape of
each character), `Random` (`Body Seed`, 0: each its own; `Body Variance`; `Body Symmetric`) or `Custom` (`Custom Body`:
legs, arms, trunk, width, head, neck against the fitted body, weight in kg, height in m, 0 keeps it). `SetBody(owner,
"Brawler")` sets one character's. `Overrides` changes behaviour parameters for every character, as the Playroom's
Parameters tab does (`shot.bodyStiffness=7, bodyBalance.useHeadLook=0`); `SetOverride(owner, "shot", "bodyStiffness",
7)` and `ClearOverrides(owner)` for one; `SetStrength(owner, 1.5f)` makes one character's muscles stronger (0: back to
`Strength`). A custom body that won't stand: "Tuning a custom body" in `docs/modding.md`. The mesh keeps its look (its bones follow the body's turns), so pick a body near the mesh: a Brawler under a tall mesh
shows its feet off the ground.

**The Playroom's other tools** (each takes the character's root):

- Weapons: `ShootWeapon(owner, from, aim, "pistol", hp)` returns the health left; `Shotgun(owner, from, aim, "shotgun", hp)`
  fans the pellets as the game does and returns how many struck. `WeaponNames()`, `LoadWeapons(textOrXml)`; the
  `Weapon Table` field (`name=set:push:damage[:pellets:between:clip:accuracy:range]` or a `weaponinfo.xml`; empty: pistol,
  smg, rifle, shotgun).
- Reactions: `SetReactions(owner, Reactions.Windmill | Reactions.Dodge)`, `NearMiss(owner, from, dir)`, `SetDive(owner,
  gap)`, `ThrownAt(owner, at, velocity)`.
- Moves: `Jump(owner, dive, dir)`, `Balance(owner, look)`, `Stop(owner)`, `Sit(owner, look, seat)`, `Pose(owner, low, high)`,
  `RollUpRelax(owner)`, `Script(owner, ms)` (only your `React` lines move it).
- Getting up: `MuscleRise(owner)` (its own muscles, the Animator's get-up clip as targets), `LearnedGetUp(owner)` (`Get Up
  Net`, the release's `getup.net`); `LearnedWalk(owner)` (`Walk Net`) with `SetStick(owner, true, x, y, cameraYaw)`.
- Fights: `SetAttacker(owner, other, armed)`; `GunReady(owner)` says when to fire.
- Reading: `GetParam(owner, message, param)`, `Raycast(owner, from, to, out point, out normal)`, `CheckSetup(owner)`,
  `Messages()` (every message and parameter, for pickers). `State(owner)` also has Settled, Awake, Jumping,
  TaskRunning, LearnedGetUp, Resting, Stance and Contacts.
- `Step Limit` (the game's balance runs out of steps; off: the Playroom's) and `Fire Reaction`. A collider's physics
  material reaches NM (ice slides); a turning mover carries what it touches round. A panic in the SDK writes
  `delirium_physics-crash.txt` in `Application.persistentDataPath`.

The full call list is in `docs/modding.md`. The BepInEx mod (`../bepinex`) is built from this same
`Runtime/DeliriumRagdolls.cs`.
