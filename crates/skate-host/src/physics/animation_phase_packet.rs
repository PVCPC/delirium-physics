use skate_core::{
    animation::output::{
        actor_packet::ExternalPhysicsInput,
        packet_reset::{AdditionalResetFields, RESET_POSE},
        physics_packet::PhysicsPosePacket,
    },
    input::animation_packet::AnimationPacketFields,
    player::input_phase::AnimationInputPacket,
};
use skate_data::collections::Collections;

pub(crate) struct AnimationProfile {
    pub maximum_ground_angle_degrees: f32,
    pub skitch_transition_time: f32,
    pub truck_tightness: f32,
    pub wheel_hardness: f32,
    pub physics_mode: u32,
    pub prevent_manual_respawn: bool,
    pub ignore_respawn_reset_button: u8,
    pub force_braking: bool,
    pub suppress_transition: bool,
    pub gesture_selections: Option<[u32; 4]>,
    pub suppress_up_gesture: bool,
    pub gesture_force_brake_bypass: bool,
}
impl AnimationProfile {
    pub fn load(data: &Collections, mode: &str) -> Result<Self, String> {
        let physics_mode = ["easy", "normal", "hardcore", "motorized", "test"]
            .iter()
            .position(|name| *name == mode)
            .ok_or_else(|| format!("Undefined player physics profile {mode}"))?
            as u32;
        Ok(Self {
            skitch_transition_time: data.float(
                "anim_skitching",
                "default",
                "LongSkitchIntoReachTime",
            )?,
            maximum_ground_angle_degrees: data.float(
                "anim_motion",
                "pushing",
                "disable_push_brake_at_slope",
            )?,
            truck_tightness: f32::from_bits(0x3f333333),
            wheel_hardness: f32::from_bits(0x3f333333),
            physics_mode,
            prevent_manual_respawn: false,
            ignore_respawn_reset_button: 0,
            force_braking: false,
            suppress_transition: false,
            gesture_selections: None,
            suppress_up_gesture: false,
            gesture_force_brake_bypass: false,
        })
    }
}

pub(crate) struct AnimationPhaseOutput {
    pub(super) reset: AdditionalResetFields,
    publication: AnimationPacketFields,
    external: ExternalPhysicsInput,
    mirrored: u8,
    weight_forwards: u8,
    flags: u32,
}
impl AnimationPhaseOutput {
    pub(super) fn new() -> Self {
        Self {
            reset: AdditionalResetFields {
                compression: 0.0,
                foot_ik_influence: [0.0; 2],
                next_step_position_valid: false,
                actor_flag_1904_bit23: false,
                actor_flag_1908_bit2: false,
                external_impulse_active: false,
                external_physics_input_active: false,
                externally_controlled: false,
                prevent_manual_respawn: false,
                ignore_respawn_reset_button: 0,
                force_braking: false,
                truck_tightness: 0.0,
                wheel_hardness: 0.0,
                auxiliary_vectors: [[0.0; 4]; 6],
                requested_physics_mode: 1,
            },
            publication: AnimationPacketFields {
                stance_byte: 0,
                timestep: 0.0,
                scalar_10388: 0.0,
                flags_10375_10496_10784: [0; 3],
                vector_10480: [0; 4],
                matrix_10704: RESET_POSE.map(|c| c.map(f32::to_bits)),
                byte_10768: 0,
                truck_tightness: 0.0,
                scalar_10792: 0.0,
                flag_10371: 0,
            },
            external: ExternalPhysicsInput {
                vectors: [[0; 4]; 10],
                flags: 0,
            },
            mirrored: 0,
            weight_forwards: 0,
            flags: 0,
        }
    }
    pub(super) fn publish(
        &mut self,
        pose: &PhysicsPosePacket,
        profile: &AnimationProfile,
        actor_flags: u32,
    ) {
        let r = &mut self.reset;
        r.actor_flag_1904_bit23 = false;
        r.actor_flag_1908_bit2 = profile.suppress_transition || actor_flags & (1 << 2) != 0;
        r.truck_tightness = profile.truck_tightness;
        r.wheel_hardness = profile.wheel_hardness;
        r.requested_physics_mode = profile.physics_mode;
        r.prevent_manual_respawn = profile.prevent_manual_respawn;
        r.ignore_respawn_reset_button = profile.ignore_respawn_reset_button;
        r.force_braking = profile.force_braking;
        self.flags = pose.flags & !((1 << 24) | (1 << 22));
        self.mirrored = u8::from(pose.mirrored);
        self.weight_forwards = u8::from(pose.weight_forwards);
        self.publication = AnimationPacketFields {
            stance_byte: u8::from(pose.board_flipped),
            timestep: pose.timestep,
            scalar_10388: r.compression,
            flags_10375_10496_10784: [
                u8::from(r.actor_flag_1904_bit23),
                0,
                0,
            ],
            vector_10480: [0; 4],
            matrix_10704: RESET_POSE.map(|c| c.map(f32::to_bits)),
            byte_10768: 0,
            truck_tightness: r.truck_tightness,
            scalar_10792: r.wheel_hardness,
            flag_10371: u8::from(pose.riding_switch),
        };
    }
    pub(super) fn publish_external_reset(
        &mut self,
        reply: skate_core::animation::output::actor_packet::ExternalReset,
    ) {
        self.publication.matrix_10704 = reply.transform;
        self.publication.byte_10768 = reply.byte64;
        self.publication.flags_10375_10496_10784[2] = 1;
    }
    pub fn packet(&self) -> AnimationInputPacket<'_> {
        let r = &self.reset;
        let vectors = r.auxiliary_vectors.map(|v| v.map(f32::to_bits));
        AnimationInputPacket {
            publication: &self.publication,
            external_physics_10512: &self.external,
            flag_10369: u8::from(r.next_step_position_valid),
            flag_10370: self.mirrored,
            flag_10373: self.weight_forwards,
            suppress_transition_10376: u8::from(r.actor_flag_1908_bit2),
            use_external_physics_10688: 0,
            external_physics_flag_10689: 0,
            flag_10786: u8::from(r.prevent_manual_respawn),
            flag_10787: r.ignore_respawn_reset_button,
            force_braking_10796: u8::from(r.force_braking),
            vector_10816: vectors[0],
            vector_10832: vectors[1],
            vector_10848: vectors[2],
            vector_10864: vectors[3],
            vector_10880: vectors[4],
            vector_10896: vectors[5],
            scalar_10912: r.foot_ik_influence[0],
            scalar_10916: r.foot_ik_influence[1],
            state_variant_10928: r.requested_physics_mode,
            flags_10932: self.flags,
        }
    }
}
