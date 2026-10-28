use crate::point_graph::PointGraph;
#[derive(Clone, Copy, Debug, Default)]
pub struct GroundJump {
    pub velocity: [f32; 4],
    pub scalar_16: f32,
    pub active: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct GroundJumpMode {
    pub minimum_height_64: f32,
    pub minimum_height_68: f32,
    pub maximum_height: f32,
}
pub struct GroundJumpSettings {
    pub vertical_response: PointGraph<16>,
    pub y_scalar_vs_normal_y: PointGraph<8>,
    pub speed_scalar_vs_angle: PointGraph<8>,
    pub minimum_height_vs_speed: PointGraph<8>,
    pub maximum_height_vs_speed: PointGraph<8>,
    pub speed_response_max_speed: f32,
    pub minimum_scalar: f32,
    pub maximum_y_bonus: f32,
    pub adjust_z_factor: f32,
    pub adjust_x_factor: f32,
    pub absolute_minimum_height: f32,
    pub hippy_minimum_height: f32,
    pub hippy_maximum_height: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct GroundJumpInput {
    pub flags_2468: u32,
    pub flags_2480: u32,
    pub flags_2484: u32,
    pub flags_2488: u32,
    pub effective_forward: [f32; 4],
    pub forward: [f32; 4],
    pub current_velocity: [f32; 4],
    pub ground_reference_position: [f32; 4],
    pub reference_up: [f32; 4],
    pub filtered_ground_normal: [f32; 4],
    pub animation_com_position: [f32; 4],
    pub prepared_velocity: [f32; 4],
    pub jump_strength: f32,
    pub jump_controls: [f32; 2],
    pub gravity_y: f32,
    pub surface_speed: f32,
}
