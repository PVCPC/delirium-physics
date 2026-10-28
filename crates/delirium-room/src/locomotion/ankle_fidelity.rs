use bevy::prelude::*;
use std::sync::OnceLock;
static ENABLED:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {*ENABLED.get_or_init(||std::env::var("DLM_ANKLE_FIDELITY").as_deref()==Ok("1"))}
fn angles(q:Quat,forward:Vec3)->Vec2 {
    let side=Vec3::Z.cross(forward);
    let long=q*Vec3::Z;
    let yaw=long.dot(side).atan2(long.dot(forward));
    let mut up=q*Vec3::Y;if up.z<0. {up=-up;}
    Vec2::new(yaw,up.dot(side).atan2(up.z))
}
fn error(actual:Quat,expected:Quat,forward:Vec3)->f32 {
    let a=angles(actual,forward);let b=angles(expected,forward);
    let yaw=((a.x-b.x)*2.).sin().atan2(((a.x-b.x)*2.).cos())*0.5;
    let roll=(a.y.abs()-b.y.abs()).max(0.);
    [yaw,roll].iter().map(|e|{
        let excess=(e.abs()-0.025).max(0.);1.-(-excess*excess/0.04).exp()
    }).sum::<f32>()*0.5
}
pub(super) fn factor(actual:[Quat;2],expected:[Quat;2],planned_height:[f32;2],forward:Vec3)->f32 {
    let forward=forward.with_z(0.).normalize_or(Vec3::X);
    let cost=(0..2).map(|i|{
        let plant=((0.05-planned_height[i])/0.03).clamp(0.,1.);
        (0.25+0.75*plant)*error(actual[i],expected[i],forward)
    }).sum::<f32>()*0.5;
    1.-0.15*cost
}
