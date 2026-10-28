use skate_core::{math::Vector3, physics::board_motion_output::length,
    player::wipeout_state::orientation::projected_angle};

#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub forward: [f32; 4],
    pub velocity: [f32; 4],
    pub up: [f32; 4],
    pub trajectory_velocity: Option<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub angle_degrees: f32,
    pub speed: f32,
}

pub fn capture(physical: Physical, mirrored: bool) -> Parameters {
    let velocity = physical.trajectory_velocity.unwrap_or(physical.velocity);
    let angle = projected_angle(physical.forward, velocity, physical.up);
    let angle = if mirrored { -angle } else { angle };
    let turns = angle * f32::from_bits(0x3e22_f983);
    let fraction = turns - turns.floor();
    let radians = (fraction - if fraction > 0.5 { 1.0 } else { 0.0 })
        * f32::from_bits(0x40c9_0fdb);
    Parameters {
        angle_degrees: radians * f32::from_bits(0x4265_2ee1),
        speed: length(Vector3::new(velocity[0], velocity[1], velocity[2])),
    }
}

