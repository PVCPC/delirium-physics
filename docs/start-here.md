# Modding: start here

Delirium turns the characters in your game into ragdolls that fight to stay up: they stagger when shot, stumble
when pushed, put their hands out when they fall, lie still, and get up. This page gets you from nothing to your first
ragdoll. Everything else is in the [full reference](modding.md).

## What you need

- **Your own copy of the original game** (the PC version from Steam, or the Xbox 360 disc). Delirium reads its
  numbers from your copy, so it won't start without one. This also means:
  **never ship the game's files, or the files you export from it, with your game or mod.** Each player uses their own.
- **One of these**, depending on where you want ragdolls:

| You're making | Download from the [release page](https://github.com/PVCPC/delirium-physics/releases/latest) | Then read |
|---|---|---|
| A mod for a Unity game you didn't make (BepInEx) | `delirium-unity-mod-kit` | [its README](../crates/delirium-sdk/bindings/bepinex/README.md) |
| Your own Unity game | `delirium-unity-package` | [its README](../crates/delirium-sdk/bindings/unity/README.md) |
| Your own Unreal Engine 5 game | `delirium-unreal-plugin` | [its README](../crates/delirium-sdk/bindings/unreal/DeliriumRagdoll/README.md) |
| Your own Godot 4 game | `delirium-godot-plugin` | [its README](../crates/delirium-sdk/bindings/godot/README.md) |
| Anything else (your own engine, C, C++, C#, Python, Rust) | the source code | [Any engine, step by step](modding.md#any-engine-step-by-step-unreal-source-your-own) |

Plus `delirium-room` (the Playroom) for your computer: you need it once, in step 1.

## The three steps

**1. Export your files.** Unzip `delirium-room` and run, in that folder:

```
delirium-room --game "C:\path\to\your\game" --export mod-files
```

Give it your game's folder, or the disc image (`.iso`). It writes three files into `mod-files`: `character.xrt`,
`build.txt` and `taskparams.txt`. Keep them on your computer; your game or mod loads them from there.

**2. Install the plugin** for your engine, as its README says (a few minutes: copy a folder, set where your game
and the three files are).

**3. Hit someone.** Each plugin has the same handful of calls. In Unity, for example:

```csharp
var nm = Delirium.Ragdoll.DeliriumRagdolls.Instance;
nm.ShootAlong(enemy.transform, cam.position, cam.forward, health: 60);   // shot: staggers or falls
```

| Call | What happens |
|---|---|
| Ragdoll | Takes the character over with no hit: it stands and keeps its balance |
| Shoot / ShootAlong | A bullet. Pass the health it has left: lots staggers, little drops it, a head shot kills |
| Push | A shove (60 is a solid one, 200 knocks most people down) |
| MeleeHit | A punch or club |
| Explode | Thrown by a blast |
| OnFire | Flails and drops while burning |
| HighFall | Arms out, legs ready, for a long drop |
| Sever | Cuts off the nearest limb (you hide its mesh) |
| Kill | Goes limp and stays down |
| GetUp | Hands the character back to your animations |

Godot spells them `shoot_along`, `push`, `get_up` and so on.

## When it doesn't work

Every plugin says why in its log (Unity's Console, BepInEx's `LogOutput.log`, Unreal's Output Log, Godot's Output):

| The log says | Fix |
|---|---|
| `call nm_unlock with your own ... exe first` | Set the plugin's **Game Exe** to your game's exe (or the disc's `default.xex`). |
| `isn't a version the numbers are read from` | That exe is a different version: use the Steam one or the disc's. |
| A file can't be found | Check the paths to the three files from step 1 (full paths are safest). |
| A body part has no bone | Your skeleton's bone names aren't recognised. Unity and BepInEx: add them to **Bone Map**, e.g. `Pelvis_jnt=Buttocks`. Unreal and Godot: rename the bones, or [open an issue](https://github.com/PVCPC/delirium-physics/issues). |

Other problems:

- **The character twists into a knot.** Its bones were matched to the wrong body parts; see the bone row above.
- **It falls but never reacts.** Make sure the three files from step 1 are all set, including `taskparams.txt`.
- **Very big or tiny characters fall over.** Leave **Size** on Auto: it runs them at a person's size.

Still stuck? [Open an issue](https://github.com/PVCPC/delirium-physics/issues) with the log.

## Words you'll see

- **Ragdoll**: a character moved by physics instead of animation. Delirium's ones use their muscles to stay up.
- **Behaviour**: one thing a ragdoll can do (keep its balance, catch its fall, brace...). Calls start the right ones.
- **The three files**: what step 1 exports. `character.xrt` is the body's muscles and reactions, `build.txt` its
  shape and weight, `taskparams.txt` how it reacts to hits.
- **Part**: one of the 21 pieces of the ragdoll's body (hips, thighs, spine, head, arms, hands...).
- **Lead / Follow**: who moves the body. Lead (the usual): Delirium does, and you copy its pose to your character.
  Follow: your engine's physics does, and Delirium adds the muscle forces.
