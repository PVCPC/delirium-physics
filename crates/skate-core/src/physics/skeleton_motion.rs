use super::{
    native_arithmetic::reciprocal_estimate,
    skeleton_animation_record::{AnimationPartTransform, IDENTITY, compose_affine},
};

pub struct SkeletonMotion {
    pub trajectory: AnimationPartTransform,
    pub inverse_trajectory: AnimationPartTransform,
    pub next_trajectory: AnimationPartTransform,
    pub velocity_world: [f32; 4],
    previous_board_at_y: f32,
}

impl Default for SkeletonMotion {
    fn default() -> Self {
        Self {
            trajectory: IDENTITY,
            inverse_trajectory: IDENTITY,
            next_trajectory: IDENTITY,
            velocity_world: [0.; 4],
            previous_board_at_y: 0.,
        }
    }
}

impl SkeletonMotion {
    pub fn reset_board_orientation_history(&mut self) {
        self.previous_board_at_y = 0.0;
    }

    pub fn process_trajectory(
        &mut self,
        trajectory_bone: &AnimationPartTransform,
        animation_to_world: &AnimationPartTransform,
        dt: f32,
    ) {
        self.trajectory = compose_affine(trajectory_bone, &self.next_trajectory);
        self.next_trajectory = self.trajectory;
        let mut reciprocal = reciprocal_estimate(dt);
        for _ in 0..2 {
            let error = (-reciprocal).mul_add(dt, 1.0);
            reciprocal = reciprocal.mul_add(error, reciprocal);
        }
        let velocity = self.trajectory[3].map(|v| v * reciprocal);
        self.velocity_world = std::array::from_fn(|lane| {
            let x = animation_to_world[0][lane] * velocity[0];
            let y = animation_to_world[1][lane].mul_add(velocity[1], x);
            animation_to_world[2][lane].mul_add(velocity[2], y)
        });
        let source = self.trajectory;
        self.inverse_trajectory = std::array::from_fn(|axis| {
            if axis < 3 {
                [source[0][axis], source[1][axis], source[2][axis], 0.]
            } else {
                [0.; 4]
            }
        });
        self.inverse_trajectory[3] = std::array::from_fn(|lane| {
            let z = (0.0 - source[3][2]) * self.inverse_trajectory[2][lane];
            let y = (0.0 - source[3][1]).mul_add(self.inverse_trajectory[1][lane], z);
            (0.0 - source[3][0]).mul_add(self.inverse_trajectory[0][lane], y)
        });
    }

    pub fn publish_unadjusted_board(board: &AnimationPartTransform, flags_2472: &mut u32) {
        if board[0][1].abs() > f32::from_bits(0x3f35_c28f) {
            *flags_2472 |= 0x8000;
        }
    }

    pub fn publish_adjusted_board(
        &mut self,
        board: &AnimationPartTransform,
        flags_2468: &mut u32,
    ) -> f32 {
        let at_y = board[2][1];
        let delta = at_y - self.previous_board_at_y;
        self.previous_board_at_y = at_y;
        let positive = at_y > 0.0;
        *flags_2468 =
            (*flags_2468 & !0x1800) | (u32::from(positive) << 12) | (u32::from(!positive) << 11);
        delta
    }
}
