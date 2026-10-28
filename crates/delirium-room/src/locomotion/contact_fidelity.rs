use bevy::prelude::*;
use delirium::{nm::Agent,ragdoll::Build};
use std::sync::OnceLock;
static ENABLED:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {*ENABLED.get_or_init(||std::env::var("DLM_CONTACT_FIDELITY").as_deref()==Ok("1"))}

fn quality(planned:[bool;2],touch:[bool;2],speed_squared:[f32;2],progress:f32)->f32 {
    let agreement=(0..2).filter(|&i|planned[i]==touch[i]).count() as f32*0.5;
    let count=planned.iter().filter(|&&x|x).count();
    let plant=if count==0 {1.} else {
        (0..2).filter(|&i|planned[i]&&touch[i]).map(|i|(-speed_squared[i].max(0.)/0.04).exp()).sum::<f32>()/count as f32
    };
    0.5+0.5*agreement*plant*progress.clamp(0.,1.)
}

pub(super) fn factor(a:&Agent,build:&Build,target:&[Transform],desired_velocity:Vec3)->f32 {
    let frames:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();
    let mut wanted=target.to_vec();let mut speed_squared=[0.;2];
    for (i,k) in [3,6].into_iter().enumerate() {
        wanted[k].rotation*=frames[k].rotation.inverse()*bounds[k].rotation;
        let up=bounds[k].rotation.inverse()*Vec3::Z;
        let r=bounds[k].rotation*(-up.signum()*build.parts[k].boxed.expect("calibrated foot boxes required"));
        let (v,w)=a.doll.motion(k);speed_squared[i]=(v+w.cross(r)).truncate().length_squared();
    }
    let requested=desired_velocity.truncate().length();
    let progress=a.doll.motion(0).0.dot(desired_velocity.with_z(0.).normalize_or(Vec3::X))/requested.max(0.1);
    let base=quality([super::bottom(build,&wanted,3)<0.025,super::bottom(build,&wanted,6)<0.025],
        [a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0],speed_squared,progress);
    if super::ankle_fidelity::enabled() {
        base*super::ankle_fidelity::factor([bounds[3].rotation,bounds[6].rotation],
            [wanted[3].rotation,wanted[6].rotation],
            [super::bottom(build,&wanted,3),super::bottom(build,&wanted,6)],desired_velocity)
    } else {base}
}

