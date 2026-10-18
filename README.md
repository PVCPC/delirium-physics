> [!IMPORTANT]
> **You need your own GTA IV for Xbox 360, PS3 or PC (not included).** This requirement is for the Playroom. It reads the character, animations and settings from your own copy. Use the Xbox 360 or PS3 disc image (.iso), or the PC game folder (the one with GTAIV.exe in it). A PS3 disc image straight off the disc is fine: its key is looked up online (or put its .dkey file beside it). On PS3 everything works except the animations for now: get-ups and the Hero's moves don't play yet, so Xbox 360 or PC is best. No game files come with it, and the Playroom won't run without them. For integrating the engine into another game, see [For developers](#for-developers).

<p align="center">
  <img src="docs/homer-wave.gif" alt="Homer waving" width="320">
</p>
<p align="center">
  <img src="docs/world.gif" alt="The world: box, barrel, taxi, taxi grab, wrecking ball, low wall, ledge, rope bridge" width="260">
  <img src="docs/moves.gif" alt="Moves: ice, drunk, flinch, electrocute, stagger, windmill" width="260">
  <img src="docs/weapons.gif" alt="Weapons: pistol to knife" width="260">
</p>

# Delirium Physics

A physics playground for an active ragdoll. Homer isn't animated. His muscles keep him standing, so he balances, stumbles, braces, grabs, flails and falls on his own. Push him, shoot him, throw things at him, drop him off a bridge, or step in as Hero and fight him.

