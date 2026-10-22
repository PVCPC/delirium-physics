use bevy::prelude::*;
use delirium::{nm::Agent, nmdbm::Body, ragdoll::Build};
use std::sync::OnceLock;

static GAIN:OnceLock<f32>=OnceLock::new();
pub(super) fn gain()->f32 {
    *GAIN.get_or_init(|| {
        let x=std::env::var("DLM_PHASE_SUPPORT").map_or(0.,|s|s.parse::<f32>().expect("invalid phase support"));
        assert!(x.is_finite()&&(0.0..=1.0).contains(&x));x
    })
}

pub(super) fn handover(seconds:f32,brisk:f32)->f32 {
    (seconds-1.).clamp(0.,1.)*(1.-brisk.clamp(0.,1.))
}

fn support(touch:[bool;2],height:[f32;2])->[i32;2] {
    std::array::from_fn(|i|if touch[i]&&height[i]<0.025 {if i==0 {3} else {6}} else {-1})
}

fn couple(torque:Vec3,dt:f32)->Option<(Vec3,Vec3)> {
    if torque.length_squared()<1e-12 {return None;}
    let axis=if torque.x.abs()<torque.y.abs() {Vec3::X} else {Vec3::Y};
    let r=torque.cross(axis).normalize_or_zero()*0.1;
    Some((r,torque.cross(r)*(dt/(2.*r.length_squared()))))
}

pub(super) fn apply(a:&mut Agent,build:&Build,target:&[Transform],dt:f32,ramp:f32) {
    let gain=gain()*ramp.clamp(0.,1.);
    if gain==0. {return;}
    let Some(ch)=a.character() else {return;};
    let touch=[a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0];
    let frames:Vec<_>=a.doll.frames().collect();
    let bounds:Vec<_>=a.doll.capsules().collect();
    let mut planned=target.to_vec();
    for k in [3,6] {planned[k].rotation*=frames[k].rotation.inverse()*bounds[k].rotation;}
    let wanted=support(touch,[super::bottom(build,&planned,3),super::bottom(build,&planned,6)]);
    if wanted==ch.support {return;}
    let native=ch.clone();let mut phase=native.clone();phase.support=wanted;
    let native_body=Body{ch:&native,build,doll:&a.doll,frames:&frames};
    let phase_body=Body{ch:&phase,build,doll:&a.doll,frames:&frames};
    let old=delirium::nmdbm::oppose_gravity(&native_body,native.attached);
    let new=delirium::nmdbm::oppose_gravity(&phase_body,phase.attached);
    for (k,(old,new)) in old.into_iter().zip(new).enumerate() {
        if let Some((r,j))=couple((new-old)*gain,dt) {
            let centre=a.doll.centre(k);
            a.doll.impulse(k,centre+r,j);a.doll.impulse(k,centre-r,-j);
        }
    }
}

