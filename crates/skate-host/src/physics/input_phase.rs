#[path = "input_teleport.rs"]
mod teleport;
use super::{
    GamePhysics, SkaterRuntime,
    animated_skeleton::AnimatedSkeleton,
    animation_input::AnimationInput,
    foot_ik::FootIk,
    ground_phase::GroundLifecycle,
    ground_runtime::{GroundRuntime, GroundState},
    player_input::{InputHostFrame, PlayerInputCallbacks},
    riding_outputs::RidingOutputs,
    settings::PhysicsSettings,
    skeleton_input_runtime::{
        CollisionInput, SkeletonInputRuntime, SkeletonOwners, SkeletonPoseInput,
    },
    skeleton_output::SkeletonOutput,
};
use skate_core::{
    animation::output::{NativeMatrix, attributes::AnimationAttribute},
    input::controller::ActionMap,
    physics::{
        board_runtime::BoardRuntime,
        board_toolkit::BoardToolkit,
        skeleton_body::{
            SkeletonBody, SkeletonCollisionFeedback, SkeletonCollisionMode, SkeletonDrives,
            SkeletonPoseErrors,
        },
    },
    player::input_phase::{AnimationInputPacket, PhysicalPlayerInput, ProcessedPhysicsInput},
};

pub(super) fn advance(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
    packet: &AnimationInputPacket<'_>,
    actions: &mut dyn ActionMap,
    input_available: bool,
) -> Result<bool, String> {
    skater.offboard_contact.begin_input();
    skater.landing_deck.manager.can_land_256 = false;
    skater.player_input.player.manager_1852_vector_176 =
        skater.landing_deck.manager.ik_offset_176.map(f32::to_bits);
    let host = InputHostFrame {
        actor_query_56: 0,
        actor_query_44: 0,
        input_available,
        transition_action: actions.value(71),
        published_board_transform: if skater.player_input.physical.state.flag_61 != 0 {
            skater
                .player_input
                .physical
                .teleport_output
                .ok_or("Teleport State61 requires its published reset transform")?
                .transform
                .map(|row| row.map(f32::from_bits))
        } else {
            super::solve::deck_frame(&physics.board)
        },
        air_counter_40: skater.player_state.selector.post_grind_jump_counter,
    };
    let resetting = skater.player_input.physical.state.flag_61 != 0
        || skater.player_input.player.flags_1296 & (1 << 19) != 0
        || skater.player_input.pending_teleport().is_some();
    if resetting {
        skater.offboard_air_selector.reset();
        super::offboard::board_manager::runtime::reset_for_teleport(physics, skater);
    }
    let collision = collision(skater);
    let mut callbacks = Callbacks {
        skeleton_input: &mut skater.skeleton_input,
        animated: &mut skater.animated_skeleton,
        skeleton_air: &mut skater.skeleton_air,
        footplant: &mut skater.footplant,
        handplant: &mut skater.handplant,
        body: &mut skater.skeleton,
        drives: &mut skater.skeleton_drives,
        ik: &mut skater.foot_ik,
        animation_input: &mut skater.animation_input,
        output: &mut skater.skeleton_output,
        pose_errors: &mut skater.pose_errors,
        collision_mode: &mut skater.skeleton_collision,
        feedback: &mut skater.collision_feedback,
        collision,
        ground: &mut skater.ground,
        life: &mut skater.ground_lifecycle,
        offboard_grab: &mut skater.offboard_grab,
        wipeout: &mut skater.wipeout,
        riding: &mut physics.riding,
        wiping_out: &mut physics.board_wiping_out,
        settings: &mut physics.settings,
        grind_materials: &physics.grind_materials,
        air_targeting_grind: skater.trajectory.selector.grind_locked_to_middle(),
        globals: &skater.animation.packet.hierarchy,
        attributes: skater.animation.attributes.entries(),
        actions,
        packet,
        teleported: false,
    };
    let ground_frame = callbacks.riding.reckoning_frames.ground;
    let continuation = skater.player_input.process_stage(
        &mut physics.board,
        &mut skater.ground_runtime,
        ground_frame,
        packet,
        host,
        &mut callbacks,
        super::player_input::InputStage::ThroughTeleport,
        &physics.world,
        &physics.grind_world,
    )?;
    if let Some(continuation) = continuation {
        skater.player_input.process_stage(
            &mut physics.board,
            &mut skater.ground_runtime,
            ground_frame,
            packet,
            host,
            &mut callbacks,
            super::player_input::InputStage::AfterTeleport(continuation),
            &physics.world,
            &physics.grind_world,
        )?;
    }
    let teleported = callbacks.teleported;
    if teleported {
        super::offboard::board_manager::runtime::reset_for_teleport(physics, skater);
    }
    physics.processed_flags_2468 = skater.player_input.processed.flags_2468;
    Ok(teleported)
}

