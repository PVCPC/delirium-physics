use super::{V, length};
use crate::{air::{body_spin::{self, BodySpinState}, reckoning::AirState}, math::Vector3,
    physics::{board_motion_output::inverse_length_squared, native_arithmetic::dot3},
    point_graph::PointGraph,
    riding::{ground_orientation::GroundOrientation, reckoning_frames::ReckoningFrames}};

pub struct Settings {
    pub ground_normal_smoothing: V,
    pub tilt_vs_rotation: PointGraph<8>,
    pub tilt_vs_slope: PointGraph<8>,
}

pub fn update(orientation: &mut GroundOrientation, frames: &mut ReckoningFrames,
    body_spin: &mut BodySpinState, air: &mut AirState, settings: &Settings,
    normal: V, heading: V, smoothing: f32, reverse_stance: bool,
    physical_body_spin: f32) {
    let old_up = lanes(orientation.up);
    orientation.dynamic_up = orientation.up;
    orientation.up_velocity = Vector3::ZERO;
    frames.target_lean_angle = 0.;
    air.secondary_lean_angle = 0.;
    orientation.ground_blend = 0.;
    let ground = orientation.ground_filter.update(settings.ground_normal_smoothing, normal);
    orientation.ground_normal = xyz(ground);
    frames.heading = heading;
    let candidate: V = core::array::from_fn(|i| old_up[i].mul_add(smoothing, normal[i] * (1. - smoothing)));
    let up = if length(candidate) > f32::from_bits(0x3586_37bd) {
        let inverse = inverse_length_squared(dot3(candidate, candidate), 2);
        candidate.map(|v| v * inverse)
    } else { old_up };
    orientation.up = xyz(up);
    orientation.target = orientation.up;
    orientation.slow_filter.filter(up);
    orientation.fast_filter.filter(up);
    orientation.slow_filter.publish_current(up);
    orientation.fast_filter.publish_current(up);
    frames.calculate_transform(up, ground);
    frames.calculate_tilt(reverse_stance, &settings.tilt_vs_rotation, &settings.tilt_vs_slope);
    body_spin::update_ground(body_spin, physical_body_spin);
}

fn xyz(v: V) -> Vector3 { Vector3::new(v[0], v[1], v[2]) }
fn lanes(v: Vector3) -> V { [v.x, v.y, v.z, 0.] }
