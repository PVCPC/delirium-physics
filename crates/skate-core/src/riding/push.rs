use crate::math::Vector3;
use crate::physics::force_queue::{BoardForceQueue, QueuedPointForce, total_body_mass};

#[derive(Clone, Copy, Debug)]
pub struct PushInput {
    pub flags_2468: u32,
    pub flags_2472: u32,
    pub target_speed: f32,
    pub current_speed: f32,
    pub absolute_body_speed: f32,
    pub scale: f32,
    pub delta_seconds: f32,
    pub direction: Vector3,
}

#[derive(Clone, Copy, Debug)]
pub struct PushLimits {
    pub maximum_pushable_speed: f32,
    pub low_speed_change: f32,
    pub high_speed_change: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PushAcceleration {
    pub vector: Vector3,
    pub local_point: Vector3,
    pub suppressed: bool,
}

pub fn calculate_acceleration(input: PushInput, limits: PushLimits) -> PushAcceleration {
    if input.flags_2468 & 0x0200_0000 == 0 {
        return PushAcceleration::default();
    }
    if input.flags_2472 & 0x200 != 0 {
        return PushAcceleration {
            suppressed: true,
            ..Default::default()
        };
    }
    let gap = input.target_speed - input.current_speed;
    let mut speed_change = if gap > 0.0 { gap } else { 0.0 };
    let speed_fraction = input.current_speed / limits.maximum_pushable_speed;
    let nonnegative = if -speed_fraction >= 0.0 {
        0.0
    } else {
        speed_fraction
    };
    let fraction = if 1.0 - nonnegative >= 0.0 {
        nonnegative
    } else {
        1.0
    };
    let limit =
        (1.0 - fraction).mul_add(limits.low_speed_change, fraction * limits.high_speed_change);
    let below_lower = speed_change < -limit;
    if speed_change > limit {
        speed_change = limit;
    }
    if below_lower {
        speed_change = -limit;
    }
    if input.absolute_body_speed + speed_change > limits.maximum_pushable_speed {
        let remaining = limits.maximum_pushable_speed - input.absolute_body_speed;
        speed_change = if remaining >= 0.0 { remaining } else { 0.0 };
    }
    let force_magnitude = (input.scale * speed_change) / input.delta_seconds;
    PushAcceleration {
        vector: Vector3::new(
            input.direction.x * force_magnitude,
            input.direction.y * force_magnitude,
            input.direction.z * force_magnitude,
        ),
        ..Default::default()
    }
}

pub fn enqueue_push(
    mut input: PushInput,
    limits: PushLimits,
    inverse_masses: &[f32],
    queue: &mut BoardForceQueue,
) -> PushAcceleration {
    input.scale = total_body_mass(inverse_masses);
    let output = calculate_acceleration(input, limits);
    queue.append(QueuedPointForce {
        tag: 3,
        force_world: output.vector,
        point_body: output.local_point,
    });
    output
}

