use crate::physics::native_arithmetic;

use super::data::PhysicsGroundState;

pub type Vector4 = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundOutputFrame {
    pub axis_464: Vector4,
    pub velocity_608: Vector4,
    pub absolute_body_speed_2616: f32,
    pub deck_speed_2652: f32,
    pub state_timer_2664: f32,
    pub scalar_2720: f32,
    pub flags_2476: u32,
    pub flags_2484: u32,
    pub selected_mode_flag_109: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundOutputSettings {
    pub pushable_speed_terms_4_8: [f32; 2],
    pub mode_speed_threshold_0: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkateboardMotionOutput {
    pub is_push_accelerating: bool,
    pub is_at_pushable_speed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundVelocityProjectionOutput {
    pub velocity_without_axis_component: Vector4,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundRecordOutput {
    pub wall_ride_exit: bool,
    pub anti_flip_nudge_present: bool,
    pub is_pinning: bool,
    pub anti_flip_torque: Vector4,
    pub time_to_skitch: f32,
    pub skitch_spline_height: f32,
    pub processed_scalar_2720: f32,
    pub processed_flag_2484_bit_13: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRecordOutput {
    pub grab_spline_type: u32,
    pub grab_spline_object_id: u32,
    pub flag_84: bool,
    pub has_world_grab_intent_without_object: bool,
    pub manual_correction_write_78: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentRecordOutput {
    pub has_world_grab_intent: bool,
    pub selected_mode_below_speed_threshold_58: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsGroundOutput {
    pub skateboard_motion_4: SkateboardMotionOutput,
    pub velocity_projection_36: Option<GroundVelocityProjectionOutput>,
    pub ground_32: GroundRecordOutput,
    pub state_28: StateRecordOutput,
    pub intents_52: IntentRecordOutput,
    pub is_grabbing_object_72_304: bool,
    pub manual_opposition_56_168: bool,
    pub push_suppressed_20_596: bool,
}

pub fn fill_physics_output(
    state: &PhysicsGroundState,
    frame: GroundOutputFrame,
    settings: GroundOutputSettings,
) -> PhysicsGroundOutput {
    let has_world_grab_intent = frame.flags_2476 & 0x0040_0000 != 0;
    let velocity_projection_36 =
        (frame.state_timer_2664 == 0.0).then(|| GroundVelocityProjectionOutput {
            velocity_without_axis_component: reject_axis_component(
                frame.velocity_608,
                frame.axis_464,
            ),
            active: true,
        });

    PhysicsGroundOutput {
        skateboard_motion_4: SkateboardMotionOutput {
            is_push_accelerating: state.flag_2721,
            is_at_pushable_speed: frame.absolute_body_speed_2616
                < settings.pushable_speed_terms_4_8[0] + settings.pushable_speed_terms_4_8[1],
        },
        velocity_projection_36,
        ground_32: GroundRecordOutput {
            wall_ride_exit: state.flag_2720,
            anti_flip_nudge_present: state.anti_flip_nudge_applied_2723,
            is_pinning: state.pinning_2727,
            anti_flip_torque: state.anti_flip_torque_2624,
            time_to_skitch: state.scalar_2664,
            skitch_spline_height: state.scalar_2668,
            processed_scalar_2720: frame.scalar_2720,
            processed_flag_2484_bit_13: frame.flags_2484 & 0x0000_2000 != 0,
        },
        state_28: StateRecordOutput {
            grab_spline_type: state.word_2560,
            grab_spline_object_id: state.word_2564,
            flag_84: state.flag_2731,
            has_world_grab_intent_without_object: has_world_grab_intent && !state.flag_2729,
            manual_correction_write_78: state.manual_correction_2732.then_some(true),
        },
        intents_52: IntentRecordOutput {
            has_world_grab_intent,
            selected_mode_below_speed_threshold_58: frame.selected_mode_flag_109
                && frame.deck_speed_2652 < settings.mode_speed_threshold_0,
        },
        is_grabbing_object_72_304: state.flag_2729,
        manual_opposition_56_168: state.manual_opposition_2733,
        push_suppressed_20_596: state.push_suppressed_2730,
    }
}

fn reject_axis_component(velocity: Vector4, axis: Vector4) -> Vector4 {
    let projection = native_arithmetic::dot3(velocity, axis);
    core::array::from_fn(|lane| {
        let scaled = flush_subnormal(axis[lane] * projection);
        flush_subnormal(velocity[lane] - scaled)
    })
}

fn flush_subnormal(value: f32) -> f32 {
    let bits = value.to_bits();
    if bits & 0x7f80_0000 == 0 && bits & 0x007f_ffff != 0 {
        f32::from_bits(bits & 0x8000_0000)
    } else {
        value
    }
}

