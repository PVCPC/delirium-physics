use crate::point_graph::PointGraph;

#[derive(Clone, Copy, Debug)]
pub struct PumpingSettings {
    pub pump_vs_speed: PointGraph<8>,
    pub pump_vs_time: PointGraph<8>,
    pub min_crouch_vs_ground_angle: PointGraph<8>,
    pub compression_vs_ground_angle: PointGraph<8>,
    pub compression_vs_deck_angle: PointGraph<8>,
    pub height_change_damping: f32,
    pub minimum_height_change: f32,
    pub maximum_height_change: f32,
    pub ground_compression_scale: f32,
    pub deck_compression_scale: f32,
    pub angular_speed_damping: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PumpingMode {
    pub maximum_absorption_per_second: f32,
    pub maximum_acceleration_per_second: f32,
    pub absorption_factor: f32,
    pub acceleration_factor: f32,
}
