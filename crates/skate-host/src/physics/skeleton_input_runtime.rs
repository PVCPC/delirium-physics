#[path = "skeleton_input_general.rs"]
mod general;
#[path = "skeleton_input_teleport.rs"]
mod teleport;

use super::{
    animated_skeleton::AnimatedSkeleton, animation_input::AnimationInput, foot_ik::FootIk,
};
use skate_core::{
    animation::output::{NativeMatrix, attributes::AnimationAttribute},
    input::controller::ActionMap,
    physics::{
        board::BodyId,
        board_runtime::BoardRuntime,
        board_toolkit::BoardToolkit,
        skeleton_animation_record::{AnimationPartTransform as Transform, IDENTITY},
        skeleton_body::{SkeletonBody, SkeletonDrives, SkeletonPoseErrors},
        skeleton_landing::LandingInput,
        skeleton_output::correction::CorrectionState,
    },
    player::input_phase::{AnimationInputPacket, PhysicalPlayerInput, ProcessedPhysicsInput},
    riding::ground_correction_math::dot_product as dot3,
};

pub(crate) struct CollisionInput {
    pub contact_4070: bool,
    pub has_pose_error_4077: bool,
    pub pose_error_16272: [f32; 4],
    pub partial_ragdoll: bool,
    pub drive_weight_4028: f32,
}

pub(crate) struct SkeletonOwners<'a> {
    pub animated: &'a mut AnimatedSkeleton,
    pub body: &'a mut SkeletonBody,
    pub drives: &'a mut SkeletonDrives,
    pub ik: &'a mut FootIk,
    pub animation_input: &'a mut AnimationInput,
    pub correction: &'a mut CorrectionState,
    pub pose_errors: &'a mut SkeletonPoseErrors,
}

pub(crate) struct SkeletonPoseInput<'a> {
    pub globals: &'a [NativeMatrix],
    pub attributes: &'a [AnimationAttribute],
    pub actions: &'a mut dyn ActionMap,
}

pub(crate) struct SkeletonInputRuntime {
    pub deck_velocity: [f32; 4],
    pub root_velocity: [f32; 4],
    pub previous_root_position: [f32; 4],
    pub animation_board_to_physics: Transform,
    pub extra_target_positions: [[f32; 4]; 3],
    pub reset_local_hips: [f32; 4],
    pub drive_frames: [Transform; 24],
    pub reenable_requested: bool,
    pub invalid_target_reset: bool,
    pub teleporting: bool,
    pub force_mode: u32,
    pub head_tracking_history: [[f32; 4]; 8],
    pub head_tracking_active: bool,
    pub grind_air_started: bool,
    pub grind_air_active: bool,
    pub grind_air_adjusting: bool,
    pub grind_air: skate_core::physics::grind_air::GrindAir,
    grind_air_settings: Option<skate_core::physics::grind_air::Settings>,
}

impl Default for SkeletonInputRuntime {
    fn default() -> Self {
        Self {
            deck_velocity: [0.; 4],
            root_velocity: [0.; 4],
            previous_root_position: [0.; 4],
            animation_board_to_physics: IDENTITY,
            extra_target_positions: [[0.; 4]; 3],
            reset_local_hips: [0.; 4],
            drive_frames: [IDENTITY; 24],
            reenable_requested: false,
            invalid_target_reset: false,
            teleporting: false,
            force_mode: 0,
            head_tracking_history: [[0.0; 4]; 8],
            head_tracking_active: false,
            grind_air_started: false,
            grind_air_active: false,
            grind_air_adjusting: false,
            grind_air: Default::default(),
            grind_air_settings: None,
        }
    }
}

