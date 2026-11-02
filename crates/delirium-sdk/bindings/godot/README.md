# Godot 4 extension

Active ragdolls for Godot 4.5 and up: they keep their balance, stagger, catch their fall, lie still and get up.
Works from GDScript and C#.

## Set up

1. **Build it**: `cargo build --release` here (or take the ready-built `addons/delirium` folder from the release's `godot-plugin` download).
2. **Copy into your project**: `delirium.gdextension` and the built library into `addons/delirium/`. Open the
   project once in the editor so Godot picks the extension up.
3. **The player's data**: each player exports `character.xrt`, `build.txt` and `taskparams.txt` from their own game
   (`delirium-room --game "<their game>" --export mod-files`). **Never ship them, or the game exe, with your game.**
4. **Project settings** (`project.godot`):

   ```ini
   [delirium]
   game_exe="C:/Games/GTAIV/GTAIV.exe"
   character="C:/.../character.xrt"
   build="C:/.../build.txt"
   task_params="C:/.../taskparams.txt"
   ```

   Paths are absolute or under the project folder. Optional: `size` (Auto, Real, Homer), `balance`, `adaptive`,
   `strength`, `headshot_kills`, `shot_force`, `melee_force`, `explosion_force`.

## Try it: click to fight

Attach `examples/click_to_fight.gd` to your Camera3D and run: aim at a character and left click to shoot (the fourth
shot kills), F to punch, G for a blast. Downed characters get up after 3 s. Its ~70 lines are the starting point for
your own weapons.

## Use

Add a **DeliriumRagdoll** node as a child of the character's `Skeleton3D`, set `character` to the character's root
(empty: the skeleton's scene root).

```gdscript
@onready var nm = $Armature/Skeleton3D/DeliriumRagdoll

nm.shoot_along(cam.global_position, -cam.global_basis.z, 40.0, "POWERFUL_WEAPON", hp)  # hits the bone the aim passes
nm.push(Vector3(60, 0, 0))                     # N s; 60 is a solid shove
nm.melee_hit(point, fist * 30.0, my_head, false)
nm.explode(blast, 5.0, 15.0)
nm.sever(point, Vector3.UP)                    # hide the limb's mesh yourself
nm.lying_still.connect(func(face_up, facing): nm.get_up())
```

Also `ragdoll()`, `shoot(point, dir, force, set, health)`, `on_fire(burning, health)`, `high_fall()`, `kill()`,
`get_up()`, `is_ragdolled()`, `get_hips()`; `get_up_after` gets it up by itself after that many seconds lying still.

- It's a skeleton modifier: while ragdolled it poses the bones over the animation, so your AnimationTree can keep
  running. Stop your own movement code while `is_ragdolled()`.
- **No snaps**: it starts from the animated pose and the character's speed (CharacterBody3D or RigidBody3D), and
  getting up blends into the animation over `get_up_blend` seconds (0.4).
- The skeleton is found by bone names (Mixamo, Unity humanoid, Biped, Unreal style).
- The level is read from static bodies' box and triangle-mesh shapes within 15 m; ragdolls near each other share a
  world and pile on each other.

`test.gd` is a headless check (`godot --headless --fixed-fps 60 --path <project> --script test.gd`): a skeleton
stands, takes a shove, goes down to a hard push, a shot and a leg cut, and gets up.
