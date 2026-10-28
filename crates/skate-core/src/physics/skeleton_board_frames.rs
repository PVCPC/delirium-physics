use super::{
    native_arithmetic::reciprocal_estimate,
    skeleton_animation_record::{
        AnimationPartTransform as Transform, IDENTITY, compose_affine, transform_point,
    },
    skeleton_root::{SkeletonRootFrames, orthonormalize},
};

pub struct SkeletonBoardFrames {
    pub physical_board: Transform,
    pub skate_root: Transform,
    pub animation_target: Transform,
    pub com_frame: Transform,
    pub lifted_com_frame: Transform,
    pub centre_of_mass: [f32; 4],
    pub previous_centre_of_mass: [f32; 4],
    pub com_velocity: [f32; 4],
    pub local_centre_of_mass: [f32; 4],
    pub local_board_position: [f32; 4],
    pub lift_height: f32,
}
impl Default for SkeletonBoardFrames {
    fn default() -> Self {
        Self {
            physical_board: IDENTITY,
            skate_root: IDENTITY,
            animation_target: IDENTITY,
            com_frame: IDENTITY,
            lifted_com_frame: IDENTITY,
            centre_of_mass: [0.; 4],
            previous_centre_of_mass: [0.; 4],
            com_velocity: [0.; 4],
            local_centre_of_mass: [0.; 4],
            local_board_position: [0.; 4],
            lift_height: 0.,
        }
    }
}
impl SkeletonBoardFrames {
    pub fn reset(&mut self, spawn: Transform) {
        self.com_frame = spawn;
        self.com_frame[3][1] += 1.0;
        self.lifted_com_frame = self.com_frame;
        self.centre_of_mass = spawn[3];
        self.local_centre_of_mass = [0.; 4];
    }
    pub fn publish_local_observations(
        &mut self,
        roots: &SkeletonRootFrames,
        physical_board: &Transform,
    ) {
        self.local_centre_of_mass = transform_point(&roots.world_to_animation, self.centre_of_mass);
        self.local_board_position = transform_point(&roots.world_to_animation, physical_board[3]);
    }

    pub fn publish_centre_of_mass(&mut self, com: [f32; 4], dt: f32, flags_2472: u32) {
        self.previous_centre_of_mass = self.centre_of_mass;
        self.centre_of_mass = com;
        let mut inverse = reciprocal_estimate(dt);
        for _ in 0..2 {
            inverse = inverse.mul_add((-inverse).mul_add(dt, 1.0), inverse);
        }
        self.com_velocity =
            std::array::from_fn(|i| (com[i] - self.previous_centre_of_mass[i]) * inverse);
        if flags_2472 & 0x400 != 0 {
            self.previous_centre_of_mass = com;
        }
    }

    pub fn prepare_ground(
        &mut self,
        roots: &SkeletonRootFrames,
        mapped_board: &Transform,
        actual_board: Transform,
        flags_2468: &mut u32,
    ) -> Transform {
        let local_target = compose_affine(&roots.animation_to_board, mapped_board);
        let mut world_position = IDENTITY;
        world_position[3] = roots.predicted_board_position;
        self.animation_target = compose_affine(&world_position, &local_target);
        *flags_2468 &= !0x80000;
        self.physical_board = actual_board;
        self.skate_root = actual_board;
        self.update_com_lift(&roots.animation_to_world, self.centre_of_mass, 0.0);
        self.animation_target
    }

    pub fn prepare_teleport(
        &mut self,
        roots: &SkeletonRootFrames,
        mapped_board: &Transform,
        actual_board: Transform,
        flags_2468: &mut u32,
    ) -> Transform {
        let local_target = compose_affine(&roots.animation_to_board, mapped_board);
        let mut world_position = IDENTITY;
        world_position[3] = roots.predicted_board_position;
        self.animation_target = compose_affine(&world_position, &local_target);
        *flags_2468 &= !0x80000;
        self.physical_board = actual_board;
        self.skate_root = actual_board;
        let mut position = actual_board[3];
        position[1] += 1.0;
        self.update_com_lift(&roots.animation_to_world, position, 0.0);
        self.animation_target
    }

    pub fn update_com_lift(
        &mut self,
        animation_to_world: &Transform,
        position: [f32; 4],
        requested_height: f32,
    ) {
        let step = f32::from_bits(0x3D23_D70A);
        let low = self.lift_height - step;
        let high = self.lift_height + step;
        let above_low = if low - requested_height >= 0.0 {
            low
        } else {
            requested_height
        };
        self.lift_height = if high - above_low >= 0.0 {
            above_low
        } else {
            high
        };
        let mut frame = *animation_to_world;
        frame[3] = position;
        self.com_frame = orthonormalize(frame);
        self.lifted_com_frame = self.com_frame;
        let height = self.lift_height + f32::from_bits(0x3E99_999A);
        self.lifted_com_frame[3] =
            std::array::from_fn(|i| self.com_frame[1][i].mul_add(height, self.com_frame[3][i]));
    }
}
