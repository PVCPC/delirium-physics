use crate::riding::pumping::{
    controller::{self, PumpingGeometry, PumpingSample},
    settings::{PumpingMode, PumpingSettings},
    state::PumpingState,
};

use super::{
    contact_state::{self, GroundContactServices},
    data::{GroundContactHistory, GroundUpdateInput, PhysicsGroundState},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundUpdateStage {
    SetElapsedSkeletonFlag,
    UpdateContactState,
    LoadPumpingFrame,
    UpdatePumping,
    UpdateSkateboard,
    PredictFutureDeck,
    SetCollisionSkeletonFlag,
    SetGroundSkeletonFlag,
    UpdateSpecialState,
    UpdateTrajectory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundUpdateError<E> {
    pub stage: GroundUpdateStage,
    pub source: E,
}

pub trait GroundUpdateServices:
    PumpingGeometry<Error = Self::UpdateError>
    + GroundContactServices<Error = Self::UpdateError, Handle = Self::ContactHandle>
{
    type UpdateError;
    type ContactHandle;

    fn set_elapsed_skeleton_flag(&mut self) -> Result<(), Self::UpdateError>;
    fn pumping_frame(
        &mut self,
    ) -> Result<(PumpingSettings, PumpingMode, PumpingSample), Self::UpdateError>;
    fn update_skateboard(
        &mut self,
        state: &mut PhysicsGroundState,
    ) -> Result<(), Self::UpdateError>;
    fn predict_future_deck(&mut self) -> Result<(), Self::UpdateError>;
    fn set_collision_skeleton_flag(&mut self) -> Result<(), Self::UpdateError>;
    fn set_ground_skeleton_flag(&mut self) -> Result<(), Self::UpdateError>;
    fn update_special_state(&mut self) -> Result<(), Self::UpdateError>;
    fn update_trajectory(&mut self) -> Result<(), Self::UpdateError>;
}

pub fn update<S: GroundUpdateServices>(
    state: &mut PhysicsGroundState,
    pumping: &mut PumpingState,
    contacts: &mut GroundContactHistory<S::ContactHandle>,
    input: GroundUpdateInput,
    services: &mut S,
) -> Result<(), GroundUpdateError<S::UpdateError>> {
    let signals = state.begin_update(input);
    if signals.set_skeleton_flag_16505 {
        call(
            GroundUpdateStage::SetElapsedSkeletonFlag,
            services.set_elapsed_skeleton_flag(),
        )?;
    }
    call(
        GroundUpdateStage::UpdateContactState,
        contact_state::update(contacts, input.contact_state, services),
    )?;
    let (settings, mode, sample) = call(
        GroundUpdateStage::LoadPumpingFrame,
        services.pumping_frame(),
    )?;
    call(
        GroundUpdateStage::UpdatePumping,
        controller::update_ground(pumping, &settings, mode, sample, services),
    )?;
    call(
        GroundUpdateStage::UpdateSkateboard,
        services.update_skateboard(state),
    )?;
    call(
        GroundUpdateStage::PredictFutureDeck,
        services.predict_future_deck(),
    )?;
    if state.flag_2722 {
        call(
            GroundUpdateStage::SetCollisionSkeletonFlag,
            services.set_collision_skeleton_flag(),
        )?;
    }
    call(
        GroundUpdateStage::SetGroundSkeletonFlag,
        services.set_ground_skeleton_flag(),
    )?;
    state.finish_update(input.timestep_2604);
    if input.contact_state.flags_2476 & 0x0040_0000 != 0 {
        call(
            GroundUpdateStage::UpdateSpecialState,
            services.update_special_state(),
        )
    } else {
        call(
            GroundUpdateStage::UpdateTrajectory,
            services.update_trajectory(),
        )
    }
}

fn call<T, E>(stage: GroundUpdateStage, result: Result<T, E>) -> Result<T, GroundUpdateError<E>> {
    result.map_err(|source| GroundUpdateError { stage, source })
}

