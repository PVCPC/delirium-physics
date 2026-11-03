# Delirium Ragdoll for Unreal Engine 5

Active ragdolls for any Unreal character: they keep their balance when pushed, stagger when shot, catch their fall,
lie still and hand the character back to get up. Built and tested on UE 5.8 with the Third Person template's Manny.

## Install

1. Copy this `DeliriumRagdoll` folder into your project's `Plugins` folder.
2. Put `delirium_physics.h`, `delirium_physics.dll` and `delirium_physics.dll.lib` in `Source/ThirdParty/delirium_physics`. The release's `unreal-plugin` download has them in place.
3. Build the project (it's a C++ plugin: a Blueprint-only project needs one C++ class added first).
4. Project Settings > Plugins > Delirium Ragdoll: point `Game Exe` at your own copy of the game and `Character` /
   `Build` at the data files (absolute, or under the project folder).

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
| `HighFall()` | Arms out, legs ready |
| `Kill()` | Limp, stays down |
| `Sever(Point, Impulse)` | Cuts off the limb nearest `Point` (hide its mesh and spawn your gib yourself) |
| `GetUp()` | Hands it back: the actor stands where the body lies, mesh, animation and movement back on |

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
killing shot and without a leg.
