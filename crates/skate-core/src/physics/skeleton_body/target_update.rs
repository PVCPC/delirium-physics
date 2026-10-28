use super::{SkeletonBody, SkeletonTargets};
use crate::{
    math::Vector3,
    physics::{
        board_pose::orthonormalize_rotation,
        native_arithmetic::dot3,
        skeleton_animation_record::{AnimationPartTransform as Transform, compose_affine},
    },
    riding::ground_orientation::clamp_length,
};

pub struct SkeletonTargetInput<'a> {
    pub animation_hips: &'a Transform,
    pub animation_board: &'a Transform,
    pub animation_to_world: &'a Transform,
    pub inverse_board: &'a Transform,
    pub skate_root: &'a Transform,
    pub com_frame: &'a Transform,
    pub lifted_com_frame: &'a Transform,
    pub teleporting: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct ExtraTargetPositions {
    pub com: [f32; 4],
    pub lifted_com: [f32; 4],
    pub following_com: [f32; 4],
}
pub struct SkeletonTargetUpdate {
    pub animation_board_to_physics: Transform,
    pub positions: ExtraTargetPositions,
    pub continuous: bool,
}

impl SkeletonTargets {
    pub fn update_hook_positions(&mut self, input: &SkeletonTargetInput<'_>) -> Transform {
        let hips = compose_affine(input.animation_to_world, input.animation_hips);
        let board = compose_affine(input.animation_to_world, input.animation_board);
        let animation_board_to_physics = compose_affine(input.inverse_board, &board);
        self.set_transform(0, orthonormalize(hips));
        self.set_transform(1, orthonormalize(*input.skate_root));
        animation_board_to_physics
    }

    pub fn update_positions(
        &mut self,
        input: SkeletonTargetInput<'_>,
        skeleton: &mut SkeletonBody,
    ) -> SkeletonTargetUpdate {
        let previous = self.transform(0)[3];
        let animation_board_to_physics = self.update_hook_positions(&input);
        let hips = self.transform(0)[3];
        let change = std::array::from_fn(|i| previous[i] - hips[i]);
        let positions =
            self.update_extra_targets(skeleton, input.com_frame, input.lifted_com_frame);
        let continuous = !input.teleporting && !(dot3(change, change) > 1.0);
        SkeletonTargetUpdate {
            animation_board_to_physics,
            positions,
            continuous,
        }
    }

    pub fn update_extra_targets(
        &mut self,
        skeleton: &mut SkeletonBody,
        com_frame: &Transform,
        lifted_com_frame: &Transform,
    ) -> ExtraTargetPositions {
        self.set_transform(2, *lifted_com_frame);
        self.set_transform(3, *com_frame);
        let positions = ExtraTargetPositions {
            com: com_frame[3],
            lifted_com: lifted_com_frame[3],
            following_com: com_frame[3],
        };
        let parts = skeleton.part_transforms();
        for (part, target) in [(24, lifted_com_frame[3]), (25, com_frame[3])] {
            let current = parts[part][3];
            let velocity = Vector3::new(
                (target[0] - current[0]) * 60.0,
                (target[1] - current[1]) * 60.0,
                (target[2] - current[2]) * 60.0,
            );
            skeleton.bodies_mut()[part].rates.linear_velocity = clamp_length(velocity, 40.0);
        }
        positions
    }
}
fn orthonormalize(frame: Transform) -> Transform {
    let words = orthonormalize_rotation(std::array::from_fn(|i| frame[i / 4][i % 4].to_bits()));
    std::array::from_fn(|i| std::array::from_fn(|j| f32::from_bits(words[i * 4 + j])))
}
