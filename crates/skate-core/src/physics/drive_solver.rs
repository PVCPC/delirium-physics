use super::{
    drive_frames::{RetailDriveFrame, RetailDriveFrames},
    drive_parameters::{RetailDriveDynamics, RetailDriveParams, RetailDriveType},
    rigid_body::{RetailPackedWorldInverseInertia, RetailQuaternion, RetailReactionCorrections},
};
use crate::math::Vector3;
mod builder;
pub use builder::build_drive_rows;

pub mod tu3 {
    pub const DRIVE_JACOBIAN_BUILD: u32 = 0x82AE_1AE8;
    pub const ITERATIVE_CONSTRAINT_SOLVER: u32 = 0x82AE_27D0;
}

pub const ACTIVE_BODY: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailDriveBodyState {
    pub reaction_index: usize,
    pub state: u32,
    pub orientation: RetailQuaternion,
    pub basis: crate::math::Basis3,
    pub center_of_mass: Vector3,
    pub linear_velocity: Vector3,
    pub angular_velocity: Vector3,
    pub force_acceleration: Vector3,
    pub torque_acceleration: Vector3,
    pub inverse_mass: f32,
    pub world_inverse_inertia: RetailPackedWorldInverseInertia,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailDriveRows {
    pub frame_a_body: RetailDriveBodyState,
    pub frame_b_body: RetailDriveBodyState,
    pub arm_a: Vector3,
    pub arm_b: Vector3,
    pub linear_axes: [Vector3; 3],
    pub angular_axes: [Vector3; 3],
    pub linear_inverse_effective_mass: [f32; 3],
    pub angular_inverse_effective_mass: [f32; 3],
    pub linear_softness: f32,
    pub angular_softness: f32,
    pub linear_target_impulse: [f32; 3],
    pub angular_target_impulse: [f32; 3],
    pub linear_maximum_impulse: [f32; 3],
    pub angular_maximum_impulse: [f32; 3],
    pub accumulated_linear_impulse: [f32; 3],
    pub accumulated_angular_impulse: [f32; 3],
}

pub fn solve_drive_rows(
    drives: &mut [RetailDriveRows],
    reactions: &mut [RetailReactionCorrections],
    maximum_iterations: u32,
) {
    super::solver::solve_constraints(&mut [], &mut [], drives, reactions, maximum_iterations);
}

pub fn solve_drive_iteration(
    drives: &mut [RetailDriveRows],
    reactions: &mut [RetailReactionCorrections],
) {
    solve_drive_rows(drives, reactions, 1);
}

fn is_active(body: RetailDriveBodyState) -> bool {
    body.state & ACTIVE_BODY == ACTIVE_BODY
}

fn basis_columns(basis: crate::math::Basis3) -> [Vector3; 3] {
    basis
        .columns
        .map(|column| Vector3::new(column[0], column[1], column[2]))
}

fn project(vector: Vector3, axes: [Vector3; 3]) -> [f32; 3] {
    axes.map(|axis| dot(vector, axis))
}

fn point_rate(linear: Vector3, angular: Vector3, arm: Vector3) -> Vector3 {
    super::constraint_frames::point_rate(linear, angular, arm)
}

const fn add(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

const fn sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

const fn scale(value: Vector3, factor: f32) -> Vector3 {
    Vector3::new(value.x * factor, value.y * factor, value.z * factor)
}

fn dot(a: Vector3, b: Vector3) -> f32 {
    super::native_arithmetic::dot3([a.x, a.y, a.z, 0.0], [b.x, b.y, b.z, 0.0])
}

fn cross(a: Vector3, b: Vector3) -> Vector3 {
    super::constraint_frames::cross(a, b)
}

