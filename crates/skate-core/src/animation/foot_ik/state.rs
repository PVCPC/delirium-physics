use super::{
    blend,
    contact::{ContactInput, ContactState},
    drive::{self, Geometry},
    external::{self, ExternalTarget},
    settings::Settings,
    status::{self, LimbStatus, Mode},
    transforms::{self, LimbBinding, LimbFrames},
};
use crate::physics::skeleton_animation_record::AnimationPartTransform as Transform;

pub const LIMBS: [LimbBinding; 4] = [
    LimbBinding {
        part: 15,
        parent_part: Some(16),
    },
    LimbBinding {
        part: 19,
        parent_part: Some(20),
    },
    LimbBinding {
        part: 3,
        parent_part: None,
    },
    LimbBinding {
        part: 7,
        parent_part: None,
    },
];

#[derive(Clone, Debug)]
pub struct FootIkState {
    pub limbs: [LimbStatus; 4],
    pub frames: [LimbFrames; 4],
    pub external_targets: [ExternalTarget; 4],
    pub contacts: ContactState,
    feet_enabled: bool,
}
impl Default for FootIkState {
    fn default() -> Self {
        Self {
            limbs: [LimbStatus::default(); 4],
            frames: [LimbFrames::default(); 4],
            external_targets: [ExternalTarget::default(); 4],
            contacts: ContactState::default(),
            feet_enabled: true,
        }
    }
}

pub struct UpdateInput<'a> {
    pub flags_2468: u32,
    pub flags_2472: u32,
    pub flags_2480: u32,
    pub animation_to_world: &'a Transform,
    pub world_to_animation: &'a Transform,
    pub physical_board: &'a Transform,
    pub contact_board: &'a Transform,
    pub inverse_contact_board: &'a Transform,
    pub animation: &'a [Transform; 24],
    pub original_animation: &'a [Transform; 24],
    pub targets: &'a [Transform; 4],
    pub hips_world_position: [f32; 4],
    pub contact_bone: usize,
    pub foot_bones: [usize; 2],
    pub current_contacts: [Option<[f32; 4]>; 2],
}

impl FootIkState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn enable_feet(&mut self, enabled: bool) {
        self.feet_enabled = enabled;
    }
    pub fn feet_enabled(&self) -> bool {
        self.feet_enabled
    }

    pub fn mark_support_failed_this_update(&mut self) {
        self.contacts.support_failed_this_update = true;
    }

    pub fn update(
        &mut self,
        input: UpdateInput<'_>,
        geometry: &Geometry,
        settings: &Settings,
        drives: &mut [Transform; 24],
    ) -> [bool; 4] {
        for limb in 0..4 {
            self.frames[limb].target = input.targets[limb];
        }
        status::update_modes(&mut self.limbs, self.feet_enabled, input.flags_2472);
        status::update_blends(
            &mut self.limbs,
            settings.deck_half_width,
            settings.deck_total_half_length,
            input.flags_2480,
            &settings.blend,
        );
        let inverse_animation_board = transforms::inverse_rigid(&input.animation[0]);
        for limb in 0..4 {
            self.frames[limb].within_contact_bounds = false;
            if matches!(self.limbs[limb].mode, Mode::External | Mode::Local) {
                external::update(
                    &mut self.external_targets[limb],
                    &mut self.limbs[limb],
                    &mut self.frames[limb],
                    LIMBS[limb],
                    input.animation,
                    input.animation_to_world,
                    input.world_to_animation,
                );
            }
            transforms::prepare_animation_target(
                &mut self.limbs[limb],
                &mut self.frames[limb],
                LIMBS[limb],
                input.animation,
                input.animation_to_world,
                &inverse_animation_board,
                settings.contact_bounds,
            );
        }
        blend::update(
            &self.limbs,
            &mut self.frames,
            &LIMBS,
            input.physical_board,
            input.animation_to_world,
            &input.animation[0],
        );
        self.contacts.update(
            &self.limbs,
            &mut self.frames,
            ContactInput {
                flags_2468: input.flags_2468,
                contact_bone: input.contact_bone,
                foot_bones: input.foot_bones,
                hips_world_position: input.hips_world_position,
                inverse_board: input.inverse_contact_board,
                board: input.contact_board,
                current_contacts: input.current_contacts,
            },
        );
        drive::update(
            &self.limbs,
            &mut self.frames,
            &LIMBS,
            geometry,
            input.original_animation,
            drives,
            input.world_to_animation,
            settings.angle_limits,
        )
    }
}
