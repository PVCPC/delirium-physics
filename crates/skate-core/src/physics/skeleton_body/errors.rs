use super::{
    ExtraTargetPositions, SkeletonCollisionFeedback, SkeletonPhysicalRecord, collision_feedback::V,
    collision_vector::*,
};
use crate::physics::skeleton_animation_record::AnimationPartTransform;

#[derive(Clone, Debug, Default)]
pub struct SkeletonPoseErrors {
    pub parts: [V; 24],
    pub extra: [V; 2],
    pub targets: [V; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct SkeletonNormalError {
    pub impulse: V,
    pub extra: [V; 2],
    pub maximum_error: f32,
}
impl SkeletonPoseErrors {
    pub fn reset_history(&mut self) {
        self.targets = [[0.0; 4]; 3];
    }
    pub fn set_targets(&mut self, positions: ExtraTargetPositions) {
        self.targets = [positions.com, positions.lifted_com, positions.following_com];
    }
    pub fn update(
        &mut self,
        physical: &SkeletonPhysicalRecord,
        animation_to_world: AnimationPartTransform,
        drive_frames: &[AnimationPartTransform; 24],
    ) {
        self.parts[0] = [0.0; 4];
        for part in 1..24 {
            self.parts[part] = sub(
                physical.pose[part][3],
                transform(animation_to_world, drive_frames[part][3]),
            );
        }
        for i in 0..2 {
            self.extra[i] = sub(physical.pose[24 + i][3], self.targets[1 + i]);
        }
    }
    pub fn normal_response(
        &self,
        collision: &SkeletonCollisionFeedback,
        support_normal: V,
    ) -> SkeletonNormalError {
        let candidates = [23, 17, 18, 16, 21, 22, 20, 1].map(|part| self.parts[part]);
        let filtered =
            candidates.map(|candidate| collision.filter_error(candidate, support_normal));
        let mut impulse = filtered[0];
        for candidate in &filtered[1..] {
            if dot(*candidate, *candidate) > dot(impulse, impulse) {
                impulse = *candidate;
            }
        }
        SkeletonNormalError {
            impulse,
            extra: [[0.0; 4]; 2],
            maximum_error: self.maximum_error(),
        }
    }
    pub fn partial_response(&self) -> SkeletonNormalError {
        SkeletonNormalError {
            impulse: self.extra[0],
            extra: self.extra,
            maximum_error: self.maximum_error(),
        }
    }
    fn maximum_error(&self) -> f32 {
        let candidates = [23, 17, 18, 16, 21, 22, 20, 1].map(|part| self.parts[part]);
        let mut maximum = if dot(candidates[0], candidates[0]) > dot(candidates[1], candidates[1]) {
            candidates[0]
        } else {
            candidates[1]
        };
        for candidate in &candidates[2..] {
            if dot(*candidate, *candidate) > dot(maximum, maximum) {
                maximum = *candidate;
            }
        }
        length(maximum)
    }
}
