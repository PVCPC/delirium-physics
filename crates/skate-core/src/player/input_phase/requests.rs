use super::types::RawVector;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundHistoryRequest {
    pub previous_position: RawVector,
    pub current_position: RawVector,
    pub previous_filtered_delta: RawVector,
    pub timestep_bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundHistoryResult {
    pub delta: RawVector,
    pub filtered_delta: RawVector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrepareJumpRequest {
    pub skateboard_vector: RawVector,
    pub previous_velocity: RawVector,
}
