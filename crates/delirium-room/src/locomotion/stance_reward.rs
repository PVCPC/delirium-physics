use bevy::prelude::*;
use delirium::{nm::Agent,ragdoll::Build};
use std::sync::OnceLock;
static ENABLED:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {
    *ENABLED.get_or_init(||std::env::var("DLM_STANCE_REWARD").as_deref()==Ok("1"))
}

fn bonus(planned:[bool;2],touch:[bool;2],speed_squared:[f32;2],forward:f32,requested:f32)->f32 {
    let count=planned.iter().filter(|&&p|p).count();
    if count==0 {return 0.;}
    let progress=(forward/requested.max(0.1)).clamp(0.,1.);
    let quality=(0..2).filter(|&i|planned[i]&&touch[i]).map(|i|(-speed_squared[i].max(0.)/0.04).exp()).sum::<f32>()/count as f32;
    0.12*progress*quality
}

pub(super) fn reward(a:&Agent,build:&Build,target:&[Transform],desired_velocity:Vec3)->f32 {
    let frames:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();
    let mut wanted=target.to_vec();let mut speed_squared=[0.;2];
    for (i,k) in [3,6].into_iter().enumerate() {
        wanted[k].rotation*=frames[k].rotation.inverse()*bounds[k].rotation;
        let up=bounds[k].rotation.inverse()*Vec3::Z;
        let r=bounds[k].rotation*(-up.signum()*build.parts[k].boxed.expect("Homer requires calibrated foot boxes"));
        let (v,w)=a.doll.motion(k);speed_squared[i]=(v+w.cross(r)).truncate().length_squared();
    }
    let direction=desired_velocity.with_z(0.).normalize_or(Vec3::X);
    bonus([super::bottom(build,&wanted,3)<0.025,super::bottom(build,&wanted,6)<0.025],
        [a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0],speed_squared,
        a.doll.motion(0).0.dot(direction),desired_velocity.truncate().length())
}

