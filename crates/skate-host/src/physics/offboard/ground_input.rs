use crate::physics::SkaterRuntime;
use skate_core::player::offboard::{ground_input::GroundInput, ground_job};

pub(crate) fn prepare(skater: &SkaterRuntime) -> ground_job::Input {
    let p = &skater.player_input.processed;
    let attributes = &skater.animation_input.fields;
    let extra = &skater.animation_input.extra;
    let frame = skater.offboard.ground.frame_80;
    let fallback = p.line_tests_960_1008_1056[2];
    ground_job::Input {
        previous_state: p.state_2504,
        frames_since_teleport: p.frames_since_teleport_2584,
        fallback_line_hit: (fallback.valid != 0).then(|| fallback.position.map(f32::from_bits)),
        controls: GroundInput {
            processed_flags_2472: p.flags_2472,
            processed_direct_2684: extra.offboard_magnitude,
            processed_direct_2680: extra.offboard_turn,
            processed_stick_2692: extra.biped_world_x,
            processed_stick_2688: extra.biped_world_z,
            processed_scale_2912: attributes.magnitude_scale,
            processed_scale_2908: attributes.turn_scale,
            frame_forward_112: [frame[2][0], frame[2][1], frame[2][2]],
        },
        flags_2476: p.flags_2476,
        flags_2480: p.flags_2480,
        flags_2484: p.flags_2484,
        flags_2488: p.flags_2488,
        animation_motion_2864: attributes.animation_translation,
        duration_2896: attributes.animation_time,
        phase_2900: attributes.cadence_end_percent,
        override_duration_2904: attributes.animation_physics_blend_seconds,
        position_592: p.vectors_544_560_592_608[2].map(f32::from_bits),
        velocity_608: p.vectors_544_560_592_608[3].map(f32::from_bits),
        skeleton_motion_16320: skater.animated_skeleton.motion.velocity_world,
        skeleton_displacements_16288_16304: skater.collision_extra_displacements,
    }
}

