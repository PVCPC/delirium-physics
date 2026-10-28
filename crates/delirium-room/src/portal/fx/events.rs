use bevy::math::Mat3;
use bevy::prelude::*;

use super::render::{ParticleEffect, MAIN_ONLY};
use crate::Sim;

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Burst { name: &'static str, pos: Vec3, basis: Mat3, kind: usize },
    Rim { kind: usize },
}

fn colour(kind: usize) -> Vec3 {
    [Vec3::new(0.0, 60.0, 255.0), Vec3::new(233.0, 78.0, 2.0)][kind]
}

fn edge_system(kind: usize) -> &'static str {
    ["portal_edge", "portal_edge_reverse"][kind]
}

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
    for kind in 0..2 {
        let Some(e) = rims[kind] else { continue };
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
