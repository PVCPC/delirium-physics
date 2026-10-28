use super::status::{LimbStatus, Mode};
use crate::physics::skeleton_animation_record::{
    AnimationPartTransform as Transform, IDENTITY, compose_affine,
};

#[derive(Clone, Copy, Debug)]
pub struct LimbBinding {
    pub part: usize,
    pub parent_part: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct LimbFrames {
    pub target: Transform,
    pub world: Transform,
    pub external_world: Transform,
    pub board: Transform,
    pub parent_world: Transform,
    pub external_parent_world: Transform,
    pub parent_board: Transform,
    pub within_contact_bounds: bool,
}
impl Default for LimbFrames {
    fn default() -> Self {
        Self {
            target: IDENTITY,
            world: IDENTITY,
            external_world: IDENTITY,
            board: IDENTITY,
            parent_world: IDENTITY,
            external_parent_world: IDENTITY,
            parent_board: IDENTITY,
            within_contact_bounds: false,
        }
    }
}

pub fn prepare_animation_target(
    status: &mut LimbStatus,
    frames: &mut LimbFrames,
    binding: LimbBinding,
    animation: &[Transform; 24],
    animation_to_world: &Transform,
    inverse_animation_board: &Transform,
    contact_bounds: [f32; 4],
) {
    if status.mode != Mode::OnDeck && !(status.board_blend > 0.0) && status.external_blend >= 1.0 {
        return;
    }
    let animated = &animation[binding.part];
    frames.board = compose_affine(inverse_animation_board, &frames.target);
    status.part_position = frames.board[3];
    frames.world = compose_affine(animation_to_world, animated);
    frames.within_contact_bounds = (0..3).all(|i| {
        !(status.part_position[i] > contact_bounds[i]
            || -contact_bounds[i] > status.part_position[i])
    });
    if let Some(parent) = binding.parent_part {
        let relative = compose_affine(&inverse_rigid(animated), &animation[parent]);
        frames.parent_board = compose_affine(&frames.board, &relative);
        frames.parent_world = compose_affine(&frames.world, &relative);
    }
}

pub fn inverse_rigid(source: &Transform) -> Transform {
    let mut result = IDENTITY;
    for axis in 0..3 {
        result[axis] = [source[0][axis], source[1][axis], source[2][axis], 0.0];
    }
    let translation = source[3].map(|v| 0.0 - v);
    result[3] = core::array::from_fn(|i| {
        let z = translation[2] * result[2][i];
        let y = translation[1].mul_add(result[1][i], z);
        translation[0].mul_add(result[0][i], y)
    });
    result
}
