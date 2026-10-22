use bevy::prelude::*;
use std::sync::OnceLock;
static WALK_REFERENCE:OnceLock<f32>=OnceLock::new();
pub(super) fn walk_reference()->f32 {
    *WALK_REFERENCE.get_or_init(|| {
        let value=std::env::var("DLM_WALK_REFERENCE_AUTHORITY").map_or(0.35,|v|v.parse::<f32>().expect("invalid walking reference authority"));
        assert!(value.is_finite()&&(0.35..=0.8).contains(&value));value
    })
}

pub(super) fn mode() -> u8 {
    match std::env::var("DLM_MOCAP_TRACKING").as_deref() {
        Ok("tracking-reward-v2") => 1,
        Ok("mocap-v2") => 2,
        Ok("mocap-walk20-v2") => 3,
        Ok("mocap-walk35-v2") => 4,
        Ok("mocap-walk-settings-v2") => 5,
        Ok("mocap-walk-combined-v2") => 6,
        Ok("mocap-walk-lean-v2") => 7,
        Ok("mocap-jog-contact-v2") => 8,
        Ok("mocap-jog-phase-v2") => 9,
        Ok("mocap-jog-drive-v2") => 10,
        Ok("mocap-jog-ballistic-v2") => 11,
        Ok("mocap-push-off-v2") => 12,
        Ok("mocap-alignment-v2") => 13,
        Ok("mocap-path-v2") => 14,
        Ok("mocap-leg-strength-v2") => 15,
        Ok("mocap-leg-skill-v2") => 16,
        Ok("mocap-actuator-v2") => 17,
        _ => 0,
    }
}

pub(super) fn terms(position: f32, velocity: f32, local: f32, root_velocity: f32) -> [f32; 4] {
    [(-10.0 * position).exp(), (-0.25 * velocity).exp(), (-10.0 * local).exp(), (-root_velocity).exp()]
}

pub(super) fn leg_target(base: Quat, reference: Quat, delta: Quat, gait: f32, mode: u8) -> Quat {
    leg_target_with_authority(base,reference,delta,gait,mode,if mode==17 {walk_reference()} else {0.35})
}
fn leg_target_with_authority(base:Quat,reference:Quat,delta:Quat,gait:f32,mode:u8,walk_reference:f32)->Quat {
    if mode >= 2 {
        let (walking_reference, walking_residual) = match mode {
            3 | 6 => (0.20, 0.25),
            4 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 => (0.35, 0.40),
            17 => (walk_reference,0.40),
            _ => (0.05, 0.10),
        };
        let authority = gait * gait;
        base.slerp(reference, authority + walking_reference * (1.0 - gait))
            * Quat::IDENTITY.slerp(delta, walking_residual + (1.0 - walking_residual) * authority)
    } else { base.slerp(reference * delta, gait * gait) }
}

pub(super) fn balance_authority(mode: u8, gait: f32) -> f32 {
    let walking = if matches!(mode, 5 | 6) { 0.75 } else { 0.25 };
    walking + (1.0 - walking) * gait
}

pub(super) fn lateral_error(actual: Vec3, target: Vec3, side: Vec3) -> f32 {
    let tilt = |v: Vec3| v.dot(side).atan2(v.z.max(0.05));
    (tilt(actual) - tilt(target)).powi(2)
}

pub(super) fn phase_correction(change: f32, advance: f32, fraction: f32, mode: u8, dt: f32) -> f32 {
    if advance <= 0.0 { return change.clamp(-dt*0.5,dt*2.0); }
    if matches!(mode,11|12|13|14|15|16|17) && fraction>0.05 {return 0.0;}
    let lag = if matches!(mode,9|10) {1.0-0.8*fraction.clamp(0.0,1.0)} else {1.0};
    change.clamp(-dt*0.25*lag,dt*0.25)
}

pub(super) fn contact_cost(actual: [f32;2], target: [f32;2], velocity: [f32;2], desired_velocity: [f32;2], root_height_error: f32) -> f32 {
    let support=|height:f32| ((0.035-height)/0.03).clamp(0.0,1.0);
    let mut cost=0.0;
    for i in 0..2 {
        cost += 0.5*(0.20*(support(actual[i])-support(target[i])).powi(2)
            +20.0*(actual[i]-target[i]).clamp(-0.2,0.2).powi(2)
            +0.01*(velocity[i]-desired_velocity[i]).clamp(-2.0,2.0).powi(2));
    }
    cost+10.0*root_height_error.clamp(-0.2,0.2).powi(2)
}

