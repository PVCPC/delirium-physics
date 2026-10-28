use super::*;
#[derive(Clone, Copy, Debug)]
pub struct PoseInput {
    pub animation_to_world: Frame,
    pub world_to_animation: Frame,
    pub physical_forward: V,
    pub animation_board_position: V,
}
pub(super) fn axis_rotation(axis: V, angle: f32) -> Frame {
    let (sin, cos) = crate::trigonometry::sin_cos(angle);
    let [x, y, z, _] = axis;
    let k = 1. - cos;
    [
        [
            (k * x).mul_add(x, cos),
            (k * x).mul_add(y, sin * z),
            (k * x).mul_add(z, -sin * y),
            0.,
        ],
        [
            (k * y).mul_add(x, -sin * z),
            (k * y).mul_add(y, cos),
            (k * y).mul_add(z, sin * x),
            0.,
        ],
        [
            (k * z).mul_add(x, sin * y),
            (k * z).mul_add(y, -sin * x),
            (k * z).mul_add(z, cos),
            0.,
        ],
        ZERO,
    ]
}
pub(super) fn direction(frame: Frame, vector: V) -> V {
    madd(
        frame[2],
        vector[2],
        madd(frame[1], vector[1], scale(frame[0], vector[0])),
    )
}
impl Adjustment {
    pub fn local_transform(self, input: PoseInput) -> Frame {
        let across = cross(UP, self.axis);
        let across = scale(across, inverse_length(dot(across, across)));
        let up = cross(self.axis, across);
        let up = scale(up, inverse_length(dot(up, up)));
        let yaw = axis_rotation(up, self.angles[1]);
        let lean = axis_rotation(self.axis, self.angles[0]);
        let pitch = axis_rotation(input.physical_forward, self.angles[2]);
        let mut local = super::super::skeleton_animation_record::IDENTITY;
        for i in 0..3 {
            local[i] = direction(
                input.world_to_animation,
                direction(
                    yaw,
                    direction(lean, direction(pitch, input.animation_to_world[i])),
                ),
            );
        }
        let pivot = input.animation_board_position;
        local[3] = sub(
            add(direction(input.world_to_animation, self.offset), pivot),
            direction(local, pivot),
        );
        local
    }
}
