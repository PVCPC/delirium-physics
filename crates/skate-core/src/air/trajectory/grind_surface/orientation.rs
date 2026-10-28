use super::*;

pub(super) fn tilted_normal(direction: V, up: V, limits: [f32; 2]) -> V {
    let [lo, hi] = limits;
    let pi = f32::from_bits(0x4049_0fdb);
    let angle = if lo < hi {
        if lo < pi && hi > pi { return up; }
        if (pi - lo).abs() < (pi - hi).abs() { lo } else { hi }
    } else { (hi + lo) * 0.5 };
    rotate(direction, scale(up, -1.), angle)
}

pub(super) fn rotate(axis: V, value: V, angle: f32) -> V {
    let (sin, cos) = crate::trigonometry::sin_cos(angle * 0.5);
    let q = scale(axis, sin);
    madd(cross(q, madd(value, cos, cross(q, value))), 2., value)
}
