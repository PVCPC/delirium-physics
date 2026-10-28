use super::vector::{clamp, dot3};
use crate::{point_graph::PointGraph, trigonometry::acos};

#[derive(Clone)]
pub struct SlideFrictionSettings {
    pub angle_response: PointGraph<16>,
    pub speed_response: PointGraph<8>,
    pub time_response: PointGraph<16>,
    pub scalar_516: f32,
    pub heading_time_limit: f32,
    pub friction: f32,
}
pub struct SlideFrictionInput {
    pub heading_time: f32,
    pub normal: [f32; 4],
    pub velocity: [f32; 4],
    pub side_axis: [f32; 4],
    pub surface_speed: f32,
    pub scalar_2764: f32,
}
pub fn calculate(s: &SlideFrictionSettings, i: &SlideFrictionInput) -> [f32; 8] {
    let y = if -1.0 > i.normal[1] {
        -1.0
    } else {
        i.normal[1]
    };
    let y = if 1.0 < y { 1.0 } else { y };
    let angle = clamp(acos(y) * f32::from_bits(0x3f22_f983), 0.0, 1.0);
    let angle = 1.0 - s.angle_response.evaluate(angle);
    let speed = 1.0 - s.speed_response.evaluate(i.surface_speed * 0.1);
    let time = s
        .time_response
        .evaluate(clamp(i.heading_time, 0.0, s.heading_time_limit) / s.heading_time_limit);
    let blend = (1.0 - s.scalar_516).mul_add(i.scalar_2764, s.scalar_516);
    let response = if angle - speed >= 0.0 { angle } else { speed };
    let scalar = -(((time * blend) * response) * s.friction);
    let normal_speed = dot3(i.velocity, i.normal);
    let tangent = core::array::from_fn(|n| i.velocity[n] - i.normal[n] * normal_speed);
    let side_speed = dot3(tangent, i.side_axis);
    let mut output = [0.0; 8];
    for n in 0..4 {
        output[n] = (i.side_axis[n] * side_speed) * scalar;
    }
    output
}
