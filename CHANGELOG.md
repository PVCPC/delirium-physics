# Changelog

## Unreleased

### Added
- **Every Playroom tool in the SDK and every plugin** (Unity, BepInEx, Godot, Unreal), so a mod can do what the
  Playroom does:
  - **Weapons table**: pistol, SMG, rifle and shotgun by default, or your own (`name=set:push:damage[:pellets:...]`)
    or GTA IV's own `weaponinfo.xml`. `ShootWeapon` shoots one and returns the health left. `Shotgun` fans its
    pellets the way GTA IV does and hits with all of them as one shot.
  - **Reactions**: the Playroom's extra reactions (windmill, dodge, look at the hit and the rest) as one switch
    set, near misses, a dive when shot at again soon after one, and ducking or catching something thrown at him.
  - **Moves**: jump and dive, GTA IV's balance, stop, sit, hold a pose from the animation, roll up then go limp,
    and script control (only your messages move him).
  - **Getting up**: on his own muscles with the get-up animation as targets, or as the learned network does
    (`getup.net`). **Learned walk** (`walk.net`) under a stick, the Playroom's drunk control.
  - **Fights**: who he's up against (he looks at them) and, if they're armed, when they're ready to fire.
  - **Reading**: any behaviour parameter as it runs, raycasts against the body, a setup check, and the full list
    of messages and parameters with defaults and descriptions (for pickers in your tools).
  - **Full state**: settled, awake, jumping, task running, learned get-up, resting, where the balancer stands and
    which parts touch something.
  - **Switches**: step limit (GTA IV's balance runs out of steps) and fire reaction.
- **Strength per character** in every plugin (`strength`, `SetStrength`), for a heavy or oddly shaped body that
  can't take a hit. **Bone Map** in Godot and Unreal (Unity had it) for bones the name guess misses.
- **Tuning a custom body**, a new section in [Modding](docs/modding.md): what to try, in order, when a body won't
  stand.
- **Physics materials reach the ragdoll** in every plugin: a collider's friction and bounce (ice slides). Turning
  movers (a wrecking ball, a turntable) carry what they touch round with them.
- **Engine physics** in Unity, Godot and Unreal (`EnginePhysics`): Delirium's own physics moves him, his parts
  push your objects and loose objects hit him. Every body stands this way, Brute, Brawler and Bruiser included.
- **Cars and movers in the plugins**: `RegisterCar` (anything can be a car) with car hits as GTA IV's, a
  `CarHit` event, bracing for a car coming (`ArmsOutEarly` throws the arms out sooner), dragged from a car door,
  and `RegisterMover` for trains and objects (what kind, brace, knock down).
- **More reactions in the plugins**: catch fall, fall down, grab, jump roll, flinch, tackle, water, the taser, and
  `React(line)` for any reaction as a Playroom console line.
- **Follow** in every plugin: the animation keeps running under the ragdoll and the ragdoll aims at it. **Hold**
  and **Recentre joints** settings.
- **State and events** in every plugin: what a ragdoll is doing (`State` / `GetState` / `get_state`), `Fell` once
  per fall and `Feedback` for the behaviours' own messages.
- **SDK**: crowds stepped over every core (`nm_world_step_agents`) in linear time, the same result on any number
  of threads; `nm_crash_report` (a panic writes a crash note to a folder you pick); `nm_msg_send_text`;
  `nm_agent_follow`, `nm_agent_hold` and `nm_agent_recentre_joint` (from 0.5.0's bug reports);
  `nm_agent_get_zero_pose` and `nm_agent_get_part_body` for building the body in your own physics engine; and
  `nm_world_set_box_kinds` / `nm_world_set_box_spins` for movers. The C++ header wraps every call.
- **Bruiser** body: Brawler-like with a bigger head, an even trunk, small shoulders and short legs set wide.
- **Skeleton skin** with hollow black eye sockets, and a new cartoon skull model.
- **Arms out early when bracing**, a new extra reaction toggle.
- The shotgun's pellets fan out and each push him as GTA IV's do.
- Thrown and melee weapons work in camera aim, with GTA IV's melee damage.
- `--custom` start flag: Custom sizes from the first frame, for recordings.
- After a graphics error or a system crash the room leaves `crash.txt` too; crash reports carry the live settings
  and the last 10 changes.

### Changed
- Ragdolls touching the ground follow GTA IV more closely (contact points on the ground's surface margin): a
  lying body used to drift up to 11 cm from where GTA IV puts it.
- Heavy bodies' muscles grow with their weight, so they stand still instead of swaying and stepping.
- The Brute weighs 140 kg (was 160): at 160 he went down to three pokes in a row three times in four.
- Short legs thin the belly with them on Custom and Random bodies (no gut over the crotch); Random keeps its drawn
  leg length.
- Body profiles' feet go by their hips' spread, and widened bodies move each limb out whole.
- The room uses less memory: about 230 MB less RAM and 320 MB less video memory when idle. Pool ripples and foam
  cost 0.8 ms a frame (was 2.3).
- The Parameters tab starts each behaviour from its row.
- Push tests throw both arms out straight; Push from front braces on the slab itself.

### Docs
- The README and the room say exactly what game you need: Grand Theft Auto IV: The Complete Edition for PC (Steam,
  version 1.2.0.59) or the original Xbox 360 disc, from your own copy.

### Fixed
- Every shot spun him left: the random numbers now run on across resets.
- Sized bodies' balance probes the ground as far as their legs reach.
- Ball joints' limits are applied in the right order.
- Plugins: ragdolls share a world only within one engine world (a dead ragdoll from another world tripped the
  next one). Unreal frees ragdolls when their actor is destroyed. Godot movers are boxed by every shape, turned.
- Unity engine physics stands (inertia set before torque) and tears down destroyed characters safely.
- A destroyed SDK agent leaves its world at once.
- Shots, pushes and the balancer's impulses reach a host's own physics bodies in FOLLOW mode (they were lost), and
  cutting a limb works there.
- The Unreal plugin builds on UE 5.8.
- The renderer no longer crashes on a view without the deferred pass. The anti-aliasing menu lists only what ray
  tracing keeps, and the ReSTIR option says why path tracing doesn't use it.
- Wrapped rows in the room's panels stay clear of the scroll bar.

## 0.6.0

### Added
- **Custom bodies**: Character tab, body Custom. Sliders for legs, arms, trunk, width, head, neck and weight, and
  an optional height. A see-through preview shows the next spawn while you move them. Short legs, long legs, wide
  and heavy bodies all stand, walk and take every reaction. Script: `!custom legs,arms,trunk,width,kg[,head]`,
  `!neck n`.
- **Neck slider** (Custom only, 0.5 to 6 times): a longer neck sits the head higher and is thinner. It is one
  stiff bone, hinged at both ends, so a long neck swings but doesn't bend in the middle. Homer (Average) always
  keeps the game's own neck.
- **Brawler** body: short thick legs, short arms, a wide trunk and a big head.
- **Random** body is repeatable: `!seed n` and `!variance v` in scripts.
- **Parameters tab** (by [LoAlCo](https://github.com/LoAlCo), from PR #42): every behaviour's parameters with their defaults and what they do. A changed value goes
  with that behaviour every time it's sent, from buttons, hits, shots or the console.
- Up to 50 Homers.
- **SDK, the same bodies as the Playroom**: `nm_build_sized` (Custom), `nm_build_named` (Small, Average, Large,
  Lanky, Fat, Stocky, Waddler, Guard, Brute, Brawler), `nm_build_random` (seed, variance, symmetric),
  `nm_build_fit_neck` (fit a skeleton with a long neck) and `nm_build_set_head` (raise and grow the head for a tall
  skull). `nm_agent_set_override` / `nm_agent_clear_overrides` are the Parameters tab for one character,
  including the messages the built-in tasks send. See [Modding](docs/modding.md).
- **Unity, Godot, Unreal and BepInEx plugins**: a `Body` setting (Fitted, any of the Playroom's bodies, Random or
  Custom) and `Overrides` (the Parameters tab), per character from code too: `SetBody`, `SetOverride`,
  `ClearOverrides` (Godot `body`, `set_override`).
- After a crash the room writes `crash.txt` (system, graphics card, settings) beside it, and the next start offers
  Report crash with a GitHub issue filled in.

### Fixed
- **PC (Steam) export**: `delirium-room --export` now writes a working `taskparams.txt` (it is packed and
  encrypted in `pc/data/game.rpf`), and the PC character file is read in its own byte order everywhere, so the SDK
  no longer gets NaN bodies from it.
- Fitted bodies' joints pivot where the fit puts them (they pivoted at the game man's places).
- The Waddler holds his own weight (his pelvis hung past his short legs and he sat on it).
- Random bodies no longer fly around (default spread 1.5 times, was 3) or stack their spine into blobs, and Reset
  pose stands them as built.
- The mouse pull grows with a heavy body's weight.
- The Unity package compiles on Unity 6.6 (it used `GetInstanceID`, an error there).
- SDK: Custom, Random and named bodies made from a fitted character keep its size scale.

### Docs
- SDK: `nm_agent_create` puts the body's lowest point on the position you give; pass `nm_agent_step` a fixed
  1/60 s, not the frame's wall-clock time.

## 0.5.2

### Fixed
- **About 3.5 times the frame rate with a sky on** (the Bevy sky is the default): the room's portal views each
  worked out the sky's lighting every frame, even when no portal was open. On an RTX 3090 at low settings with the
  Bevy sky: 72 FPS before, 268 now.
- Shadows all the way off (Shadow quality "off") now turns off contact shadows too; they still drew a grainy
  shadow. Their slider greys out while shadows are off.

### Changed
- `LICENSE`: an added permission to combine Delirium with Unreal Engine and share the result, free of charge only.
  No selling, and never any game data.
- The SDK's comments and examples load the exported `character.xrt`. On PC the game keeps it loose, as a `.art`
  file; `delirium-room --export` writes it out for you.

## 0.5.1

### Fixed
- The Playroom quit at the start with ray tracing on and Resolution not at 100%. Resolution now steps aside while
  ray tracing is on, as it does for DLSS, FSR 3 and XeSS.

### Added
- **Click to fight** examples for each engine: aim at a character, click to shoot, F to punch, G for a blast.
  Unity: the package's Click to fight sample. Godot: `examples/click_to_fight.gd`. Unreal: the Delirium Click To
  Fight component. Each plugin's README says how to add it.
- [Modding: start here](docs/start-here.md): which download to use, your first ragdoll in three steps, and what
  to do when the log shows an error.
- `llms.txt`, so AI tools can find the docs and the API.

### Changed
- The modding guide's examples start the SDK with the player's own copy of the game, as the public build needs.


## 0.5.0 - The Modders Guide

Delirium in other games and engines: Unity, Unreal, Godot, or your own.

### Added
- **Plugins for three engines**, each with the same calls (shoot, shoot along the aim, push, punch, explode, set on
  fire, long fall, cut off a limb, kill, get up):
  - Unity: the `DeliriumRagdolls` component in the Unity package (`bindings/unity`), for your own game.
  - Unreal Engine 5: the Delirium Ragdoll component (`bindings/unreal`).
  - Godot 4.5+: the `DeliriumRagdoll` node, from GDScript or C# (`bindings/godot`).
  - Mono Unity games (BepInEx 5): aim at any character and press F9. Ready-built in the `unity-mod-kit` download.
- **No snaps**: a character goes into ragdoll in the pose its animation had it in, moving as it was moving
  (`nm_rig_take`), and blends back into its animation when it gets up.
- **Any skeleton, any bone count**: the rig (`nm_rig_*`) drives your game's own bones by turning them only, so
  nothing stretches. Elbows and knees bend the right way whatever the skeleton's conventions; twist bones, fingers
  and tails ride along; fewer spine bones, or no neck or clavicles, work too. Unity humanoid, Mixamo, 3ds Max Biped
  and Unreal bone names are recognised. `nm_rig_check` says why a skeleton contorts.
- **Odd shapes and sizes**: bodies are fitted to the character's own bone lengths and weighed from its skin; each
  kind of body gets its own balance settings, searched once in the background and kept for later runs. Giants and
  tiny characters run at a person's size in a scaled world, so they balance and fall at their own pace.
- **New reactions in the SDK**: explosions, catching fire (fire spreads between bodies), melee hits, long falls,
  jumping out of a moving car, bracing for a car, being hit by a car, being dragged from a car, grabbing, tackles,
  falling down, cut-off limbs, water, and pushed down stairs he gives up and rolls. Headshots kill.
- Bodies in one world push and pile on each other; dead bodies lying still rest until touched.
- Strength: every body's muscles times a number (1.5-2 for tough enemies).
- The Playroom: a Random body (with a Symmetric toggle and a Variance slider) and three odd-shaped peds
  (Waddler, Guard, Brute) in the body picker.
- The Playroom's `--export <folder>` writes the three files a mod loads out of your own game. Never ship them with
  a mod: each player exports their own.
- The Playroom writes `crash.dmp` when it crashes in a way it can't catch, and the launcher asks you to send it.
- Modding guide: start here, any engine step by step, every call with what it does.

### Changed
- The Unity package's `NmCharacter` only turns bones below the hips (moving them stretched skeletons shaped
  differently from the body).

## 0.2.7

### Added
- Homer fights with his fists on the game's own fighting AI: it picks when he swings, blocks, dodges and counters,
  reading what the Hero is doing, with his model's own fighting style.
- The Homer button now cycles three modes: Passive (only takes hits), Fights (the game's AI) and Aggressive (the
  0.2.6 fight: swings and guards all the time).
- Gun AI button (Delirium / Game): on Game, Homer with a gun uses the game's choices (shoot, back off, or charge in
  with his fists) and its timing between shots.

### Changed
- Engine updated to the latest upstream engine code.

### Fixed
- Even fight (#37) did nothing while Homer was passive; turning it on now makes him fight back.
- A held bat or knife sat tilted, sticking out of the fingers; it now runs through the fist.

## 0.2.6

### Changed
- The engine's numbers are now read straight from your own game's program: the PC version's GTAIV.exe (Steam) or
  default.xex on the Xbox 360 disc (its key is fetched from the internet the first time). The repo holds none of them,
  and the physics is unchanged, bit for bit, on both.
- The SDK's `nm_unlock` takes that GTAIV.exe or default.xex (or the folder either is in); call it first.
- The PS3 version isn't supported for now.

## 0.2.5

### Changed
- The game's whole numbers, and the numbers in Homer's behaviour messages, now load from your own game files too, as
  the decimals already did: the repo holds none of them. Physics runs about 3% slower for it.

### Fixed
- DirectX 12 mode (#32): most of Homer and the floor were missing; drawn correctly now.
- DirectX 12 mode starts several times faster: after a fallback, the launcher fetches Microsoft's DirectX shader
  compiler (from Microsoft; not part of Delirium).
- Older Vulkan drivers (#32): the fallback Vulkan mode asks the driver only for the features the room needs (Intel HD
  530's driver crashed on all of them).
- The safer graphics modes start only after two driver crashes in a row, and only for that graphics card and driver: a
  new card or a driver update gets full graphics again.

## 0.2.4

### Fixed
- Intel integrated graphics (#32): Safe Mode and the crash fallback no longer use bindless textures, for older
  drivers that showed a black scene.
- Safe Mode, and the first start on integrated graphics, render at 50% upscaled with FSR 1 instead of 25%.

## 0.2.3

### Fixed
- Intel integrated graphics (#32): 0.2.1's DLSS and XeSS hooks crashed some drivers as the room started. On a crash the
  room now starts again on plain Vulkan, as 0.2.0 did (no DLSS, FSR 3 or XeSS), and only then tries DirectX 12. Safe
  Mode always starts on plain Vulkan.
- The first start on integrated graphics uses Safe Mode's lowest settings.

## 0.2.2

### New
- **Skate 3 mode** (J): Homer on a skateboard, run by the Skate 3 Rust Engine on your own copy of Skate 3 (converted
  from your disc at setup; nothing of the game ships). Skate 3's own riding, walking, running, jumping, board throw,
  drop and recall, and its camera; the full pad on the keyboard. He rides through portals and keeps the portal gun.
  New skate park stage.
- **Hall of Meat**: bails scored as Skate 3 does, with its results screen, injuries, X-ray and film. Bails are Skate
  3's own wipeouts or Delirium's ragdoll (Settings); both are scored.
- **Additional experimental reactions**: each one has its own switch.

### Fixed
- Cards whose Vulkan driver crashes the room as it starts (some Intel integrated graphics, #32): the room
  switches itself to DirectX 12 and starts again. Safe Mode always runs on DirectX 12.
- Skate park ramps: no catch at the bottom, no bump at the top.

## 0.2.1

### New
- **Real water**: a GPU water simulation in the pool. Homer, the Hero and objects push it and are pushed, it spills
  over the floor as a clear film and leaves a wet trail that dries. Ripples, foam, spray, mist, caustics, refraction,
  the surface seen from below and the haze under it each have their own toggle. Water puts fires out.
- **Spawn Homers**: up to 8, of any body type, from the top bar (Shift+N spawns, Ctrl+N removes, N switches). Every
  Homer burns, bleeds, tears, swims and gets up like the first.
- **Fire**: voxel or realistic flames, smoke and embers, lighting the room and Homer. Burning chars his skin.
- **Layered damage**: hits tear Homer's skin to red muscle, then to the bone.
- **Blood**: chunky pixel splatter that lands and stays a while; wounds spurt with his heartbeat (Extra bloody toggle).
- **Taser and stun baton**: a hit locks him up and jolts him, his cartoon X-ray skeleton flashing through a black body.
- **Skeleton view styles**: Lines, Muscles and Stick.
- **Ray tracing and path tracing** (Video): ray-traced lighting, or full path tracing, with DLSS Ray Reconstruction on
  RTX cards. Fire, blasts and the coloured lights glow and light the room in both.
- **Upscalers and anti-aliasing**: one dropdown for FXAA, SMAA, MSAA, TAA, DLAA, DLSS, FSR 1, FSR 3 and XeSS;
  sharpening; resolution from 25% to 200%; frame rate limit; pick the graphics card.
- **Graphics**: photo sky, sun size with real soft shadows, contact shadows, god rays, coloured light through stained
  glass, vignette, lens distortion and chromatic aberration sliders. Every effect has its own toggle; the quality
  slider only sets their defaults.
- **High ledge** (Extras): a 12 m drop over open ground. **High fall** button.
- **Coloured lights hall** (Extras): glowing floor panels lighting a white hall.
- **Additional reactions**: Homer puts his hands on walls and holds rails he staggers into, braces on crates, watches
  and ducks things falling on him, leans away from things thrown at him and looks back at what poked him. With
  Additional experimental reactions on, he tumbles off a high ledge and dives when shot at twice.
- **Even fight**: a knocked-out fighting Homer is back up as fast as the Hero.
- **Reset here** (Shift+R): Homer stands back up where he is.
- **Safe Mode**: the delirium-room-safe launcher starts every quality setting at its lowest.
- The room has its own name and Homer's head as its icon.

### Changed
- **High fall**: falling from a height, Homer goes into the high-fall reaction, lands and gets up.
- **Near misses**: ducked under a shot while balancing, he no longer curls up and falls.

### Fixed
- Homer no longer folds over an imaginary wall after a hard landing, or sinks his head and hands into the floor.
- A crate pinned between Homer and the floor no longer throws them both into the air.
- Restoring a maximized window no longer crashes; leaving MSAA no longer freezes the view.
- DLSS no longer crashes the room at start on a card that isn't NVIDIA's.
- Torn parts and the taser skeleton show in path tracing.

## 0.2.0

### New
- **Portal mode**: portals, the portal gun, the opening animation, particles and the air funnel, ported from Stink-O's
  Portal 2 Rust Engine. It reads Portal 2's files from your own install; none are included.
- **Companion cube**: with Portal mode on, thrown boxes are the companion cube.
- **Portal sounds**: the portal gun's shots and portals opening play Portal 2's sounds (from your install).
- **Experimental walk** (preview, this release only): a learned walk, fast walk and jog. Turn it on under Character →
  Experimental movement, then hold W to walk and Shift+W to jog (it ramps up over about 6 seconds). It walks in a
  straight line, stops by itself at walls, and slows to a walk before stopping when you let go. Trained on Ubisoft's
  LAFAN1 motion capture (non-commercial; see CREDITS.md). It will be replaced next release.
- **Audio sliders**: master, effects and portal volume under View → Audio.
- **Docs**: [How it works](docs/how-it-works.md) and [Modding](docs/modding.md), covering quick starts for each
  language, every message and its parameters, Playroom scripting and settings.

### Changed
- **Shotguns** fire their full pellet spread, each pellet its own hit, on a click and from the floating gun. A headshot
  or a point-blank chest blast kills.
- **Weapons panel** is grouped (Guns, Thrown, Melee, Portal 2) with a stats line for each weapon.
- **Floating portal gun** aims at the wall under the pointer and no longer locks onto Homer.
- **Near misses**: Homer flinches and keeps his balance instead of collapsing.
- **Shadows** have a wider softness slider.
- **Faster physics**: about 1.4x faster steps with the same results, and about 1.36x more on Windows and Linux x86
  builds (AVX2+FMA).
