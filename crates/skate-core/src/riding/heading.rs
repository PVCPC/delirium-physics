use crate::{physics::reciprocal_sqrt::estimate, point_graph::PointGraph, trigonometry::acos};

#[derive(Clone)]
pub struct HeadingSettings {
    pub manual_wrong_wheel_scalar: f32,
    pub manual_damping: f32,
    pub speed_max: f32,
    pub heading_strength: f32,
    pub turn_strength: f32,
    pub angular_response: PointGraph<8>,
    pub manual_response: PointGraph<8>,
    pub inclination_response: PointGraph<8>,
    pub speed_response: PointGraph<8>,
    pub normal_threshold: [f32; 4],
}

pub struct HeadingInput {
    pub balance: f32,
    pub manual_turn: f32,
    pub flags_2472: u32,
    pub timestep: f32,
    pub signed_speed: f32,
    pub manual_curve_input: f32,
    pub turn_2712: f32,
    pub scalar_2740: f32,
    pub velocity: [f32; 4],
    pub normal: [f32; 4],
    pub angular_velocity: [f32; 4],
    pub transform: [[f32; 4]; 4],
}

pub fn calculate(settings: &HeadingSettings, input: &HeadingInput, previous: &mut f32) -> [f32; 4] {
    if input.balance != 0.0 && input.manual_turn != 0.0 {
        let wrong = (input.flags_2472 & 0x0800_0000 != 0 && input.balance < 0.0)
            || (input.flags_2472 & 0x0400_0000 != 0 && input.balance > 0.0);
        let turn = if wrong {
            input.manual_turn * settings.manual_wrong_wheel_scalar
        } else {
            input.manual_turn
        };
        let target = settings.manual_response.evaluate(input.manual_curve_input)
            * f32::from_bits(0x3C8E_FA35);
        *previous =
            (1.0 - settings.manual_damping).mul_add(*previous, target * settings.manual_damping);
        let scalar = (input.timestep * *previous) * turn;
        return input.normal.map(|n| n * scalar);
    }
    *previous = 0.0;
    let matrix = input.transform;
    let rows: [[f32; 4]; 3] =
        core::array::from_fn(|i| [matrix[0][i], matrix[1][i], matrix[2][i], 0.0]);
    let cosine = clamp(rows[1][1], -1.0, 1.0);
    let angle = acos(cosine) * f32::from_bits(0x3F22_F983);
    let angle = if angle > 1.2 { 0.0 } else { angle };
    let mut downhill: [f32; 4] = core::array::from_fn(|i| {
        let v = rows[0][i] * 0.0;
        let v = rows[1][i].mul_add(-1.0, v);
        rows[2][i].mul_add(0.0, v)
    });
    downhill[1] = 0.0;
    let squared = dot3(downhill, downhill);
    let mut inverse = estimate(squared);
    for _ in 0..2 {
        let correction = (-squared).mul_add(inverse * inverse, 1.0);
        inverse = (inverse * 0.5).mul_add(correction, inverse);
    }
    let length = if squared == 0.0 {
        0.0
    } else {
        squared * inverse
    };
    let inclination = settings.inclination_response.evaluate(angle);
    for i in 0..4 {
        downhill[i] = (if length > settings.normal_threshold[i] {
            downhill[i] * inverse
        } else {
            0.0
        }) * inclination;
    }
    let mut velocity: [f32; 4] = core::array::from_fn(|i| {
        let v = rows[0][i] * input.velocity[0];
        let v = rows[1][i].mul_add(input.velocity[1], v);
        rows[2][i].mul_add(input.velocity[2], v)
    });
    velocity[1] = 0.0;
    let cross_y = (-velocity[0]).mul_add(downhill[2], velocity[2] * downhill[0]);
    let speed = clamp(input.signed_speed.abs(), 0.0, settings.speed_max);
    let heading = (cross_y * settings.heading_strength)
        * settings.speed_response.evaluate(speed / settings.speed_max);
    let opposing = dot3(input.angular_velocity, input.normal) * -input.turn_2712;
    let opposing = if opposing >= 0.0 { opposing } else { 0.0 };
    let turn = ((settings.angular_response.evaluate(opposing) * input.scalar_2740)
        * settings.turn_strength)
        * input.turn_2712;
    let scalar = if !(heading * turn < 0.0) && !(turn.abs() > heading.abs()) {
        heading
    } else {
        turn
    };
    input.normal.map(|n| n * scalar)
}

fn clamp(value: f32, lower: f32, upper: f32) -> f32 {
    let v = if lower - value >= 0.0 { lower } else { value };
    if upper - v >= 0.0 { v } else { upper }
}

fn dot3(a: [f32; 4], b: [f32; 4]) -> f32 {
    super::vector::dot3(a, b)
}
