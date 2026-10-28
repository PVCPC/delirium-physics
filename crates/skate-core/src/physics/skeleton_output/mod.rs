pub mod board;
pub mod wobble;
pub mod correction;

use super::skeleton_animation_record::{AnimationPartTransform as Transform, compose_affine};
use crate::animation::foot_ik::{drive::Geometry, transforms::inverse_rigid};

pub struct SkeletonOutput {
    pub bone_indices: [usize; 24],
    pub geometry: Geometry,
    pub board_bones: board::BoneIndices,
    pub board_settings: board::Settings,
}
pub struct Input<'a> {
    pub physical_parts: &'a [Transform; 24],
    pub world_to_animation: &'a Transform,
    pub board: board::Input<'a>,
}
impl SkeletonOutput {
    pub fn publish(
        &self,
        input: Input<'_>,
        globals: &mut [Transform],
        locals: &mut [Transform],
    ) -> Result<(), &'static str> {
        if globals.len() != locals.len()
            || self.bone_indices.iter().any(|&i| i >= globals.len())
            || self.board_bones.all().iter().any(|&i| i >= locals.len())
        {
            return Err("physical pose output does not match the stock animation hierarchy");
        }
        for part in 0..24 {
            let world = compose_affine(
                &input.physical_parts[part],
                &self.geometry.inverse_part_frames[part],
            );
            globals[self.bone_indices[part]] = compose_affine(input.world_to_animation, &world);
        }
        for part in (0..24).rev() {
            let bone = self.bone_indices[part];
            locals[bone] = if let Some(parent) = self.geometry.parents[part] {
                compose_affine(
                    &inverse_rigid(&globals[self.bone_indices[parent]]),
                    &globals[bone],
                )
            } else {
                globals[bone]
            };
        }
        board::publish(input.board, &self.board_settings, self.board_bones, locals);
        Ok(())
    }
}

