//! Portal 2's particle effects: a port of `p2_fx` from the Portal 2 Rust Engine (see CREDITS.md).
//! The `.pcf` particle systems (`portals.pcf`, `portal_projectile.pcf`, `portalgun.pcf`) are read from the
//! linked Portal 2 install, simulated on the CPU (`sim`) and drawn as sprite and rope meshes (`render`).
//! The portal shots drive them through `events`.
#![allow(dead_code, unused_imports)]

mod dmx;
mod events;
mod fs;
mod kv;
mod pcf;
mod render;
mod sim;
mod vmt;

pub use events::{facing, Event};
pub use render::{plugin, MAIN_ONLY};