pub(super) fn update_ground(
    physics: &GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    let collision = collision(skater);
    let mut owners = SkeletonOwners {
        animated: &mut skater.animated_skeleton,
        body: &mut skater.skeleton,
        drives: &mut skater.skeleton_drives,
        ik: &mut skater.foot_ik,
        animation_input: &mut skater.animation_input,
        correction: &mut skater.skeleton_output.correction,
        pose_errors: &mut skater.pose_errors,
    };
    let target = skater.skeleton_input.update_ground(
        &physics.board,
        &physics.riding.reckoning_frames.system,
        &mut skater.player_input.processed,
        &mut owners,
        &skater.animation.packet.hierarchy,
        &collision,
        physics.settings.step.simulation,
    )?;
    skater
        .skeleton_air
        .capture_physics_error(&physics.board, &target);
    Ok(())
}

pub(super) fn collision(skater: &SkaterRuntime) -> CollisionInput {
    CollisionInput {
        contact_4070: skater.collision_feedback.flags.compliant,
        has_pose_error_4077: skater.collision_feedback.flags.has_impulse,
        pose_error_16272: skater.collision_pose_error,
        partial_ragdoll: skater.skeleton_collision.partial_ragdoll,
        drive_weight_4028: skater.collision_feedback.drive_weight,
    }
}

struct Callbacks<'a, 'p> {
    skeleton_input: &'a mut SkeletonInputRuntime,
    animated: &'a mut AnimatedSkeleton,
    skeleton_air: &'a mut super::skeleton_air::SkeletonAir,
    footplant: &'a mut super::footplant::Footplant,
    handplant: &'a mut super::handplant::Handplant,
    body: &'a mut SkeletonBody,
    drives: &'a mut SkeletonDrives,
    ik: &'a mut FootIk,
    animation_input: &'a mut AnimationInput,
    output: &'a mut SkeletonOutput,
    pose_errors: &'a mut SkeletonPoseErrors,
    collision_mode: &'a mut SkeletonCollisionMode,
    feedback: &'a mut SkeletonCollisionFeedback,
    collision: CollisionInput,
    ground: &'a mut GroundState,
    life: &'a mut GroundLifecycle,
    offboard_grab: &'a mut super::biped_ground::grab_runtime::Owner,
    wipeout: &'a mut super::wipeout::Wipeout,
    riding: &'a mut RidingOutputs,
    wiping_out: &'a mut bool,
    settings: &'a mut PhysicsSettings,
    grind_materials: &'a super::grind_materials::GrindMaterials,
    air_targeting_grind: bool,
    globals: &'a [NativeMatrix],
    attributes: &'a [AnimationAttribute],
    actions: &'a mut dyn ActionMap,
    packet: &'a AnimationInputPacket<'p>,
    teleported: bool,
}
impl PlayerInputCallbacks for Callbacks<'_, '_> {
    fn prepare_grind(
        &mut self,
        board: &mut BoardRuntime,
        grind: &mut super::player_input::grind::GrindInputState,
        processed: &ProcessedPhysicsInput,
        world: &skate_core::physics::board_world::BoardWorld,
        provider: &crate::grind_world::StaticProvider,
        air_counter: i32,
    ) -> Result<super::player_input::grind::Pending, String> {
        let extra = &self.animation_input.extra;
        let context = super::player_input::grind::PreContext {
            board: super::solve::deck_frame(board),
            air_counter,
            tip_state: processed.state_2504,
            air_targeting_grind_9653: self.air_targeting_grind,
            balance_2720: self.animation_input.fields.balance,
            translation_2796: extra.grind_translation,
            stability_nudge_2800: extra.grind_stability_nudge,
            up_down_2804: extra.grind_up_down,
            grab_min_height_2808: extra.grind_grab_min_height,
        };
        let mut host = super::grind_host::LiveHost {
            board,
            settings: self.settings,
            materials: self.grind_materials,
        };
        grind.pre_update(processed, provider, world, context, &mut host)
    }
    fn process_skeleton(
        &mut self,
        board: &mut BoardRuntime,
        toolkit: &BoardToolkit,
        packet: &AnimationInputPacket<'_>,
        physical: &mut PhysicalPlayerInput,
        processed: &mut ProcessedPhysicsInput,
    ) -> Result<(), String> {
        self.animation_input.select_physics_mode(processed.state_variant_index_2528)?;
        self.skeleton_input.process_data(
            board,
            toolkit,
            packet,
            physical,
            processed,
            &mut SkeletonOwners {
                animated: self.animated,
                body: self.body,
                drives: self.drives,
                ik: self.ik,
                animation_input: self.animation_input,
                correction: &mut self.output.correction,
                pose_errors: self.pose_errors,
            },
            SkeletonPoseInput {
                globals: self.globals,
                attributes: self.attributes,
                actions: self.actions,
            },
            &self.collision,
        )
    }
    fn teleport(
        &mut self,
        board: &mut BoardRuntime,
        runtime: &mut GroundRuntime,
        target: NativeMatrix,
        player: &mut skate_core::player::input_phase::PlayerInputState,
        physical: &mut PhysicalPlayerInput,
        processed: &mut ProcessedPhysicsInput,
    ) -> Result<(), String> {
        self.reset_player(board, runtime, target, player, physical, processed)
    }
}
