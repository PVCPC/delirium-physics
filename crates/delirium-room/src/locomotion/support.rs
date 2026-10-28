use bevy::prelude::*;
use std::sync::OnceLock;
static BRISK_SUPPORT:OnceLock<f32>=OnceLock::new();
static STYLE_TRACKING:OnceLock<bool>=OnceLock::new();
static NATIVE_OBSERVATIONS:OnceLock<bool>=OnceLock::new();
pub(super) fn native_observations()->bool {
    *NATIVE_OBSERVATIONS.get_or_init(||std::env::var("DLM_NATIVE_SUPPORT_OBSERVATIONS").as_deref()==Ok("1"))
}
pub(super) fn native_feedback(touch:[bool;2],support:[i32;2])->[f32;4] {
    let state=|value:i32|match value {-2=>0.,-1=>-1.,_=>1.};
    [f32::from(touch[0]),f32::from(touch[1]),state(support[0]),state(support[1])]
}
pub(super) fn style_tracking()->bool {
    *STYLE_TRACKING.get_or_init(||std::env::var("DLM_FOOT_KNEE_TRACKING").as_deref()==Ok("1"))
}

pub(super) fn style_cost(up:Vec3,expected_up:Vec3,knee:f32,expected_knee:f32,velocity:f32,expected_velocity:f32,stance:f32)->f32 {
    let sole=(1.-up.dot(expected_up).clamp(-1.,1.))/0.06030738;
    0.08*stance.clamp(0.,1.)*sole.min(4.)
        +0.08*((knee-expected_knee)/0.06).clamp(-2.,2.).powi(2)
        +0.04*((velocity-expected_velocity)/0.5).clamp(-2.,2.).powi(2)
}

pub(super) fn style_bonus(cost:f32)->f32 {
    0.20*(-cost.max(0.)).exp()
}

pub(super) fn brisk_support(gait:f32,jog:f32)->f32 {
    let gain=*BRISK_SUPPORT.get_or_init(|| {
        let value=std::env::var("DLM_BRISK_SUPPORT").map_or(0.,|s|s.parse::<f32>().expect("invalid brisk support"));
        assert!(value.is_finite()&&(0.0..=1.0).contains(&value));value
    });
    support_fraction(gait,jog,gain)
}

fn support_fraction(gait:f32,jog:f32,gain:f32)->f32 {
    jog.max(gain*gait.clamp(0.,1.)*(1.-jog.clamp(0.,1.)))
}

pub(super) fn practice_blend(index:usize,resets:usize,current_limit:f32,mixed:bool)->f32 {
    if mixed&&(index+resets)%2==0 {current_limit} else {1.0}
}

pub(super) fn response(action:f32,fraction:f32)->f32 {
    2.0_f32.powf(action.clamp(-1.,1.)*fraction.clamp(0.,1.))
}

pub(super) fn restore_response(current:f32,base:f32,applied:f32)->f32 {
    if current.to_bits()==applied.to_bits() {base} else {current}
}

pub(super) fn leg_strength(base:f32, action:f32, fraction:f32)->f32 {
    (base+6.0*action.clamp(-1.,1.)*fraction.clamp(0.,1.)).clamp(6.,24.)
}

pub(super) fn stiffness(base:f32, action:f32, contact:bool, planned_height:f32, fraction:f32)->f32 {
    let stance=contact||planned_height<0.025;
    let strength=if stance {(base+5.0+3.0*action.clamp(-1.0,1.0)).min(24.0)} else {base.min(12.0)};
    base+fraction.clamp(0.0,1.0)*(strength-base)
}

pub(super) fn cost(actual_vz:f32, desired_vz:f32, contact:[bool;2], height:[f32;2], desired:[f32;2], torso:Vec3)->f32 {
    let timing=(0..2).map(|i|f32::from(contact[i]!=(desired[i]<0.025))).sum::<f32>()*0.25;
    let vertical=0.6*(actual_vz-desired_vz).clamp(-2.,2.).powi(2);
    let landing=if contact.iter().any(|&x|x) {0.10*(-actual_vz-1.2).max(0.).min(2.).powi(2)} else {0.};
    let expected_air=desired.iter().all(|&z|z>0.025);
    let upright=torso.z>0.5&&torso.truncate().length()<torso.z*0.5;
    let flight=if expected_air&&upright&&contact.iter().all(|&x|!x)&&height.iter().all(|&z|z>0.025) {-0.25} else {0.};
    timing+vertical+landing+flight
}

pub(super) fn reward(dense:f32, support_cost:f32, contact_cost:f32, lateral:f32, jitter:f32, fraction:f32)->f32 {
    (dense+0.3+0.4*fraction*(-support_cost.max(0.)).exp()
        -0.25*contact_cost-16.0*lateral-0.05*jitter).max(0.02)
}

