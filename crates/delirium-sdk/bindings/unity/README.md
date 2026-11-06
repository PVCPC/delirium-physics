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
7)` and `ClearOverrides(owner)` for one. The mesh keeps its look (its bones follow the body's turns), so pick a body near the mesh: a Brawler under a tall mesh
shows its feet off the ground.

The full call list is in `docs/modding.md`. The BepInEx mod (`../bepinex`) is built from this same
`Runtime/DeliriumRagdolls.cs`.
