//! The effects a portal shot and its portals ask for: queued by [`crate::Sim`] (which has no `Commands`),
//! drained here once a frame.
//!
//! * a failed shot: `portal_badsurface` at the hit point, so people can see where a portal won't go;
//! * a new portal: `portal_success` at the spot, and a looping rim (`portal_edge` blue, `portal_edge_reverse`
//!   orange) that follows the portal and restarts when it is re-placed;
//! * a replaced portal: `portal_close` at the old spot.
use bevy::math::Mat3;
use bevy::prelude::*;

use super::render::{ParticleEffect, MAIN_ONLY};
use crate::Sim;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// A one-shot system at a point, oriented by `basis` (its Z axis is the surface normal).
    Burst { name: &'static str, pos: Vec3, basis: Mat3, kind: usize },
    /// (Re)start the rim of portal `kind` at a new spot.
    Rim { kind: usize },
}

/// The colour of a portal as 0-255 RGB, for the colour control points (Portal 2: blue (0, 60, 255),
/// orange (233, 78, 2)).
fn colour(kind: usize) -> Vec3 {
    [Vec3::new(0.0, 60.0, 255.0), Vec3::new(233.0, 78.0, 2.0)][kind]
}

fn edge_system(kind: usize) -> &'static str {
    ["portal_edge", "portal_edge_reverse"][kind]
}

/// A basis whose Z axis is `normal`.
pub fn facing(normal: Vec3) -> Mat3 {
    Mat3::from_quat(Quat::from_rotation_arc(Vec3::Z, normal.normalize_or(Vec3::Z)))
}

pub(super) fn drain(mut sim: NonSendMut<Sim>, mut commands: Commands, mut rims: Local<[Option<Entity>; 2]>, mut effects: Query<&mut ParticleEffect>) {
    let queued: Vec<Event> = std::mem::take(&mut sim.portal.fx);
    let ready = sim.portal.fx_ready;
    for ev in queued.into_iter().filter(|_| ready) {
        match ev {
            Event::Burst { name, pos, basis, kind } => {
                commands.spawn((Name::new(format!("{name} effect")), ParticleEffect::new(name).with_cp(0, pos, basis).with_cp_vector(2, colour(kind))));
            }
            Event::Rim { kind } => {
                if let Some(old) = rims[kind].take() {
                    if let Ok(mut e) = effects.get_mut(old) {
                        e.stop();
                    }
                }
                let Some(f) = sim.portal.ends[kind] else { continue };
                let mut fx = ParticleEffect::new(edge_system(kind)).with_cp(0, f.at, Mat3::from_quat(f.rot)).with_cp_vector(7, colour(kind));
                fx.restart_when_done = true;
                fx.despawn_when_done = false;
                fx.render_layer = Some(MAIN_ONLY);
                rims[kind] = Some(commands.spawn((Name::new("portal rim"), fx)).id());
            }
        }
    }
    // Rims follow their portals; a closed portal's rim plays out and goes.
    for kind in 0..2 {
        let Some(e) = rims[kind] else { continue };
        // A rim started this frame isn't queryable yet; rims are only freed through `stop`, so never forget one here.
        let Ok(mut fx) = effects.get_mut(e) else { continue };
        match sim.portal.ends[kind] {
            Some(f) => {
                fx.set_cp(0, f.at, Mat3::from_quat(f.rot));
                fx.set_cp_vector(7, colour(kind));
            }
            None => {
                fx.stop();
                rims[kind] = None;
            }
        }
    }
}
