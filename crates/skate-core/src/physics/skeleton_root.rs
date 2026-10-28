use super::{
    board_ground::angle_between,
    board_motion_output::inverse_length_squared,
    native_arithmetic::dot3,
    skeleton_animation_record::{AnimationPartTransform, IDENTITY, compose_affine},
};
use crate::{math::Vector3, trigonometry};

#[derive(Clone, Debug)]
pub struct SkeletonRootFrames {
    pub board: AnimationPartTransform,
    pub inverse_board: AnimationPartTransform,
    pub previous_board_position: [f32; 4],
    pub predicted_board_position: [f32; 4],
    pub supplied_prediction: Option<[f32; 4]>,
    pub animation_to_board: AnimationPartTransform,
    pub animation_to_world: AnimationPartTransform,
    pub world_to_animation: AnimationPartTransform,
    pub heading_alignment: AnimationPartTransform,
    pub initialize_heading: bool,
}
impl Default for SkeletonRootFrames {
    fn default() -> Self {
        Self {
            board: IDENTITY,
            inverse_board: IDENTITY,
            previous_board_position: [0.0; 4],
            predicted_board_position: [0.0; 4],
            supplied_prediction: None,
            animation_to_board: IDENTITY,
            animation_to_world: IDENTITY,
            world_to_animation: IDENTITY,
            heading_alignment: IDENTITY,
            initialize_heading: true,
        }
    }
}
impl SkeletonRootFrames {
    pub fn update_teleport(
        &mut self,
        board: AnimationPartTransform,
        animation_board: &AnimationPartTransform,
        reckoning_frame_816: &AnimationPartTransform,
    ) {
        self.initialize_heading = true;
        self.supplied_prediction = Some(board[3]);
        self.update(board, [0.0; 4], 0.0, animation_board, reckoning_frame_816);
        self.previous_board_position = board[3];
    }

    pub fn reset_initial_alignment(&mut self, animation_to_world: AnimationPartTransform) {
        self.animation_to_world = animation_to_world;
        self.world_to_animation = inverse_rigid(&animation_to_world);
    }

    pub fn update(
        &mut self,
        board: AnimationPartTransform,
        board_velocity: [f32; 4],
        time_step: f32,
        animation_board: &AnimationPartTransform,
        reckoning: &AnimationPartTransform,
    ) {
        self.previous_board_position = self.board[3];
        self.board = board;
        self.inverse_board = inverse_rigid(&board);
        self.predicted_board_position = self.supplied_prediction.take().unwrap_or_else(|| {
            std::array::from_fn(|i| board_velocity[i].mul_add(time_step, board[3][i]))
        });
        if self.initialize_heading {
            let at = Vector3::new(animation_board[2][0], 0.0, animation_board[2][2]);
            let forward = Vector3::new(0.0, 0.0, 1.0);
            let mut angle = angle_between(at, forward);
            let at4 = [at.x, at.y, at.z, 0.0];
            let sq = dot3(at4, at4);
            if sq > f32::from_bits(0x38D1_B717) {
                let inverse = inverse_length_squared(sq, 1);
                let x = at.x * inverse;
                if 0.0 > -x {
                    angle = f32::from_bits(0x40C9_0FDB) - angle;
                }
            }
            let (sin, cos) = trigonometry::sin_cos(angle);
            self.heading_alignment = [
                [cos, 0.0, -sin, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [sin, 0.0, cos, 0.0],
                [0.0; 4],
            ];
            self.initialize_heading = false;
        }
        let mut remove_board_translation = IDENTITY;
        remove_board_translation[3] = animation_board[3].map(|v| -v);
        let local = compose_affine(&self.heading_alignment, &remove_board_translation);
        self.animation_to_board = compose_affine(reckoning, &local);
        let mut world_translation = IDENTITY;
        world_translation[3] = self.predicted_board_position;
        self.animation_to_world =
            orthonormalize(compose_affine(&world_translation, &self.animation_to_board));
        self.world_to_animation = inverse_rigid(&self.animation_to_world);
    }
}

pub(crate) fn orthonormalize(source: AnimationPartTransform) -> AnimationPartTransform {
    let mut result = source;
    for axis in (0..3).rev() {
        let mut vector = source[axis];
        for prior in ((axis + 1)..3).rev() {
            let projection = dot3(result[prior], source[axis]);
            for (lane, value) in vector.iter_mut().enumerate() {
                *value -= result[prior][lane] * projection;
            }
        }
        let reciprocal = inverse_length_squared(dot3(vector, vector), 2);
        result[axis] = vector.map(|v| v * reciprocal);
    }
    result
}

pub(crate) fn inverse_rigid(source: &AnimationPartTransform) -> AnimationPartTransform {
    let mut result = IDENTITY;
    for axis in 0..3 {
        result[axis] = [source[0][axis], source[1][axis], source[2][axis], 0.0];
    }
    let translation = source[3].map(|v| 0.0 - v);
    result[3] = std::array::from_fn(|i| {
        let z = translation[2] * result[2][i];
        let y = translation[1].mul_add(result[1][i], z);
        translation[0].mul_add(result[0][i], y)
    });
    result
}

