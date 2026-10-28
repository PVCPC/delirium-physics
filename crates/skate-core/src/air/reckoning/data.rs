use crate::{
    air::{body_flip::BodyFlipSettings, body_spin::BodySpinSettings},
    physics::skeleton_animation_record::{AnimationPartTransform, IDENTITY},
    point_graph::PointGraph,
};
#[derive(Clone, Debug, PartialEq)]
pub struct AirState {
    pub spin_angle: f32,
    pub spin_speed: f32,
    pub secondary_lean_angle: f32,
    pub flip_angle: f32,
    pub flip_speed: f32,
    pub flip_requested_speed: f32,
    pub spin_transform: AnimationPartTransform,
    pub flip_axis: [f32; 4],
    pub flip_active: bool,
    pub flip_side: bool,
}
impl AirState {
    pub fn new() -> Self {
        Self {
            spin_angle: 0.0,
            spin_speed: 0.0,
            secondary_lean_angle: 0.0,
            flip_angle: 0.0,
            flip_speed: 0.0,
            flip_requested_speed: 0.0,
            spin_transform: IDENTITY,
            flip_axis: [1.0, 0.0, 0.0, 0.0],
            flip_active: false,
            flip_side: false,
        }
    }
    pub fn reset_spin(&mut self) {
        self.spin_angle = 0.0;
        self.spin_speed = 0.0;
    }
}
pub struct Settings {
    pub ground_normal_smoothing: [f32; 4],
    pub max_up_angle_delta: PointGraph<8>,
    pub tilt_vs_rotation: PointGraph<8>,
    pub tilt_vs_slope: PointGraph<8>,
    pub body_spin: BodySpinSettings,
    pub body_flip: BodyFlipSettings,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub landing_normal: [f32; 4],
    pub normal_blend: f32,
    pub target_spin: f32,
    pub flip_request: f32,
    pub com_to_deck: [f32; 4],
    pub timestep: f32,
    pub physical_body_spin: f32,
    pub grind_adjusted_body_spin: f32,
    pub additive_spin: bool,
    pub direct_spin: bool,
    pub reverse_stance: bool,
    pub easy_body_spins: bool,
    pub perfect_body_flips: bool,
}
