use crate::physics::{
    foot_ik::PhysicalInput,
    skeleton_input_runtime::{SkeletonInputRuntime, SkeletonOwners},
};
use skate_core::{
    animation::output::NativeMatrix,
    physics::{skeleton_animation_record::IDENTITY, skeleton_body::SkeletonTargetInput},
    player::{
        input_phase::ProcessedPhysicsInput,
        wipeout_state::{drives, skeleton},
    },
};

pub(crate) fn update(
    input: &mut SkeletonInputRuntime,
    owners: &mut SkeletonOwners<'_>,
    processed: &ProcessedPhysicsInput,
    globals: &[NativeMatrix],
    settings: &drives::Settings,
    start: f32,
    end: f32,
    controlled: f32,
    extra: f32,
    skip_targets: bool,
) -> Result<f32, String> {
    let s = &mut owners.animated;
    s.roots.initialize_heading = false;
    skeleton::update_roots(
        &mut s.roots,
        &s.animation_hips,
        &owners.body.record.pose[23],
        owners.body.record.velocities[23],
        s.board_frames.centre_of_mass,
        processed.vectors_544_560_592_608[3].map(f32::from_bits),
        processed.state_timer_2664,
    );
    let target_input = SkeletonTargetInput {
        animation_hips: &s.animation_hips,
        animation_board: &s.animation_board,
        animation_to_world: &s.roots.animation_to_world,
        inverse_board: &s.roots.inverse_board,
        skate_root: &s.board_frames.skate_root,
        com_frame: &s.board_frames.com_frame,
        lifted_com_frame: &s.board_frames.lifted_com_frame,
        teleporting: input.teleporting,
    };
    if !skip_targets {
        input.animation_board_to_physics =
            owners.drives.targets.update_hook_positions(&target_input);
        let positions = owners.drives.targets.update_extra_targets(
            owners.body,
            target_input.com_frame,
            target_input.lifted_com_frame,
        );
        input.extra_target_positions =
            [positions.com, positions.lifted_com, positions.following_com];
        owners.pose_errors.set_targets(positions);
    }
    s.board_frames.skate_root = s.roots.animation_to_world;
    s.board_frames.update_com_lift(
        &s.roots.animation_to_world,
        s.board_frames.centre_of_mass,
        0.0,
    );
    if processed.state_timer_2664 == 0.0 {
        let contacts = std::array::from_fn(|i| {
            let query = processed.line_tests_960_1008_1056[i];
            (query.valid != 0).then(|| query.position.map(f32::from_bits))
        });
        let result = owners.ik.update(
            s,
            globals,
            PhysicalInput {
                state_id: processed.state_2508,
                flags_2468: processed.flags_2468,
                flags_2472: processed.flags_2472,
                flags_2480: processed.flags_2480,
                contact_bone: owners.animation_input.contacts.bone as usize,
                physical_board: &s.board_frames.physical_board,
                hips_world_position: owners.body.part_transforms()[23][3],
                current_contacts: contacts,
            },
        )?;
        input.drive_frames = result.frames;
    }
    let residual = drives::update(
        owners.drives,
        &input.drive_frames,
        settings,
        drives::Weights {
            start,
            end,
            controlled,
            upper_extra: if processed.flags_2484 & 0x20 != 0 {
                extra
            } else {
                0.0
            },
            lower_extra: if processed.flags_2484 & 0x10 != 0 {
                extra
            } else {
                0.0
            },
        },
    );
    s.motion.next_trajectory = IDENTITY;
    Ok(residual)
}

