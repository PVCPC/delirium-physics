use super::{GamePhysics, SkaterRuntime};
use crate::camera::{
    CameraAirOutput, CameraAnimationOutput, CameraEventsOutput, CameraGrindOutput,
    CameraOffboardOutput, CameraPreferences, CameraPublicationInputs, CameraStateOutput,
};
use skate_core::{
    animation::physical_feedback::PhysicalFeedback,
    camera::{MovingObstacleProvider, PathObstacle},
    physics::centre_of_mass_filter::CentreOfMassOutput,
};

pub(crate) fn advance(
    physics: &GamePhysics,
    skater: &SkaterRuntime,
    feedback: &PhysicalFeedback,
    camera: &mut crate::camera::CameraRuntime,
) -> Result<(), String> {
    let output = physics
        .exchange
        .output()
        .ok_or("Camera requires the completed physical output snapshot")?;
    let completed_tick = physics
        .ticks
        .checked_sub(1)
        .ok_or("Camera received physical output before the first completed tick")?;
    if output.tick != completed_tick || output.state != skater.player_state.current() {
        return Err(format!(
            "Camera received stale physical output: output_tick={}, completed_tick={}, output_state={:?}, selected_state={:?}",
            output.tick,
            completed_tick,
            output.state,
            skater.player_state.current(),
        ));
    }
    let inputs = publish(
        skater,
        output,
        feedback,
        skater.centre_of_mass_output,
        CameraPreferences {
            invert_look: [false; 2],
            shake_variant: 0,
            value_32: 0.0,
        },
        u8::from(skater.animation.stance().1),
        1,
        output.tick,
    )?;
    let snapshot = crate::camera::publish_camera_subject(physics, skater, &inputs)?;
    let environment = crate::camera::CameraGraphEnvironment {
        camera_type: 1,
        on_road: false,
        ledge_left: false,
        ledge_right: false,
        volumes: Vec::new(),
    };
    let gravity = physics.settings.step.simulation.gravity_acceleration;
    camera
        .advance(
            physics.settings.step.simulation.time_step,
            snapshot,
            physics.world(),
            [gravity.x, gravity.y, gravity.z, 0.0],
            &environment,
            &mut StaticWorld,
        )
        .map(|_| ())
}

struct StaticWorld;
impl MovingObstacleProvider for StaticWorld {
    fn collect(&mut self, _: [f32; 4], _: [f32; 4], _: f32, _: &mut [PathObstacle; 50]) -> usize {
        0
    }
}

pub(crate) fn publish(
    skater: &SkaterRuntime,
    output: &skate_core::physics::phase::PhysicalOutputSnapshot,
    feedback: &PhysicalFeedback,
    com: CentreOfMassOutput,
    preferences: CameraPreferences,
    skater_animation_stance: u8,
    context: u32,
    tick: u64,
) -> Result<CameraPublicationInputs, String> {
    let p = &skater.player_input.processed;
    let physical = &skater.player_input.physical;
    let fields = &skater.animation_input.fields;
    let intents = skater.animation_input.output.flags;
    let packet = &skater.animation.packet;
    Ok(CameraPublicationInputs {
        tick,
        state: CameraStateOutput {
            height_32: physical.state.surface_height_32,
            physically_pushing_55: bit(p.flags_2468, 25),
            wiping_out_59: bit(p.flags_2468, 18),
            manual_60: u8::from(fields.balance != 0.0),
            reset_62: bit(p.flags_2472, 10),
            use_skeleton_root_75: u8::from(physical.state.category_12 == 500),
            flag_79: u8::from(p.state_variant_index_2528 == 3),
            flag_81: u8::from(skater.player_state.state_flags[81 - 52]),
        },
        animation: CameraAnimationOutput {
            conditioned_turn: feedback.conditioned_turn,
            input_turn_64: fields.turn,
            input_kickturn_68: p.spin_input_2672,
            time_since_input_128: p.time_since_last_input_2748,
            wipeout_tweak_148: physical.animation.profile_148,
            stance_155: u8::from(packet.riding_fakie),
            running_out_160: bit(p.flags_2480, 2),
            skater_animation_stance,
        },
        air: CameraAirOutput {
            apex_0: physical.air.trajectory_apex_0.map(f32::from_bits),
            landing_position_16: physical.air.collision_position_16.map(f32::from_bits),
            landing_normal_32: physical.air.landing_normal_32.map(f32::from_bits),
            launch_position_48: physical.air.selector_vector_48.map(f32::from_bits),
            heading_80: physical.air.landing_heading_80.map(f32::from_bits),
            time_176: physical.air.time_in_state_176,
            duration_180: physical.air.collision_time_180,
            apex_time_196: physical.air.time_to_apex_196,
            flag_440: bit(p.flags_2468, 22),
        },
        offboard: offboard_output(&physical.off_board),
        grinds: CameraGrindOutput {
            direction_0: physical.grinds.direction_0.map(f32::from_bits),
            camera_target_96: physical.grinds.camera_target_96.map(f32::from_bits),
            grinding_316: physical.grinds.grinding_316,
        },
        events: CameraEventsOutput {
            intent_51: bit(intents, 28),
            preparing_52: bit(intents, 27),
            dropping_in_63: bit(intents, 22),
            trick_125: bit(p.flags_2480, 11),
            hippy_jump_322: physical.ground.hippy_jumping_322,
            broken_bone_duration_200: 0.0,
            capabilities_204: physical.scoring.capabilities_204,
        },
        damped_com_80: com.position,
        ground_up_80: [
            output.ground_normal.x,
            output.ground_normal.y,
            output.ground_normal.z,
            0.0,
        ],
        ground_scalar_288: 0.0,
        look_552_556: [
            skater.animation_input.extra.look_x,
            skater.animation_input.extra.look_y,
        ],
        collision_look_target_64: [
            output.predicted_position.x,
            output.predicted_position.y,
            output.predicted_position.z,
            0.0,
        ],
        preferences,
        context,
    })
}

fn bit(value: u32, shift: u32) -> u8 {
    ((value >> shift) & 1) as u8
}

fn offboard_output(
    output: &skate_core::player::input_phase::OffBoardOutputFields,
) -> CameraOffboardOutput {
    CameraOffboardOutput {
        duration_92: output.scalar_92,
        time_152: output.scalar_152,
        apex_time_156: output.scalar_156,
        launch_normal_160: output.vector_160.map(f32::from_bits),
        launch_position_176: output.vector_176.map(f32::from_bits),
        landing_normal_192: output.vector_192.map(f32::from_bits),
        landing_position_208: output.vector_208.map(f32::from_bits),
        heading_224: output.vector_224.map(f32::from_bits),
        apex_240: output.vector_240.map(f32::from_bits),
        object_held_304: output.flag_308,
        hurdle_317: output.hippy_hurdling_317,
        use_trajectory_331: output.trajectory_valid_331,
        dropping_in_334: output.flag_334,
    }
}

