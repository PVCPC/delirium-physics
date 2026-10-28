mod geometry;
mod input;
mod publication;

use crate::math::Vector3;

#[derive(Clone, Debug, PartialEq)]
pub struct ContactPreparation {
    pub arms: [Vector3; 2],
    pub axes: [Vector3; 3],
    pub active: [bool; 2],
    pub inverse_mass: [f32; 2],
    pub point_acceleration: [Vector3; 2],
    pub angular_response_a: [[f32; 4]; 3],
    pub angular_response_b: [[f32; 4]; 3],
    pub effective_mass: [f32; 3],
    pub separation_projection: [f32; 3],
    pub restitution_projection: [f32; 3],
    pub predicted_separation_projection: [f32; 3],
    reaction_ids: [u32; 2],
    body_ids: [u32; 2],
    static_friction_bits: u32,
    dynamic_friction_bits: u32,
    contact_tag: u32,
    combined_state_bit_8: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactBuildError {
    ReciprocalEstimateUnavailable { effective_mass: [f32; 3] },
}

pub trait ContactMassResponse {
    fn inverse_effective_mass(
        &mut self,
        effective_mass: [f32; 3],
    ) -> Result<[f32; 3], ContactBuildError>;
}

pub struct UnavailableContactMassResponse;

impl ContactMassResponse for UnavailableContactMassResponse {
    fn inverse_effective_mass(
        &mut self,
        effective_mass: [f32; 3],
    ) -> Result<[f32; 3], ContactBuildError> {
        Err(ContactBuildError::ReciprocalEstimateUnavailable { effective_mass })
    }
}

pub struct NativeContactMassResponse;

impl ContactMassResponse for NativeContactMassResponse {
    fn inverse_effective_mass(
        &mut self,
        effective_mass: [f32; 3],
    ) -> Result<[f32; 3], ContactBuildError> {
        Ok(effective_mass.map(crate::physics::native_arithmetic::reciprocal_estimate))
    }
}

pub fn prepare_contact(record: &[u32; 64], time_step: f32) -> ContactPreparation {
    geometry::prepare(input::ContactInput::decode(record), time_step)
}

pub fn build_with_response(
    record: &mut [u32; 64],
    time_step: f32,
    response: &mut impl ContactMassResponse,
) -> Result<(), ContactBuildError> {
    let prepared = prepare_contact(record, time_step);
    let inverse_response = response.inverse_effective_mass(prepared.effective_mass)?;
    *record = publication::compile(&prepared, inverse_response);
    Ok(())
}

#[track_caller]
pub(super) fn build(record: &mut [u32; 64], time_step: f32) {
    build_with_response(record, time_step, &mut NativeContactMassResponse)
        .expect("the native contact mass response is infallible");
}
