use super::{ANIMATION_PART_COUNT, PART_COUNT};
use crate::physics::skeleton_animation_record::{AnimationPartTransform, IDENTITY};

#[derive(Clone, Debug)]
pub struct SkeletonPhysicalRecord {
    pub pose: [AnimationPartTransform; PART_COUNT],
    pub positions: [[f32; 4]; PART_COUNT],
    pub velocities: [[f32; 4]; PART_COUNT],
    pub velocity_changes: [[f32; 4]; PART_COUNT],
    pub centre_of_mass: [f32; 4],
    pub centre_of_mass_velocity: [f32; 4],
    pub timestep: f32,
}
impl Default for SkeletonPhysicalRecord {
    fn default() -> Self {
        Self { pose: [IDENTITY; PART_COUNT], positions: [[0.0; 4]; PART_COUNT],
            velocities: [[0.0; 4]; PART_COUNT], velocity_changes: [[0.0; 4]; PART_COUNT],
            centre_of_mass: [0.0; 4], centre_of_mass_velocity: [0.0; 4], timestep: 0.0 }
    }
}
impl SkeletonPhysicalRecord {
    pub fn reset(&mut self, parts: &[AnimationPartTransform; PART_COUNT]) {
        self.pose = *parts;
        for (i, part) in parts.iter().enumerate() {
            self.positions[i] = part[3];
            self.velocities[i] = [0.0; 4];
        }
    }

    pub fn update(&mut self, parts: &[AnimationPartTransform; PART_COUNT],
        board_frame: AnimationPartTransform, fractional: &[f32; ANIMATION_PART_COUNT]) {
        self.timestep = f32::from_bits(0x3C88_8889);
        for (i, part) in parts.iter().enumerate() {
            let transform = if i == 0 { board_frame } else { *part };
            self.pose[i] = transform;
            for lane in 0..4 {
                let velocity = (transform[3][lane] - self.positions[i][lane])
                    * f32::from_bits(0x426F_FFFF);
                self.velocity_changes[i][lane] = velocity - self.velocities[i][lane];
                self.velocities[i][lane] = velocity;
            }
            self.positions[i] = transform[3];
        }
        let mut position = [0.0f32; 4];
        let mut velocity = [0.0f32; 4];
        for (i, weight) in fractional.iter().enumerate() {
            for lane in 0..4 {
                position[lane] = self.pose[i][3][lane].mul_add(*weight, position[lane]);
                velocity[lane] = self.velocities[i][lane].mul_add(*weight, velocity[lane]);
            }
        }
        self.centre_of_mass = position;
        self.centre_of_mass_velocity = velocity;
    }
}
