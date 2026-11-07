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
   `strength`, `engine_physics` (his parts as Godot bodies: NM's own physics still moves him, the parts follow it and push your RigidBody3Ds, and loose RigidBody3Ds within 5 m hit him as movers (MOVER_OBJECT, boxed by their first shape's bounds)), `headshot_kills`, `shot_force`, `melee_force`, `explosion_force`, `recentre_joints` (re-centre the joints
   on the hand-over pose), `hold` (`mask=stiffness:damping:gravity`, e.g. `fb=6:1:0.5`: joints at least this stiff), `follow` (`mask=weight`,
   e.g. `ua=1`: those joints keep playing the animation while the rest ragdolls; pair with `hold`, `ua=10:1:1`).

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
var p = nm.leash(hand, hook, 2.0)              # a leash: hangs and swings, never further than 2 m
nm.leash_length(p, 1.0)                        # shorter: reels him in; nm.unleash(-1) lets go
nm.lying_still.connect(func(face_up, facing): nm.get_up())
```

Also `ragdoll()`, `shoot(point, dir, force, set, health)`, `on_fire(burning, health)`, `tase(seconds)`, `high_fall()`, `kill()`,
`get_up()`, `is_ragdolled()`, `get_hips()`, `react(line)` (any reaction as the Playroom console's line: `"rollUp start=true"`,
`"bodyWrithe start=true"`, `"stopAllBehaviours"`), `catch_fall()`, `fall_down(heading)`, `grab(left, left_normal, right,
right_normal, hold)`, `jump_roll()`, `flinch(from)`, `tackle(target, low, keep)`, `set_water(level)`;
cars: `DeliriumRagdoll.register_car(node, size, player_driven)` (anything you call a car; zero size: its BoxShape3D's),
`unregister_car(node)`, `car_coming(car)` (brace), `car_door(car)`, signal `car_hit(damage, car)`; setting `arms_out_early`; `get_up_after` gets it up by itself after that many seconds lying still.
A turning mover (a wrecking ball, a turntable) carries what it touches round with it.

`strength` (0: the `delirium/strength` setting's) makes one character's muscles stronger, 1.3-2 for a heavy custom
body; it takes effect at once. A custom body that won't stand: "Tuning a custom body" in `docs/modding.md`.

**The Playroom's other tools:**

- Weapons: `shoot_weapon(name, point, dir, health)` returns the health left; `shotgun(name, from, aim, health, seed)` fans
  the weapon's pellets as the game does and returns how many struck. `DeliriumRagdoll.weapon_names()`,
  `DeliriumRagdoll.load_weapons(text_or_xml)`; setting `delirium/weapons` (`name=set:push:damage[:pellets:between:clip:accuracy:range]`,
  or the game's `weaponinfo.xml`; empty: pistol, smg, rifle, shotgun).
- Reactions: `set_reactions(REACT_WINDMILL | REACT_DODGE | ...)` (the Playroom's extra reactions), `near_miss(from, dir)`,
  `set_dive(gap)` (shot at again soon after a near miss, it dives), `thrown_at(at, velocity, look, avoid)`.
- Moves: `jump(dive, dir)`, `balance(look)`, `stop()`, `sit(look, seat)`, `pose(low, high)` (the animation's pose as muscle
  targets), `roll_up_relax(stiffness, damping, roll_ms)`, `script(ms)` (only your `react` messages move it).
- Getting up: `muscle_rise(on, help)` (its own muscles, the animation's get-up clip as targets), `learned_getup(net)`
  (the release's `getup.net`; setting `delirium/getup_net`); `learned_walk(net, cycle)` with `set_stick(on, x, y, camera)`.
- Fights: `set_attacker(other_ragdoll, armed)` (it looks at them; armed, `gun_ready()` says when to fire).
- Reading: `get_param(message, param)`, `raycast(from, to)` ({part, point, normal}), `check_setup()`,
  `DeliriumRagdoll.messages()` (every message and parameter with its kind, default and doc, for pickers).
- Settings `delirium/step_limit` (the game's balance runs out of steps; off: the Playroom's endless stepping) and
  `delirium/fire_reaction`. A physics material's friction and bounce on a static body reach NM (ice slides). A panic in
  the SDK writes `user://delirium_physics-crash.txt`.

- **Bodies and parameters (the Playroom's):** `body` is `Fitted` (its own skeleton's, the default), `Small`,
  `Average`, `Large`, `Lanky`, `Fat`, `Stocky`, `Waddler`, `Guard`, `Brute`, `Brawler` or `Bruiser` (that shape of this
  character), `Random` (`body_seed`, 0: its own; `body_variance`; `body_symmetric`) or `Custom` (`custom_body`: legs,
  arms, trunk, width, head, neck against the fitted body, weight in kg, height in m, 0 keeps it). `overrides` changes
  behaviour parameters as the Playroom's Parameters tab does (`shot.bodyStiffness=7, bodyBalance.useHeadLook=0`);
  `set_override(behaviour, param, value)` and `clear_overrides()` in code. The mesh keeps its look (its bones follow
  the body's turns), so pick a body near the mesh.
- It's a skeleton modifier: while ragdolled it poses the bones over the animation, so your AnimationTree can keep
  running. Stop your own movement code while `is_ragdolled()`.
- **No snaps**: it starts from the animated pose and the character's speed (CharacterBody3D or RigidBody3D), and
  getting up blends into the animation over `get_up_blend` seconds (0.4).
- The skeleton is found by bone names (Mixamo, Unity humanoid, Biped, Unreal style); `bone_map` fixes the rest (`Pelvis_jnt=Buttocks, Tail=None`).
- The level is read from static bodies' box and triangle-mesh shapes within 15 m; ragdolls near each other share a
  world and pile on each other.

`test.gd` is a headless check (`godot --headless --fixed-fps 60 --path <project> --script test.gd`): a skeleton
stands, takes a shove, goes down to a hard push, a shot and a leg cut, and gets up; then every call above once, and
ice under a shove (`DELIRIUM_NETS=<folder with getup-reward.net and walk.net>` adds the learned get-up and walk).

## State and events

`get_state()` returns a Dictionary: ragdolled, balancing, fallen, catching_fall, rolled_up, relaxing, dead, in_water,
burning, tackling (2 diving, 1 holding), lying_for (s), centre, velocity, mass, settled, awake, jumping, task_running,
learned_getup, resting, stance (where the balancer stands; absent when not balancing), contacts (21 flags). Signals: `fell()` once per fall,
`feedback(name, slot, value)` for NM's own behaviour messages (the Playroom's Feedback list), plus `lying_still` and
`car_hit`.


## Tuning reactions

Walls, floors and falls need nothing: ragdolls get the level's collision and catch, stagger and brace on it by
themselves. Moving things that hit characters are registered once (`DeliriumRagdoll.register_mover(node, MOVER_*, size, player_driven, brace, knock_down, centre)`, kinds Car, Object, Train) and each picks
its own: box size and centre, whether characters in its way brace (`brace`) or are knocked down at all (`knockDown`).
Each character's health and agility (`hit_health, hit_agility`) decide which reaction the game picks when hit (agility under 3 is weak).

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
