mod output;
mod placement;
mod state;
mod step;
pub(super) use output::build_frame;
pub use placement::PlacementInput;
use super::{movement_intent, movement_velocity};
use crate::point_graph::PointGraph;
pub use output::GroundResult;
pub use state::{ClipMetric, State};
pub type Vector = [f32; 4];
pub type Frame = [Vector; 4];

pub struct Settings {
    pub movement_intent: movement_intent::Settings,
    pub movement_velocity: movement_velocity::Settings,
    pub slide_vs_slope: PointGraph<8>,
    pub slide_vs_speed: PointGraph<8>,
}

#[derive(Clone, Copy, Debug)]
pub struct GroundJob {
    pub contact_position: Vector,
    pub contact_normal: Vector,
    pub support_frame: Frame,
    pub target_position: Vector,
    pub target_normal: Vector,
    pub edge_position: Vector,
    pub edge_normal: Vector,
    pub flags: u32,
    pub support_id: u32,
    pub collision_displacements: [Vector; 2],
    pub animation_motion: Vector,
    pub animation_velocity: Vector,
    pub desired_direction: Vector,
    pub animation_position: Vector,
    pub requested_duration: f32,
    pub mirrored: bool,
    pub requested_phase: f32,
    pub override_duration: f32,
    pub animation_directed: bool,
    pub movement: f32,
    pub steering: f32,
    pub sprint_pressed: bool,
    pub suppress_lean: bool,
    pub suppress_minimum: bool,
    pub target_frame_present: bool,
    pub edge_active: bool,
    pub target_frame: Frame,
    pub ignore_obstacle: bool,
}

pub struct Controller {
    pub state: State,
    pub settings: Settings,
}
impl Controller {
    pub fn new(settings: Settings, metrics: [Option<ClipMetric>; 3]) -> Self {
        Self {
            state: State::new(metrics),
            settings,
        }
    }
    pub fn reset(&mut self) {
        self.state.reset();
    }
    pub fn place(&mut self, input: PlacementInput) {
        self.state.place(input);
    }
    pub fn step_ground(&mut self, job: &GroundJob) -> GroundResult {
        step_ground(&mut self.state, &self.settings, job)
    }
}

pub fn step_ground(state: &mut State, settings: &Settings, job: &GroundJob) -> GroundResult {
    step::update(state, settings, job);
    output::export(state)
}
