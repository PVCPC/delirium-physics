use super::math::{self, V};
use crate::trigonometry::sin_cos;

pub fn alignment_up(x_degrees: f32, y_degrees: f32) -> V {
    let radians = f32::from_bits(0x3C8E_FA35);
    let (sx, cx) = sin_cos((x_degrees * radians) * 0.5);
    let (sy, cy) = sin_cos((y_degrees * radians) * 0.5);
    let (sz, cz) = sin_cos(0.0);
    let cxcz = cx * cz;
    let sxcz = sx * cz;
    let sxsz = sx * sz;
    let cxsz = cx * sz;
    let qy = cy.mul_add(sxsz, sy * cxcz);
    let qw = cy.mul_add(cxcz, sy * sxsz);
    let qz = cy * cxsz - sy * sxcz;
    let qx = cy * sxcz - sy * cxsz;
    rotate([qx, qy, qz, qw], [0.0, 1.0, 0.0, 0.0])
}

pub fn rotate(quaternion: V, vector: V) -> V {
    let first = math::cross(quaternion, vector);
    let middle = math::madd(vector, quaternion[3], first);
    math::madd(math::cross(quaternion, middle), 2.0, vector)
}

pub fn signed_angle(left: V, right: V, axis: V) -> f32 {
    use crate::physics::board_motion_output::inverse_length_squared;
    let a = math::dot(left, left);
    let b = math::dot(right, right);
    if !(a > f32::from_bits(0x38D1_B717) && b > f32::from_bits(0x38D1_B717)) {
        return 0.0;
    }
    let left = math::scale(left, inverse_length_squared(a, 1));
    let right = math::scale(right, inverse_length_squared(b, 1));
    let angle = crate::trigonometry::acos(math::clamp(math::dot(left, right), -1.0, 1.0));
    if math::dot(math::cross(left, right), axis) < 0.0 {
        f32::from_bits(0x40C9_0FDB) - angle
    } else {
        angle
    }
}

pub fn projected_angle(left: V, right: V, axis: V) -> f32 {
    if !(math::dot(left, left) * math::dot(right, right) > f32::from_bits(0x3780_0000)) {
        return 0.0;
    }
    signed_angle(
        math::sub(left, math::scale(axis, math::dot(axis, left))),
        math::sub(right, math::scale(axis, math::dot(axis, right))),
        axis,
    )
}
