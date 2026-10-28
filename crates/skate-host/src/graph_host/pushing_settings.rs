use skate_core::{
    animation::{clip_clock::ClipClock, skeleton_input::name::encode},
    point_graph::PointGraph,
    riding::{
        push_animation::PushAnimationCurves,
        push_behaviors::{PushAttributes, PushClipMetrics},
    },
};
use skate_data::{animation_metadata::AnimationMetadata, collections::Collections};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct PushTreeAttribute {
    pub name: String,
    pub first_payload: f32,
}
#[derive(Clone, Debug)]
pub struct PushTreeMetadata {
    pub length: f32,
    pub attributes: Vec<PushTreeAttribute>,
}
pub trait PushTreeSource {
    fn metadata(
        &mut self,
        tree_name: &str,
        attribute_mask: u32,
    ) -> Result<PushTreeMetadata, String>;
}

impl PushTreeSource for AnimationMetadata {
    fn metadata(
        &mut self,
        tree_name: &str,
        attribute_mask: u32,
    ) -> Result<PushTreeMetadata, String> {
        let clip = self.clip(tree_name)?;
        let frames = f32::from_bits(clip.frames_bits);
        let fps = f32::from_bits(clip.fps_bits);
        let base_speed = f32::from_bits(clip.base_speed_bits);
        let length = (frames - 1.0) / ((1.0 * base_speed) * fps);
        let clock = ClipClock {
            frames,
            fps,
            base_speed,
            speed: 1.0,
            length,
            time: 0.0,
            previous_time: 0.0,
            loops_since_evaluation: 0,
            looping: clip.flags_word & 0x1000_0000 != 0,
            phase_controlled: clip.flags_word & 0x4000_0000 != 0,
        };
        let mut attributes = Vec::new();
        for attribute in &clip.attributes {
            let begin = f32::from_bits(attribute.begin_bits);
            if begin != -1.0 {
                let status =
                    u32::from(clock.attribute_status(begin, f32::from_bits(attribute.end_bits)));
                if attribute_mask & 0x1c & status == 0 || attribute_mask & 3 & status == 0 {
                    continue;
                }
            }
            if !matches!(attribute.type_id, 0 | 1 | 3) {
                return Err(format!(
                    "{tree_name}: attribute {} needs type{} evaluation",
                    attribute.name, attribute.type_id,
                ));
            }
            attributes.push(PushTreeAttribute {
                name: attribute.name.clone(),
                first_payload: f32::from_bits(attribute.payload_words[0]),
            });
        }
        Ok(PushTreeMetadata { length, attributes })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PushingSettings {
    pub curves: PushAnimationCurves,
    pub teleport_window: f32,
    pub maximum_holding_acceleration: f32,
    pub out_speed_weight: f32,
    pub maximum_out_factor: f32,
    pub regular: PushAttributes,
    pub mongo: PushAttributes,
}
impl PushingSettings {
    pub fn load_stock(data: &Collections, asset_root: &Path) -> Result<Self, String> {
        let mut trees =
            skate_data::animation_banks::AnimationBanks::load(asset_root)?.metadata()?;
        Self::load(data, &mut trees)
    }
    pub fn load(data: &Collections, trees: &mut impl PushTreeSource) -> Result<Self, String> {
        let float = |name| data.float("anim_motion", "pushing", name);
        Ok(Self {
            curves: PushAnimationCurves {
                button_time_max: curve(data, "button_time_max")?,
                button_time_to_dv: curve(data, "button_time_to_dv")?,
                blend_speed_over_frames: curve(data, "blend_speed_over_frames")?,
                blend_acc_over_frames: curve(data, "blend_acc_over_frames")?,
            },
            teleport_window: float("pushing_usemaxpushfromteleporttime")?,
            maximum_holding_acceleration: float("max_holding_acc")?,
            out_speed_weight: float("dynamic_out_factor_speed_vs_acc")?,
            maximum_out_factor: float("last_push_out_time")?,
            regular: attributes(
                trees,
                [
                    "R_PUSHLSP_HSTR_N_0_CYC1",
                    "R_PUSHHSP_HSTR_N_0_CYC1",
                    "R_PUSHLSP_LSTR_N_0_CYC1",
                    "R_PUSHHSP_LSTR_N_0_CYC1",
                ],
            )?,
            mongo: attributes(
                trees,
                [
                    "R_PUSHLSP_HSTR_MONGO_0_CYC1",
                    "R_PUSHHSP_HSTR_MONGO_0_CYC1",
                    "R_PUSHLSP_LSTR_MONGO_0_CYC1",
                    "R_PUSHHSP_LSTR_MONGO_0_CYC1",
                ],
            )?,
        })
    }
    pub fn attributes(&self, regular_attributes: bool, is_switch: bool) -> &PushAttributes {
        if regular_attributes != is_switch {
            &self.regular
        } else {
            &self.mongo
        }
    }
}

fn curve(data: &Collections, name: &str) -> Result<PointGraph<8>, String> {
    let words = data.words::<20>("anim_motion", "pushing", name)?;
    Ok(PointGraph {
        x: std::array::from_fn(|i| f32::from_bits(words[4 + i])),
        y: std::array::from_fn(|i| f32::from_bits(words[12 + i])),
    })
}
fn attributes(trees: &mut impl PushTreeSource, names: [&str; 4]) -> Result<PushAttributes, String> {
    let mut metrics = Vec::with_capacity(4);
    for name in names {
        let tree = trees.metadata(name, 15)?;
        let mut begin = None;
        let mut end = None;
        for attribute in tree.attributes {
            let key = encode(attribute.name.as_bytes());
            if key == encode(b"HStr_Vel_B") || key == encode(b"LStr_Vel_B") {
                begin = Some(attribute.first_payload);
            } else if key == encode(b"Vel_E") {
                end = Some(attribute.first_payload);
            }
        }
        metrics.push(PushClipMetrics {
            length: tree.length,
            begin_velocity: begin
                .ok_or_else(|| format!("{name}: missing begin-velocity attribute"))?,
            end_velocity: end.ok_or_else(|| format!("{name}: missing Vel_E attribute"))?,
        });
    }
    Ok(PushAttributes::from_clips(metrics.try_into().unwrap()))
}

