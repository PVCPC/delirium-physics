use crate::point_graph::PointGraph;

#[derive(Clone, Copy, Debug)]
pub struct ManualGains {
    pub proportional: f32,
    pub integral: f32,
    pub derivative: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ManualSettings {
    pub noise_vs_speed: PointGraph<8>,
    pub torque_scale_without_contact: f32,
    pub torque_bleed_without_contact: f32,
    pub start_torque_scale: f32,
    pub procedural_noise_scale: f32,
    pub procedural_noise_frequency: f32,
    pub powerslide: ManualGains,
    pub manual: ManualGains,
    pub maximum_tilt_degrees: f32,
    pub maximum_angle_error: f32,
    pub derivative_limit: f32,
    pub brake_tilt_degrees: f32,
    pub animation_noise_scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ManualMode {
    pub correction_angular_speed_threshold: f32,
    pub corrective_force_enabled: bool,
}
