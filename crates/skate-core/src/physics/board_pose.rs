mod arithmetic;
mod normalization;
mod part;
use arithmetic::*;
pub use normalization::{orthonormalize_part_basis, orthonormalize_rotation};
pub use part::{PartPose, part_transform, set_part_transform};

pub type PoseMatrix = [u32; 16];

pub fn set_board_transform(parts: &mut [PartPose; 7], hook: &mut PartPose, requested: PoseMatrix) {
    let old_deck = matrix(part_transform(&parts[6]));
    let normalized = orthonormalize_rotation(requested);
    let delta = compose(matrix(normalized), inverse_rigid(old_deck));
    set_part_transform(&mut parts[6], normalized);
    for part in &mut parts[..6] {
        let moved = compose(delta, matrix(part_transform(part)));
        set_part_transform(part, orthonormalize_rotation(words(moved)));
    }
    set_part_transform(hook, requested);
}
