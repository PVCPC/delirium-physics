use crate::{math::Vector3, physics::force_queue::QueuedPointForce};

#[derive(Clone, Copy, Debug)]
pub struct BrakeSettings {
    pub input_force: f32,
    pub override_force: f32,
    pub minimum_speed: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct BrakeInput {
    pub flags_2468: u32,
    pub input_2728: f32,
    pub signed_speed: f32,
    pub absolute_body_speed: f32,
    pub surface_factor: f32,
    pub direction: Vector3,
}

pub fn calculate_braking(input: BrakeInput, settings: BrakeSettings) -> QueuedPointForce {
    let mut amount = if input.flags_2468 & 0x4000_0000 != 0 {
        input.input_2728 * settings.input_force
    } else {
        0.0
    };
    if input.flags_2468 & 0x2000_0000 != 0 {
        amount = settings.override_force;
    }
    let scaled = input.surface_factor * amount;
    let sign = if input.signed_speed < 0.0 { 1.0 } else { -1.0 };
    amount = sign * scaled;
    if input.absolute_body_speed < settings.minimum_speed {
        amount = 0.0;
    }
    QueuedPointForce {
        tag: 2,
        force_world: Vector3::new(
            input.direction.x * amount,
            input.direction.y * amount,
            input.direction.z * amount,
        ),
        point_body: Vector3::ZERO,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LinearDragSettings {
    pub brake_speed: f32,
    pub balance_speed: f32,
    pub comparison_threshold: f32,
    pub balance_drag: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct LinearDragInput {
    pub flags_2468: u32,
    pub absolute_body_speed: f32,
    pub balance_2720: f32,
    pub scalar_2724: f32,
    pub comparison_scalar: f32,
}

pub fn calculate_linear_drag(input: LinearDragInput, settings: LinearDragSettings) -> f32 {
    let brake_override = input.absolute_body_speed < settings.brake_speed * 2.0
        && input.flags_2468 & 0x6000_0000 != 0
        && !(input.scalar_2724 > 0.0)
        && input.balance_2720 == 0.0;
    let balance_drag = input.balance_2720 != 0.0
        && input.absolute_body_speed < settings.balance_speed
        && input.comparison_scalar > settings.comparison_threshold;
    if brake_override {
        1.0
    } else if balance_drag {
        settings.balance_drag
    } else {
        0.0
    }
}
