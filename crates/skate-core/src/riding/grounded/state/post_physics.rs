#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundPostPhysicsStage {
    CheckForGroundWipeout,
    UpdateBoardPath,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundPostPhysicsError<E> {
    pub stage: GroundPostPhysicsStage,
    pub source: E,
}

pub trait GroundPostPhysicsServices {
    type Error;

    fn check_for_ground_wipeout_n52(&mut self) -> Result<(), Self::Error>;
    fn update_board_path_n53(&mut self) -> Result<(), Self::Error>;
}

pub fn update_post_physics<S: GroundPostPhysicsServices>(
    offboard_trajectory_state_1776: i32,
    services: &mut S,
) -> Result<(), GroundPostPhysicsError<S::Error>> {
    services
        .check_for_ground_wipeout_n52()
        .map_err(|source| GroundPostPhysicsError {
            stage: GroundPostPhysicsStage::CheckForGroundWipeout,
            source,
        })?;
    if offboard_trajectory_state_1776 < 0 {
        services
            .update_board_path_n53()
            .map_err(|source| GroundPostPhysicsError {
                stage: GroundPostPhysicsStage::UpdateBoardPath,
                source,
            })?;
    }
    Ok(())
}
