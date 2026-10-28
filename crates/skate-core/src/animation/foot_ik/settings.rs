use super::status::BlendSettings;
use super::two_bone::AngleLimits;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub angle_limits: AngleLimits,
    pub blend: BlendSettings,
    pub post_ik_padding: [f32; 4],
    pub contact_bounds: [f32; 4],
    pub foot_on_deck_padding: [f32; 4],
    pub wipeout_feet_offset: f32,
    pub deck_half_width: f32,
    pub deck_half_length: f32,
    pub deck_total_half_length: f32,
    pub deck_front_angle_degrees: f32,
}
