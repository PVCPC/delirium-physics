use crate::math::{Basis3, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailForceAccumulator {
    pub force_acceleration: Vector3,
    pub torque_acceleration: Vector3,
    pub cool_down: u32,
}

pub fn accumulate_point_force(
    mut accumulator: RetailForceAccumulator,
    force_world: Vector3,
    application_point_body: Vector3,
    deck_basis: Basis3,
    inverse_mass: f32,
    world_inverse_inertia: Basis3,
) -> RetailForceAccumulator {
    accumulator.force_acceleration.x += force_world.x * inverse_mass;
    accumulator.force_acceleration.y += force_world.y * inverse_mass;
    accumulator.force_acceleration.z += force_world.z * inverse_mass;
    let arm = multiply_basis(deck_basis, application_point_body);
    let torque = Vector3::new(
        (-arm.z).mul_add(force_world.y, arm.y * force_world.z),
        (-arm.x).mul_add(force_world.z, arm.z * force_world.x),
        (-arm.y).mul_add(force_world.x, arm.x * force_world.y),
    );
    let angular = multiply_basis(world_inverse_inertia, torque);
    accumulator.torque_acceleration.x += angular.x;
    accumulator.torque_acceleration.y += angular.y;
    accumulator.torque_acceleration.z += angular.z;
    accumulator.cool_down = 0;
    accumulator
}

fn multiply_basis(basis: Basis3, v: Vector3) -> Vector3 {
    let lane = |i: usize| {
        basis.columns[2][i].mul_add(
            v.z,
            basis.columns[1][i].mul_add(v.y, basis.columns[0][i] * v.x),
        )
    };
    Vector3::new(lane(0), lane(1), lane(2))
}