impl SkeletonInputRuntime {
    pub fn load(data: &skate_data::collections::Collections) -> Result<Self, String> {
        Ok(Self {
            grind_air_settings: Some(super::grind_air_settings::load(data)?),
            ..Self::default()
        })
    }
    pub fn process_data(
        &mut self,
        board: &mut BoardRuntime,
        toolkit: &BoardToolkit,
        packet: &AnimationInputPacket<'_>,
        _physical: &mut PhysicalPlayerInput,
        p: &mut ProcessedPhysicsInput,
        owners: &mut SkeletonOwners<'_>,
        pose: SkeletonPoseInput<'_>,
        collision: &CollisionInput,
    ) -> Result<(), String> {
        let velocity = board.bodies()[BodyId::Deck.index()].rates.linear_velocity;
        self.deck_velocity = [velocity.x, velocity.y, velocity.z, 0.];
        for body in owners.body.bodies_mut() {
            if body.state_flags & 7 == 2 {
                body.state_flags = (body.state_flags & 8) | 4;
            }
            body.rates.cool_down = 0;
        }
        let input = &mut owners.animation_input;
        input.reset_processed();
        input.fields.flags2468 = p.flags_2468;
        input.fields.flags2472 = p.flags_2472;
        input.fields.flags2476 = p.flags_2476;
        input.fields.flags2484 = p.flags_2484;
        input.fields.flags2488 = p.flags_2488;
        input.extra.flags2480 = p.flags_2480;
        input.process(
            pose.attributes,
            pose.globals,
            packet.publication.timestep,
            packet.flags_10932,
            packet.publication.flags_10375_10496_10784[1] != 0,
            pose.actions,
        )?;
        publish_attribute_flags(input, p);
        p.spin_input_2672 = input.fields.spin;

        let reparented_hands = owners.animated.reparented_hand_indices();
        super::offboard::pose_adjust::update(owners.animated, pose.globals, reparented_hands, p)?;
        if p.flags_2476 & 0x100 == 0 {
            self.grind_air_adjusting = false;
            if self.grind_air_active {
                let settings = self
                    .grind_air_settings
                    .as_ref()
                    .ok_or("GrindAirAdjust stock settings were not loaded")?;
                self.grind_air_adjusting = super::skeleton_grind_air::update(
                    &mut self.grind_air,
                    settings,
                    toolkit.deck,
                    p,
                    owners.animated,
                    pose.globals,
                )?;
            }
        }
        let up = p.vectors_544_560_592_608[0].map(f32::from_bits);
        let com_velocity = p.vectors_544_560_592_608[3].map(f32::from_bits);
        let com_delta = std::array::from_fn(|i| {
            owners.body.record.centre_of_mass[i] - owners.body.record.positions[0][i]
        });
        let landing = LandingInput {
            filtered_state: p.filtered_state_2524,
            flags_2468: p.flags_2468,
            flags_2472: p.flags_2472,
            flags_2476: p.flags_2476,
            balance: input.fields.balance,
            physical_com_velocity_along_up: dot3(com_velocity, up),
            physical_com_height: dot3(com_delta, up),
            animation_com_height: owners.animated.record.centre_of_mass[1],
        };
        owners.animated.process_pose(
            pose.globals,
            landing,
            packet.publication.timestep,
            &mut p.flags_2468,
            &mut p.flags_2472,
            (p.flags_2476 & 0x10100 == 0x100).then_some((
                toolkit.deck,
                p.vectors_720_784_800_816_832_864[5].map(f32::from_bits),
                p.flags_2480,
                p.off_board_scalar_2832,
            )),
        )?;
        self.drive_frames = owners.animated.record.pose;
        p.board_at_y_delta_2768 = owners.animated.board_at_y_delta;
        p.flags_2472 = (p.flags_2472 & !(1 << 17)) | (u32::from(collision.contact_4070) << 17);
        if collision.has_pose_error_4077 {
            p.collision_pose_error_736 = collision.pose_error_16272.map(f32::to_bits);
        }
        p.animation_com_to_deck_752 = owners.animated.record.com_to_deck_world.map(f32::to_bits);
        p.animation_com_to_deck_delta_768 = owners
            .animated
            .record
            .com_to_deck_world_delta
            .map(f32::to_bits);
        input.fields.flags2468 = p.flags_2468;
        input.fields.flags2472 = p.flags_2472;
        if self.reenable_requested {
            self.reenable_requested = false;
            owners.ik.enable_feet(true);
            general::restore_board_target_drives(owners.drives);
            owners.animated.roots.supplied_prediction = None;
        }
        Ok(())
    }
}

fn publish_attribute_flags(input: &AnimationInput, p: &mut ProcessedPhysicsInput) {
    let scalar = &input.fields;
    p.flags_2468 = scalar.flags2468;
    p.flags_2472 = scalar.flags2472;
    p.flags_2476 = scalar.flags2476;
    p.flags_2484 = scalar.flags2484;
    p.flags_2488 = scalar.flags2488;
    p.flags_2480 = input.extra.flags2480;
}
