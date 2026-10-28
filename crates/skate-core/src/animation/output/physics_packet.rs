use super::NativeMatrix;
use crate::animation::commands::{
    batch::CompletedSetData,
    buffers::{BoneSlice, BufferError},
};

pub struct AnimationSignal {
    pub name_hash: u32,
    pub active: u8,
}

pub struct SkaterPublicationState {
    pub orientation_bit31: bool,
    pub mirrored: bool,
    pub riding_fakie: bool,
    pub weight_on_nose: bool,
    pub relative_stance: i32,
    pub natural_stance: i32,
    pub request_bit16: bool,
    pub request_bit15: bool,
    pub air_dismount_revert_requested: bool,
    pub air_dismount_revert_frames: i32,
    pub signal: Option<AnimationSignal>,
}

pub struct ActorPoseBuffers {
    pub hierarchy: BoneSlice<NativeMatrix>,
    pub local: BoneSlice<NativeMatrix>,
}

pub struct PhysicsPosePacket {
    pub bone_count: u32,
    pub hierarchy: Vec<NativeMatrix>,
    pub local: Vec<NativeMatrix>,
    pub timestep: f32,
    pub foot_surface_ids: [u32; 2],
    pub flags: u32,
    pub board_flipped: bool,
    pub mirrored: bool,
    pub riding_switch: bool,
    pub riding_fakie: bool,
    pub weight_forwards: bool,
    pub regular_stance: bool,
    pub air_dismount_revert_frames: i32,
}

pub fn publish(
    state: &mut SkaterPublicationState,
    completed: &CompletedSetData,
    poses: &ActorPoseBuffers,
    packet: &mut PhysicsPosePacket,
    timestep: f64,
    signal_name: &[u8],
) -> Result<i32, BufferError> {
    let count = packet.bone_count as usize;
    if packet.hierarchy.len() < count || packet.local.len() < count {
        return Err(BufferError::RangeOutsideAllocation);
    }
    let hierarchy = completed.buffers().matrices.read(poses.hierarchy, count)?;
    let local = completed.buffers().matrices.read(poses.local, count)?;

    publish_evaluated(state, &hierarchy, &local, packet, timestep, signal_name)
}

pub fn publish_evaluated(
    state: &mut SkaterPublicationState,
    hierarchy: &[NativeMatrix],
    local: &[NativeMatrix],
    packet: &mut PhysicsPosePacket,
    timestep: f64,
    signal_name: &[u8],
) -> Result<i32, BufferError> {
    let count=packet.bone_count as usize;
    if hierarchy.len()<count || local.len()<count || packet.hierarchy.len()<count || packet.local.len()<count {
        return Err(BufferError::RangeOutsideAllocation);
    }

    packet.timestep = timestep as f32;
    packet.foot_surface_ids = [0; 2];
    replace_flag(&mut packet.flags, 27, state.request_bit16);
    state.request_bit16 = false;
    replace_flag(&mut packet.flags, 23, state.request_bit15);
    state.request_bit15 = false;
    packet.flags &= !((1 << 25) | (1 << 26));
    if let Some(signal) = &mut state.signal {
        signal.name_hash = signal_hash(signal_name);
        signal.active = 0;
    }
    packet.hierarchy[..count].copy_from_slice(&hierarchy[..count]);
    packet.local[..count].copy_from_slice(&local[..count]);
    packet.board_flipped = state.orientation_bit31 ^ state.mirrored;
    packet.mirrored = state.mirrored;
    packet.riding_switch = state.relative_stance == 1;
    packet.riding_fakie = state.riding_fakie;
    packet.weight_forwards = if state.riding_fakie {
        !state.weight_on_nose
    } else {
        state.weight_on_nose
    };
    packet.regular_stance = state.natural_stance == 0;
    replace_flag(&mut packet.flags, 28, state.air_dismount_revert_requested);
    state.air_dismount_revert_requested = false;
    packet.air_dismount_revert_frames = state.air_dismount_revert_frames;
    Ok(state.air_dismount_revert_frames)
}

fn replace_flag(flags: &mut u32, bit: u32, value: bool) {
    *flags = (*flags & !(1 << bit)) | (u32::from(value) << bit);
}

pub fn signal_hash(name: &[u8]) -> u32 {
    let mut hash = 0_u32;
    for &byte in name.iter().take_while(|&&byte| byte != 0) {
        hash = hash
            .wrapping_shl(4)
            .wrapping_add((byte as i8 as i32) as u32);
        let high = hash & 0xf000_0000;
        if high != 0 {
            hash ^= (high >> 23) ^ high;
        }
    }
    hash
}
