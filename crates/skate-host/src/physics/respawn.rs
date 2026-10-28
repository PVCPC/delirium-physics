use super::{GamePhysics, SkaterRuntime, offboard::contact_queries, teleport_state::Checkpoint};
use skate_core::{
    math::Vector3,
    physics::{
        board_world::{BoardWorld, query_metadata::Bounds},
        skeleton_animation_record::AnimationPartTransform as Matrix,
    },
    player::{
        respawn::{Candidate, Ground, History, Observation, Validation, surface_allowed},
    },
};
use skate_data::collections::Collections;
use super::offboard::contact_queries::Probe;

pub(super) struct Runtime {
    history: History,
    settings: Settings,
    measurements: i32,
}
struct Settings {
    height: f32,
    radius: f32,
    drop: f32,
    normal_y: f32,
    minimum_frames: u32,
}
impl Runtime {
    pub fn load(data: &Collections, transform: Matrix, stance: u32) -> Result<Self, String> {
        let class = "Hash_12B64C0E804B0853";
        let value = |hash| data.float(class, "default", hash);
        Ok(Self {
            history: History::new(Candidate {
                transform,
                stance,
                offboard: false,
                score: 0.,
            }),
            measurements: 0,
            settings: Settings {
                height: value("Hash_C0526C883AF0ECCA")?,
                radius: value("Hash_CEB092E418A5B001")?,
                drop: value("Hash_8ABE098D3806D273")?,
                normal_y: value("Hash_ADD032CACF6A1C15")?,
                minimum_frames: data.integer(class, "default", "Hash_10B7C3A9CC8D3721")? as u32,
            },
        })
    }
    pub fn reset_measurements(&mut self) {
        self.measurements = 0;
    }
}

pub(super) fn request(physics: &GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let stance = skater.animation.checkpoint_stance();
    let runtime = &mut skater.respawn;
    let candidate = runtime.history.automatic(
        stance,
        &mut Scene {
            world: &physics.world,
            settings: &runtime.settings,
        },
    )?;
    skater.animation.request_checkpoint_stance(candidate.stance);
    skater.teleport_state.reply(Checkpoint {
        transform: candidate.transform,
        on_board: !candidate.offboard,
    });
    bevy::log::info!(
        "BAIL_CHECKPOINT position={:?} heading={:?} stance={} offboard={} score={}",
        candidate.transform[3],
        candidate.transform[2],
        candidate.stance,
        candidate.offboard,
        candidate.score
    );
    Ok(())
}

pub(super) fn observe(physics: &GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let physical = &skater.player_input.physical;
    let processed = &skater.player_input.processed;
    let mut deck = super::solve::deck_frame(&physics.board);
    if processed.flags_2468 & 0x0010_0000 != 0 {
        flip(&mut deck);
    }
    deck[3][1] += 0.2;
    let mut offboard = skater.animated_skeleton.roots.animation_to_world;
    if processed.flags_2476 & 4 != 0 {
        flip(&mut offboard);
    }
    let velocity = physical.skateboard.vector_80.map(f32::from_bits);
    let stance = skater.animation.checkpoint_stance();
    let runtime = &mut skater.respawn;
    runtime.measurements = runtime.measurements.wrapping_add(1);
    let observation = Observation {
        measurements: runtime.measurements,
        root_position: skater.animated_skeleton.roots.animation_to_world[3],
        com_position: physical.reckoning.vector_64.map(f32::from_bits),
        teleport_requested: physical.state.flag_69 != 0,
        physical_state: physical.state.state_16,
        state_frames: skater.player_state.state_count,
        ground_suppressed: processed.grind.flags_1516 & 0x0800_0000 != 0,
        offboard_correction: skater.biped_ground.controller.state.contact.active,
        ground_category: super::ground_runtime::active_surface(&physics.riding, &physics.board)
            & 0xffff,
        foot_categories: [(processed.left_surface_2596 >> 7) & 31; 2],
        riding_transform: heading(deck, velocity),
        offboard_transform: offboard,
        stance,
        alternate_world: false,
    };
    runtime.history.observe(
        &observation,
        runtime.settings.minimum_frames,
        &mut Scene {
            world: &physics.world,
            settings: &runtime.settings,
        },
    )
}

fn flip(matrix: &mut Matrix) {
    for i in [0, 2] {
        matrix[i] = matrix[i].map(|v| -v);
    }
}
fn heading(mut matrix: Matrix, velocity: [f32; 4]) -> Matrix {
    let square = velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2];
    if square > 0.25 {
        let inv = square.sqrt().recip();
        let forward = [velocity[0] * inv, 0., velocity[2] * inv, 0.];
        if forward[0] * forward[0] + forward[2] * forward[2] > 0.9 {
            matrix[0] = [forward[2], 0., -forward[0], 0.];
            matrix[1] = [0., 1., 0., 0.];
            matrix[2] = forward;
        }
    }
    matrix
}

struct Scene<'a> {
    world: &'a BoardWorld,
    settings: &'a Settings,
}
impl Validation for Scene<'_> {
    type Error = String;
    fn ground(&mut self, transform: &Matrix) -> Result<Option<Ground>, String> {
        let mut start = transform[3];
        start[1] += 0.1;
        let mut end = start;
        end[1] -= 10.;
        let probe = |start, end, radius| Probe {
            start,
            end,
            radius,
        };
        let Some(hit) = contact_queries::query(self.world, probe(start, end, 0.), 0)? else {
            return Ok(None);
        };
        let category = (u32::from(hit.packed_surface) >> 7) & 31;
        if hit.geometry.normal.y < self.settings.normal_y
            || start[1] - hit.geometry.position.y > self.settings.drop
            || !surface_allowed(category)
        {
            return Ok(None);
        }
        let mut bottom = start;
        bottom[1] += self.settings.radius;
        let mut top = bottom;
        top[1] += self.settings.height;
        if contact_queries::query(self.world, probe(bottom, top, self.settings.radius), 0)?
            .is_some()
        {
            return Ok(None);
        }
        let p = hit.geometry.position;
        Ok(Some(Ground {
            position: [p.x, p.y, p.z, 0.],
            offboard: category == 8,
        }))
    }
    fn location(&mut self, _: &Matrix) -> Result<bool, String> {
        Ok(true)
    }
    fn occupants(&mut self, _: &Matrix) -> Result<bool, String> {
        self.world.query_metadata().map_err(str::to_owned)?;
        Ok(true)
    }
    fn edges(&mut self, transform: &Matrix) -> Result<bool, String> {
        let p = transform[3];
        let bounds = Bounds {
            min: Vector3::new(p[0] - 0.3, p[1] - 0.6, p[2] - 0.3),
            max: Vector3::new(p[0] + 0.3, p[1] + 0.6, p[2] + 0.3),
        };
        let metadata = self.world.query_metadata().map_err(str::to_owned)?;
        for edge in metadata
            .static_edges
            .iter()
            .filter(|e| e.local_bounds.overlaps(bounds))
            .take(40)
        {
            let start = [edge.start.x, edge.start.y, edge.start.z];
            let end = [edge.end.x, edge.end.y, edge.end.z];
            let delta: [f32; 3] = std::array::from_fn(|i| end[i] - start[i]);
            let square: f32 = delta.iter().map(|v| v * v).sum();
            let along: f32 = (0..3).map(|i| (p[i] - start[i]) * delta[i]).sum();
            let fraction = if square > 0. {
                (along / square).clamp(0., 1.)
            } else {
                0.
            };
            let distance: f32 = (0..3)
                .map(|i| (p[i] - start[i] - delta[i] * fraction).powi(2))
                .sum();
            if distance < 0.09 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

