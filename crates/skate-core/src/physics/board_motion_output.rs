use crate::math::{Basis3, Vector3};
use super::{board::BodyId, board_runtime::BoardRuntime, native_arithmetic};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardMotionOutput {
    pub angular_velocity: Vector3,
    pub linear_velocity: Vector3,
    pub ground_velocity: Vector3,
    pub speed: f32,
    pub ground_speed: f32,
    pub forward_speed: f32,
    pub effective_basis: Basis3,
}

impl BoardMotionOutput {
    pub fn from_board(
        board: &BoardRuntime,
        reckoning_ground_normal: Vector3,
        processed_flags_2468: u32,
    ) -> Self {
        let deck = board.bodies()[BodyId::Deck.index()].rates;
        let mut effective_basis = board.part_transforms()[BodyId::Deck.index()].basis;
        if processed_flags_2468 & 0x0010_0000 != 0 {
            for axis in [0, 2] {
                effective_basis.columns[axis] = effective_basis.columns[axis].map(|v| -v);
            }
        }
        let normal_speed = dot(deck.linear_velocity, reckoning_ground_normal);
        let ground_velocity = subtract(
            deck.linear_velocity,
            scale(reckoning_ground_normal, normal_speed),
        );
        let z = effective_basis.columns[2];
        Self {
            angular_velocity: deck.angular_velocity,
            linear_velocity: deck.linear_velocity,
            ground_velocity,
            speed: length(deck.linear_velocity),
            ground_speed: length(ground_velocity),
            forward_speed: dot(deck.linear_velocity, Vector3::new(z[0], z[1], z[2])),
            effective_basis,
        }
    }
}

pub fn dot(a: Vector3, b: Vector3) -> f32 {
    native_arithmetic::dot3([a.x, a.y, a.z, 0.0], [b.x, b.y, b.z, 0.0])
}
pub(crate) fn scale(v: Vector3, s: f32) -> Vector3 {
    Vector3::new(v.x * s, v.y * s, v.z * s)
}
pub(crate) fn add(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}
pub(crate) fn subtract(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}
pub(crate) fn inverse_length_squared(squared: f32, refinements: usize) -> f32 {
    let mut inverse = native_arithmetic::reciprocal_square_root_estimate(squared);
    for _ in 0..refinements {
        let correction = (-squared).mul_add(inverse * inverse, 1.0);
        inverse = (inverse * 0.5).mul_add(correction, inverse);
    }
    inverse
}
pub fn length(v: Vector3) -> f32 {
    let squared = dot(v, v);
    let value = squared * inverse_length_squared(squared, 2);
    if squared == 0.0 { 0.0 } else { value }
}
