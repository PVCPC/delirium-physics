use super::{
    contact_queries::{Input, V},
    contact_segments::{length_inverse, reciprocal},
};
use crate::physics::native_arithmetic::dot3;
pub fn slope_limit(input: Input, height: f32, rising: bool) -> f32 {
    let large = height > f32::from_bits(0x3ecc_cccd);
    let end = if large {
        if rising { 35. } else { 30. }
    } else {
        15.
    };
    let start = if large { 50. } else { 30. };
    let speed = length_inverse(input.velocity).0;
    let lower = if 2. - speed >= 0. { 2. } else { speed };
    let clamped = if 6. - lower >= 0. { lower } else { 6. };
    let degrees = ((clamped - 2.) * (end - start)).mul_add(0.25, start);
    crate::animation::foot_ik::post_contact::tangent(degrees * f32::from_bits(0x3c8e_fa35))
}
pub fn slope_between(input: Input, start: V, end: V) -> f32 {
    let delta = std::array::from_fn(|i| end[i] - start[i]);
    let distance = dot3(input.surface_forward, delta);
    if f32::from_bits(0x3a83_126f) > distance {
        1000.
    } else {
        dot3(input.surface_up, delta) * reciprocal(distance)
    }
}
pub fn intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let cd = [d[0] - c[0], d[1] - c[1]];
    let determinant = cd[1] * ab[0] - cd[0] * ab[1];
    if !(determinant.abs() >= f32::from_bits(0x3727_c5ac)) {
        return f32::from_bits(0x5015_02f9);
    }
    let ac = [a[0] - c[0], a[1] - c[1]];
    (1. / determinant) * (cd[0] * ac[1] - cd[1] * ac[0])
}
