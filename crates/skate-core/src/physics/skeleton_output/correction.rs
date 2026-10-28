use crate::physics::{
    native_arithmetic::{dot3, vector_min},
    skeleton_body::SkeletonBody,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CorrectionState {
    pub board_prediction_error: [f32; 4],
    pub pending: bool,
}
impl CorrectionState {
    pub fn observe_board(&mut self, actual: [f32; 4], predicted: [f32; 4]) {
        self.board_prediction_error = core::array::from_fn(|i| actual[i] - predicted[i]);
    }
    pub fn apply(
        &mut self,
        body: &mut SkeletonBody,
        ground_normal: [f32; 4],
        wipeout: bool,
        flags_2468: u32,
        flags_2476: u32,
    ) {
        apply(
            body,
            &mut self.pending,
            self.board_prediction_error,
            ground_normal,
            wipeout,
            flags_2468,
            flags_2476,
        );
    }
}

pub fn apply(
    body: &mut SkeletonBody,
    pending: &mut bool,
    error: [f32; 4],
    ground_normal: [f32; 4],
    wipeout: bool,
    flags_2468: u32,
    flags_2476: u32,
) {
    if wipeout && flags_2476 & (1 << 30) != 0 {
        if 1.0 > dot3(error, error) {
            for part in 0..24 {
                let mut frame = body.record.pose[part];
                for lane in 0..4 {
                    frame[3][lane] += error[lane];
                }
                body.set_part_transform(part, frame);
            }
        }
        *pending = false;
    } else if !wipeout && flags_2468 & (1 << 18) == 0 && *pending {
        let distance = dot3(error, ground_normal);
        let allowed = vector_min(0.0, distance);
        let offset: [f32; 4] = core::array::from_fn(|i| {
            let tangent = error[i] - ground_normal[i] * distance;
            ground_normal[i].mul_add(allowed, tangent)
        });
        for part in 1..24 {
            for lane in 0..4 {
                body.record.pose[part][3][lane] += offset[lane];
            }
        }
        *pending = false;
    }
}
