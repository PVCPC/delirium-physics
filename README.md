# Delirium Physics

A character physics engine (balance, reactions, falls) in Rust, with an SDK (C, C#, Unity, Python) and a room to try it in.

## The room

Windows: run `delirium-room.bat` (or drag your disc image onto it). macOS / Linux: `delirium-room.command`.
The first run installs Rust if needed and builds; later runs start in seconds.

The room reads the character, animations and settings from your own copy of the game; none of it is included here.

## Layout

- `crates/delirium`: the engine
- `crates/delirium-sdk`: the C API (`include/delirium_physics.h`) and bindings
- `crates/delirium-room`: the room
- `crates/delirium-data`: readers for the game's file formats

## License

GPL-3.0-only (see LICENSE).
