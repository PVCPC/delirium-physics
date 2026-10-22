use bevy::prelude::*;
use std::sync::OnceLock;
static ENABLED:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {*ENABLED.get_or_init(||std::env::var("DLM_FOOT_STATE_FEEDBACK").as_deref()==Ok("1"))}
pub(super) fn encode(heading_inverse:Quat,angular:[Vec3;2],contact:[bool;2])->[f32;8] {
    let left=(heading_inverse*angular[0]*0.2).clamp(Vec3::splat(-4.),Vec3::splat(4.));
    let right=(heading_inverse*angular[1]*0.2).clamp(Vec3::splat(-4.),Vec3::splat(4.));
    [left.x,left.y,left.z,right.x,right.y,right.z,f32::from(contact[0]),f32::from(contact[1])]
}
