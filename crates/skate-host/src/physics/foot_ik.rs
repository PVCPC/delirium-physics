use super::animated_skeleton::AnimatedSkeleton;
use skate_core::{
    animation::foot_ik::{
        drive::Geometry,
        post_contact::SettingsPost,
        post_physics,
        settings::Settings,
        state::{FootIkState, LIMBS, UpdateInput},
        status::BlendSettings,
        transforms::inverse_rigid,
        two_bone::AngleLimits,
    },
    physics::skeleton_animation_record::{AnimationPartTransform as Transform, compose_affine},
};
use skate_data::{animation_frames::AnimationFrames, collections::Collections};

pub(crate) struct FootIk {
    pub state: FootIkState,
    geometry: Geometry,
    settings: Settings,
    post_settings: SettingsPost,
    bone_indices: [usize; 24],
}

pub(crate) struct PhysicalInput<'a> {
    pub state_id: u32,
    pub flags_2468: u32,
    pub flags_2472: u32,
    pub flags_2480: u32,
    pub contact_bone: usize,
    pub physical_board: &'a Transform,
    pub hips_world_position: [f32; 4],
    pub current_contacts: [Option<[f32; 4]>; 2],
}

pub(crate) struct DriveOutput {
    pub frames: [Transform; 24],
}

impl FootIk {
    pub fn load(
        data: &Collections,
        animation: &AnimationFrames,
        skeleton: &AnimatedSkeleton,
    ) -> Result<Self, String> {
        let mut parents = [None; 24];
        for (part, &bone) in skeleton.bone_indices.iter().enumerate() {
            let mut ancestor = *animation
                .parents
                .get(bone)
                .ok_or("IK bone exceeds stock hierarchy")?;
            let mut visited = vec![false; animation.parents.len()];
            while ancestor >= 0 {
                let index = ancestor as usize;
                if index >= visited.len() || visited[index] {
                    return Err(format!(
                        "Invalid stock IK hierarchy ancestor for part {part}"
                    ));
                }
                visited[index] = true;
                if let Some(parent) = skeleton.bone_indices.iter().position(|&bone| bone == index) {
                    parents[part] = Some(parent);
                    break;
                }
                ancestor = animation.parents[index];
            }
        }
        let geometry = Geometry::new(parents, &skeleton.physics_frames)?;
        geometry.validate_limbs(&LIMBS)?;
        Ok(Self {
            state: FootIkState::default(),
            geometry,
            settings: load_settings(data)?,
            post_settings: SettingsPost {
                minimum_board_up: data.float(
                    "physics_animation",
                    "default",
                    "PostIKMinYAxisVal",
                )?,
                wipeout_height: data.float("animation", "default", "FeetRelativeHeightWipeout")?,
                riding_height: data.float("animation", "default", "FeetRelativeHeightOnDeck")?,
            },
            bone_indices: skeleton.bone_indices,
        })
    }

    pub fn enable_feet(&mut self, enabled: bool) {
        self.state.enable_feet(enabled);
    }

    pub fn physical_toe_positions(
        &self,
        record: &skate_core::physics::skeleton_body::SkeletonPhysicalRecord,
    ) -> [[f32; 4]; 2] {
        use skate_core::physics::skeleton_animation_record::transform_point;
        [
            transform_point(&record.pose[19], self.geometry.inverse_part_frames[20][3]),
            transform_point(&record.pose[15], self.geometry.inverse_part_frames[16][3]),
        ]
    }

    pub fn post_physics(
        &mut self,
        body: &mut skate_core::physics::skeleton_body::SkeletonBody,
        input: post_physics::Input<'_>,
    ) -> [bool; 4] {
        post_physics::update(
            &mut self.state,
            body,
            &self.geometry,
            &self.settings,
            &self.post_settings,
            input,
        )
    }

    pub fn update(
        &mut self,
        skeleton: &AnimatedSkeleton,
        globals: &[Transform],
        input: PhysicalInput<'_>,
    ) -> Result<DriveOutput, String> {
        let mut drives = skeleton.record.pose;
        if input.state_id != 702 {
            let mut originals = skeleton.record.pose;
            for (part, &bone) in self.bone_indices.iter().enumerate() {
                originals[part] = *globals
                    .get(bone)
                    .ok_or("IK original joint is absent from the current pose")?;
            }
            let animated_world =
                compose_affine(&skeleton.roots.animation_to_world, &skeleton.record.pose[0]);
            let special_inverse = inverse_rigid(&animated_world);
            let (physical_board, contact_board, inverse_contact_board) = if input.state_id == 503 {
                (&animated_world, &animated_world, &special_inverse)
            } else {
                (
                    input.physical_board,
                    &skeleton.roots.board,
                    &skeleton.roots.inverse_board,
                )
            };
            let _ = self.state.update(
                UpdateInput {
                    flags_2468: input.flags_2468,
                    flags_2472: input.flags_2472,
                    flags_2480: input.flags_2480,
                    animation_to_world: &skeleton.roots.animation_to_world,
                    world_to_animation: &skeleton.roots.world_to_animation,
                    physical_board,
                    contact_board,
                    inverse_contact_board,
                    animation: &skeleton.record.pose,
                    original_animation: &originals,
                    targets: &skeleton.targets,
                    hips_world_position: input.hips_world_position,
                    contact_bone: input.contact_bone,
                    foot_bones: [self.bone_indices[15], self.bone_indices[19]],
                    current_contacts: input.current_contacts,
                },
                &self.geometry,
                &self.settings,
                &mut drives,
            );
        }
        Ok(DriveOutput { frames: drives })
    }
}

pub(crate) fn load_settings(data: &Collections) -> Result<Settings, String> {
    let vector = |name| {
        data.words::<4>("physics_skeletonik", "default", name)
            .map(|words| words.map(f32::from_bits))
    };
    let scalar = |name| data.float("physics_skeletonik", "default", name);
    let half_width = data.float("physicsdeck", "default", "DeckWidth")? * 0.5;
    let half_length = data.float("physicsdeck", "default", "DeckMidLength")? * 0.5;
    Ok(Settings {
        angle_limits: AngleLimits {
            minimum_degrees: data.float("physics_animation", "default", "IKMinAngle")?,
            maximum_degrees: data.float("physics_animation", "default", "IKMaxAngle")?,
        },
        blend: BlendSettings {
            hand_inner_padding: vector("IKOnPadding")?,
            hand_outer_padding: vector("IKOffPadding")?,
            external_blend_step: scalar("FeetIKMaxBlendDeltaOut")?,
            board_blend_step: scalar("FeetIKMaxBlendDelta")?,
        },
        post_ik_padding: vector("PostIKPadding")?,
        contact_bounds: vector("IKBoneOnDeckBBox")?,
        foot_on_deck_padding: vector("FootOnDeckPadding")?,
        wipeout_feet_offset: scalar("WipeoutFeetOffset")?,
        deck_half_width: half_width,
        deck_half_length: half_length,
        deck_total_half_length: data.float("physicsdeck", "default", "DeckFrontEndSize")?
            + half_length,
        deck_front_angle_degrees: data.float("physicsdeck", "default", "DeckFrontEndAngle")?,
    })
}