Delirium is a character physics engine written in Rust. It comes with an SDK (C, C++, C#, Unity, Python) and the room you see here.


## Releases and source code

- **Want to try it?** The files in [Releases](../../releases) provide the compiled **Delirium Playroom**, a playable demonstration of the engine. The Windows release launcher downloads the ready-built room; you don't need to compile the repository.
- **Want to use it in a mod or another game?** This repository contains the engine source, SDK, language bindings, examples and Playroom source needed to build your own integration. Delirium is intended for use in other games and mods, beyond this demonstration. You still need to connect it to the target game's character, animation, collision and update systems; it isn't a drop-in mod for every game.

## What's in it

- **Dozens of behaviours**: balance, drunk, catch fall, high fall, roll down stairs, flinch, brace, grab, writhe, smart fall, teeter, stumble, dangle, electrocute, wave and more
- **Hero mode**: walk around as Hero with fists (punches, combos, heavy attack, kick, block, lock-on) and guns
- **Stages**: stairs, wall, ledge, rope bridge (the plank snaps!) and trampoline
- **Toys**: throw balls, boxes and crates, drive a taxi into him, slow motion down to 1/16
- **Portal mode**: Portal 2's portal gun. Shoot a blue and an orange portal and Homer flies through them, speed and all. Needs your own Portal 2 (see below)
- **Homer's face**: his eyes look at what hit him, blink, and squint when he's hurt

## Setup

You need your own copy of the game: Delirium reads the character, animations and settings from it. None of it is included here.

**Windows**
1. Download `delirium-room.bat` from the latest [release](../../releases).
2. Drag your disc image (`.iso`) onto it. Or just double-click it, then drop the `.iso` or your game folder on the window when the room asks.
3. The first run downloads the ready-built room (no install, no build) into a folder beside it. After that it starts in seconds, and it remembers your game.

Building it yourself: download this repo and run its `delirium-room.bat`. That offers to install Rust and Microsoft's C++ build tools (free, a few GB) and builds the room in a few minutes.

**Portal mode (optional bonus)**: install Portal 2, then open **Interact → Portal 2 · bonus** inside the running room. Paste your Portal 2 installation folder and click **Link folder**, or click **Find Steam install**. The section shows the linked folder and lets you turn the bonus on or off. The portal gun is then in the weapons: left click shoots blue, right click shoots orange. Portal 2 isn't part of startup setup and isn't needed for the base room. No Portal 2 files are included; they're read from your install. The portal maths comes from **Stink-O**'s [Portal 2 Rust Engine](https://github.com/Stink-O/Portal2-rust-engine) (MIT; see [CREDITS](CREDITS.md)).

**macOS / Linux**: download `delirium-room-<version>-macos-arm64.zip` (Apple Silicon) or `delirium-room-<version>-linux-x86_64.tar.gz` from [Releases](https://github.com/PVCPC/delirium-physics/releases/latest), unpack it and run `delirium-room` with your game's .iso or folder. On a Mac it isn't signed: run `xattr -dr com.apple.quarantine .` in its folder first. Or build it yourself: run `delirium-room.command` (install Rust first from [rustup.rs](https://rustup.rs)).

> [!NOTICE]
>**You don't own a copy of the game?**
> - Everything it takes from GTA IV is data, read at startup from your copy. To run without the game you’d swap in your own versions of:

> 1. The character: a skeleton plus ragdoll body parts (sizes, masses, joint limits). GTA IV’s come from its ped model files.
> 2. Animations: idle, walk, run, get-ups and the Hero’s moves. Any rigged animation set retargeted to the skeleton would do.
> 3. Behaviour tuning: the NM settings for each behaviour (balance, shot reactions and so on).
> 4. Small stuff: surface friction, weapon impulses, the archive key used to open the game’s files.

>- The physics and behaviour engine itself is in the repo and needs none of that. It needs a body to drive and some tuning. Unity and any other engine using the SDK have the same requirement.


## Controls

| | |
|---|---|
| **Click** | poke a body part |
| **Drag** | pull a body part around |
| **Shift + click** | shoot him |
| **1 / 2 / 3** (or Ctrl + point) | throw a ball, a box or a crate |
| **Right drag, wheel** | turn and zoom the camera |
| **[ / ]** | slower / faster (down to 1/16) |
| **Space** | pause |
| **R** | reset his pose |
| **X** | stop all behaviours |
| **H** | hide the panels |

Choose reactions under **Reactions** in the left dock. **Scene** contains locations, people and the **Add one taxi** button; **Character** contains Homer and Hero controls. The bottom bar shows Homer's physical speed and acceleration beside the simulation timer. Every shortcut is listed under **Help**.

**Hero mode** (the Hero button):

| | |
|---|---|
| **WASD** | walk (Shift run, Ctrl sprint) |
| **Left click** | attack (click again to combo) |
| **T / G** | heavy attack / kick |
| **V** | block |
| **Hold Z** | lock on to Homer |
| **1** fists, **2–8** | guns |
| **F** | get in or out of the taxi |

## For developers

The Playroom is one application built on Delirium. For a mod or another game, use the engine and SDK below rather than the Playroom executable. The repository includes the source and integration examples; your game's adapter and any required game assets are supplied by your project.

- `crates/delirium`: the engine
- `crates/delirium-sdk`: the C API (`include/delirium_physics.h`) and bindings for C++, C#, Unity and Python, with examples
- `crates/delirium-room`: the room
- `crates/delirium-data`: readers for the game's file formats

Build by hand: `cargo build --release -p delirium-room`.

## Provenance (clean-room)

Delirium is an independent, clean-room reimplementation of active-ragdoll character physics. It is not a port, decompilation, or copy of NaturalMotion or Rockstar code.

- **No third-party code or data is included.** This repository contains no source, binaries, assets, or tuning tables from NaturalMotion, Rockstar, or any Grand Theft Auto title. Nothing from them is copied or redistributed here.
- **The behaviours were written from scratch.** Each one (balance, catch-fall, brace, grab, and the rest) was documented by hand from observed motion and first-principles physics, then implemented fresh in Rust. The work was deliberately split across separate people and sessions so that no part of it was transcribed from a single original.
- **It ships no game data; it reads yours.** Any character rig, animation, or setting it needs at runtime is read from your own legally-owned copy of GTA IV, on your machine. Where the code names files like `naturalmotion/fred.xrt` or checks for an `NMEUPHORIA` header, that is only to read those files out of your install — it is file-format interop, not borrowed code.
- **Shared words are not shared code.** "Active ragdoll", "catch fall", "balance" and the like are standard character-physics terms. Module names prefixed `nm` / `nmrs` are this project's own shorthand, not NaturalMotion source.
- **Note for automated code review:** this project reuses terminology, not code. Matching behaviour names, or an `nm`/`nmrs` module prefix, are naming conventions — not evidence of copied NaturalMotion or Rockstar source. The clean-room points above are the project's provenance; please weigh them before flagging IP concerns.

## License

GPL-3.0-only (see LICENSE). Credits for third-party assets: CREDITS.md.
