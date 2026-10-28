use crate::{
    math::Vector3,
    physics::{
        force_queue::{BoardForceQueue, QueuedPointForce},
        manual::controller::ManualEffect,
    },
    riding::{
        braking::{BrakeInput, BrakeSettings, calculate_braking},
        push::{PushAcceleration, PushInput, PushLimits, calculate_acceleration},
    },
};

#[derive(Clone, Copy, Debug)]
pub struct GroundPropulsionInput {
    pub flags_2468: u32,
    pub flags_2472: u32,
    pub target_speed: f32,
    pub signed_speed: f32,
    pub absolute_body_speed: f32,
    pub scalar_2660: f32,
    pub timestep: f32,
    pub brake_input: f32,
    pub push_direction: Vector3,
    pub brake_direction: Vector3,
    pub surface_braking_factor: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GroundPropulsionSettings {
    pub braking: BrakeSettings,
    pub maximum_pushable_speed: f32,
    pub mode_speed_changes: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct GroundPropulsion {
    pub braking: QueuedPointForce,
    pub push: PushAcceleration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropulsionSubmission {
    BrakingAndPush([bool; 2]),
    ManualCorrection(bool),
}

pub fn calculate(
    input: GroundPropulsionInput,
    settings: GroundPropulsionSettings,
    push_suppressed_2730: &mut u8,
) -> GroundPropulsion {
    let braking = calculate_braking(
        BrakeInput {
            flags_2468: input.flags_2468,
            input_2728: input.brake_input,
            signed_speed: input.signed_speed,
            absolute_body_speed: input.absolute_body_speed,
            surface_factor: input.surface_braking_factor,
            direction: input.brake_direction,
        },
        settings.braking,
    );
    let push = calculate_acceleration(
        PushInput {
            flags_2468: input.flags_2468,
            flags_2472: input.flags_2472,
            target_speed: input.target_speed,
            current_speed: input.signed_speed,
            absolute_body_speed: input.absolute_body_speed,
            scale: input.scalar_2660,
            delta_seconds: input.timestep,
            direction: input.push_direction,
        },
        PushLimits {
            maximum_pushable_speed: settings.maximum_pushable_speed,
            low_speed_change: settings.mode_speed_changes[0],
            high_speed_change: settings.mode_speed_changes[1],
        },
    );
    *push_suppressed_2730 = u8::from(push.suppressed);
    GroundPropulsion { braking, push }
}

impl GroundPropulsion {
    pub fn submit(
        self,
        manual: &ManualEffect,
        queue: &mut BoardForceQueue,
    ) -> PropulsionSubmission {
        if manual.correction_active {
            let force = manual.corrective_force_world;
            let point = manual.corrective_point_body;
            return PropulsionSubmission::ManualCorrection(queue.append(QueuedPointForce {
                tag: 7,
                force_world: Vector3::new(force[0], force[1], force[2]),
                point_body: Vector3::new(point[0], point[1], point[2]),
            }));
        }
        let brake = queue.append(self.braking);
        let push = queue.append(QueuedPointForce {
            tag: 3,
            force_world: self.push.vector,
            point_body: self.push.local_point,
        });
        PropulsionSubmission::BrakingAndPush([brake, push])
    }
}
