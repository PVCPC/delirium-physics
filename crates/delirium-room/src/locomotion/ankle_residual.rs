use bevy::prelude::*;
use std::sync::OnceLock;
static ENABLED:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {*ENABLED.get_or_init(||std::env::var("DLM_ANKLE_RESIDUAL").as_deref()==Ok("1"))}
pub(super) fn correction(original:Quat,forward:Vec3,yaw:f32,roll:f32,seconds:f32)->Quat {
    let fade=(seconds-1.).clamp(0.,1.);
    let yaw=yaw.clamp(-1.,1.)*0.10*fade;
    let roll=roll.clamp(-1.,1.)*0.10*fade;
    if yaw==0.&&roll==0. {return original;}
    let forward=forward.with_z(0.).normalize_or(Vec3::X);
    Quat::from_rotation_z(yaw)*Quat::from_axis_angle(forward,roll)*original
}
