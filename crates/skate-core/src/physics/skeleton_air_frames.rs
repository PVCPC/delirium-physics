use super::{
    skeleton_animation_record::{AnimationPartTransform as Transform, IDENTITY, compose_affine},
    skeleton_board_frames::SkeletonBoardFrames,
    skeleton_root::{SkeletonRootFrames, inverse_rigid, orthonormalize},
};
use crate::trigonometry;

#[derive(Clone, Copy, Debug)]
pub struct AirDismountRevert {
    pub requested: bool,
    pub frames: u32,
    pub goofy: bool,
}

pub fn prepare_animated(
    roots: &SkeletonRootFrames,
    board: &mut SkeletonBoardFrames,
    mapped_board: &Transform,
    flags: &mut u32,
) -> Transform {
    let local = compose_affine(&roots.animation_to_board, mapped_board);
    let mut placement = IDENTITY;
    placement[3] = roots.predicted_board_position;
    let target = compose_affine(&placement, &local);
    *flags |= 1 << 19;
    board.animation_target = target;
    board.skate_root = target;
    board.update_com_lift(&roots.animation_to_world, board.centre_of_mass, 0.0);
    target
}

pub fn update_known_air_roots(
    roots: &mut SkeletonRootFrames,
    reckoning: &Transform,
    target_com: [f32; 4],
    animation_com: [f32; 4],
    revert: AirDismountRevert,
) {
    if roots.initialize_heading {
        roots.heading_alignment =
            compose_affine(&inverse_rigid(reckoning), &roots.animation_to_world);
        roots.heading_alignment[3] = [0.0; 4];
        roots.initialize_heading = false;
    }
    if revert.requested {
        let mut angle = f32::from_bits(0x4049_0fdb) / revert.frames as f32;
        if !revert.goofy {
            angle = -angle;
        }
        let (sin, cos) = trigonometry::sin_cos(angle);
        let rotation = [
            [cos, 0.0, -sin, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [sin, 0.0, cos, 0.0],
            [0.0; 4],
        ];
        roots.heading_alignment =
            orthonormalize(compose_affine(&roots.heading_alignment, &rotation));
    }
    let mut world = compose_affine(reckoning, &roots.heading_alignment);
    world[3] = std::array::from_fn(|lane| {
        let x = world[0][lane] * animation_com[0];
        let y = world[1][lane].mul_add(animation_com[1], x);
        target_com[lane] - world[2][lane].mul_add(animation_com[2], y)
    });
    roots.animation_to_world = orthonormalize(world);
    roots.world_to_animation = inverse_rigid(&roots.animation_to_world);
}

pub fn prepare_known_air(
    roots: &SkeletonRootFrames,
    board: &mut SkeletonBoardFrames,
    mapped_board: &Transform,
    flags: &mut u32,
) -> Transform {
    let target = compose_affine(&roots.animation_to_world, mapped_board);
    *flags |= 1 << 19;
    board.animation_target = target;
    target
}

pub fn finish_known_air(
    roots: &mut SkeletonRootFrames,
    board: &mut SkeletonBoardFrames,
    effective_board: Transform,
) {
    board.physical_board = effective_board;
    board.skate_root = effective_board;
    board.update_com_lift(&roots.animation_to_world, board.centre_of_mass, 0.0);
    roots.predicted_board_position = effective_board[3];
    roots.supplied_prediction = Some(effective_board[3]);
}

pub fn update_plant_roots(
    roots: &mut SkeletonRootFrames,
    reckoning: &Transform,
    world_anchor: [f32; 4],
    animation_anchor: [f32; 4],
) {
    let mut world = compose_affine(reckoning, &roots.heading_alignment);
    world[3] = std::array::from_fn(|i| {
        let x = world[0][i] * animation_anchor[0];
        let y = world[1][i].mul_add(animation_anchor[1], x);
        world_anchor[i] - world[2][i].mul_add(animation_anchor[2], y)
    });
    roots.animation_to_world = orthonormalize(world);
    roots.world_to_animation = inverse_rigid(&roots.animation_to_world);
}
