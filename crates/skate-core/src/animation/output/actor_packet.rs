use super::{packet_reset::AdditionalResetFields, physics_packet::PhysicsPosePacket};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalPhysicsInput {
    pub vectors: [[u32; 4]; 10],
    pub flags: u32,
}
impl ExternalPhysicsInput {
    pub fn copy_from(&mut self, source: &Self) {
        self.vectors = source.vectors;
        self.flags = (self.flags & 0x01ff_ffff) | (source.flags & 0xfe00_0000);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExternalReset {
    pub transform: [[u32; 4]; 4],
    pub byte64: u8,
}

pub struct ActorPacketFields {
    pub external_impulse: [u32; 4],
    pub external_physics: ExternalPhysicsInput,
    pub external_reset: ExternalReset,
    pub actor_flag_1904_bit29: bool,
}

pub struct ActorPublicationState {
    pub flags1904: u32,
    pub flags1908: u32,
    pub external_controller_present: bool,
    pub external_impulse: [u32; 4],
    pub external_physics: ExternalPhysicsInput,
    pub external_reset: ExternalReset,
    pub player_index: u32,
}

pub trait PlayerPhysicsSettingsSource {
    type Error;
    fn truck_tightness(&mut self, player_index: u32) -> Result<f32, Self::Error>;
    fn wheel_hardness(&mut self, player_index: u32) -> Result<f32, Self::Error>;
    fn requested_physics_mode(&mut self, player_index: u32) -> Result<u32, Self::Error>;
}

pub trait ActorPublicationEnvironment {
    type Error;
    type Settings: PlayerPhysicsSettingsSource<Error = Self::Error>;
    fn current_scene_mode(&mut self) -> Result<Option<i32>, Self::Error>;
    fn ignore_respawn_reset_button(&mut self) -> Result<u8, Self::Error>;
    fn actor_subobject56_slot16(
        &mut self,
        actor: &mut ActorPublicationState,
    ) -> Result<u8, Self::Error>;
    fn capture_physics_settings(&mut self) -> Result<Self::Settings, Self::Error>;
}

pub fn publish<E: ActorPublicationEnvironment>(
    actor: &mut ActorPublicationState,
    pose: &mut PhysicsPosePacket,
    reset: &mut AdditionalResetFields,
    fields: &mut ActorPacketFields,
    environment: &mut E,
) -> Result<(), E::Error> {
    reset.actor_flag_1904_bit23 = actor.flags1904 & (1 << 23) != 0;
    reset.actor_flag_1908_bit2 = actor.flags1908 & (1 << 2) != 0;
    reset.external_impulse_active = actor.flags1904 & (1 << 30) != 0;
    fields.external_impulse = actor.external_impulse;
    fields.external_physics.copy_from(&actor.external_physics);
    reset.external_physics_input_active = actor.flags1904 & (1 << 27) != 0;
    let scene_mode_flag = !actor.external_controller_present
        && actor.flags1904 & (1 << 26) == 0
        && matches!(environment.current_scene_mode()?, Some(19 | 20));
    replace_flag(&mut pose.flags, 22, scene_mode_flag);
    reset.externally_controlled = actor.external_controller_present;
    fields.external_reset = actor.external_reset;
    fields.actor_flag_1904_bit29 = actor.flags1904 & (1 << 29) != 0;
    reset.prevent_manual_respawn = actor.flags1904 & (1 << 28) != 0;
    reset.force_braking = actor.flags1904 & (1 << 26) != 0;
    replace_flag(&mut pose.flags, 24, actor.flags1904 & (1 << 25) != 0);
    actor.flags1904 &= !(1 << 25);
    reset.ignore_respawn_reset_button = environment.ignore_respawn_reset_button()?;
    actor.flags1904 &= !(1 << 28);
    let mut settings = environment.capture_physics_settings()?;
    let use_fixed_settings = environment.actor_subobject56_slot16(actor)? != 0;
    if use_fixed_settings {
        reset.truck_tightness = f32::from_bits(0x3f33_3333);
        reset.wheel_hardness = f32::from_bits(0x3f33_3333);
    } else {
        reset.truck_tightness = settings.truck_tightness(actor.player_index)?;
        reset.wheel_hardness = settings.wheel_hardness(actor.player_index)?;
    }
    reset.requested_physics_mode = settings.requested_physics_mode(actor.player_index)?;
    Ok(())
}

fn replace_flag(flags: &mut u32, bit: u32, value: bool) {
    *flags = (*flags & !(1 << bit)) | (u32::from(value) << bit);
}
