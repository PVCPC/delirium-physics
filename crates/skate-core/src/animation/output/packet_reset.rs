use super::{NativeMatrix, physics_packet::PhysicsPosePacket};
use crate::animation::commands::buffers::BufferError;

pub struct AdditionalResetFields {
    pub compression: f32,
    pub foot_ik_influence: [f32; 2],
    pub next_step_position_valid: bool,
    pub actor_flag_1904_bit23: bool,
    pub actor_flag_1908_bit2: bool,
    pub external_impulse_active: bool,
    pub external_physics_input_active: bool,
    pub externally_controlled: bool,
    pub prevent_manual_respawn: bool,
    pub ignore_respawn_reset_button: u8,
    pub force_braking: bool,
    pub truck_tightness: f32,
    pub wheel_hardness: f32,
    pub auxiliary_vectors: [[f32; 4]; 6],
    pub requested_physics_mode: u32,
}

pub const RESET_POSE: NativeMatrix = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0; 4],
];

pub fn reset(
    packet: &mut PhysicsPosePacket,
    fields: &mut AdditionalResetFields,
) -> Result<(), BufferError> {
    let count = (packet.bone_count as i32).max(0) as usize;
    if packet.hierarchy.len() < count || packet.local.len() < count {
        return Err(BufferError::RangeOutsideAllocation);
    }
    packet.timestep = f32::from_bits(0x3c88_8889);
    fields.external_impulse_active = false;
    fields.compression = 0.5;
    fields.foot_ik_influence[0] = 0.0;
    fields.external_physics_input_active = false;
    fields.foot_ik_influence[1] = 0.0;
    fields.externally_controlled = false;
    fields.truck_tightness = 0.0;
    packet.mirrored = false;
    packet.riding_switch = false;
    packet.riding_fakie = false;
    packet.weight_forwards = false;
    packet.regular_stance = false;
    fields.prevent_manual_respawn = false;
    fields.ignore_respawn_reset_button = 0;
    fields.force_braking = false;
    packet.air_dismount_revert_frames = 0;
    packet.board_flipped = false;
    fields.next_step_position_valid = false;
    fields.actor_flag_1904_bit23 = false;
    packet.flags &= 0x013f_ffff;
    fields.actor_flag_1908_bit2 = false;
    packet.foot_surface_ids[0] = 0;
    fields.wheel_hardness = 0.0;
    packet.foot_surface_ids[1] = 0;
    fields.auxiliary_vectors[0] = [0.0; 4];
    fields.requested_physics_mode = 1;
    for vector in &mut fields.auxiliary_vectors[1..] {
        *vector = [0.0; 4];
    }
    for bone in 0..count {
        packet.local[bone] = RESET_POSE;
        packet.hierarchy[bone] = RESET_POSE;
    }
    Ok(())
}
