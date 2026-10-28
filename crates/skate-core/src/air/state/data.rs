use crate::point_graph::PointGraph;

pub type Vector4 = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirTrajectory {
    pub position: Vector4,
    pub velocity: Vector4,
    pub acceleration: Vector4,
    pub scalar_48: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsAirState {
    pub centre_of_mass_trajectory: AirTrajectory,
    pub landing_normal: Vector4,
    pub time_in_state: f32,
    pub start_y: f32,
    pub max_y: f32,
    pub reached_apex: bool,
    pub use_centre_of_mass_velocity: bool,
    pub selector_latch_174: bool,
    pub trajectory_query_countdown: i32,
}

impl Default for PhysicsAirState {
    fn default() -> Self {
        Self {
            centre_of_mass_trajectory: AirTrajectory {
                position: [0.0; 4], velocity: [0.0; 4], acceleration: [0.0; 4], scalar_48: -1.0,
            },
            landing_normal: [0.0, 1.0, 0.0, 0.0],
            time_in_state: 0.0, start_y: 0.0, max_y: 0.0,
            reached_apex: false, use_centre_of_mass_velocity: false,
            selector_latch_174: false, trajectory_query_countdown: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsAirFrame {
    pub current_velocity_400: Vector4,
    pub ground_position_y_500: f32,
    pub trajectory_position_592: Vector4,
    pub trajectory_velocity_608: Vector4,
    pub jump_velocity_848: Vector4,
    pub flags_2468: u32,
    pub previous_physics_state_2504: i32,
    pub previous_physics_category_2516: i32,
    pub frames_since_jump_correction_2576: i32,
    pub delta_time_2604: f32,
    pub body_spin_input_2640: f32,
    pub gravity_y_2648: f32,
    pub state_timer_2664: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsAirSettings {
    pub body_spin_over_time_320: PointGraph<8>,
    pub landing_normal_blend_388: f32,
    pub body_spin_scale_428: f32,
    pub landing_normal_angle_limit_444: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsAirReckoningFields {
    pub current_landing_normal_1152: Vector4,
    pub collision_reference_normal_1216: Vector4,
    pub body_spin_angle_1568: f32,
    pub body_spin_speed_1572: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirBoardForce {
    pub force_world: Vector4,
    pub point_board_local: Vector4,
}

impl AirBoardForce {
    pub const ZERO: Self = Self {
        force_world: [0.0; 4],
        point_board_local: [0.0; 4],
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsAirOutput {
    pub is_at_apex: bool,
    pub jump_height: f32,
    pub landing_normal: Vector4,
    pub scalar_184_write: Option<f32>,
}
